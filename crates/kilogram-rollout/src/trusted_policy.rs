//! Rotation-aware, self-contained release-policy bundle.

use std::fmt;

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ReleaseArtifactId, ReleasePolicyAuthority, RolloutDisposition, RolloutError, RolloutPhase,
    SignedReleasePolicy,
    trust::{
        ReleaseKeyringId, ReleasePolicyPurpose, ReleaseSigningKeyId, ReleaseTrustError,
        ReleaseTrustRoot, SignedReleaseKeyring,
    },
};

const TRUSTED_POLICY_BUNDLE_VERSION: u8 = 1;
const TRUSTED_POLICY_BUNDLE_ID_DOMAIN: &[u8] = b"kilogram:rollout:trusted-policy-bundle-id:v1\0";
pub const MAX_TRUSTED_RELEASE_POLICY_BUNDLE_BYTES: usize = 8_192;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrustedPolicyBundleId([u8; 32]);

impl TrustedPolicyBundleId {
    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub(crate) fn from_bytes_for_store(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for TrustedPolicyBundleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TrustedPolicyBundleId")
            .field(&hex_lower(&self.0))
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TrustedReleasePolicyBundleWire {
    version: u8,
    keyring_id: [u8; 32],
    signer_key_id: [u8; 32],
    purpose: ReleasePolicyPurpose,
    signed_keyring: Vec<u8>,
    signed_policy: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedReleasePolicyBundle {
    wire: TrustedReleasePolicyBundleWire,
    keyring: SignedReleaseKeyring,
    policy: SignedReleasePolicy,
}

impl TrustedReleasePolicyBundle {
    pub fn issue_genesis_hold(
        trust_root: ReleaseTrustRoot,
        keyring: &SignedReleaseKeyring,
        policy_signing_key: &SigningKey,
        release_artifact_id: ReleaseArtifactId,
    ) -> Result<Self, TrustedPolicyError> {
        keyring.verify(trust_root)?;
        let signer_key_id =
            ReleaseSigningKeyId::from_public_key(policy_signing_key.verifying_key().to_bytes())?;
        let authority =
            keyring.authorize_policy_signer(signer_key_id, ReleasePolicyPurpose::Normal)?;
        let policy =
            SignedReleasePolicy::issue_genesis_hold(policy_signing_key, release_artifact_id)?;
        policy.verify(authority)?;
        Self::assemble(keyring, signer_key_id, ReleasePolicyPurpose::Normal, policy)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn issue_successor(
        trust_root: ReleaseTrustRoot,
        keyring: &SignedReleaseKeyring,
        policy_signing_key: &SigningKey,
        previous: &Self,
        release_artifact_id: ReleaseArtifactId,
        phase: RolloutPhase,
        disposition: RolloutDisposition,
        cohort_upper_bound_basis_points: u16,
    ) -> Result<Self, TrustedPolicyError> {
        keyring.verify(trust_root)?;
        previous.verify(trust_root)?;
        if keyring.keyring_id()? != previous.keyring.keyring_id()? {
            keyring.verify_successor_of(&previous.keyring)?;
        }
        let purpose = purpose_for_disposition(disposition);
        let signer_key_id =
            ReleaseSigningKeyId::from_public_key(policy_signing_key.verifying_key().to_bytes())?;
        let authority = keyring.authorize_policy_signer(signer_key_id, purpose)?;
        let previous_authority = previous.policy_authority()?;
        let policy = SignedReleasePolicy::issue_successor_with_previous_authority(
            policy_signing_key,
            &previous.policy,
            previous_authority,
            release_artifact_id,
            phase,
            disposition,
            cohort_upper_bound_basis_points,
        )?;
        policy.verify(authority)?;
        Self::assemble(keyring, signer_key_id, purpose, policy)
    }

    fn assemble(
        keyring: &SignedReleaseKeyring,
        signer_key_id: ReleaseSigningKeyId,
        purpose: ReleasePolicyPurpose,
        policy: SignedReleasePolicy,
    ) -> Result<Self, TrustedPolicyError> {
        if purpose_for_disposition(policy.disposition()) != purpose {
            return Err(TrustedPolicyError::PurposeMismatch);
        }
        let wire = TrustedReleasePolicyBundleWire {
            version: TRUSTED_POLICY_BUNDLE_VERSION,
            keyring_id: keyring.keyring_id()?.as_bytes(),
            signer_key_id: signer_key_id.as_bytes(),
            purpose,
            signed_keyring: keyring.encode()?,
            signed_policy: policy.encode()?,
        };
        let bundle = Self {
            wire,
            keyring: keyring.clone(),
            policy,
        };
        bundle.validate_internal()?;
        Ok(bundle)
    }

    pub fn encode(&self) -> Result<Vec<u8>, TrustedPolicyError> {
        self.validate_internal()?;
        let encoded = postcard::to_allocvec(&self.wire).map_err(TrustedPolicyError::BundleCodec)?;
        if encoded.is_empty() || encoded.len() > MAX_TRUSTED_RELEASE_POLICY_BUNDLE_BYTES {
            return Err(TrustedPolicyError::BundleLength(encoded.len()));
        }
        Ok(encoded)
    }

    pub fn decode_and_verify(
        encoded: &[u8],
        trust_root: ReleaseTrustRoot,
    ) -> Result<Self, TrustedPolicyError> {
        if encoded.is_empty() || encoded.len() > MAX_TRUSTED_RELEASE_POLICY_BUNDLE_BYTES {
            return Err(TrustedPolicyError::BundleLength(encoded.len()));
        }
        let wire: TrustedReleasePolicyBundleWire =
            postcard::from_bytes(encoded).map_err(TrustedPolicyError::BundleCodec)?;
        if wire.version != TRUSTED_POLICY_BUNDLE_VERSION {
            return Err(TrustedPolicyError::BundleVersion(wire.version));
        }
        let keyring = SignedReleaseKeyring::decode_and_verify(&wire.signed_keyring, trust_root)?;
        if keyring.keyring_id()?.as_bytes() != wire.keyring_id {
            return Err(TrustedPolicyError::KeyringBinding);
        }
        let signer_key_id = ReleaseSigningKeyId::from_bytes_for_bundle(wire.signer_key_id);
        let authority = keyring.authorize_policy_signer(signer_key_id, wire.purpose)?;
        let policy = SignedReleasePolicy::decode_and_verify(&wire.signed_policy, authority)?;
        if purpose_for_disposition(policy.disposition()) != wire.purpose {
            return Err(TrustedPolicyError::PurposeMismatch);
        }
        let bundle = Self {
            wire,
            keyring,
            policy,
        };
        bundle.validate_internal()?;
        if bundle.encode()? != encoded {
            return Err(TrustedPolicyError::NonCanonicalBundle);
        }
        Ok(bundle)
    }

    pub fn verify(&self, trust_root: ReleaseTrustRoot) -> Result<(), TrustedPolicyError> {
        let encoded = self.encode()?;
        let decoded = Self::decode_and_verify(&encoded, trust_root)?;
        if decoded != *self {
            return Err(TrustedPolicyError::NonCanonicalBundle);
        }
        Ok(())
    }

    pub fn verify_successor_of(&self, previous: &Self) -> Result<(), TrustedPolicyError> {
        self.policy.verify_successor_of(&previous.policy)?;
        if self.keyring.keyring_id()? != previous.keyring.keyring_id()? {
            self.keyring.verify_successor_of(&previous.keyring)?;
        }
        Ok(())
    }

    pub fn keyring_id(&self) -> ReleaseKeyringId {
        ReleaseKeyringId::from_bytes_for_bundle(self.wire.keyring_id)
    }

    pub fn bundle_id(&self) -> Result<TrustedPolicyBundleId, TrustedPolicyError> {
        let encoded = self.encode()?;
        let mut hasher = Sha256::new();
        hasher.update(TRUSTED_POLICY_BUNDLE_ID_DOMAIN);
        hasher.update(encoded);
        Ok(TrustedPolicyBundleId(hasher.finalize().into()))
    }

    pub fn signer_key_id(&self) -> ReleaseSigningKeyId {
        ReleaseSigningKeyId::from_bytes_for_bundle(self.wire.signer_key_id)
    }

    pub fn policy(&self) -> &SignedReleasePolicy {
        &self.policy
    }

    pub fn keyring(&self) -> &SignedReleaseKeyring {
        &self.keyring
    }

    pub fn policy_authority(&self) -> Result<ReleasePolicyAuthority, TrustedPolicyError> {
        self.keyring
            .authorize_policy_signer(self.signer_key_id(), self.wire.purpose)
            .map_err(TrustedPolicyError::Trust)
    }

    fn validate_internal(&self) -> Result<(), TrustedPolicyError> {
        if self.wire.version != TRUSTED_POLICY_BUNDLE_VERSION {
            return Err(TrustedPolicyError::BundleVersion(self.wire.version));
        }
        if self.keyring.keyring_id()?.as_bytes() != self.wire.keyring_id
            || self.keyring.encode()? != self.wire.signed_keyring
            || self.policy.encode()? != self.wire.signed_policy
        {
            return Err(TrustedPolicyError::KeyringBinding);
        }
        let authority = self.policy_authority()?;
        self.policy.verify(authority)?;
        if purpose_for_disposition(self.policy.disposition()) != self.wire.purpose {
            return Err(TrustedPolicyError::PurposeMismatch);
        }
        Ok(())
    }
}

fn purpose_for_disposition(disposition: RolloutDisposition) -> ReleasePolicyPurpose {
    match disposition {
        RolloutDisposition::Normal => ReleasePolicyPurpose::Normal,
        RolloutDisposition::Containment => ReleasePolicyPurpose::Containment,
    }
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
pub enum TrustedPolicyError {
    #[error("trusted release-policy bundle has invalid length {0}")]
    BundleLength(usize),
    #[error("trusted release-policy bundle uses unsupported version {0}")]
    BundleVersion(u8),
    #[error("trusted release-policy bundle encoding is invalid: {0}")]
    BundleCodec(postcard::Error),
    #[error("trusted release-policy bundle is not canonically encoded")]
    NonCanonicalBundle,
    #[error("trusted release-policy bundle is not bound to its exact keyring")]
    KeyringBinding,
    #[error("trusted release-policy signer purpose does not match policy disposition")]
    PurposeMismatch,
    #[error(transparent)]
    Trust(#[from] ReleaseTrustError),
    #[error(transparent)]
    Policy(#[from] RolloutError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::{ReleaseSigningKeySpec, ReleaseSigningPermissions, ReleaseTrustRoot};

    fn key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    fn root(key: &SigningKey) -> Result<ReleaseTrustRoot, ReleaseTrustError> {
        ReleaseTrustRoot::from_bytes(key.verifying_key().to_bytes())
    }

    fn artifact(byte: u8) -> Result<ReleaseArtifactId, RolloutError> {
        ReleaseArtifactId::from_bytes([byte; 32])
    }

    #[test]
    fn bundle_is_self_contained_canonical_and_root_bound() -> Result<(), TrustedPolicyError> {
        let trust_root = key(31);
        let signer = key(32);
        let keyring = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &signer,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let bundle = TrustedReleasePolicyBundle::issue_genesis_hold(
            root(&trust_root)?,
            &keyring,
            &signer,
            artifact(1)?,
        )?;
        let encoded = bundle.encode()?;
        assert_eq!(
            TrustedReleasePolicyBundle::decode_and_verify(&encoded, root(&trust_root)?)?,
            bundle
        );
        assert!(TrustedReleasePolicyBundle::decode_and_verify(&encoded, root(&key(33))?).is_err());
        Ok(())
    }

    #[test]
    fn policy_chain_crosses_an_exact_operational_key_rotation() -> Result<(), TrustedPolicyError> {
        let trust_root = key(34);
        let old = key(35);
        let new = key(36);
        let first_keyring = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &old,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let first = TrustedReleasePolicyBundle::issue_genesis_hold(
            root(&trust_root)?,
            &first_keyring,
            &old,
            artifact(1)?,
        )?;
        let second_keyring = SignedReleaseKeyring::issue_successor(
            &trust_root,
            &first_keyring,
            &[ReleaseSigningKeySpec::from_signing_key(
                &new,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
            &[ReleaseSigningKeyId::from_public_key(
                old.verifying_key().to_bytes(),
            )?],
        )?;
        let second = TrustedReleasePolicyBundle::issue_successor(
            root(&trust_root)?,
            &second_keyring,
            &new,
            &first,
            artifact(1)?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            50,
        )?;
        second.verify(root(&trust_root)?)?;
        second.verify_successor_of(&first)?;
        assert_eq!(second.policy().policy_epoch(), 2);
        Ok(())
    }

    #[test]
    fn containment_key_cannot_expand_rollout() -> Result<(), TrustedPolicyError> {
        let trust_root = key(37);
        let normal = key(38);
        let containment = key(39);
        let keyring = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[
                ReleaseSigningKeySpec::from_signing_key(
                    &normal,
                    ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
                ),
                ReleaseSigningKeySpec::from_signing_key(
                    &containment,
                    ReleaseSigningPermissions::CONTAINMENT_ONLY,
                ),
            ],
        )?;
        let first = TrustedReleasePolicyBundle::issue_genesis_hold(
            root(&trust_root)?,
            &keyring,
            &normal,
            artifact(1)?,
        )?;
        assert!(
            TrustedReleasePolicyBundle::issue_successor(
                root(&trust_root)?,
                &keyring,
                &containment,
                &first,
                artifact(1)?,
                RolloutPhase::AdvertiseCanary,
                RolloutDisposition::Normal,
                10,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn containment_policy_can_use_narrow_emergency_key() -> Result<(), TrustedPolicyError> {
        let trust_root = key(40);
        let normal = key(41);
        let containment = key(42);
        let keyring = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[
                ReleaseSigningKeySpec::from_signing_key(
                    &normal,
                    ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
                ),
                ReleaseSigningKeySpec::from_signing_key(
                    &containment,
                    ReleaseSigningPermissions::CONTAINMENT_ONLY,
                ),
            ],
        )?;
        let hold = TrustedReleasePolicyBundle::issue_genesis_hold(
            root(&trust_root)?,
            &keyring,
            &normal,
            artifact(1)?,
        )?;
        let advertise = TrustedReleasePolicyBundle::issue_successor(
            root(&trust_root)?,
            &keyring,
            &normal,
            &hold,
            artifact(1)?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            10,
        )?;
        let stopped = TrustedReleasePolicyBundle::issue_successor(
            root(&trust_root)?,
            &keyring,
            &containment,
            &advertise,
            artifact(1)?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Containment,
            0,
        )?;
        stopped.verify_successor_of(&advertise)?;
        Ok(())
    }

    #[test]
    fn bundle_rejects_keyring_and_purpose_substitution() -> Result<(), TrustedPolicyError> {
        let trust_root = key(43);
        let signer = key(44);
        let keyring = SignedReleaseKeyring::issue_genesis(
            &trust_root,
            &[ReleaseSigningKeySpec::from_signing_key(
                &signer,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let bundle = TrustedReleasePolicyBundle::issue_genesis_hold(
            root(&trust_root)?,
            &keyring,
            &signer,
            artifact(1)?,
        )?;
        let mut wrong_purpose: TrustedReleasePolicyBundleWire =
            postcard::from_bytes(&bundle.encode()?).map_err(TrustedPolicyError::BundleCodec)?;
        wrong_purpose.purpose = ReleasePolicyPurpose::Containment;
        let encoded =
            postcard::to_allocvec(&wrong_purpose).map_err(TrustedPolicyError::BundleCodec)?;
        assert!(
            TrustedReleasePolicyBundle::decode_and_verify(&encoded, root(&trust_root)?).is_err()
        );
        let mut wrong_keyring: TrustedReleasePolicyBundleWire =
            postcard::from_bytes(&bundle.encode()?).map_err(TrustedPolicyError::BundleCodec)?;
        wrong_keyring.keyring_id[0] ^= 1;
        let encoded =
            postcard::to_allocvec(&wrong_keyring).map_err(TrustedPolicyError::BundleCodec)?;
        assert!(
            TrustedReleasePolicyBundle::decode_and_verify(&encoded, root(&trust_root)?).is_err()
        );
        Ok(())
    }
}
