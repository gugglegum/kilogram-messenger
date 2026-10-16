#![forbid(unsafe_code)]

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity, IdentityError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CANONICAL_SYNC_ALPN: &str = "kilogram/m1/sync/cbor/2";
pub const LEGACY_SYNC_ALPN: &str = "kilogram/m0/sync/8";
pub const SYNC_TRANSPORT_CAPABILITY_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:sync-transport-capability-signature:v2\0";
pub const SYNC_TRANSPORT_CAPABILITY_VERSION: u8 = 2;
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

impl SignedSyncTransportCapability {
    pub fn issue_legacy_with_canonical_candidate(
        listener_identity: &DeviceIdentity,
        binding: &SyncTransportCapabilityBinding<'_>,
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
            candidate_protocols: vec![SyncTransportProtocol::CanonicalCborV2],
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
    if content.active_protocols != [SyncTransportProtocol::LegacyPostcardV6]
        || content.candidate_protocols != [SyncTransportProtocol::CanonicalCborV2]
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
}
