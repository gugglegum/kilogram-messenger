//! Dormant offline trust-root and bounded operational release-key rotation.
//!
//! The trust root signs complete monotonic keyring snapshots. Operational keys
//! may authorize normal rollout policies, containment policies, or both. This
//! module performs no I/O and is not consumed by the runtime.

use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ReleasePolicyAuthority;

const KEYRING_VERSION: u8 = 1;
const KEYRING_SIGNATURE_DOMAIN: &[u8] = b"kilogram:rollout:keyring-signature:v1\0";
const KEYRING_ID_DOMAIN: &[u8] = b"kilogram:rollout:keyring-id:v1\0";
const SIGNING_KEY_ID_DOMAIN: &[u8] = b"kilogram:rollout:signing-key-id:v1\0";

pub const MAX_RELEASE_SIGNING_KEYS: usize = 8;
pub const MAX_SIGNED_RELEASE_KEYRING_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseTrustRoot([u8; 32]);

impl ReleaseTrustRoot {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, ReleaseTrustError> {
        VerifyingKey::from_bytes(&bytes).map_err(ReleaseTrustError::InvalidTrustRoot)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    fn verifying_key(self) -> Result<VerifyingKey, ReleaseTrustError> {
        VerifyingKey::from_bytes(&self.0).map_err(ReleaseTrustError::InvalidTrustRoot)
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleaseKeyringId([u8; 32]);

impl ReleaseKeyringId {
    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) fn from_bytes_for_bundle(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for ReleaseKeyringId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ReleaseKeyringId")
            .field(&hex_lower(&self.0))
            .finish()
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleaseSigningKeyId([u8; 32]);

impl ReleaseSigningKeyId {
    pub fn from_public_key(public_key: [u8; 32]) -> Result<Self, ReleaseTrustError> {
        VerifyingKey::from_bytes(&public_key).map_err(ReleaseTrustError::InvalidSigningKey)?;
        let mut hasher = Sha256::new();
        hasher.update(SIGNING_KEY_ID_DOMAIN);
        hasher.update(public_key);
        Ok(Self(hasher.finalize().into()))
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) fn from_bytes_for_bundle(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for ReleaseSigningKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ReleaseSigningKeyId")
            .field(&hex_lower(&self.0))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReleaseSigningPermissions {
    pub normal_policy: bool,
    pub containment_policy: bool,
}

impl ReleaseSigningPermissions {
    pub const NORMAL_AND_CONTAINMENT: Self = Self {
        normal_policy: true,
        containment_policy: true,
    };

    pub const CONTAINMENT_ONLY: Self = Self {
        normal_policy: false,
        containment_policy: true,
    };

    fn validate(self) -> Result<(), ReleaseTrustError> {
        if !self.normal_policy && !self.containment_policy {
            return Err(ReleaseTrustError::EmptySigningPermissions);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ReleasePolicyPurpose {
    Normal,
    Containment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseSigningKeySpec {
    pub public_key: [u8; 32],
    pub permissions: ReleaseSigningPermissions,
}

impl ReleaseSigningKeySpec {
    pub fn from_signing_key(
        signing_key: &SigningKey,
        permissions: ReleaseSigningPermissions,
    ) -> Self {
        Self {
            public_key: signing_key.verifying_key().to_bytes(),
            permissions,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReleaseSigningKeyCertificate {
    key_id: [u8; 32],
    public_key: [u8; 32],
    permissions: ReleaseSigningPermissions,
    introduced_at_keyring_epoch: u64,
    revoked_at_keyring_epoch: Option<u64>,
}

impl ReleaseSigningKeyCertificate {
    fn from_spec(spec: ReleaseSigningKeySpec, epoch: u64) -> Result<Self, ReleaseTrustError> {
        spec.permissions.validate()?;
        let key_id = ReleaseSigningKeyId::from_public_key(spec.public_key)?;
        Ok(Self {
            key_id: key_id.0,
            public_key: spec.public_key,
            permissions: spec.permissions,
            introduced_at_keyring_epoch: epoch,
            revoked_at_keyring_epoch: None,
        })
    }

    fn id(&self) -> ReleaseSigningKeyId {
        ReleaseSigningKeyId(self.key_id)
    }

    fn is_active_at(&self, epoch: u64) -> bool {
        self.introduced_at_keyring_epoch <= epoch
            && self
                .revoked_at_keyring_epoch
                .is_none_or(|revoked| revoked > epoch)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ReleaseKeyringContent {
    version: u8,
    keyring_epoch: u64,
    previous_keyring_id: Option<[u8; 32]>,
    signing_keys: Vec<ReleaseSigningKeyCertificate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedReleaseKeyring {
    content: ReleaseKeyringContent,
    signature: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
struct SignedReleaseKeyringWire {
    content: ReleaseKeyringContent,
    signature: Vec<u8>,
}

impl SignedReleaseKeyring {
    pub fn issue_genesis(
        trust_root: &SigningKey,
        signing_keys: &[ReleaseSigningKeySpec],
    ) -> Result<Self, ReleaseTrustError> {
        let certificates = signing_keys
            .iter()
            .copied()
            .map(|spec| ReleaseSigningKeyCertificate::from_spec(spec, 1))
            .collect::<Result<Vec<_>, _>>()?;
        Self::issue(trust_root, 1, None, certificates)
    }

    pub fn issue_successor(
        trust_root: &SigningKey,
        previous: &Self,
        additions: &[ReleaseSigningKeySpec],
        revocations: &[ReleaseSigningKeyId],
    ) -> Result<Self, ReleaseTrustError> {
        let root = ReleaseTrustRoot::from_bytes(trust_root.verifying_key().to_bytes())?;
        previous.verify(root)?;
        let epoch = previous
            .keyring_epoch()
            .checked_add(1)
            .ok_or(ReleaseTrustError::KeyringEpochOverflow)?;
        let mut certificates = previous.content.signing_keys.clone();
        for revoked in revocations {
            let certificate = certificates
                .iter_mut()
                .find(|candidate| candidate.id() == *revoked)
                .ok_or(ReleaseTrustError::UnknownRevocationTarget)?;
            if certificate.revoked_at_keyring_epoch.is_some() {
                return Err(ReleaseTrustError::AlreadyRevoked);
            }
            certificate.revoked_at_keyring_epoch = Some(epoch);
        }
        for spec in additions {
            let certificate = ReleaseSigningKeyCertificate::from_spec(*spec, epoch)?;
            if certificates
                .iter()
                .any(|known| known.id() == certificate.id())
            {
                return Err(ReleaseTrustError::DuplicateSigningKey);
            }
            certificates.push(certificate);
        }
        Self::issue(
            trust_root,
            epoch,
            Some(previous.keyring_id()?.0),
            certificates,
        )
    }

    fn issue(
        trust_root: &SigningKey,
        keyring_epoch: u64,
        previous_keyring_id: Option<[u8; 32]>,
        mut signing_keys: Vec<ReleaseSigningKeyCertificate>,
    ) -> Result<Self, ReleaseTrustError> {
        signing_keys.sort_by_key(ReleaseSigningKeyCertificate::id);
        let content = ReleaseKeyringContent {
            version: KEYRING_VERSION,
            keyring_epoch,
            previous_keyring_id,
            signing_keys,
        };
        validate_keyring_content(&content)?;
        let signature = trust_root
            .sign(&keyring_signing_bytes(&content)?)
            .to_bytes()
            .to_vec();
        let keyring = Self { content, signature };
        keyring.verify(ReleaseTrustRoot::from_bytes(
            trust_root.verifying_key().to_bytes(),
        )?)?;
        Ok(keyring)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ReleaseTrustError> {
        validate_keyring_content(&self.content)?;
        let wire = SignedReleaseKeyringWire {
            content: self.content.clone(),
            signature: self.signature.clone(),
        };
        let encoded = postcard::to_allocvec(&wire).map_err(ReleaseTrustError::KeyringCodec)?;
        if encoded.is_empty() || encoded.len() > MAX_SIGNED_RELEASE_KEYRING_BYTES {
            return Err(ReleaseTrustError::KeyringLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        trust_root: ReleaseTrustRoot,
    ) -> Result<Self, ReleaseTrustError> {
        if encoded.is_empty() || encoded.len() > MAX_SIGNED_RELEASE_KEYRING_BYTES {
            return Err(ReleaseTrustError::KeyringLength(encoded.len()));
        }
        let wire: SignedReleaseKeyringWire =
            postcard::from_bytes(encoded).map_err(ReleaseTrustError::KeyringCodec)?;
        let keyring = Self {
            content: wire.content,
            signature: wire.signature,
        };
        keyring.verify(trust_root)?;
        if keyring.encode()? != encoded {
            return Err(ReleaseTrustError::NonCanonicalKeyring);
        }
        Ok(keyring)
    }

    pub fn verify(&self, trust_root: ReleaseTrustRoot) -> Result<(), ReleaseTrustError> {
        validate_keyring_content(&self.content)?;
        let signature = Signature::try_from(self.signature.as_slice())
            .map_err(ReleaseTrustError::InvalidKeyringSignature)?;
        trust_root
            .verifying_key()?
            .verify_strict(&keyring_signing_bytes(&self.content)?, &signature)
            .map_err(ReleaseTrustError::KeyringAuthorization)
    }

    pub fn verify_successor_of(&self, previous: &Self) -> Result<(), ReleaseTrustError> {
        let expected = previous
            .keyring_epoch()
            .checked_add(1)
            .ok_or(ReleaseTrustError::KeyringEpochOverflow)?;
        if self.keyring_epoch() != expected {
            return Err(ReleaseTrustError::KeyringEpochGap {
                expected,
                found: self.keyring_epoch(),
            });
        }
        if self.previous_keyring_id() != Some(previous.keyring_id()?) {
            return Err(ReleaseTrustError::PreviousKeyringMismatch);
        }
        for old in &previous.content.signing_keys {
            let candidate = self
                .content
                .signing_keys
                .iter()
                .find(|candidate| candidate.key_id == old.key_id)
                .ok_or(ReleaseTrustError::SigningKeyRemoved)?;
            if candidate.public_key != old.public_key
                || candidate.permissions != old.permissions
                || candidate.introduced_at_keyring_epoch != old.introduced_at_keyring_epoch
            {
                return Err(ReleaseTrustError::SigningKeyMutated);
            }
            match (
                old.revoked_at_keyring_epoch,
                candidate.revoked_at_keyring_epoch,
            ) {
                (Some(old_epoch), Some(candidate_epoch)) if old_epoch == candidate_epoch => {}
                (None, None) => {}
                (None, Some(candidate_epoch)) if candidate_epoch == self.keyring_epoch() => {}
                _ => return Err(ReleaseTrustError::InvalidRevocationTransition),
            }
        }
        for candidate in &self.content.signing_keys {
            if previous
                .content
                .signing_keys
                .iter()
                .all(|old| old.key_id != candidate.key_id)
                && (candidate.introduced_at_keyring_epoch != self.keyring_epoch()
                    || candidate.revoked_at_keyring_epoch.is_some())
            {
                return Err(ReleaseTrustError::InvalidKeyIntroduction);
            }
        }
        Ok(())
    }

    pub fn keyring_id(&self) -> Result<ReleaseKeyringId, ReleaseTrustError> {
        let encoded = self.encode()?;
        let mut hasher = Sha256::new();
        hasher.update(KEYRING_ID_DOMAIN);
        hasher.update(encoded);
        Ok(ReleaseKeyringId(hasher.finalize().into()))
    }

    pub fn keyring_epoch(&self) -> u64 {
        self.content.keyring_epoch
    }

    pub fn previous_keyring_id(&self) -> Option<ReleaseKeyringId> {
        self.content.previous_keyring_id.map(ReleaseKeyringId)
    }

    pub fn signing_key_count(&self) -> usize {
        self.content.signing_keys.len()
    }

    pub fn authorize_policy_signer(
        &self,
        key_id: ReleaseSigningKeyId,
        purpose: ReleasePolicyPurpose,
    ) -> Result<ReleasePolicyAuthority, ReleaseTrustError> {
        let certificate = self
            .content
            .signing_keys
            .iter()
            .find(|candidate| candidate.id() == key_id)
            .ok_or(ReleaseTrustError::UnknownSigningKey)?;
        if !certificate.is_active_at(self.keyring_epoch()) {
            return Err(ReleaseTrustError::RevokedSigningKey);
        }
        let permitted = match purpose {
            ReleasePolicyPurpose::Normal => certificate.permissions.normal_policy,
            ReleasePolicyPurpose::Containment => certificate.permissions.containment_policy,
        };
        if !permitted {
            return Err(ReleaseTrustError::SigningPurposeDenied);
        }
        ReleasePolicyAuthority::from_bytes(certificate.public_key)
            .map_err(|error| ReleaseTrustError::ReleaseAuthority(error.to_string()))
    }
}

fn keyring_signing_bytes(content: &ReleaseKeyringContent) -> Result<Vec<u8>, ReleaseTrustError> {
    let encoded = postcard::to_allocvec(content).map_err(ReleaseTrustError::KeyringCodec)?;
    let mut bytes = Vec::with_capacity(KEYRING_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(KEYRING_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn validate_keyring_content(content: &ReleaseKeyringContent) -> Result<(), ReleaseTrustError> {
    if content.version != KEYRING_VERSION {
        return Err(ReleaseTrustError::KeyringVersion(content.version));
    }
    if content.keyring_epoch == 0 {
        return Err(ReleaseTrustError::ZeroKeyringEpoch);
    }
    if (content.keyring_epoch == 1) != content.previous_keyring_id.is_none() {
        return Err(ReleaseTrustError::InvalidKeyringGenesis);
    }
    if content.signing_keys.is_empty() || content.signing_keys.len() > MAX_RELEASE_SIGNING_KEYS {
        return Err(ReleaseTrustError::SigningKeyCount(
            content.signing_keys.len(),
        ));
    }
    let mut previous_id = None;
    let mut active_containment_key = false;
    for certificate in &content.signing_keys {
        certificate.permissions.validate()?;
        let expected_id = ReleaseSigningKeyId::from_public_key(certificate.public_key)?;
        if certificate.id() != expected_id {
            return Err(ReleaseTrustError::SigningKeyIdMismatch);
        }
        if previous_id.is_some_and(|previous| previous >= expected_id) {
            return Err(ReleaseTrustError::SigningKeysNotStrictlyOrdered);
        }
        previous_id = Some(expected_id);
        if certificate.introduced_at_keyring_epoch == 0
            || certificate.introduced_at_keyring_epoch > content.keyring_epoch
        {
            return Err(ReleaseTrustError::InvalidKeyIntroduction);
        }
        if certificate.revoked_at_keyring_epoch.is_some_and(|revoked| {
            revoked < certificate.introduced_at_keyring_epoch || revoked > content.keyring_epoch
        }) {
            return Err(ReleaseTrustError::InvalidRevocationEpoch);
        }
        if certificate.is_active_at(content.keyring_epoch)
            && certificate.permissions.containment_policy
        {
            active_containment_key = true;
        }
    }
    if !active_containment_key {
        return Err(ReleaseTrustError::NoActiveContainmentKey);
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[derive(Debug, Error)]
pub enum ReleaseTrustError {
    #[error("invalid offline release trust root: {0}")]
    InvalidTrustRoot(ed25519_dalek::SignatureError),
    #[error("invalid operational release signing key: {0}")]
    InvalidSigningKey(ed25519_dalek::SignatureError),
    #[error("release signing key permissions must not be empty")]
    EmptySigningPermissions,
    #[error("release keyring has invalid length {0}")]
    KeyringLength(usize),
    #[error("release keyring uses unsupported version {0}")]
    KeyringVersion(u8),
    #[error("release keyring encoding is invalid: {0}")]
    KeyringCodec(postcard::Error),
    #[error("release keyring is not canonically encoded")]
    NonCanonicalKeyring,
    #[error("release keyring signature is malformed: {0}")]
    InvalidKeyringSignature(ed25519_dalek::SignatureError),
    #[error("release keyring is not authorized by the offline trust root: {0}")]
    KeyringAuthorization(ed25519_dalek::SignatureError),
    #[error("release keyring epoch must not be zero")]
    ZeroKeyringEpoch,
    #[error("release keyring genesis/predecessor fields are inconsistent")]
    InvalidKeyringGenesis,
    #[error("release keyring epoch overflowed")]
    KeyringEpochOverflow,
    #[error("release keyring epoch must be {expected}, found {found}")]
    KeyringEpochGap { expected: u64, found: u64 },
    #[error("release keyring predecessor does not match the durable head")]
    PreviousKeyringMismatch,
    #[error("release keyring contains invalid signing-key count {0}")]
    SigningKeyCount(usize),
    #[error("release signing keys must be strictly ordered without duplicates")]
    SigningKeysNotStrictlyOrdered,
    #[error("release signing-key identifier does not match its public key")]
    SigningKeyIdMismatch,
    #[error("release signing key was removed from a successor snapshot")]
    SigningKeyRemoved,
    #[error("release signing key was mutated in a successor snapshot")]
    SigningKeyMutated,
    #[error("release signing key introduction is invalid")]
    InvalidKeyIntroduction,
    #[error("release signing-key revocation epoch is invalid")]
    InvalidRevocationEpoch,
    #[error("release signing-key revocation transition is invalid")]
    InvalidRevocationTransition,
    #[error("release keyring must retain at least one active containment key")]
    NoActiveContainmentKey,
    #[error("release signing-key revocation targets an unknown key")]
    UnknownRevocationTarget,
    #[error("release signing key is already revoked")]
    AlreadyRevoked,
    #[error("release signing key already exists")]
    DuplicateSigningKey,
    #[error("release signing key is unknown")]
    UnknownSigningKey,
    #[error("release signing key is revoked")]
    RevokedSigningKey,
    #[error("release signing key is not authorized for this policy purpose")]
    SigningPurposeDenied,
    #[error("authorized operational key is not a valid release-policy authority: {0}")]
    ReleaseAuthority(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    fn root(key: &SigningKey) -> Result<ReleaseTrustRoot, ReleaseTrustError> {
        ReleaseTrustRoot::from_bytes(key.verifying_key().to_bytes())
    }

    fn id(key: &SigningKey) -> Result<ReleaseSigningKeyId, ReleaseTrustError> {
        ReleaseSigningKeyId::from_public_key(key.verifying_key().to_bytes())
    }

    #[test]
    fn keyring_is_canonical_root_bound_and_tamper_evident() -> Result<(), ReleaseTrustError> {
        let trust_root = key(1);
        let foreign_root = key(2);
        let operational = key(3);
        let keyring = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &operational,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let encoded = keyring.encode()?;
        assert_eq!(
            SignedReleaseKeyring::decode_and_verify(&encoded, root(&trust_root)?)?,
            keyring
        );
        assert!(SignedReleaseKeyring::decode_and_verify(&encoded, root(&foreign_root)?).is_err());
        let mut tampered = encoded;
        let last = tampered
            .last_mut()
            .ok_or(ReleaseTrustError::KeyringLength(0))?;
        *last ^= 1;
        assert!(SignedReleaseKeyring::decode_and_verify(&tampered, root(&trust_root)?).is_err());
        Ok(())
    }

    #[test]
    fn rotation_revokes_old_key_without_removing_history() -> Result<(), ReleaseTrustError> {
        let trust_root = key(4);
        let old = key(5);
        let new = key(6);
        let genesis = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &old,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let rotated = SignedReleaseKeyring::issue_successor(
            &trust_root,
            &genesis,
            &[ReleaseSigningKeySpec::from_signing_key(
                &new,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
            &[id(&old)?],
        )?;
        rotated.verify_successor_of(&genesis)?;
        assert_eq!(rotated.keyring_epoch(), 2);
        assert_eq!(rotated.signing_key_count(), 2);
        assert!(
            rotated
                .authorize_policy_signer(id(&old)?, ReleasePolicyPurpose::Containment)
                .is_err()
        );
        assert!(
            rotated
                .authorize_policy_signer(id(&new)?, ReleasePolicyPurpose::Normal)
                .is_ok()
        );
        Ok(())
    }

    #[test]
    fn containment_only_key_cannot_authorize_normal_policy() -> Result<(), ReleaseTrustError> {
        let trust_root = key(7);
        let containment = key(8);
        let keyring = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &containment,
                ReleaseSigningPermissions::CONTAINMENT_ONLY,
            )],
        )?;
        assert!(
            keyring
                .authorize_policy_signer(id(&containment)?, ReleasePolicyPurpose::Normal)
                .is_err()
        );
        assert!(
            keyring
                .authorize_policy_signer(id(&containment)?, ReleasePolicyPurpose::Containment)
                .is_ok()
        );
        Ok(())
    }

    #[test]
    fn successor_rejects_unknown_duplicate_and_repeated_revocation() -> Result<(), ReleaseTrustError>
    {
        let trust_root = key(9);
        let old = key(10);
        let genesis = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &old,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        assert!(
            SignedReleaseKeyring::issue_successor(&trust_root, &genesis, &[], &[id(&key(11))?])
                .is_err()
        );
        assert!(
            SignedReleaseKeyring::issue_successor(
                &trust_root,
                &genesis,
                &[ReleaseSigningKeySpec::from_signing_key(
                    &old,
                    ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
                )],
                &[]
            )
            .is_err()
        );
        let next = SignedReleaseKeyring::issue_successor(
            &trust_root,
            &genesis,
            &[ReleaseSigningKeySpec::from_signing_key(
                &key(12),
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
            &[id(&old)?],
        )?;
        assert!(
            SignedReleaseKeyring::issue_successor(&trust_root, &next, &[], &[id(&old)?]).is_err()
        );
        Ok(())
    }

    #[test]
    fn keyring_has_a_hard_key_count_bound() -> Result<(), ReleaseTrustError> {
        let trust_root = key(13);
        let specs = (20_u8..=28)
            .map(|byte| {
                ReleaseSigningKeySpec::from_signing_key(
                    &key(byte),
                    ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(specs.len(), MAX_RELEASE_SIGNING_KEYS + 1);
        assert!(SignedReleaseKeyring::issue_genesis(&trust_root, &specs).is_err());
        Ok(())
    }

    #[test]
    fn successor_validation_rejects_fork_and_epoch_skip() -> Result<(), ReleaseTrustError> {
        let trust_root = key(14);
        let first = key(15);
        let genesis = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &first,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let second = SignedReleaseKeyring::issue_successor(
            &trust_root,
            &genesis,
            &[ReleaseSigningKeySpec::from_signing_key(
                &key(16),
                ReleaseSigningPermissions::CONTAINMENT_ONLY,
            )],
            &[],
        )?;
        let fork = SignedReleaseKeyring::issue_successor(
            &trust_root,
            &genesis,
            &[ReleaseSigningKeySpec::from_signing_key(
                &key(17),
                ReleaseSigningPermissions::CONTAINMENT_ONLY,
            )],
            &[],
        )?;
        assert!(fork.verify_successor_of(&second).is_err());
        let third = SignedReleaseKeyring::issue_successor(&trust_root, &second, &[], &[])?;
        assert!(third.verify_successor_of(&genesis).is_err());
        Ok(())
    }
}
