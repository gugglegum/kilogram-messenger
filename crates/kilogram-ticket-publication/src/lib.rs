//! Narrow shared contract for authorizing writes to an opaque ticket store.
//!
//! A write key is a random-looking per-peer pseudonym, not a Kilogram Account
//! or Device identity. Its hash is the lookup channel, so an unrelated key
//! cannot claim an already known channel. Every PUT signs the exact channel,
//! generation, length, and body digest.

use std::{fmt, str::FromStr};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

const CHANNEL_ID_DOMAIN: &[u8] = b"kilogram:ticket-publication-capability-channel:v1\0";
const WRITE_AUTHORIZATION_DOMAIN: &[u8] = b"kilogram:ticket-publication-write-authorization:v1\0";
const WRITE_CAPABILITY_DERIVATION_CONTEXT: &str =
    "kilogram ticket publication per-peer write capability v1";
const KEY_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;

pub const WRITE_KEY_HEADER: &str = "x-kilogram-write-key";
pub const WRITE_SIGNATURE_HEADER: &str = "x-kilogram-write-signature";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketPublicationChannelId([u8; KEY_BYTES]);

impl TicketPublicationChannelId {
    pub fn from_write_key(write_key: TicketPublicationWriteKey) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHANNEL_ID_DOMAIN);
        hasher.update(write_key.as_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }
}

impl fmt::Display for TicketPublicationChannelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

impl FromStr for TicketPublicationChannelId {
    type Err = TicketPublicationCapabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(decode_hex::<KEY_BYTES>(value)?))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketPublicationWriteKey([u8; KEY_BYTES]);

impl TicketPublicationWriteKey {
    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Result<Self, TicketPublicationCapabilityError> {
        VerifyingKey::from_bytes(&bytes)
            .map_err(TicketPublicationCapabilityError::InvalidWriteKey)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    pub fn channel_id(&self) -> TicketPublicationChannelId {
        TicketPublicationChannelId::from_write_key(*self)
    }

    pub fn verify(&self) -> Result<(), TicketPublicationCapabilityError> {
        Self::from_bytes(self.0).map(|_| ())
    }

    pub fn verify_authorization(
        &self,
        channel_id: TicketPublicationChannelId,
        generation: u64,
        body: &[u8],
        signature: &[u8],
    ) -> Result<(), TicketPublicationCapabilityError> {
        if self.channel_id() != channel_id {
            return Err(TicketPublicationCapabilityError::ChannelMismatch);
        }
        let verifying_key = VerifyingKey::from_bytes(&self.0)
            .map_err(TicketPublicationCapabilityError::InvalidWriteKey)?;
        let signature = Signature::try_from(signature)
            .map_err(TicketPublicationCapabilityError::InvalidSignatureEncoding)?;
        verifying_key
            .verify_strict(
                &authorization_bytes(channel_id, generation, body),
                &signature,
            )
            .map_err(TicketPublicationCapabilityError::AuthorizationFailed)
    }
}

impl fmt::Display for TicketPublicationWriteKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

impl FromStr for TicketPublicationWriteKey {
    type Err = TicketPublicationCapabilityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_bytes(decode_hex::<KEY_BYTES>(value)?)
    }
}

pub struct TicketPublicationWriteCapability {
    signing_key: SigningKey,
}

impl TicketPublicationWriteCapability {
    /// Derives a distinct write capability from a protected local device seed
    /// and a peer-specific opaque scope. Neither input is sent to the store.
    pub fn derive(device_secret: [u8; KEY_BYTES], peer_scope: &[u8]) -> Self {
        let device_secret = Zeroizing::new(device_secret);
        let mut material = Zeroizing::new(Vec::with_capacity(KEY_BYTES + peer_scope.len()));
        material.extend_from_slice(device_secret.as_ref());
        material.extend_from_slice(peer_scope);
        let capability_secret = Zeroizing::new(blake3::derive_key(
            WRITE_CAPABILITY_DERIVATION_CONTEXT,
            material.as_slice(),
        ));
        Self {
            signing_key: SigningKey::from_bytes(&capability_secret),
        }
    }

    pub fn write_key(&self) -> TicketPublicationWriteKey {
        TicketPublicationWriteKey(self.signing_key.verifying_key().to_bytes())
    }

    pub fn authorize(
        &self,
        channel_id: TicketPublicationChannelId,
        generation: u64,
        body: &[u8],
    ) -> [u8; SIGNATURE_BYTES] {
        self.signing_key
            .sign(&authorization_bytes(channel_id, generation, body))
            .to_bytes()
    }
}

pub fn encode_signature(signature: &[u8; SIGNATURE_BYTES]) -> String {
    let mut encoded = String::with_capacity(SIGNATURE_BYTES * 2);
    for byte in signature {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

pub fn decode_signature(
    encoded: &str,
) -> Result<[u8; SIGNATURE_BYTES], TicketPublicationCapabilityError> {
    decode_hex(encoded)
}

fn authorization_bytes(
    channel_id: TicketPublicationChannelId,
    generation: u64,
    body: &[u8],
) -> Vec<u8> {
    let body_digest = blake3::hash(body);
    let mut bytes = Vec::with_capacity(WRITE_AUTHORIZATION_DOMAIN.len() + 80);
    bytes.extend_from_slice(WRITE_AUTHORIZATION_DOMAIN);
    bytes.extend_from_slice(channel_id.as_bytes());
    bytes.extend_from_slice(&generation.to_be_bytes());
    bytes.extend_from_slice(&(body.len() as u64).to_be_bytes());
    bytes.extend_from_slice(body_digest.as_bytes());
    bytes
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], TicketPublicationCapabilityError> {
    if value.len() != N * 2 {
        return Err(TicketPublicationCapabilityError::InvalidHexLength {
            actual: value.len(),
            expected: N * 2,
        });
    }
    let mut bytes = [0_u8; N];
    for (index, output) in bytes.iter_mut().enumerate() {
        let high = hex_nibble(value.as_bytes()[index * 2])
            .ok_or(TicketPublicationCapabilityError::InvalidHex)?;
        let low = hex_nibble(value.as_bytes()[index * 2 + 1])
            .ok_or(TicketPublicationCapabilityError::InvalidHex)?;
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
pub enum TicketPublicationCapabilityError {
    #[error("hex value has {actual} characters; expected {expected}")]
    InvalidHexLength { actual: usize, expected: usize },
    #[error("hex value is not canonical lowercase hexadecimal")]
    InvalidHex,
    #[error("ticket publication write key is invalid")]
    InvalidWriteKey(#[source] ed25519_dalek::SignatureError),
    #[error("ticket publication write signature encoding is invalid")]
    InvalidSignatureEncoding(#[source] ed25519_dalek::SignatureError),
    #[error("ticket publication channel does not match its write key")]
    ChannelMismatch,
    #[error("ticket publication write authorization failed")]
    AuthorizationFailed(#[source] ed25519_dalek::SignatureError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_capability_is_stable_unlinkable_and_body_bound()
    -> Result<(), TicketPublicationCapabilityError> {
        let first = TicketPublicationWriteCapability::derive([7_u8; 32], b"peer-a");
        let restarted = TicketPublicationWriteCapability::derive([7_u8; 32], b"peer-a");
        let other_peer = TicketPublicationWriteCapability::derive([7_u8; 32], b"peer-b");
        assert_eq!(first.write_key(), restarted.write_key());
        assert_ne!(first.write_key(), other_peer.write_key());
        let channel = first.write_key().channel_id();
        let signature = first.authorize(channel, 4, b"opaque body");
        first
            .write_key()
            .verify_authorization(channel, 4, b"opaque body", &signature)?;
        assert!(
            first
                .write_key()
                .verify_authorization(channel, 5, b"opaque body", &signature)
                .is_err()
        );
        assert!(
            first
                .write_key()
                .verify_authorization(channel, 4, b"changed", &signature)
                .is_err()
        );
        assert!(
            other_peer
                .write_key()
                .verify_authorization(channel, 4, b"opaque body", &signature)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn wire_values_round_trip_and_reject_non_canonical_hex()
    -> Result<(), Box<dyn std::error::Error>> {
        let capability = TicketPublicationWriteCapability::derive([9_u8; 32], b"peer");
        let key = capability.write_key();
        assert_eq!(key.to_string().parse::<TicketPublicationWriteKey>()?, key);
        let channel = key.channel_id();
        assert_eq!(
            channel.to_string().parse::<TicketPublicationChannelId>()?,
            channel
        );
        let signature = capability.authorize(channel, 1, b"body");
        assert_eq!(decode_signature(&encode_signature(&signature))?, signature);
        assert!(
            key.to_string()
                .to_uppercase()
                .parse::<TicketPublicationWriteKey>()
                .is_err()
        );
        Ok(())
    }
}
