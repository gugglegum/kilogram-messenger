//! Blind capability mailbox contract.
//!
//! The storage boundary sees only unrelated Ed25519 capability keys, derived
//! mailbox/item identifiers, ciphertext bytes, TTL and access timing. It never
//! receives a Kilogram Account, Device, conversation or event identifier.

mod store;
mod wire;

use std::{fmt, str::FromStr};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kilogram_crypto::{DeviceEncryptionIdentity, EncryptionPublicKey, SealedMessage};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

pub use store::{
    BlindMailboxStore, CleanupReport, DEFAULT_MAX_ITEMS_PER_MAILBOX, DeleteOutcome,
    MailboxPutOutcome, MailboxStoreConfig, StoredMailboxItem, StoredMailboxPage,
};
pub use wire::{
    MAX_MAILBOX_PEER_REQUEST_BYTES, MAX_MAILBOX_PEER_RESPONSE_BYTES,
    MAX_MAILBOX_WIRE_REQUEST_BYTES, MAX_MAILBOX_WIRE_RESPONSE_BYTES, MailboxDeleteRequest,
    MailboxDeleteResponse, MailboxListRequest, MailboxListResponse, MailboxPeerOperation,
    MailboxPeerRejection, MailboxPeerRequest, MailboxPeerResponse, MailboxPutRequest,
    MailboxPutResponse,
};

const MAILBOX_ID_DOMAIN: &[u8] = b"kilogram:blind-mailbox-id:v1\0";
const WRITE_AUTHORIZATION_DOMAIN: &[u8] = b"kilogram:blind-mailbox-write:v1\0";
const READ_AUTHORIZATION_DOMAIN: &[u8] = b"kilogram:blind-mailbox-read:v1\0";
const STORED_RECEIPT_DOMAIN: &[u8] = b"kilogram:blind-mailbox-stored-receipt:v1\0";
const DELETE_RECEIPT_DOMAIN: &[u8] = b"kilogram:blind-mailbox-delete-receipt:v1\0";
const STORAGE_OFFER_DOMAIN: &[u8] = b"kilogram:blind-mailbox-storage-offer:v1\0";
const RECEIPT_ID_DOMAIN: &[u8] = b"kilogram:blind-mailbox-receipt-id:v1\0";
const DELETE_RECEIPT_ID_DOMAIN: &[u8] = b"kilogram:blind-mailbox-delete-id:v1\0";
const ENVELOPE_HPKE_INFO: &[u8] = b"kilogram:blind-mailbox-envelope:v1\0";
const VERSION: u8 = 1;
const KEY_BYTES: usize = 32;
const MAX_ADDRESS_BYTES: usize = 256;
const MAX_AUTHORIZATION_BYTES: usize = 1_024;
const MAX_RECEIPT_BYTES: usize = 1_024;
pub const MAX_MAILBOX_STORAGE_ENDPOINT_BYTES: usize = 16 * 1024;
pub const MAX_MAILBOX_STORAGE_OFFER_BYTES: usize = 24 * 1024;
pub const MIN_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS: u64 = 30;
pub const MAX_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS: u64 = 60 * 60;

pub const MIN_MAILBOX_TTL_SECONDS: u64 = 60;
pub const MAX_MAILBOX_TTL_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const MAX_MAILBOX_PLAINTEXT_BYTES: usize = 512 * 1024;
pub const MAX_MAILBOX_ENVELOPE_BYTES: usize = 1024 * 1024;
pub const MAX_MAILBOX_PAGE_ITEMS: u16 = 8;

macro_rules! public_key_type {
    ($name:ident, $kind:literal) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        pub struct $name([u8; KEY_BYTES]);

        impl $name {
            pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Result<Self, MailboxError> {
                VerifyingKey::from_bytes(&bytes).map_err(|source| {
                    MailboxError::InvalidPublicKey {
                        kind: $kind,
                        source,
                    }
                })?;
                Ok(Self(bytes))
            }

            pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
                &self.0
            }

            fn verifying_key(&self) -> Result<VerifyingKey, MailboxError> {
                VerifyingKey::from_bytes(&self.0).map_err(|source| MailboxError::InvalidPublicKey {
                    kind: $kind,
                    source,
                })
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_hex(formatter, &self.0)
            }
        }

        impl FromStr for $name {
            type Err = MailboxError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::from_bytes(decode_hex(value)?)
            }
        }
    };
}

public_key_type!(MailboxReadKey, "read");
public_key_type!(MailboxWriteKey, "write");
public_key_type!(MailboxStoreKey, "store");

macro_rules! opaque_id_type {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        pub struct $name([u8; KEY_BYTES]);

        impl $name {
            pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Self {
                Self(bytes)
            }

            pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_hex(formatter, &self.0)
            }
        }

        impl FromStr for $name {
            type Err = MailboxError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Ok(Self(decode_hex(value)?))
            }
        }
    };
}

opaque_id_type!(MailboxId);
opaque_id_type!(MailboxItemId);
opaque_id_type!(MailboxRequestNonce);
opaque_id_type!(MailboxReceiptId);
opaque_id_type!(MailboxDeleteReceiptId);

impl MailboxItemId {
    pub fn generate() -> Result<Self, MailboxError> {
        Ok(Self(random_bytes()?))
    }
}

impl MailboxRequestNonce {
    pub fn generate() -> Result<Self, MailboxError> {
        Ok(Self(random_bytes()?))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxAddress {
    version: u8,
    read_key: MailboxReadKey,
    write_key: MailboxWriteKey,
}

impl MailboxAddress {
    pub fn new(read_key: MailboxReadKey, write_key: MailboxWriteKey) -> Self {
        Self {
            version: VERSION,
            read_key,
            write_key,
        }
    }

    pub fn mailbox_id(&self) -> MailboxId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(MAILBOX_ID_DOMAIN);
        hasher.update(self.read_key.as_bytes());
        hasher.update(self.write_key.as_bytes());
        MailboxId(*hasher.finalize().as_bytes())
    }

    pub fn read_key(&self) -> MailboxReadKey {
        self.read_key
    }

    pub fn write_key(&self) -> MailboxWriteKey {
        self.write_key
    }

    pub fn verify(&self) -> Result<(), MailboxError> {
        if self.version != VERSION {
            return Err(MailboxError::Invalid("unsupported mailbox address version"));
        }
        self.read_key.verifying_key()?;
        self.write_key.verifying_key()?;
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        self.verify()?;
        let bytes = postcard::to_allocvec(self)?;
        if bytes.len() > MAX_ADDRESS_BYTES {
            return Err(MailboxError::Invalid("mailbox address is too large"));
        }
        Ok(bytes)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, MailboxError> {
        if bytes.is_empty() || bytes.len() > MAX_ADDRESS_BYTES {
            return Err(MailboxError::Invalid("mailbox address size is invalid"));
        }
        let address: Self = postcard::from_bytes(bytes)?;
        address.verify()?;
        Ok(address)
    }
}

pub struct MailboxReadCapability(SigningKey);

impl MailboxReadCapability {
    pub fn generate() -> Result<Self, MailboxError> {
        Ok(Self(random_signing_key()?))
    }

    pub fn from_secret_bytes(secret: [u8; KEY_BYTES]) -> Self {
        Self(SigningKey::from_bytes(&secret))
    }

    pub fn secret_bytes(&self) -> [u8; KEY_BYTES] {
        self.0.to_bytes()
    }

    pub fn read_key(&self) -> MailboxReadKey {
        MailboxReadKey(self.0.verifying_key().to_bytes())
    }

    pub fn authorize_list(
        &self,
        address: MailboxAddress,
        nonce: MailboxRequestNonce,
    ) -> Result<MailboxReadAuthorization, MailboxError> {
        self.authorize_list_page(address, nonce, None, MAX_MAILBOX_PAGE_ITEMS)
    }

    pub fn authorize_list_page(
        &self,
        address: MailboxAddress,
        nonce: MailboxRequestNonce,
        after_item_id: Option<MailboxItemId>,
        limit: u16,
    ) -> Result<MailboxReadAuthorization, MailboxError> {
        if !(1..=MAX_MAILBOX_PAGE_ITEMS).contains(&limit) {
            return Err(MailboxError::Invalid(
                "mailbox list page limit is outside protocol bounds",
            ));
        }
        self.authorize(
            address,
            MailboxReadOperation::List {
                nonce,
                after_item_id,
                limit,
            },
        )
    }

    pub fn authorize_delete(
        &self,
        address: MailboxAddress,
        item_id: MailboxItemId,
        receipt_id: MailboxReceiptId,
    ) -> Result<MailboxReadAuthorization, MailboxError> {
        self.authorize(
            address,
            MailboxReadOperation::Delete {
                item_id,
                receipt_id,
            },
        )
    }

    fn authorize(
        &self,
        address: MailboxAddress,
        operation: MailboxReadOperation,
    ) -> Result<MailboxReadAuthorization, MailboxError> {
        address.verify()?;
        if address.read_key() != self.read_key() {
            return Err(MailboxError::CapabilityMismatch);
        }
        let content = MailboxReadAuthorizationContent {
            version: VERSION,
            mailbox_id: address.mailbox_id(),
            read_key: self.read_key(),
            operation,
        };
        let signature = self
            .0
            .sign(&signing_bytes(READ_AUTHORIZATION_DOMAIN, &content)?);
        Ok(MailboxReadAuthorization {
            content,
            signature: signature.to_bytes().to_vec(),
        })
    }
}

pub struct MailboxWriteCapability(SigningKey);

impl MailboxWriteCapability {
    pub fn generate() -> Result<Self, MailboxError> {
        Ok(Self(random_signing_key()?))
    }

    pub fn from_secret_bytes(secret: [u8; KEY_BYTES]) -> Self {
        Self(SigningKey::from_bytes(&secret))
    }

    pub fn secret_bytes(&self) -> [u8; KEY_BYTES] {
        self.0.to_bytes()
    }

    pub fn write_key(&self) -> MailboxWriteKey {
        MailboxWriteKey(self.0.verifying_key().to_bytes())
    }

    pub fn authorize(
        &self,
        address: MailboxAddress,
        item_id: MailboxItemId,
        requested_ttl_seconds: u64,
        envelope: &[u8],
    ) -> Result<MailboxWriteAuthorization, MailboxError> {
        address.verify()?;
        validate_ttl(requested_ttl_seconds)?;
        validate_envelope_bytes(envelope)?;
        if address.write_key() != self.write_key() {
            return Err(MailboxError::CapabilityMismatch);
        }
        let content = MailboxWriteAuthorizationContent {
            version: VERSION,
            mailbox_id: address.mailbox_id(),
            item_id,
            write_key: self.write_key(),
            requested_ttl_seconds,
            envelope_bytes: envelope.len() as u64,
            envelope_digest: *blake3::hash(envelope).as_bytes(),
        };
        let signature = self
            .0
            .sign(&signing_bytes(WRITE_AUTHORIZATION_DOMAIN, &content)?);
        Ok(MailboxWriteAuthorization {
            content,
            signature: signature.to_bytes().to_vec(),
        })
    }
}

pub struct MailboxStoreIdentity(SigningKey);

impl MailboxStoreIdentity {
    pub fn generate() -> Result<Self, MailboxError> {
        Ok(Self(random_signing_key()?))
    }

    pub fn from_secret_bytes(secret: [u8; KEY_BYTES]) -> Self {
        Self(SigningKey::from_bytes(&secret))
    }

    pub fn secret_bytes(&self) -> [u8; KEY_BYTES] {
        self.0.to_bytes()
    }

    pub fn store_key(&self) -> MailboxStoreKey {
        MailboxStoreKey(self.0.verifying_key().to_bytes())
    }

    pub fn storage_offer(
        &self,
        provider_endpoint: Vec<u8>,
        capacity_hint_bytes: u64,
        max_record_bytes: u64,
        issued_at_unix_seconds: u64,
        validity_seconds: u64,
    ) -> Result<SignedMailboxStorageOffer, MailboxError> {
        let expires_at_unix_seconds =
            issued_at_unix_seconds
                .checked_add(validity_seconds)
                .ok_or(MailboxError::Invalid(
                    "mailbox storage offer expiry overflow",
                ))?;
        let content = MailboxStorageOfferContent {
            version: VERSION,
            provider_endpoint,
            store_key: self.store_key(),
            policy_class: MailboxStoragePolicyClass::BoundedVolunteer,
            capacity_hint_bytes,
            max_record_bytes,
            issued_at_unix_seconds,
            expires_at_unix_seconds,
            nonce: random_bytes()?,
        };
        let signature = self.0.sign(&signing_bytes(STORAGE_OFFER_DOMAIN, &content)?);
        let offer = SignedMailboxStorageOffer {
            content,
            signature: signature.to_bytes().to_vec(),
        };
        offer.verify_at(issued_at_unix_seconds)?;
        Ok(offer)
    }

    pub(crate) fn stored_receipt(
        &self,
        mailbox_id: MailboxId,
        item_id: MailboxItemId,
        envelope: &[u8],
        stored_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> Result<MailboxStoredReceipt, MailboxError> {
        let content = MailboxStoredReceiptContent {
            version: VERSION,
            store_key: self.store_key(),
            mailbox_id,
            item_id,
            envelope_bytes: envelope.len() as u64,
            envelope_digest: *blake3::hash(envelope).as_bytes(),
            stored_at_unix_seconds,
            expires_at_unix_seconds,
        };
        let signature = self
            .0
            .sign(&signing_bytes(STORED_RECEIPT_DOMAIN, &content)?);
        Ok(MailboxStoredReceipt {
            content,
            signature: signature.to_bytes().to_vec(),
        })
    }

    pub(crate) fn delete_receipt(
        &self,
        mailbox_id: MailboxId,
        item_id: MailboxItemId,
        stored_receipt_id: MailboxReceiptId,
        deleted_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
    ) -> Result<MailboxDeleteReceipt, MailboxError> {
        let content = MailboxDeleteReceiptContent {
            version: VERSION,
            store_key: self.store_key(),
            mailbox_id,
            item_id,
            stored_receipt_id,
            deleted_at_unix_seconds,
            expires_at_unix_seconds,
        };
        let signature = self
            .0
            .sign(&signing_bytes(DELETE_RECEIPT_DOMAIN, &content)?);
        Ok(MailboxDeleteReceipt {
            content,
            signature: signature.to_bytes().to_vec(),
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MailboxStoragePolicyClass {
    BoundedVolunteer,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxStorageOfferContent {
    version: u8,
    provider_endpoint: Vec<u8>,
    store_key: MailboxStoreKey,
    policy_class: MailboxStoragePolicyClass,
    capacity_hint_bytes: u64,
    max_record_bytes: u64,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    nonce: [u8; KEY_BYTES],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedMailboxStorageOffer {
    content: MailboxStorageOfferContent,
    signature: Vec<u8>,
}

impl SignedMailboxStorageOffer {
    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<(), MailboxError> {
        let validity = self
            .content
            .expires_at_unix_seconds
            .checked_sub(self.content.issued_at_unix_seconds)
            .ok_or(MailboxError::Invalid(
                "mailbox storage offer time is invalid",
            ))?;
        if self.content.version != VERSION
            || self.content.provider_endpoint.is_empty()
            || self.content.provider_endpoint.len() > MAX_MAILBOX_STORAGE_ENDPOINT_BYTES
            || !(MIN_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS
                ..=MAX_MAILBOX_STORAGE_OFFER_VALIDITY_SECONDS)
                .contains(&validity)
            || self.content.capacity_hint_bytes == 0
            || self.content.max_record_bytes == 0
            || self.content.max_record_bytes > self.content.capacity_hint_bytes
            || now_unix_seconds < self.content.issued_at_unix_seconds
            || now_unix_seconds >= self.content.expires_at_unix_seconds
        {
            return Err(MailboxError::Invalid(
                "mailbox storage offer fields or lifetime are invalid",
            ));
        }
        verify_signature(
            self.content.store_key.verifying_key()?,
            STORAGE_OFFER_DOMAIN,
            &self.content,
            &self.signature,
        )
    }

    pub fn encode(&self, now_unix_seconds: u64) -> Result<Vec<u8>, MailboxError> {
        self.verify_at(now_unix_seconds)?;
        let bytes = postcard::to_allocvec(self)?;
        if bytes.is_empty() || bytes.len() > MAX_MAILBOX_STORAGE_OFFER_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox storage offer size is invalid",
            ));
        }
        Ok(bytes)
    }

    pub fn decode_and_verify(bytes: &[u8], now_unix_seconds: u64) -> Result<Self, MailboxError> {
        if bytes.is_empty() || bytes.len() > MAX_MAILBOX_STORAGE_OFFER_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox storage offer size is invalid",
            ));
        }
        let offer: Self = postcard::from_bytes(bytes)?;
        offer.verify_at(now_unix_seconds)?;
        Ok(offer)
    }

    pub fn provider_endpoint(&self) -> &[u8] {
        &self.content.provider_endpoint
    }

    pub fn store_key(&self) -> MailboxStoreKey {
        self.content.store_key
    }

    pub fn policy_class(&self) -> MailboxStoragePolicyClass {
        self.content.policy_class
    }

    pub fn capacity_hint_bytes(&self) -> u64 {
        self.content.capacity_hint_bytes
    }

    pub fn max_record_bytes(&self) -> u64 {
        self.content.max_record_bytes
    }

    pub fn issued_at_unix_seconds(&self) -> u64 {
        self.content.issued_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxWriteAuthorizationContent {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    write_key: MailboxWriteKey,
    requested_ttl_seconds: u64,
    envelope_bytes: u64,
    envelope_digest: [u8; KEY_BYTES],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxWriteAuthorization {
    content: MailboxWriteAuthorizationContent,
    signature: Vec<u8>,
}

impl MailboxWriteAuthorization {
    pub fn verify(&self, address: MailboxAddress, envelope: &[u8]) -> Result<(), MailboxError> {
        address.verify()?;
        validate_envelope_bytes(envelope)?;
        validate_ttl(self.content.requested_ttl_seconds)?;
        if self.content.version != VERSION
            || self.content.mailbox_id != address.mailbox_id()
            || self.content.write_key != address.write_key()
            || self.content.envelope_bytes != envelope.len() as u64
            || self.content.envelope_digest != *blake3::hash(envelope).as_bytes()
        {
            return Err(MailboxError::Invalid(
                "mailbox write authorization does not match request",
            ));
        }
        verify_signature(
            self.content.write_key.verifying_key()?,
            WRITE_AUTHORIZATION_DOMAIN,
            &self.content,
            &self.signature,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        let bytes = postcard::to_allocvec(self)?;
        if bytes.len() > MAX_AUTHORIZATION_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox write authorization is too large",
            ));
        }
        Ok(bytes)
    }

    pub fn decode_and_verify(
        bytes: &[u8],
        address: MailboxAddress,
        envelope: &[u8],
    ) -> Result<Self, MailboxError> {
        if bytes.is_empty() || bytes.len() > MAX_AUTHORIZATION_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox write authorization size is invalid",
            ));
        }
        let authorization: Self = postcard::from_bytes(bytes)?;
        authorization.verify(address, envelope)?;
        Ok(authorization)
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.content.mailbox_id
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.content.item_id
    }

    pub fn requested_ttl_seconds(&self) -> u64 {
        self.content.requested_ttl_seconds
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MailboxReadOperation {
    List {
        nonce: MailboxRequestNonce,
        after_item_id: Option<MailboxItemId>,
        limit: u16,
    },
    Delete {
        item_id: MailboxItemId,
        receipt_id: MailboxReceiptId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxReadAuthorizationContent {
    version: u8,
    mailbox_id: MailboxId,
    read_key: MailboxReadKey,
    operation: MailboxReadOperation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxReadAuthorization {
    content: MailboxReadAuthorizationContent,
    signature: Vec<u8>,
}

impl MailboxReadAuthorization {
    pub fn verify(&self, address: MailboxAddress) -> Result<(), MailboxError> {
        address.verify()?;
        if let MailboxReadOperation::List { limit, .. } = self.content.operation
            && !(1..=MAX_MAILBOX_PAGE_ITEMS).contains(&limit)
        {
            return Err(MailboxError::Invalid(
                "mailbox list page limit is outside protocol bounds",
            ));
        }
        if self.content.version != VERSION
            || self.content.mailbox_id != address.mailbox_id()
            || self.content.read_key != address.read_key()
        {
            return Err(MailboxError::Invalid(
                "mailbox read authorization does not match address",
            ));
        }
        verify_signature(
            self.content.read_key.verifying_key()?,
            READ_AUTHORIZATION_DOMAIN,
            &self.content,
            &self.signature,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        let bytes = postcard::to_allocvec(self)?;
        if bytes.len() > MAX_AUTHORIZATION_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox read authorization is too large",
            ));
        }
        Ok(bytes)
    }

    pub fn decode_and_verify(bytes: &[u8], address: MailboxAddress) -> Result<Self, MailboxError> {
        if bytes.is_empty() || bytes.len() > MAX_AUTHORIZATION_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox read authorization size is invalid",
            ));
        }
        let authorization: Self = postcard::from_bytes(bytes)?;
        authorization.verify(address)?;
        Ok(authorization)
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.content.mailbox_id
    }

    pub fn operation(&self) -> MailboxReadOperation {
        self.content.operation
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxEnvelopeContext {
    version: u8,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    created_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxEnvelope {
    context: MailboxEnvelopeContext,
    sealed: SealedMessage,
}

impl MailboxEnvelope {
    pub fn seal(
        mailbox_id: MailboxId,
        item_id: MailboxItemId,
        created_at_unix_seconds: u64,
        expires_at_unix_seconds: u64,
        recipient_key: EncryptionPublicKey,
        plaintext: &[u8],
    ) -> Result<Self, MailboxError> {
        validate_envelope_time(created_at_unix_seconds, expires_at_unix_seconds)?;
        if plaintext.is_empty() || plaintext.len() > MAX_MAILBOX_PLAINTEXT_BYTES {
            return Err(MailboxError::Invalid("mailbox plaintext size is invalid"));
        }
        let context = MailboxEnvelopeContext {
            version: VERSION,
            mailbox_id,
            item_id,
            created_at_unix_seconds,
            expires_at_unix_seconds,
        };
        let aad = postcard::to_allocvec(&context)?;
        let sealed = recipient_key.seal(plaintext, ENVELOPE_HPKE_INFO, &aad)?;
        let envelope = Self { context, sealed };
        validate_envelope_bytes(&envelope.encode()?)?;
        Ok(envelope)
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        self.validate()?;
        let bytes = postcard::to_allocvec(self)?;
        validate_envelope_bytes(&bytes)?;
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, MailboxError> {
        validate_envelope_bytes(bytes)?;
        let envelope: Self = postcard::from_bytes(bytes)?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn open(
        &self,
        expected_mailbox_id: MailboxId,
        expected_item_id: MailboxItemId,
        recipient: &DeviceEncryptionIdentity,
        now_unix_seconds: u64,
    ) -> Result<Vec<u8>, MailboxError> {
        self.validate()?;
        if self.context.mailbox_id != expected_mailbox_id
            || self.context.item_id != expected_item_id
        {
            return Err(MailboxError::Invalid(
                "mailbox envelope address does not match",
            ));
        }
        if self.context.expires_at_unix_seconds <= now_unix_seconds {
            return Err(MailboxError::Expired);
        }
        let aad = postcard::to_allocvec(&self.context)?;
        let plaintext = recipient.open(&self.sealed, ENVELOPE_HPKE_INFO, &aad)?;
        if plaintext.is_empty() || plaintext.len() > MAX_MAILBOX_PLAINTEXT_BYTES {
            return Err(MailboxError::Invalid(
                "opened mailbox plaintext size is invalid",
            ));
        }
        Ok(plaintext)
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.context.mailbox_id
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.context.item_id
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.context.expires_at_unix_seconds
    }

    fn validate(&self) -> Result<(), MailboxError> {
        if self.context.version != VERSION {
            return Err(MailboxError::Invalid(
                "unsupported mailbox envelope version",
            ));
        }
        validate_envelope_time(
            self.context.created_at_unix_seconds,
            self.context.expires_at_unix_seconds,
        )?;
        if self.sealed.encapsulated_key.len() != KEY_BYTES || self.sealed.ciphertext.is_empty() {
            return Err(MailboxError::Invalid("mailbox HPKE payload is invalid"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxStoredReceiptContent {
    version: u8,
    store_key: MailboxStoreKey,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    envelope_bytes: u64,
    envelope_digest: [u8; KEY_BYTES],
    stored_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxStoredReceipt {
    content: MailboxStoredReceiptContent,
    signature: Vec<u8>,
}

impl MailboxStoredReceipt {
    pub fn verify(&self, envelope: &[u8]) -> Result<(), MailboxError> {
        validate_envelope_bytes(envelope)?;
        if self.content.version != VERSION
            || self.content.envelope_bytes != envelope.len() as u64
            || self.content.envelope_digest != *blake3::hash(envelope).as_bytes()
            || self.content.stored_at_unix_seconds >= self.content.expires_at_unix_seconds
        {
            return Err(MailboxError::Invalid(
                "mailbox stored receipt does not match envelope",
            ));
        }
        verify_signature(
            self.content.store_key.verifying_key()?,
            STORED_RECEIPT_DOMAIN,
            &self.content,
            &self.signature,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        self.verify_signature()?;
        let bytes = postcard::to_allocvec(self)?;
        if bytes.len() > MAX_RECEIPT_BYTES {
            return Err(MailboxError::Invalid("mailbox stored receipt is too large"));
        }
        Ok(bytes)
    }

    pub fn decode_and_verify(bytes: &[u8], envelope: &[u8]) -> Result<Self, MailboxError> {
        if bytes.is_empty() || bytes.len() > MAX_RECEIPT_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox stored receipt size is invalid",
            ));
        }
        let receipt: Self = postcard::from_bytes(bytes)?;
        receipt.verify(envelope)?;
        Ok(receipt)
    }

    pub fn receipt_id(&self) -> Result<MailboxReceiptId, MailboxError> {
        self.verify_signature()?;
        let encoded = self.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(RECEIPT_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(MailboxReceiptId(*hasher.finalize().as_bytes()))
    }

    pub fn store_key(&self) -> MailboxStoreKey {
        self.content.store_key
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.content.mailbox_id
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.content.item_id
    }

    pub fn stored_at_unix_seconds(&self) -> u64 {
        self.content.stored_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    pub fn verify_signature(&self) -> Result<(), MailboxError> {
        if self.content.version != VERSION
            || self.content.stored_at_unix_seconds >= self.content.expires_at_unix_seconds
        {
            return Err(MailboxError::Invalid(
                "mailbox stored receipt time is invalid",
            ));
        }
        verify_signature(
            self.content.store_key.verifying_key()?,
            STORED_RECEIPT_DOMAIN,
            &self.content,
            &self.signature,
        )
    }

    pub fn envelope_bytes(&self) -> u64 {
        self.content.envelope_bytes
    }

    pub fn envelope_digest(&self) -> [u8; KEY_BYTES] {
        self.content.envelope_digest
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct MailboxDeleteReceiptContent {
    version: u8,
    store_key: MailboxStoreKey,
    mailbox_id: MailboxId,
    item_id: MailboxItemId,
    stored_receipt_id: MailboxReceiptId,
    deleted_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxDeleteReceipt {
    content: MailboxDeleteReceiptContent,
    signature: Vec<u8>,
}

impl MailboxDeleteReceipt {
    pub fn verify(&self) -> Result<(), MailboxError> {
        if self.content.version != VERSION
            || self.content.deleted_at_unix_seconds >= self.content.expires_at_unix_seconds
        {
            return Err(MailboxError::Invalid(
                "mailbox delete receipt time is invalid",
            ));
        }
        verify_signature(
            self.content.store_key.verifying_key()?,
            DELETE_RECEIPT_DOMAIN,
            &self.content,
            &self.signature,
        )
    }

    pub fn receipt_id(&self) -> Result<MailboxDeleteReceiptId, MailboxError> {
        self.verify()?;
        let encoded = self.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(DELETE_RECEIPT_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(MailboxDeleteReceiptId(*hasher.finalize().as_bytes()))
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        self.verify()?;
        let bytes = postcard::to_allocvec(self)?;
        if bytes.len() > MAX_RECEIPT_BYTES {
            return Err(MailboxError::Invalid("mailbox delete receipt is too large"));
        }
        Ok(bytes)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, MailboxError> {
        if bytes.is_empty() || bytes.len() > MAX_RECEIPT_BYTES {
            return Err(MailboxError::Invalid(
                "mailbox delete receipt size is invalid",
            ));
        }
        let receipt: Self = postcard::from_bytes(bytes)?;
        receipt.verify()?;
        Ok(receipt)
    }

    pub fn stored_receipt_id(&self) -> MailboxReceiptId {
        self.content.stored_receipt_id
    }

    pub fn store_key(&self) -> MailboxStoreKey {
        self.content.store_key
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.content.mailbox_id
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.content.item_id
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }
}

fn validate_ttl(ttl_seconds: u64) -> Result<(), MailboxError> {
    if !(MIN_MAILBOX_TTL_SECONDS..=MAX_MAILBOX_TTL_SECONDS).contains(&ttl_seconds) {
        return Err(MailboxError::Invalid(
            "mailbox TTL is outside protocol bounds",
        ));
    }
    Ok(())
}

fn validate_envelope_time(created: u64, expires: u64) -> Result<(), MailboxError> {
    let ttl = expires
        .checked_sub(created)
        .ok_or(MailboxError::Invalid("mailbox envelope time is invalid"))?;
    validate_ttl(ttl)
}

fn validate_envelope_bytes(envelope: &[u8]) -> Result<(), MailboxError> {
    if envelope.is_empty() || envelope.len() > MAX_MAILBOX_ENVELOPE_BYTES {
        return Err(MailboxError::Invalid("mailbox envelope size is invalid"));
    }
    Ok(())
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>, MailboxError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn verify_signature<T: Serialize>(
    key: VerifyingKey,
    domain: &[u8],
    content: &T,
    signature: &[u8],
) -> Result<(), MailboxError> {
    let signature = Signature::try_from(signature).map_err(MailboxError::InvalidSignature)?;
    key.verify_strict(&signing_bytes(domain, content)?, &signature)
        .map_err(MailboxError::AuthorizationFailed)
}

fn random_signing_key() -> Result<SigningKey, MailboxError> {
    let mut secret = Zeroizing::new([0_u8; KEY_BYTES]);
    getrandom::fill(secret.as_mut()).map_err(MailboxError::SecureRandom)?;
    Ok(SigningKey::from_bytes(&secret))
}

fn random_bytes() -> Result<[u8; KEY_BYTES], MailboxError> {
    let mut bytes = [0_u8; KEY_BYTES];
    getrandom::fill(&mut bytes).map_err(MailboxError::SecureRandom)?;
    Ok(bytes)
}

fn decode_hex(value: &str) -> Result<[u8; KEY_BYTES], MailboxError> {
    if value.len() != KEY_BYTES * 2 {
        return Err(MailboxError::InvalidHex);
    }
    let mut bytes = [0_u8; KEY_BYTES];
    for (index, output) in bytes.iter_mut().enumerate() {
        let high = hex_nibble(value.as_bytes()[index * 2]).ok_or(MailboxError::InvalidHex)?;
        let low = hex_nibble(value.as_bytes()[index * 2 + 1]).ok_or(MailboxError::InvalidHex)?;
        *output = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum MailboxError {
    #[error("mailbox encoding is invalid")]
    Encoding(#[from] postcard::Error),
    #[error("mailbox encryption failed")]
    Crypto(#[from] kilogram_crypto::CryptoError),
    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),
    #[error("mailbox {kind} public key is invalid")]
    InvalidPublicKey {
        kind: &'static str,
        #[source]
        source: ed25519_dalek::SignatureError,
    },
    #[error("mailbox signature encoding is invalid")]
    InvalidSignature(#[source] ed25519_dalek::SignatureError),
    #[error("mailbox authorization signature failed")]
    AuthorizationFailed(#[source] ed25519_dalek::SignatureError),
    #[error("mailbox capability does not match the address")]
    CapabilityMismatch,
    #[error("mailbox value is invalid: {0}")]
    Invalid(&'static str),
    #[error("mailbox hexadecimal value is not canonical lowercase 32-byte data")]
    InvalidHex,
    #[error("mailbox envelope has expired")]
    Expired,
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    fn fixture() -> Result<(
        MailboxReadCapability,
        MailboxWriteCapability,
        MailboxAddress,
        DeviceEncryptionIdentity,
    )> {
        let read = MailboxReadCapability::from_secret_bytes([1_u8; 32]);
        let write = MailboxWriteCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let address = MailboxAddress::decode_and_verify(&address.encode()?)?;
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([3_u8; 32]);
        Ok((read, write, address, recipient))
    }

    #[test]
    fn opaque_capabilities_authorize_hpke_delivery_without_application_ids() -> Result<()> {
        let (read, write, address, recipient) = fixture()?;
        let other_write = MailboxWriteCapability::from_secret_bytes([4_u8; 32]);
        let item = MailboxItemId::from_bytes([5_u8; 32]);
        let envelope = MailboxEnvelope::seal(
            address.mailbox_id(),
            item,
            1_000,
            1_600,
            recipient.public_key(),
            b"authorized-event-bytes",
        )?;
        let encoded = envelope.encode()?;
        let authorization = write.authorize(address, item, 600, &encoded)?;
        authorization.verify(address, &encoded)?;
        assert!(other_write.authorize(address, item, 600, &encoded).is_err());

        let decoded = MailboxEnvelope::decode(&encoded)?;
        assert_eq!(
            decoded.open(address.mailbox_id(), item, &recipient, 1_001)?,
            b"authorized-event-bytes"
        );
        assert!(
            decoded
                .open(address.mailbox_id(), item, &recipient, 1_600)
                .is_err()
        );

        let list = read.authorize_list(address, MailboxRequestNonce::from_bytes([6_u8; 32]))?;
        list.verify(address)?;
        assert!(matches!(
            list.operation(),
            MailboxReadOperation::List { .. }
        ));
        assert!(
            !encoded
                .windows(b"authorized-event-bytes".len())
                .any(|window| window == b"authorized-event-bytes")
        );
        Ok(())
    }

    #[test]
    fn storage_offer_is_store_signed_bounded_and_expiring() -> Result<()> {
        let identity = MailboxStoreIdentity::from_secret_bytes([9_u8; 32]);
        let offer = identity.storage_offer(vec![7_u8; 128], 200 * 1024 * 1024, 1024, 1_000, 300)?;
        let encoded = offer.encode(1_001)?;
        let decoded = SignedMailboxStorageOffer::decode_and_verify(&encoded, 1_001)?;
        assert_eq!(decoded.store_key(), identity.store_key());
        assert_eq!(
            decoded.policy_class(),
            MailboxStoragePolicyClass::BoundedVolunteer
        );
        assert_eq!(decoded.capacity_hint_bytes(), 200 * 1024 * 1024);
        assert_eq!(decoded.max_record_bytes(), 1024);
        assert_eq!(decoded.provider_endpoint(), &[7_u8; 128]);
        assert!(SignedMailboxStorageOffer::decode_and_verify(&encoded, 1_300).is_err());

        let mut tampered = decoded;
        tampered.content.capacity_hint_bytes += 1;
        assert!(tampered.verify_at(1_001).is_err());
        assert!(
            identity
                .storage_offer(Vec::new(), 200, 100, 1_000, 300)
                .is_err()
        );
        Ok(())
    }
}
