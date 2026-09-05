use std::fmt;

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};

use crate::{
    runtime_publication::{MAX_TICKET_PUBLICATION_TTL_SECONDS, MIN_TICKET_PUBLICATION_TTL_SECONDS},
    runtime_queue::MAX_RUNTIME_RECORD_BYTES,
    runtime_ticket_automation::{MAX_REFRESH_BEFORE_SECONDS, MIN_REFRESH_BEFORE_SECONDS},
};

const POLICY_VERSION: u8 = 1;
const POLICY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:own-device-ticket-discovery-policy:v1\0";
const MAX_SERVICE_URL_BYTES: usize = 2_048;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OwnDeviceTicketDiscoveryPolicyId([u8; 32]);

impl fmt::Display for OwnDeviceTicketDiscoveryPolicyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OwnDeviceTicketDiscoveryPolicyContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    recipient_device_id: DeviceId,
    generation: u64,
    previous_policy_id: Option<OwnDeviceTicketDiscoveryPolicyId>,
    configured_at_unix_seconds: u64,
    enabled: bool,
    service_base_url: String,
    ttl_seconds: u64,
    refresh_before_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedOwnDeviceTicketDiscoveryPolicy {
    content: OwnDeviceTicketDiscoveryPolicyContent,
    signature: Vec<u8>,
}

impl SignedOwnDeviceTicketDiscoveryPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        recipient_device_id: DeviceId,
        configured_at_unix_seconds: u64,
        enabled: bool,
        service_base_url: String,
        ttl_seconds: u64,
        refresh_before_seconds: u64,
        previous: Option<&Self>,
    ) -> Result<Self> {
        ensure!(
            recipient_device_id != identity.device_id(),
            "own-device ticket discovery recipient must be another device"
        );
        let (generation, previous_policy_id) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    previous.local_account_id() == local_account_id
                        && previous.local_device_id() == identity.device_id()
                        && previous.recipient_device_id() == recipient_device_id,
                    "own-device ticket discovery policy chain changes identity"
                );
                ensure!(
                    configured_at_unix_seconds >= previous.configured_at_unix_seconds(),
                    "own-device ticket discovery policy time moves backwards"
                );
                (
                    previous
                        .generation()
                        .checked_add(1)
                        .context("own-device ticket discovery policy generation overflow")?,
                    Some(previous.policy_id()?),
                )
            }
            None => (1, None),
        };
        let content = OwnDeviceTicketDiscoveryPolicyContent {
            version: POLICY_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            recipient_device_id,
            generation,
            previous_policy_id,
            configured_at_unix_seconds,
            enabled,
            service_base_url,
            ttl_seconds,
            refresh_before_seconds,
        };
        let signature = identity
            .sign(&signing_bytes(POLICY_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let policy = Self { content, signature };
        policy.verify(previous)?;
        Ok(policy)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        let bytes =
            postcard::to_allocvec(self).context("encode own-device ticket discovery policy")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device ticket discovery policy is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device ticket discovery policy size is invalid"
        );
        let policy: Self =
            postcard::from_bytes(bytes).context("decode own-device ticket discovery policy")?;
        policy.verify_signature()?;
        Ok(policy)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    self.local_account_id() == previous.local_account_id()
                        && self.local_device_id() == previous.local_device_id()
                        && self.recipient_device_id() == previous.recipient_device_id()
                        && self.generation() == previous.generation().saturating_add(1)
                        && self.content.previous_policy_id == Some(previous.policy_id()?)
                        && self.configured_at_unix_seconds()
                            >= previous.configured_at_unix_seconds(),
                    "own-device ticket discovery policy chain is not contiguous"
                );
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_policy_id.is_none(),
                "own-device ticket discovery policy chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    pub(crate) fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == POLICY_VERSION
                && self.content.local_device_id != self.content.recipient_device_id,
            "own-device ticket discovery policy metadata is invalid"
        );
        ensure!(
            !self.content.service_base_url.is_empty()
                && self.content.service_base_url.len() <= MAX_SERVICE_URL_BYTES,
            "own-device ticket discovery service URL is invalid"
        );
        ensure!(
            (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
                .contains(&self.content.ttl_seconds),
            "own-device ticket discovery TTL is outside bounds"
        );
        ensure!(
            (MIN_REFRESH_BEFORE_SECONDS..=MAX_REFRESH_BEFORE_SECONDS)
                .contains(&self.content.refresh_before_seconds)
                && self.content.refresh_before_seconds < self.content.ttl_seconds,
            "own-device ticket discovery refresh lead is outside bounds"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(POLICY_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify own-device ticket discovery policy signature")
    }

    pub fn policy_id(&self) -> Result<OwnDeviceTicketDiscoveryPolicyId> {
        Ok(OwnDeviceTicketDiscoveryPolicyId(
            *blake3::hash(&self.encode()?).as_bytes(),
        ))
    }

    pub fn same_configuration(&self, other: &Self) -> bool {
        let mut left = self.content.clone();
        let mut right = other.content.clone();
        left.generation = 0;
        right.generation = 0;
        left.previous_policy_id = None;
        right.previous_policy_id = None;
        left.configured_at_unix_seconds = 0;
        right.configured_at_unix_seconds = 0;
        left == right
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
    pub fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }
    pub fn generation(&self) -> u64 {
        self.content.generation
    }
    pub fn configured_at_unix_seconds(&self) -> u64 {
        self.content.configured_at_unix_seconds
    }
    pub fn enabled(&self) -> bool {
        self.content.enabled
    }
    pub fn service_base_url(&self) -> &str {
        &self.content.service_base_url
    }
    pub fn ttl_seconds(&self) -> u64 {
        self.content.ttl_seconds
    }
    pub fn refresh_before_seconds(&self) -> u64 {
        self.content.refresh_before_seconds
    }
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let mut bytes = Vec::from(domain);
    bytes.extend(
        postcard::to_allocvec(content)
            .context("encode own-device ticket discovery signed content")?,
    );
    Ok(bytes)
}

fn write_hex(formatter: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_chain_is_signed_monotonic_and_idempotence_comparable() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let first = SignedOwnDeviceTicketDiscoveryPolicy::sign(
            &identity,
            AccountId::from_bytes([7; 32]),
            recipient.device_id(),
            100,
            true,
            "https://store.example/".to_owned(),
            900,
            300,
            None,
        )?;
        assert_eq!(
            SignedOwnDeviceTicketDiscoveryPolicy::decode(&first.encode()?)?,
            first
        );
        let identical = SignedOwnDeviceTicketDiscoveryPolicy::sign(
            &identity,
            first.local_account_id(),
            recipient.device_id(),
            101,
            true,
            "https://store.example/".to_owned(),
            900,
            300,
            Some(&first),
        )?;
        assert!(first.same_configuration(&identical));
        let disabled = SignedOwnDeviceTicketDiscoveryPolicy::sign(
            &identity,
            first.local_account_id(),
            recipient.device_id(),
            102,
            false,
            "https://store.example/".to_owned(),
            900,
            300,
            Some(&first),
        )?;
        assert!(!first.same_configuration(&disabled));
        assert_eq!(disabled.generation(), 2);
        Ok(())
    }
}
