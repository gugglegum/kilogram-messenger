use std::fmt;

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use kilogram_runtime_ipc::RuntimeIpcNetworkClass;
use serde::{Deserialize, Serialize};

use crate::{
    runtime_endpoint_announcement::MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS,
    runtime_own_device_automation::{
        MAX_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS, MAX_OWN_DEVICE_ANNOUNCEMENT_RETRY_SECONDS,
        MIN_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS,
    },
    runtime_publication::{MAX_TICKET_PUBLICATION_TTL_SECONDS, MIN_TICKET_PUBLICATION_TTL_SECONDS},
    runtime_queue::MAX_RUNTIME_RECORD_BYTES,
    runtime_ticket_automation::{MAX_REFRESH_BEFORE_SECONDS, MIN_REFRESH_BEFORE_SECONDS},
};

const POLICY_VERSION: u8 = 1;
const POLICY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:own-device-roster-policy:v1\0";
const MAX_SERVICE_URL_BYTES: usize = 2_048;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OwnDeviceRosterPolicyId([u8; 32]);

impl fmt::Display for OwnDeviceRosterPolicyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OwnDeviceRosterPolicyContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    generation: u64,
    previous_policy_id: Option<OwnDeviceRosterPolicyId>,
    configured_at_unix_seconds: u64,
    authority_revision: u64,
    device_list_digest: [u8; 32],
    enabled: bool,
    service_base_url: String,
    ttl_seconds: u64,
    refresh_before_seconds: u64,
    interval_seconds: u64,
    validity_seconds: u64,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    allow_ethernet: bool,
    allow_wifi: bool,
    allow_mobile: bool,
    allow_unknown_network: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedOwnDeviceRosterPolicy {
    content: OwnDeviceRosterPolicyContent,
    signature: Vec<u8>,
}

impl SignedOwnDeviceRosterPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        configured_at_unix_seconds: u64,
        authority_revision: u64,
        device_list_digest: [u8; 32],
        enabled: bool,
        service_base_url: String,
        ttl_seconds: u64,
        refresh_before_seconds: u64,
        interval_seconds: u64,
        validity_seconds: u64,
        retry_base_seconds: u64,
        retry_max_seconds: u64,
        allow_ethernet: bool,
        allow_wifi: bool,
        allow_mobile: bool,
        allow_unknown_network: bool,
        previous: Option<&Self>,
    ) -> Result<Self> {
        let (generation, previous_policy_id) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    previous.local_account_id() == local_account_id
                        && previous.local_device_id() == identity.device_id(),
                    "own-device roster policy chain changes identity"
                );
                ensure!(
                    configured_at_unix_seconds >= previous.configured_at_unix_seconds(),
                    "own-device roster policy time moves backwards"
                );
                ensure!(
                    authority_revision >= previous.authority_revision(),
                    "own-device roster policy rolls authority back"
                );
                if authority_revision == previous.authority_revision() {
                    ensure!(
                        device_list_digest == previous.device_list_digest(),
                        "own-device roster policy equivocates at one authority revision"
                    );
                }
                (
                    previous
                        .generation()
                        .checked_add(1)
                        .context("own-device roster policy generation overflow")?,
                    Some(previous.policy_id()?),
                )
            }
            None => (1, None),
        };
        let content = OwnDeviceRosterPolicyContent {
            version: POLICY_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            generation,
            previous_policy_id,
            configured_at_unix_seconds,
            authority_revision,
            device_list_digest,
            enabled,
            service_base_url,
            ttl_seconds,
            refresh_before_seconds,
            interval_seconds,
            validity_seconds,
            retry_base_seconds,
            retry_max_seconds,
            allow_ethernet,
            allow_wifi,
            allow_mobile,
            allow_unknown_network,
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
        let bytes = postcard::to_allocvec(self).context("encode own-device roster policy")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device roster policy is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device roster policy size is invalid"
        );
        let policy: Self =
            postcard::from_bytes(bytes).context("decode own-device roster policy")?;
        policy.verify_signature()?;
        Ok(policy)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                let expected_generation = previous
                    .generation()
                    .checked_add(1)
                    .context("own-device roster policy generation overflow")?;
                ensure!(
                    self.local_account_id() == previous.local_account_id()
                        && self.local_device_id() == previous.local_device_id()
                        && self.generation() == expected_generation
                        && self.content.previous_policy_id == Some(previous.policy_id()?)
                        && self.configured_at_unix_seconds()
                            >= previous.configured_at_unix_seconds(),
                    "own-device roster policy chain is not contiguous"
                );
                ensure!(
                    self.authority_revision() >= previous.authority_revision(),
                    "own-device roster policy chain rolls authority back"
                );
                if self.authority_revision() == previous.authority_revision() {
                    ensure!(
                        self.device_list_digest() == previous.device_list_digest(),
                        "own-device roster policy chain equivocates at one authority revision"
                    );
                }
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_policy_id.is_none(),
                "own-device roster policy chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    pub(crate) fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == POLICY_VERSION,
            "unsupported own-device roster policy version"
        );
        ensure!(
            self.content.device_list_digest != [0_u8; 32],
            "own-device roster policy has an invalid roster digest"
        );
        ensure!(
            !self.content.service_base_url.is_empty()
                && self.content.service_base_url.len() <= MAX_SERVICE_URL_BYTES,
            "own-device roster service URL is invalid"
        );
        ensure!(
            (MIN_TICKET_PUBLICATION_TTL_SECONDS..=MAX_TICKET_PUBLICATION_TTL_SECONDS)
                .contains(&self.content.ttl_seconds),
            "own-device roster ticket TTL is outside bounds"
        );
        ensure!(
            (MIN_REFRESH_BEFORE_SECONDS..=MAX_REFRESH_BEFORE_SECONDS)
                .contains(&self.content.refresh_before_seconds)
                && self.content.refresh_before_seconds < self.content.ttl_seconds,
            "own-device roster refresh lead is outside bounds"
        );
        ensure!(
            (MIN_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS
                ..=MAX_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS)
                .contains(&self.content.interval_seconds),
            "own-device roster announcement interval is outside bounds"
        );
        ensure!(
            (1..=MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS)
                .contains(&self.content.validity_seconds),
            "own-device roster announcement validity is outside bounds"
        );
        ensure!(
            (1..=MAX_OWN_DEVICE_ANNOUNCEMENT_RETRY_SECONDS)
                .contains(&self.content.retry_base_seconds)
                && self.content.retry_max_seconds >= self.content.retry_base_seconds
                && self.content.retry_max_seconds <= MAX_OWN_DEVICE_ANNOUNCEMENT_RETRY_SECONDS,
            "own-device roster retry bounds are invalid"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(POLICY_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify own-device roster policy signature")
    }

    pub fn policy_id(&self) -> Result<OwnDeviceRosterPolicyId> {
        Ok(OwnDeviceRosterPolicyId(
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

    pub fn allows_network(&self, network: RuntimeIpcNetworkClass) -> bool {
        match network {
            RuntimeIpcNetworkClass::Ethernet => self.content.allow_ethernet,
            RuntimeIpcNetworkClass::Wifi => self.content.allow_wifi,
            RuntimeIpcNetworkClass::Mobile => self.content.allow_mobile,
            RuntimeIpcNetworkClass::Unknown => self.content.allow_unknown_network,
        }
    }

    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
    pub fn generation(&self) -> u64 {
        self.content.generation
    }
    pub fn configured_at_unix_seconds(&self) -> u64 {
        self.content.configured_at_unix_seconds
    }
    pub fn authority_revision(&self) -> u64 {
        self.content.authority_revision
    }
    pub fn device_list_digest(&self) -> [u8; 32] {
        self.content.device_list_digest
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
    pub fn interval_seconds(&self) -> u64 {
        self.content.interval_seconds
    }
    pub fn validity_seconds(&self) -> u64 {
        self.content.validity_seconds
    }
    pub fn retry_base_seconds(&self) -> u64 {
        self.content.retry_base_seconds
    }
    pub fn retry_max_seconds(&self) -> u64 {
        self.content.retry_max_seconds
    }
    pub fn allow_ethernet(&self) -> bool {
        self.content.allow_ethernet
    }
    pub fn allow_wifi(&self) -> bool {
        self.content.allow_wifi
    }
    pub fn allow_mobile(&self) -> bool {
        self.content.allow_mobile
    }
    pub fn allow_unknown_network(&self) -> bool {
        self.content.allow_unknown_network
    }
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let mut bytes = Vec::from(domain);
    bytes
        .extend(postcard::to_allocvec(content).context("encode own-device roster signed content")?);
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
    fn roster_policy_is_signed_bounded_and_append_only() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let account_id = AccountId::from_bytes([9_u8; 32]);
        let first = SignedOwnDeviceRosterPolicy::sign(
            &identity,
            account_id,
            100,
            1,
            [7_u8; 32],
            true,
            "https://store.invalid/".to_owned(),
            600,
            120,
            60,
            300,
            5,
            60,
            true,
            true,
            false,
            false,
            None,
        )?;
        first.verify(None)?;
        assert_eq!(
            SignedOwnDeviceRosterPolicy::decode(&first.encode()?)?,
            first
        );
        assert!(first.allows_network(RuntimeIpcNetworkClass::Wifi));
        assert!(!first.allows_network(RuntimeIpcNetworkClass::Mobile));

        let second = SignedOwnDeviceRosterPolicy::sign(
            &identity,
            account_id,
            101,
            1,
            [7_u8; 32],
            false,
            first.service_base_url().to_owned(),
            first.ttl_seconds(),
            first.refresh_before_seconds(),
            first.interval_seconds(),
            first.validity_seconds(),
            first.retry_base_seconds(),
            first.retry_max_seconds(),
            first.allow_ethernet(),
            first.allow_wifi(),
            first.allow_mobile(),
            first.allow_unknown_network(),
            Some(&first),
        )?;
        second.verify(Some(&first))?;
        assert_eq!(second.generation(), 2);
        assert!(!second.same_configuration(&first));
        assert!(
            SignedOwnDeviceRosterPolicy::sign(
                &identity,
                account_id,
                102,
                1,
                [8_u8; 32],
                true,
                first.service_base_url().to_owned(),
                first.ttl_seconds(),
                first.refresh_before_seconds(),
                first.interval_seconds(),
                first.validity_seconds(),
                first.retry_base_seconds(),
                first.retry_max_seconds(),
                first.allow_ethernet(),
                first.allow_wifi(),
                first.allow_mobile(),
                first.allow_unknown_network(),
                Some(&second),
            )
            .is_err()
        );
        Ok(())
    }
}
