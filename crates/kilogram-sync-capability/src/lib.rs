#![forbid(unsafe_code)]

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CANONICAL_SYNC_ALPN: &str = "kilogram/m1/sync/cbor/2";
pub const LEGACY_SYNC_ALPN: &str = "kilogram/m0/sync/8";
pub const SYNC_TRANSPORT_CAPABILITY_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:sync-transport-capability-signature:v2\0";
pub const SYNC_TRANSPORT_CAPABILITY_ARTIFACT_ID_DOMAIN: &[u8] =
    b"kilogram:sync-transport-capability-artifact-id:v2\0";
pub const SYNC_TRANSPORT_CANDIDATE_OFFER_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:sync-transport-candidate-offer-signature:v1\0";
pub const SYNC_TRANSPORT_CANDIDATE_OFFER_ID_DOMAIN: &[u8] =
    b"kilogram:sync-transport-candidate-offer-id:v1\0";
pub const SYNC_TRANSPORT_CANDIDATE_ACCEPTANCE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:sync-transport-candidate-acceptance-signature:v1\0";
pub const SYNC_TRANSPORT_CAPABILITY_VERSION: u8 = 2;
pub const SYNC_TRANSPORT_CANDIDATE_SELECTION_VERSION: u8 = 1;
pub const MAX_SYNC_TRANSPORT_CAPABILITY_VALIDITY_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const SYNC_TRANSPORT_CAPABILITY_CLOCK_SKEW_SECONDS: u64 = 5 * 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyncTransportProtocol {
    LegacyPostcardV6,
    CanonicalCborV2,
}

impl SyncTransportProtocol {
    pub const fn alpn(self) -> &'static str {
        match self {
            Self::LegacyPostcardV6 => LEGACY_SYNC_ALPN,
            Self::CanonicalCborV2 => CANONICAL_SYNC_ALPN,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SyncTransportCapabilityContent {
    version: u8,
    listener_account_id: AccountId,
    listener_device_id: DeviceId,
    authority_revision: u64,
    allowed_requester_account_id: AccountId,
    endpoint_id: String,
    ticket_publication_channel_epoch: u64,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
    active_protocols: Vec<SyncTransportProtocol>,
    candidate_protocols: Vec<SyncTransportProtocol>,
    canonical_dial_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedSyncTransportCapability {
    content: SyncTransportCapabilityContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct SyncTransportCapabilityBinding<'a> {
    pub listener_account_id: AccountId,
    pub listener_device_id: DeviceId,
    pub authority_revision: u64,
    pub allowed_requester_account_id: AccountId,
    pub endpoint_id: &'a str,
    pub ticket_publication_channel_epoch: u64,
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SyncTransportCandidateOfferContent {
    version: u8,
    listener_capability_id: [u8; 32],
    conversation_id: [u8; 32],
    listener_account_id: AccountId,
    listener_device_id: DeviceId,
    requester_account_id: AccountId,
    requester_device_id: DeviceId,
    session_binding: [u8; 32],
    listener_authority_revision: u64,
    requester_authority_revision: u64,
    migration_epoch: u64,
    candidate_protocol: SyncTransportProtocol,
    fallback_after_selected_failure: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedSyncTransportCandidateOffer {
    content: SyncTransportCandidateOfferContent,
    signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct SyncTransportCandidateOfferBinding {
    pub listener_capability_id: [u8; 32],
    pub conversation_id: [u8; 32],
    pub listener_account_id: AccountId,
    pub listener_device_id: DeviceId,
    pub requester_account_id: AccountId,
    pub requester_device_id: DeviceId,
    pub session_binding: [u8; 32],
    pub listener_authority_revision: u64,
    pub requester_authority_revision: u64,
    pub migration_epoch: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SyncTransportCandidateAcceptanceContent {
    version: u8,
    offer_id: [u8; 32],
    conversation_id: [u8; 32],
    listener_device_id: DeviceId,
    requester_device_id: DeviceId,
    session_binding: [u8; 32],
    migration_epoch: u64,
    selected_protocol: SyncTransportProtocol,
    fallback_after_selected_failure: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedSyncTransportCandidateAcceptance {
    content: SyncTransportCandidateAcceptanceContent,
    signature: Vec<u8>,
}

impl SignedSyncTransportCapability {
    pub fn issue_legacy_only(
        listener_identity: &DeviceIdentity,
        binding: &SyncTransportCapabilityBinding<'_>,
    ) -> Result<Self, SyncTransportCapabilityError> {
        Self::issue(listener_identity, binding, Vec::new())
    }

    pub fn issue_legacy_with_canonical_candidate(
        listener_identity: &DeviceIdentity,
        binding: &SyncTransportCapabilityBinding<'_>,
    ) -> Result<Self, SyncTransportCapabilityError> {
        Self::issue(
            listener_identity,
            binding,
            vec![SyncTransportProtocol::CanonicalCborV2],
        )
    }

    fn issue(
        listener_identity: &DeviceIdentity,
        binding: &SyncTransportCapabilityBinding<'_>,
        candidate_protocols: Vec<SyncTransportProtocol>,
    ) -> Result<Self, SyncTransportCapabilityError> {
        let content = SyncTransportCapabilityContent {
            version: SYNC_TRANSPORT_CAPABILITY_VERSION,
            listener_account_id: binding.listener_account_id,
            listener_device_id: binding.listener_device_id,
            authority_revision: binding.authority_revision,
            allowed_requester_account_id: binding.allowed_requester_account_id,
            endpoint_id: binding.endpoint_id.to_owned(),
            ticket_publication_channel_epoch: binding.ticket_publication_channel_epoch,
            issued_at_unix_seconds: binding.issued_at_unix_seconds,
            expires_at_unix_seconds: binding.expires_at_unix_seconds,
            active_protocols: vec![SyncTransportProtocol::LegacyPostcardV6],
            candidate_protocols,
            canonical_dial_enabled: false,
        };
        if listener_identity.device_id() != binding.listener_device_id {
            return Err(SyncTransportCapabilityError::SignerMismatch);
        }
        let signature = listener_identity.sign(&signing_bytes(&content)?).to_vec();
        let capability = Self { content, signature };
        capability.verify_binding(binding)?;
        Ok(capability)
    }

    pub fn verify(&self) -> Result<(), SyncTransportCapabilityError> {
        validate_content(&self.content)?;
        self.content
            .listener_device_id
            .verify(&signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    pub fn verify_at(&self, now_unix_seconds: u64) -> Result<(), SyncTransportCapabilityError> {
        self.verify()?;
        if now_unix_seconds.saturating_add(SYNC_TRANSPORT_CAPABILITY_CLOCK_SKEW_SECONDS)
            < self.content.issued_at_unix_seconds
        {
            return Err(SyncTransportCapabilityError::NotYetValid {
                issued_at: self.content.issued_at_unix_seconds,
                now: now_unix_seconds,
            });
        }
        if now_unix_seconds
            > self
                .content
                .expires_at_unix_seconds
                .saturating_add(SYNC_TRANSPORT_CAPABILITY_CLOCK_SKEW_SECONDS)
        {
            return Err(SyncTransportCapabilityError::Expired {
                expires_at: self.content.expires_at_unix_seconds,
                now: now_unix_seconds,
            });
        }
        Ok(())
    }

    pub fn verify_binding(
        &self,
        expected: &SyncTransportCapabilityBinding<'_>,
    ) -> Result<(), SyncTransportCapabilityError> {
        self.verify()?;
        let actual = &self.content;
        if actual.listener_account_id != expected.listener_account_id
            || actual.listener_device_id != expected.listener_device_id
            || actual.authority_revision != expected.authority_revision
            || actual.allowed_requester_account_id != expected.allowed_requester_account_id
            || actual.endpoint_id != expected.endpoint_id
            || actual.ticket_publication_channel_epoch != expected.ticket_publication_channel_epoch
            || actual.issued_at_unix_seconds != expected.issued_at_unix_seconds
            || actual.expires_at_unix_seconds != expected.expires_at_unix_seconds
        {
            return Err(SyncTransportCapabilityError::BindingMismatch);
        }
        Ok(())
    }

    pub fn listener_account_id(&self) -> AccountId {
        self.content.listener_account_id
    }

    pub fn listener_device_id(&self) -> DeviceId {
        self.content.listener_device_id
    }

    pub fn authority_revision(&self) -> u64 {
        self.content.authority_revision
    }

    pub fn allowed_requester_account_id(&self) -> AccountId {
        self.content.allowed_requester_account_id
    }

    pub fn endpoint_id(&self) -> &str {
        &self.content.endpoint_id
    }

    pub fn ticket_publication_channel_epoch(&self) -> u64 {
        self.content.ticket_publication_channel_epoch
    }

    pub fn issued_at_unix_seconds(&self) -> u64 {
        self.content.issued_at_unix_seconds
    }

    pub fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    pub fn active_protocols(&self) -> &[SyncTransportProtocol] {
        &self.content.active_protocols
    }

    pub fn candidate_protocols(&self) -> &[SyncTransportProtocol] {
        &self.content.candidate_protocols
    }

    pub fn canonical_dial_enabled(&self) -> bool {
        self.content.canonical_dial_enabled
    }

    pub fn artifact_id(&self) -> Result<[u8; 32], SyncTransportCapabilityError> {
        self.verify()?;
        Ok(domain_separated_id(
            SYNC_TRANSPORT_CAPABILITY_ARTIFACT_ID_DOMAIN,
            &serde_json::to_vec(self)?,
        ))
    }
}

impl SignedSyncTransportCandidateOffer {
    pub fn issue(
        requester_identity: &DeviceIdentity,
        binding: SyncTransportCandidateOfferBinding,
    ) -> Result<Self, SyncTransportCapabilityError> {
        if requester_identity.device_id() != binding.requester_device_id {
            return Err(SyncTransportCapabilityError::CandidateOfferSignerMismatch);
        }
        let content = SyncTransportCandidateOfferContent {
            version: SYNC_TRANSPORT_CANDIDATE_SELECTION_VERSION,
            listener_capability_id: binding.listener_capability_id,
            conversation_id: binding.conversation_id,
            listener_account_id: binding.listener_account_id,
            listener_device_id: binding.listener_device_id,
            requester_account_id: binding.requester_account_id,
            requester_device_id: binding.requester_device_id,
            session_binding: binding.session_binding,
            listener_authority_revision: binding.listener_authority_revision,
            requester_authority_revision: binding.requester_authority_revision,
            migration_epoch: binding.migration_epoch,
            candidate_protocol: SyncTransportProtocol::CanonicalCborV2,
            fallback_after_selected_failure: false,
        };
        validate_candidate_offer_content(&content)?;
        let signature = requester_identity
            .sign(&candidate_offer_signing_bytes(&content)?)
            .to_vec();
        let offer = Self { content, signature };
        offer.verify()?;
        Ok(offer)
    }

    pub fn verify(&self) -> Result<(), SyncTransportCapabilityError> {
        validate_candidate_offer_content(&self.content)?;
        self.content.requester_device_id.verify(
            &candidate_offer_signing_bytes(&self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn offer_id(&self) -> Result<[u8; 32], SyncTransportCapabilityError> {
        self.verify()?;
        Ok(domain_separated_id(
            SYNC_TRANSPORT_CANDIDATE_OFFER_ID_DOMAIN,
            &serde_json::to_vec(self)?,
        ))
    }

    pub fn listener_capability_id(&self) -> [u8; 32] {
        self.content.listener_capability_id
    }

    pub fn conversation_id(&self) -> [u8; 32] {
        self.content.conversation_id
    }

    pub fn listener_account_id(&self) -> AccountId {
        self.content.listener_account_id
    }

    pub fn listener_device_id(&self) -> DeviceId {
        self.content.listener_device_id
    }

    pub fn requester_account_id(&self) -> AccountId {
        self.content.requester_account_id
    }

    pub fn requester_device_id(&self) -> DeviceId {
        self.content.requester_device_id
    }

    pub fn session_binding(&self) -> [u8; 32] {
        self.content.session_binding
    }

    pub fn listener_authority_revision(&self) -> u64 {
        self.content.listener_authority_revision
    }

    pub fn requester_authority_revision(&self) -> u64 {
        self.content.requester_authority_revision
    }

    pub fn migration_epoch(&self) -> u64 {
        self.content.migration_epoch
    }

    pub fn candidate_protocol(&self) -> SyncTransportProtocol {
        self.content.candidate_protocol
    }

    pub fn fallback_after_selected_failure(&self) -> bool {
        self.content.fallback_after_selected_failure
    }
}

impl SignedSyncTransportCandidateAcceptance {
    pub fn issue(
        listener_identity: &DeviceIdentity,
        offer: &SignedSyncTransportCandidateOffer,
    ) -> Result<Self, SyncTransportCapabilityError> {
        offer.verify()?;
        if listener_identity.device_id() != offer.listener_device_id() {
            return Err(SyncTransportCapabilityError::CandidateAcceptanceSignerMismatch);
        }
        let content = SyncTransportCandidateAcceptanceContent {
            version: SYNC_TRANSPORT_CANDIDATE_SELECTION_VERSION,
            offer_id: offer.offer_id()?,
            conversation_id: offer.conversation_id(),
            listener_device_id: offer.listener_device_id(),
            requester_device_id: offer.requester_device_id(),
            session_binding: offer.session_binding(),
            migration_epoch: offer.migration_epoch(),
            selected_protocol: SyncTransportProtocol::CanonicalCborV2,
            fallback_after_selected_failure: false,
        };
        validate_candidate_acceptance_content(&content)?;
        let signature = listener_identity
            .sign(&candidate_acceptance_signing_bytes(&content)?)
            .to_vec();
        let acceptance = Self { content, signature };
        acceptance.verify_for_offer(offer)?;
        Ok(acceptance)
    }

    pub fn verify(&self) -> Result<(), SyncTransportCapabilityError> {
        validate_candidate_acceptance_content(&self.content)?;
        self.content.listener_device_id.verify(
            &candidate_acceptance_signing_bytes(&self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn verify_for_offer(
        &self,
        offer: &SignedSyncTransportCandidateOffer,
    ) -> Result<(), SyncTransportCapabilityError> {
        offer.verify()?;
        self.verify()?;
        if self.content.offer_id != offer.offer_id()?
            || self.content.conversation_id != offer.conversation_id()
            || self.content.listener_device_id != offer.listener_device_id()
            || self.content.requester_device_id != offer.requester_device_id()
            || self.content.session_binding != offer.session_binding()
            || self.content.migration_epoch != offer.migration_epoch()
            || self.content.selected_protocol != offer.candidate_protocol()
            || self.content.fallback_after_selected_failure
                != offer.fallback_after_selected_failure()
        {
            return Err(SyncTransportCapabilityError::CandidateAcceptanceMismatch);
        }
        Ok(())
    }

    pub fn offer_id(&self) -> [u8; 32] {
        self.content.offer_id
    }

    pub fn selected_protocol(&self) -> SyncTransportProtocol {
        self.content.selected_protocol
    }
}

pub fn verify_mutual_canonical_candidate_selection(
    listener_capability: &SignedSyncTransportCapability,
    offer: &SignedSyncTransportCandidateOffer,
    acceptance: &SignedSyncTransportCandidateAcceptance,
) -> Result<(), SyncTransportCapabilityError> {
    listener_capability.verify()?;
    offer.verify()?;
    acceptance.verify_for_offer(offer)?;
    if listener_capability.artifact_id()? != offer.listener_capability_id()
        || listener_capability.listener_account_id() != offer.listener_account_id()
        || listener_capability.listener_device_id() != offer.listener_device_id()
        || listener_capability.authority_revision() != offer.listener_authority_revision()
        || listener_capability.allowed_requester_account_id() != offer.requester_account_id()
        || listener_capability.candidate_protocols() != [SyncTransportProtocol::CanonicalCborV2]
        || listener_capability.canonical_dial_enabled()
        || offer.candidate_protocol() != SyncTransportProtocol::CanonicalCborV2
        || acceptance.selected_protocol() != SyncTransportProtocol::CanonicalCborV2
    {
        return Err(SyncTransportCapabilityError::MutualCandidateSelectionMismatch);
    }
    Ok(())
}

fn validate_candidate_offer_content(
    content: &SyncTransportCandidateOfferContent,
) -> Result<(), SyncTransportCapabilityError> {
    if content.version != SYNC_TRANSPORT_CANDIDATE_SELECTION_VERSION {
        return Err(
            SyncTransportCapabilityError::UnsupportedCandidateSelectionVersion(content.version),
        );
    }
    if content.listener_capability_id == [0; 32]
        || content.conversation_id == [0; 32]
        || content.session_binding == [0; 32]
        || content.listener_device_id == content.requester_device_id
        || content.listener_authority_revision == 0
        || content.requester_authority_revision == 0
        || content.migration_epoch == 0
        || content.candidate_protocol != SyncTransportProtocol::CanonicalCborV2
        || content.fallback_after_selected_failure
    {
        return Err(SyncTransportCapabilityError::UnsafeCandidateSelectionState);
    }
    Ok(())
}

fn validate_candidate_acceptance_content(
    content: &SyncTransportCandidateAcceptanceContent,
) -> Result<(), SyncTransportCapabilityError> {
    if content.version != SYNC_TRANSPORT_CANDIDATE_SELECTION_VERSION {
        return Err(
            SyncTransportCapabilityError::UnsupportedCandidateSelectionVersion(content.version),
        );
    }
    if content.offer_id == [0; 32]
        || content.conversation_id == [0; 32]
        || content.session_binding == [0; 32]
        || content.listener_device_id == content.requester_device_id
        || content.migration_epoch == 0
        || content.selected_protocol != SyncTransportProtocol::CanonicalCborV2
        || content.fallback_after_selected_failure
    {
        return Err(SyncTransportCapabilityError::UnsafeCandidateSelectionState);
    }
    Ok(())
}

fn candidate_offer_signing_bytes(
    content: &SyncTransportCandidateOfferContent,
) -> Result<Vec<u8>, SyncTransportCapabilityError> {
    prefixed_json(SYNC_TRANSPORT_CANDIDATE_OFFER_SIGNATURE_DOMAIN, content)
}

fn candidate_acceptance_signing_bytes(
    content: &SyncTransportCandidateAcceptanceContent,
) -> Result<Vec<u8>, SyncTransportCapabilityError> {
    prefixed_json(
        SYNC_TRANSPORT_CANDIDATE_ACCEPTANCE_SIGNATURE_DOMAIN,
        content,
    )
}

fn prefixed_json<T: Serialize>(
    domain: &[u8],
    content: &T,
) -> Result<Vec<u8>, SyncTransportCapabilityError> {
    let encoded = serde_json::to_vec(content)?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn domain_separated_id(domain: &[u8], encoded: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

fn validate_content(
    content: &SyncTransportCapabilityContent,
) -> Result<(), SyncTransportCapabilityError> {
    if content.version != SYNC_TRANSPORT_CAPABILITY_VERSION {
        return Err(SyncTransportCapabilityError::UnsupportedVersion(
            content.version,
        ));
    }
    if content.endpoint_id.len() != 64
        || !content
            .endpoint_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SyncTransportCapabilityError::InvalidEndpointId);
    }
    if content.issued_at_unix_seconds >= content.expires_at_unix_seconds {
        return Err(SyncTransportCapabilityError::InvalidValidityInterval);
    }
    let validity = content
        .expires_at_unix_seconds
        .saturating_sub(content.issued_at_unix_seconds);
    if validity > MAX_SYNC_TRANSPORT_CAPABILITY_VALIDITY_SECONDS {
        return Err(SyncTransportCapabilityError::ValidityTooLong(validity));
    }
    let safe_candidates = content.candidate_protocols.is_empty()
        || content.candidate_protocols == [SyncTransportProtocol::CanonicalCborV2];
    if content.active_protocols != [SyncTransportProtocol::LegacyPostcardV6]
        || !safe_candidates
        || content.canonical_dial_enabled
    {
        return Err(SyncTransportCapabilityError::UnsafeProtocolState);
    }
    Ok(())
}

fn signing_bytes(
    content: &SyncTransportCapabilityContent,
) -> Result<Vec<u8>, SyncTransportCapabilityError> {
    let encoded = serde_json::to_vec(content)?;
    let mut bytes =
        Vec::with_capacity(SYNC_TRANSPORT_CAPABILITY_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SYNC_TRANSPORT_CAPABILITY_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

#[derive(Debug, Error)]
pub enum SyncTransportCapabilityError {
    #[error("unsupported synchronization transport capability version {0}")]
    UnsupportedVersion(u8),
    #[error("synchronization transport capability endpoint ID is not lowercase 32-byte hex")]
    InvalidEndpointId,
    #[error("synchronization transport capability validity interval is invalid")]
    InvalidValidityInterval,
    #[error(
        "synchronization transport capability validity is {0} seconds; maximum is {MAX_SYNC_TRANSPORT_CAPABILITY_VALIDITY_SECONDS}"
    )]
    ValidityTooLong(u64),
    #[error("synchronization transport capability protocol state is unsafe")]
    UnsafeProtocolState,
    #[error("synchronization transport capability signer does not match listener Device")]
    SignerMismatch,
    #[error("synchronization transport capability binding does not match its connection ticket")]
    BindingMismatch,
    #[error("synchronization transport capability is not valid until {issued_at}; now={now}")]
    NotYetValid { issued_at: u64, now: u64 },
    #[error("synchronization transport capability expired at {expires_at}; now={now}")]
    Expired { expires_at: u64, now: u64 },
    #[error("unsupported synchronization candidate selection version {0}")]
    UnsupportedCandidateSelectionVersion(u8),
    #[error("synchronization candidate selection state is unsafe")]
    UnsafeCandidateSelectionState,
    #[error("synchronization candidate offer signer does not match requester Device")]
    CandidateOfferSignerMismatch,
    #[error("synchronization candidate acceptance signer does not match listener Device")]
    CandidateAcceptanceSignerMismatch,
    #[error("synchronization candidate acceptance does not match the exact requester offer")]
    CandidateAcceptanceMismatch,
    #[error("mutual synchronization candidate selection does not match listener capability")]
    MutualCandidateSelectionMismatch,
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn binding<'a>(
        identity: &DeviceIdentity,
        endpoint_id: &'a str,
    ) -> Result<SyncTransportCapabilityBinding<'a>, SyncTransportCapabilityError> {
        Ok(SyncTransportCapabilityBinding {
            listener_account_id: AccountId::from_str(&"11".repeat(32))?,
            listener_device_id: identity.device_id(),
            authority_revision: 7,
            allowed_requester_account_id: AccountId::from_str(&"22".repeat(32))?,
            endpoint_id,
            ticket_publication_channel_epoch: 3,
            issued_at_unix_seconds: 1_000,
            expires_at_unix_seconds: 2_000,
        })
    }

    #[test]
    fn signed_capability_is_exact_bound_and_legacy_only() -> Result<(), SyncTransportCapabilityError>
    {
        let identity = DeviceIdentity::generate()?;
        let expected = binding(
            &identity,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )?;
        let capability = SignedSyncTransportCapability::issue_legacy_with_canonical_candidate(
            &identity, &expected,
        )?;

        capability.verify_at(1_500)?;
        capability.verify_binding(&expected)?;
        assert_eq!(
            capability.active_protocols(),
            [SyncTransportProtocol::LegacyPostcardV6]
        );
        assert_eq!(
            capability.candidate_protocols(),
            [SyncTransportProtocol::CanonicalCborV2]
        );
        assert!(!capability.canonical_dial_enabled());
        assert_eq!(
            SyncTransportProtocol::LegacyPostcardV6.alpn(),
            LEGACY_SYNC_ALPN
        );
        assert_eq!(
            SyncTransportProtocol::CanonicalCborV2.alpn(),
            CANONICAL_SYNC_ALPN
        );
        Ok(())
    }

    #[test]
    fn default_capability_does_not_advertise_a_canonical_candidate()
    -> Result<(), SyncTransportCapabilityError> {
        let identity = DeviceIdentity::generate()?;
        let expected = binding(
            &identity,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )?;
        let capability = SignedSyncTransportCapability::issue_legacy_only(&identity, &expected)?;

        capability.verify_at(1_500)?;
        capability.verify_binding(&expected)?;
        assert_eq!(
            capability.active_protocols(),
            [SyncTransportProtocol::LegacyPostcardV6]
        );
        assert!(capability.candidate_protocols().is_empty());
        assert!(!capability.canonical_dial_enabled());
        Ok(())
    }

    #[test]
    fn previous_capability_and_candidate_generation_fail_closed()
    -> Result<(), SyncTransportCapabilityError> {
        let identity = DeviceIdentity::generate()?;
        let expected = binding(
            &identity,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )?;
        let capability = SignedSyncTransportCapability::issue_legacy_with_canonical_candidate(
            &identity, &expected,
        )?;

        let mut previous_version = capability.clone();
        previous_version.content.version = SYNC_TRANSPORT_CAPABILITY_VERSION - 1;
        assert!(matches!(
            previous_version.verify(),
            Err(SyncTransportCapabilityError::UnsupportedVersion(1))
        ));

        let encoded = serde_json::to_string(&capability)?;
        let previous_candidate = encoded.replace("canonical-cbor-v2", "canonical-cbor-v1");
        assert!(
            serde_json::from_str::<SignedSyncTransportCapability>(&previous_candidate).is_err()
        );
        Ok(())
    }

    #[test]
    fn changed_context_and_time_fail_closed() -> Result<(), SyncTransportCapabilityError> {
        let identity = DeviceIdentity::generate()?;
        let endpoint = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let expected = binding(&identity, endpoint)?;
        let capability = SignedSyncTransportCapability::issue_legacy_with_canonical_candidate(
            &identity, &expected,
        )?;
        let mut wrong_endpoint = binding(
            &identity,
            "1123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )?;
        assert!(matches!(
            capability.verify_binding(&wrong_endpoint),
            Err(SyncTransportCapabilityError::BindingMismatch)
        ));
        wrong_endpoint.endpoint_id = endpoint;
        wrong_endpoint.authority_revision += 1;
        assert!(matches!(
            capability.verify_binding(&wrong_endpoint),
            Err(SyncTransportCapabilityError::BindingMismatch)
        ));
        assert!(matches!(
            capability.verify_at(10_000),
            Err(SyncTransportCapabilityError::Expired { .. })
        ));
        Ok(())
    }

    fn candidate_binding(
        listener: &DeviceIdentity,
        requester: &DeviceIdentity,
        capability: &SignedSyncTransportCapability,
        migration_epoch: u64,
    ) -> Result<SyncTransportCandidateOfferBinding, SyncTransportCapabilityError> {
        Ok(SyncTransportCandidateOfferBinding {
            listener_capability_id: capability.artifact_id()?,
            conversation_id: [0x31; 32],
            listener_account_id: capability.listener_account_id(),
            listener_device_id: listener.device_id(),
            requester_account_id: capability.allowed_requester_account_id(),
            requester_device_id: requester.device_id(),
            session_binding: [0x41; 32],
            listener_authority_revision: capability.authority_revision(),
            requester_authority_revision: 9,
            migration_epoch,
        })
    }

    #[test]
    fn mutual_candidate_selection_binds_both_device_signatures_and_capability()
    -> Result<(), SyncTransportCapabilityError> {
        let listener = DeviceIdentity::from_secret_bytes([0x51; 32]);
        let requester = DeviceIdentity::from_secret_bytes([0x61; 32]);
        let expected = binding(
            &listener,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )?;
        let capability = SignedSyncTransportCapability::issue_legacy_with_canonical_candidate(
            &listener, &expected,
        )?;
        let offer = SignedSyncTransportCandidateOffer::issue(
            &requester,
            candidate_binding(&listener, &requester, &capability, 11)?,
        )?;
        let acceptance = SignedSyncTransportCandidateAcceptance::issue(&listener, &offer)?;

        verify_mutual_canonical_candidate_selection(&capability, &offer, &acceptance)?;
        assert_eq!(
            acceptance.selected_protocol(),
            SyncTransportProtocol::CanonicalCborV2
        );
        assert!(!offer.fallback_after_selected_failure());
        assert!(!capability.canonical_dial_enabled());
        assert_ne!(capability.artifact_id()?, offer.offer_id()?);

        let encoded_offer = serde_json::to_vec(&offer)?;
        let decoded_offer: SignedSyncTransportCandidateOffer =
            serde_json::from_slice(&encoded_offer)?;
        let encoded_acceptance = serde_json::to_vec(&acceptance)?;
        let decoded_acceptance: SignedSyncTransportCandidateAcceptance =
            serde_json::from_slice(&encoded_acceptance)?;
        assert_eq!(decoded_offer, offer);
        assert_eq!(decoded_acceptance, acceptance);
        verify_mutual_canonical_candidate_selection(
            &capability,
            &decoded_offer,
            &decoded_acceptance,
        )?;
        Ok(())
    }

    #[test]
    fn candidate_selection_substitutions_fail_closed() -> Result<(), SyncTransportCapabilityError> {
        let listener = DeviceIdentity::from_secret_bytes([0x52; 32]);
        let requester = DeviceIdentity::from_secret_bytes([0x62; 32]);
        let other = DeviceIdentity::from_secret_bytes([0x72; 32]);
        let expected = binding(
            &listener,
            "1123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )?;
        let capability = SignedSyncTransportCapability::issue_legacy_with_canonical_candidate(
            &listener, &expected,
        )?;
        let binding = candidate_binding(&listener, &requester, &capability, 12)?;
        for unsafe_binding in [
            SyncTransportCandidateOfferBinding {
                conversation_id: [0; 32],
                ..binding
            },
            SyncTransportCandidateOfferBinding {
                session_binding: [0; 32],
                ..binding
            },
            SyncTransportCandidateOfferBinding {
                listener_authority_revision: 0,
                ..binding
            },
            SyncTransportCandidateOfferBinding {
                requester_authority_revision: 0,
                ..binding
            },
        ] {
            assert!(matches!(
                SignedSyncTransportCandidateOffer::issue(&requester, unsafe_binding),
                Err(SyncTransportCapabilityError::UnsafeCandidateSelectionState)
            ));
        }
        assert!(matches!(
            SignedSyncTransportCandidateOffer::issue(&other, binding),
            Err(SyncTransportCapabilityError::CandidateOfferSignerMismatch)
        ));
        let offer = SignedSyncTransportCandidateOffer::issue(&requester, binding)?;
        assert!(matches!(
            SignedSyncTransportCandidateAcceptance::issue(&other, &offer),
            Err(SyncTransportCapabilityError::CandidateAcceptanceSignerMismatch)
        ));
        let acceptance = SignedSyncTransportCandidateAcceptance::issue(&listener, &offer)?;

        let other_offer = SignedSyncTransportCandidateOffer::issue(
            &requester,
            candidate_binding(&listener, &requester, &capability, 13)?,
        )?;
        assert!(matches!(
            acceptance.verify_for_offer(&other_offer),
            Err(SyncTransportCapabilityError::CandidateAcceptanceMismatch)
        ));

        let mut wrong_capability = offer.clone();
        wrong_capability.content.listener_capability_id[0] ^= 1;
        wrong_capability.signature = requester
            .sign(&candidate_offer_signing_bytes(&wrong_capability.content)?)
            .to_vec();
        assert!(matches!(
            verify_mutual_canonical_candidate_selection(
                &capability,
                &wrong_capability,
                &SignedSyncTransportCandidateAcceptance::issue(&listener, &wrong_capability)?,
            ),
            Err(SyncTransportCapabilityError::MutualCandidateSelectionMismatch)
        ));

        let mut fallback = offer.clone();
        fallback.content.fallback_after_selected_failure = true;
        fallback.signature = requester
            .sign(&candidate_offer_signing_bytes(&fallback.content)?)
            .to_vec();
        assert!(matches!(
            fallback.verify(),
            Err(SyncTransportCapabilityError::UnsafeCandidateSelectionState)
        ));

        let mut legacy_acceptance = acceptance;
        legacy_acceptance.content.selected_protocol = SyncTransportProtocol::LegacyPostcardV6;
        legacy_acceptance.signature = listener
            .sign(&candidate_acceptance_signing_bytes(
                &legacy_acceptance.content,
            )?)
            .to_vec();
        assert!(matches!(
            legacy_acceptance.verify(),
            Err(SyncTransportCapabilityError::UnsafeCandidateSelectionState)
        ));
        Ok(())
    }
}
