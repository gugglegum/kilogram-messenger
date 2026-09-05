//! Narrow application-facing wrapper around the HPKE ciphersuite used by Kilogram.

use hpke::{
    Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable, aead::ChaCha20Poly1305,
    kdf::HkdfSha256, kem::X25519HkdfSha256, single_shot_open, single_shot_seal,
};
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{ZeroizeOnDrop, Zeroizing};

pub const ENCRYPTION_KEY_BYTES: usize = 32;

type Kem = X25519HkdfSha256;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct EncryptionPublicKey([u8; ENCRYPTION_KEY_BYTES]);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SealedMessage {
    pub encapsulated_key: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Persistent input keying material from which the RFC 9180 X25519 keypair is derived.
#[derive(ZeroizeOnDrop)]
pub struct DeviceEncryptionIdentity {
    key_seed: [u8; ENCRYPTION_KEY_BYTES],
}

impl DeviceEncryptionIdentity {
    pub fn generate() -> Result<Self, CryptoError> {
        let mut key_seed = [0_u8; ENCRYPTION_KEY_BYTES];
        getrandom::fill(&mut key_seed).map_err(CryptoError::SecureRandom)?;
        Ok(Self { key_seed })
    }

    pub fn from_secret_bytes(key_seed: [u8; ENCRYPTION_KEY_BYTES]) -> Self {
        Self { key_seed }
    }

    pub fn secret_bytes(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
        self.key_seed
    }

    pub fn public_key(&self) -> EncryptionPublicKey {
        let (_, public_key) = Kem::derive_keypair(&self.key_seed);
        EncryptionPublicKey(copy_fixed(&public_key.to_bytes()))
    }

    /// Derives a symmetric, domain-separated key with another persistent
    /// Device encryption identity. The raw X25519 result is never returned.
    pub fn derive_pairwise_key(
        &self,
        peer: EncryptionPublicKey,
        context: &str,
    ) -> Result<[u8; ENCRYPTION_KEY_BYTES], CryptoError> {
        if context.is_empty() {
            return Err(CryptoError::InvalidPairwiseContext);
        }
        let (private_key, public_key) = Kem::derive_keypair(&self.key_seed);
        let private_key = Zeroizing::new(copy_fixed(&private_key.to_bytes()));
        let local_public_key = copy_fixed(&public_key.to_bytes());
        let shared_secret = Zeroizing::new(x25519_dalek::x25519(*private_key, peer.0));
        if shared_secret.iter().all(|byte| *byte == 0) {
            return Err(CryptoError::NonContributoryPairwiseKey);
        }
        let (first_public_key, second_public_key) = if local_public_key <= peer.0 {
            (local_public_key, peer.0)
        } else {
            (peer.0, local_public_key)
        };
        let mut material = Zeroizing::new(Vec::with_capacity(96));
        material.extend_from_slice(shared_secret.as_ref());
        material.extend_from_slice(&first_public_key);
        material.extend_from_slice(&second_public_key);
        Ok(blake3::derive_key(context, material.as_slice()))
    }

    pub fn open(
        &self,
        sealed: &SealedMessage,
        info: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let (private_key, _) = Kem::derive_keypair(&self.key_seed);
        let encapsulated_key =
            <Kem as KemTrait>::EncappedKey::from_bytes(&sealed.encapsulated_key)?;
        Ok(single_shot_open::<ChaCha20Poly1305, HkdfSha256, Kem>(
            &OpModeR::Base,
            &private_key,
            &encapsulated_key,
            info,
            &sealed.ciphertext,
            aad,
        )?)
    }
}

impl EncryptionPublicKey {
    pub fn from_bytes(bytes: [u8; ENCRYPTION_KEY_BYTES]) -> Result<Self, CryptoError> {
        <Kem as KemTrait>::PublicKey::from_bytes(&bytes)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; ENCRYPTION_KEY_BYTES] {
        &self.0
    }

    pub fn seal(
        &self,
        plaintext: &[u8],
        info: &[u8],
        aad: &[u8],
    ) -> Result<SealedMessage, CryptoError> {
        let public_key = <Kem as KemTrait>::PublicKey::from_bytes(&self.0)?;
        let (encapsulated_key, ciphertext) = single_shot_seal::<ChaCha20Poly1305, HkdfSha256, Kem>(
            &OpModeS::Base,
            &public_key,
            info,
            plaintext,
            aad,
        )?;
        Ok(SealedMessage {
            encapsulated_key: encapsulated_key.to_bytes().to_vec(),
            ciphertext,
        })
    }
}

impl fmt::Display for EncryptionPublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn copy_fixed(bytes: &[u8]) -> [u8; ENCRYPTION_KEY_BYTES] {
    let mut fixed = [0_u8; ENCRYPTION_KEY_BYTES];
    fixed.copy_from_slice(bytes);
    fixed
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("secure random generation failed: {0}")]
    SecureRandom(getrandom::Error),

    #[error("HPKE operation failed: {0}")]
    Hpke(#[from] hpke::HpkeError),

    #[error("pairwise key context must not be empty")]
    InvalidPairwiseContext,

    #[error("pairwise X25519 key agreement is non-contributory")]
    NonContributoryPairwiseKey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_message_round_trips() -> Result<(), CryptoError> {
        let recipient = DeviceEncryptionIdentity::generate()?;
        let sealed =
            recipient
                .public_key()
                .seal(b"secret message", b"kilogram test", b"metadata")?;

        assert_eq!(
            recipient.open(&sealed, b"kilogram test", b"metadata")?,
            b"secret message"
        );
        Ok(())
    }

    #[test]
    fn wrong_device_and_tampered_metadata_are_rejected() -> Result<(), CryptoError> {
        let recipient = DeviceEncryptionIdentity::generate()?;
        let other = DeviceEncryptionIdentity::generate()?;
        let sealed =
            recipient
                .public_key()
                .seal(b"secret message", b"kilogram test", b"metadata")?;

        assert!(other.open(&sealed, b"kilogram test", b"metadata").is_err());
        assert!(
            recipient
                .open(&sealed, b"kilogram test", b"tampered")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn pairwise_key_is_symmetric_context_bound_and_peer_specific() -> Result<(), CryptoError> {
        let first = DeviceEncryptionIdentity::generate()?;
        let second = DeviceEncryptionIdentity::generate()?;
        let third = DeviceEncryptionIdentity::generate()?;
        let left = first.derive_pairwise_key(second.public_key(), "kilogram pairwise test v1")?;
        let right = second.derive_pairwise_key(first.public_key(), "kilogram pairwise test v1")?;
        assert_eq!(left, right);
        assert_ne!(
            left,
            first.derive_pairwise_key(third.public_key(), "kilogram pairwise test v1")?
        );
        assert_ne!(
            left,
            first.derive_pairwise_key(second.public_key(), "kilogram pairwise test v2")?
        );
        assert!(first.derive_pairwise_key(second.public_key(), "").is_err());
        Ok(())
    }
}
