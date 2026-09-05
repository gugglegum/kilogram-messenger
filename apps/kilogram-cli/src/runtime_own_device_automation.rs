use std::{
    fmt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use kilogram_runtime_ipc::RuntimeIpcNetworkClass;
use serde::{Deserialize, Serialize};

use crate::runtime_queue::MAX_RUNTIME_RECORD_BYTES;

const POLICY_VERSION: u8 = 1;
const ATTEMPT_VERSION: u8 = 1;
const POLICY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:own-device-announcement-policy:v1\0";
const ATTEMPT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:own-device-announcement-attempt:v1\0";
const MAX_TICKET_PATH_BYTES: usize = 4_096;
pub const DEFAULT_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS: u64 = 5 * 60;
pub const MIN_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS: u64 = 30;
pub const MAX_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
pub const DEFAULT_OWN_DEVICE_ANNOUNCEMENT_RETRY_BASE_SECONDS: u64 = 5;
pub const DEFAULT_OWN_DEVICE_ANNOUNCEMENT_RETRY_MAX_SECONDS: u64 = 5 * 60;
pub const MAX_OWN_DEVICE_ANNOUNCEMENT_RETRY_SECONDS: u64 = 60 * 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OwnDeviceAnnouncementPolicyId([u8; 32]);

impl fmt::Display for OwnDeviceAnnouncementPolicyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OwnDeviceAnnouncementAttemptId([u8; 32]);

impl fmt::Display for OwnDeviceAnnouncementAttemptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OwnDeviceAnnouncementPolicyContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    recipient_device_id: DeviceId,
    generation: u64,
    previous_policy_id: Option<OwnDeviceAnnouncementPolicyId>,
    configured_at_unix_seconds: u64,
    enabled: bool,
    recipient_ticket_file: PathBuf,
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
pub struct SignedOwnDeviceAnnouncementPolicy {
    content: OwnDeviceAnnouncementPolicyContent,
    signature: Vec<u8>,
}

impl SignedOwnDeviceAnnouncementPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        recipient_device_id: DeviceId,
        configured_at_unix_seconds: u64,
        enabled: bool,
        recipient_ticket_file: PathBuf,
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
        ensure!(
            recipient_device_id != identity.device_id(),
            "own-device announcement recipient must be another device"
        );
        let (generation, previous_policy_id) = match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    previous.local_account_id() == local_account_id
                        && previous.local_device_id() == identity.device_id()
                        && previous.recipient_device_id() == recipient_device_id,
                    "own-device announcement policy chain changes identity"
                );
                ensure!(
                    configured_at_unix_seconds >= previous.configured_at_unix_seconds(),
                    "own-device announcement policy time moves backwards"
                );
                (
                    previous
                        .generation()
                        .checked_add(1)
                        .context("own-device announcement policy generation overflow")?,
                    Some(previous.policy_id()?),
                )
            }
            None => (1, None),
        };
        let content = OwnDeviceAnnouncementPolicyContent {
            version: POLICY_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            recipient_device_id,
            generation,
            previous_policy_id,
            configured_at_unix_seconds,
            enabled,
            recipient_ticket_file,
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
        let bytes = postcard::to_allocvec(self).context("encode own-device announcement policy")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device announcement policy is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device announcement policy size is invalid"
        );
        let policy: Self =
            postcard::from_bytes(bytes).context("decode own-device announcement policy")?;
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
                        && self.recipient_device_id() == previous.recipient_device_id(),
                    "own-device announcement policy chain changes identity"
                );
                ensure!(
                    self.generation() == previous.generation().saturating_add(1)
                        && self.content.previous_policy_id == Some(previous.policy_id()?)
                        && self.configured_at_unix_seconds()
                            >= previous.configured_at_unix_seconds(),
                    "own-device announcement policy chain is not contiguous"
                );
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_policy_id.is_none(),
                "own-device announcement policy chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    pub(crate) fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == POLICY_VERSION,
            "unsupported own-device announcement policy version"
        );
        ensure!(
            self.content.local_device_id != self.content.recipient_device_id,
            "own-device announcement policy names the local device as recipient"
        );
        validate_ticket_path(&self.content.recipient_ticket_file)?;
        ensure!(
            (MIN_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS
                ..=MAX_OWN_DEVICE_ANNOUNCEMENT_INTERVAL_SECONDS)
                .contains(&self.content.interval_seconds),
            "own-device announcement interval is outside bounds"
        );
        ensure!(
            (1..=crate::runtime_endpoint_announcement::MAX_ENDPOINT_ANNOUNCEMENT_VALIDITY_SECONDS)
                .contains(&self.content.validity_seconds),
            "own-device announcement validity is outside bounds"
        );
        ensure!(
            (1..=MAX_OWN_DEVICE_ANNOUNCEMENT_RETRY_SECONDS)
                .contains(&self.content.retry_base_seconds)
                && self.content.retry_max_seconds >= self.content.retry_base_seconds
                && self.content.retry_max_seconds <= MAX_OWN_DEVICE_ANNOUNCEMENT_RETRY_SECONDS,
            "own-device announcement retry bounds are invalid"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(POLICY_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify own-device announcement policy signature")
    }

    pub fn policy_id(&self) -> Result<OwnDeviceAnnouncementPolicyId> {
        Ok(OwnDeviceAnnouncementPolicyId(
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
    pub fn recipient_ticket_file(&self) -> &Path {
        &self.content.recipient_ticket_file
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum OwnDeviceAnnouncementAttemptResult {
    Succeeded {
        bundle_id: String,
        transport_path: String,
    },
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct OwnDeviceAnnouncementAttemptContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    recipient_device_id: DeviceId,
    generation: u64,
    previous_attempt_id: Option<OwnDeviceAnnouncementAttemptId>,
    policy_generation: u64,
    attempted_at_unix_seconds: u64,
    not_before_unix_seconds: u64,
    consecutive_failures: u32,
    result: OwnDeviceAnnouncementAttemptResult,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedOwnDeviceAnnouncementAttempt {
    content: OwnDeviceAnnouncementAttemptContent,
    signature: Vec<u8>,
}

impl SignedOwnDeviceAnnouncementAttempt {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        policy: &SignedOwnDeviceAnnouncementPolicy,
        attempted_at_unix_seconds: u64,
        result: Option<(String, String)>,
        previous: Option<&Self>,
    ) -> Result<Self> {
        policy.verify_signature()?;
        let generation = previous.map_or(Ok(1), |previous| {
            previous
                .generation()
                .checked_add(1)
                .context("own-device announcement attempt generation overflow")
        })?;
        let previous_attempt_id = previous.map(Self::attempt_id).transpose()?;
        let prior_failures = previous
            .filter(|previous| {
                previous.policy_generation() == policy.generation() && !previous.succeeded()
            })
            .map_or(0, Self::consecutive_failures);
        let (result, consecutive_failures, not_before_unix_seconds) = match result {
            Some((bundle_id, transport_path)) => (
                OwnDeviceAnnouncementAttemptResult::Succeeded {
                    bundle_id,
                    transport_path,
                },
                0,
                attempted_at_unix_seconds.saturating_add(policy.interval_seconds()),
            ),
            None => {
                let consecutive_failures = prior_failures.saturating_add(1);
                let shift = consecutive_failures.saturating_sub(1).min(31);
                let delay = policy
                    .retry_base_seconds()
                    .saturating_mul(1_u64 << shift)
                    .min(policy.retry_max_seconds());
                (
                    OwnDeviceAnnouncementAttemptResult::Failed,
                    consecutive_failures,
                    attempted_at_unix_seconds.saturating_add(delay),
                )
            }
        };
        let content = OwnDeviceAnnouncementAttemptContent {
            version: ATTEMPT_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            recipient_device_id: policy.recipient_device_id(),
            generation,
            previous_attempt_id,
            policy_generation: policy.generation(),
            attempted_at_unix_seconds,
            not_before_unix_seconds,
            consecutive_failures,
            result,
        };
        let signature = identity
            .sign(&signing_bytes(ATTEMPT_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let attempt = Self { content, signature };
        attempt.verify(previous)?;
        Ok(attempt)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.verify_signature()?;
        let bytes =
            postcard::to_allocvec(self).context("encode own-device announcement attempt")?;
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device announcement attempt is too large"
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "own-device announcement attempt size is invalid"
        );
        let attempt: Self =
            postcard::from_bytes(bytes).context("decode own-device announcement attempt")?;
        attempt.verify_signature()?;
        Ok(attempt)
    }

    pub fn verify(&self, previous: Option<&Self>) -> Result<()> {
        self.verify_signature()?;
        match previous {
            Some(previous) => {
                previous.verify_signature()?;
                ensure!(
                    self.local_account_id() == previous.local_account_id()
                        && self.local_device_id() == previous.local_device_id()
                        && self.recipient_device_id() == previous.recipient_device_id(),
                    "own-device announcement attempt chain changes identity"
                );
                ensure!(
                    self.generation() == previous.generation().saturating_add(1)
                        && self.content.previous_attempt_id == Some(previous.attempt_id()?)
                        && self.attempted_at_unix_seconds() >= previous.attempted_at_unix_seconds(),
                    "own-device announcement attempt chain is not contiguous"
                );
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_attempt_id.is_none(),
                "own-device announcement attempt chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    pub(crate) fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == ATTEMPT_VERSION
                && self.content.policy_generation != 0
                && self.content.local_device_id != self.content.recipient_device_id
                && self.content.not_before_unix_seconds >= self.content.attempted_at_unix_seconds,
            "own-device announcement attempt metadata is invalid"
        );
        match &self.content.result {
            OwnDeviceAnnouncementAttemptResult::Succeeded {
                bundle_id,
                transport_path,
            } => {
                ensure!(
                    bundle_id.len() == 64
                        && bundle_id.bytes().all(|byte| byte.is_ascii_hexdigit())
                        && matches!(transport_path.as_str(), "direct" | "relay")
                        && self.content.consecutive_failures == 0
                        && self.content.not_before_unix_seconds
                            > self.content.attempted_at_unix_seconds,
                    "own-device announcement success state is invalid"
                );
            }
            OwnDeviceAnnouncementAttemptResult::Failed => ensure!(
                self.content.consecutive_failures != 0,
                "own-device announcement failure count is zero"
            ),
        }
        self.content
            .local_device_id
            .verify(
                &signing_bytes(ATTEMPT_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify own-device announcement attempt signature")
    }

    pub fn attempt_id(&self) -> Result<OwnDeviceAnnouncementAttemptId> {
        Ok(OwnDeviceAnnouncementAttemptId(
            *blake3::hash(&self.encode()?).as_bytes(),
        ))
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
    pub fn policy_generation(&self) -> u64 {
        self.content.policy_generation
    }
    pub fn attempted_at_unix_seconds(&self) -> u64 {
        self.content.attempted_at_unix_seconds
    }
    pub fn not_before_unix_seconds(&self) -> u64 {
        self.content.not_before_unix_seconds
    }
    pub fn consecutive_failures(&self) -> u32 {
        self.content.consecutive_failures
    }
    pub fn succeeded(&self) -> bool {
        matches!(
            self.content.result,
            OwnDeviceAnnouncementAttemptResult::Succeeded { .. }
        )
    }
    pub fn bundle_id(&self) -> Option<&str> {
        match &self.content.result {
            OwnDeviceAnnouncementAttemptResult::Succeeded { bundle_id, .. } => Some(bundle_id),
            OwnDeviceAnnouncementAttemptResult::Failed => None,
        }
    }
    pub fn transport_path(&self) -> Option<&str> {
        match &self.content.result {
            OwnDeviceAnnouncementAttemptResult::Succeeded { transport_path, .. } => {
                Some(transport_path)
            }
            OwnDeviceAnnouncementAttemptResult::Failed => None,
        }
    }
}

fn validate_ticket_path(path: &Path) -> Result<()> {
    let text = path.to_string_lossy();
    ensure!(
        path.is_absolute() && !text.is_empty() && text.len() <= MAX_TICKET_PATH_BYTES,
        "own-device announcement ticket path is invalid"
    );
    Ok(())
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let mut bytes = Vec::from(domain);
    bytes.extend(
        postcard::to_allocvec(content).context("encode own-device announcement signed content")?,
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
    fn policy_and_attempt_chains_are_signed_restart_safe_and_bounded() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let ticket_directory = tempfile::tempdir()?;
        let ticket = ticket_directory.path().join("recipient.ticket");
        let policy = SignedOwnDeviceAnnouncementPolicy::sign(
            &identity,
            AccountId::from_bytes([7; 32]),
            recipient.device_id(),
            100,
            true,
            ticket,
            300,
            900,
            5,
            300,
            true,
            true,
            false,
            false,
            None,
        )?;
        assert!(policy.allows_network(RuntimeIpcNetworkClass::Wifi));
        assert!(!policy.allows_network(RuntimeIpcNetworkClass::Mobile));
        assert_eq!(
            SignedOwnDeviceAnnouncementPolicy::decode(&policy.encode()?)?,
            policy
        );

        let failed = SignedOwnDeviceAnnouncementAttempt::sign(
            &identity,
            policy.local_account_id(),
            &policy,
            110,
            None,
            None,
        )?;
        assert_eq!(failed.not_before_unix_seconds(), 115);
        let succeeded = SignedOwnDeviceAnnouncementAttempt::sign(
            &identity,
            policy.local_account_id(),
            &policy,
            115,
            Some(("ab".repeat(32), "direct".to_owned())),
            Some(&failed),
        )?;
        assert_eq!(succeeded.not_before_unix_seconds(), 415);
        assert_eq!(
            succeeded.bundle_id(),
            Some("abababababababababababababababababababababababababababababababab")
        );
        assert_eq!(
            SignedOwnDeviceAnnouncementAttempt::decode(&succeeded.encode()?)?,
            succeeded
        );
        Ok(())
    }
}
