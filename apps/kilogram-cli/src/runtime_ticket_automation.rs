use std::fmt;

use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use kilogram_protocol::ConversationId;
use kilogram_runtime_ipc::RuntimeIpcNetworkClass;
use serde::{Deserialize, Serialize};

use crate::runtime_queue::{MAX_RUNTIME_RECORD_BYTES, RuntimeContactId};

const POLICY_VERSION: u8 = 1;
const ATTEMPT_VERSION: u8 = 1;
const POLICY_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-automation-policy:v1\0";
const ATTEMPT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:ticket-automation-attempt:v1\0";
const MAX_SERVICE_URL_BYTES: usize = 2_048;
pub const DEFAULT_REFRESH_BEFORE_SECONDS: u64 = 5 * 60;
pub const MIN_REFRESH_BEFORE_SECONDS: u64 = 10;
pub const MAX_REFRESH_BEFORE_SECONDS: u64 = 30 * 60;
pub const DEFAULT_AUTOMATION_RETRY_BASE_SECONDS: u64 = 5;
pub const DEFAULT_AUTOMATION_RETRY_MAX_SECONDS: u64 = 5 * 60;
pub const MAX_AUTOMATION_RETRY_SECONDS: u64 = 60 * 60;
const MIN_SUCCESS_RECHECK_SECONDS: u64 = 30;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketAutomationPolicyId([u8; 32]);

impl fmt::Display for TicketAutomationPolicyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TicketAutomationAttemptId([u8; 32]);

impl fmt::Display for TicketAutomationAttemptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TicketAutomationAction {
    Publish,
    Refresh,
}

impl TicketAutomationAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Refresh => "refresh",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TicketAutomationPolicyContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    contact_id: RuntimeContactId,
    conversation: String,
    conversation_id: ConversationId,
    peer_account_id: AccountId,
    generation: u64,
    previous_policy_id: Option<TicketAutomationPolicyId>,
    configured_at_unix_seconds: u64,
    enabled: bool,
    service_base_url: String,
    ttl_seconds: u64,
    refresh_before_seconds: u64,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    allow_ethernet: bool,
    allow_wifi: bool,
    allow_mobile: bool,
    allow_unknown_network: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedTicketAutomationPolicy {
    content: TicketAutomationPolicyContent,
    signature: Vec<u8>,
}

impl SignedTicketAutomationPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        contact_id: RuntimeContactId,
        conversation: String,
        peer_account_id: AccountId,
        configured_at_unix_seconds: u64,
        enabled: bool,
        service_base_url: String,
        ttl_seconds: u64,
        refresh_before_seconds: u64,
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
                        && previous.local_device_id() == identity.device_id()
                        && previous.contact_id() == contact_id
                        && previous.conversation() == conversation
                        && previous.peer_account_id() == peer_account_id,
                    "ticket automation policy chain changes contact identity"
                );
                ensure!(
                    configured_at_unix_seconds >= previous.configured_at_unix_seconds(),
                    "ticket automation policy time moves backwards"
                );
                (
                    previous
                        .generation()
                        .checked_add(1)
                        .context("ticket automation policy generation overflow")?,
                    Some(previous.policy_id()?),
                )
            }
            None => (1, None),
        };
        let content = TicketAutomationPolicyContent {
            version: POLICY_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            contact_id,
            conversation_id: ConversationId::from_label(&conversation),
            conversation,
            peer_account_id,
            generation,
            previous_policy_id,
            configured_at_unix_seconds,
            enabled,
            service_base_url,
            ttl_seconds,
            refresh_before_seconds,
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
        postcard::to_allocvec(self).context("encode ticket automation policy")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "ticket automation policy is too large"
        );
        let policy: Self =
            postcard::from_bytes(bytes).context("decode ticket automation policy")?;
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
                        && self.contact_id() == previous.contact_id()
                        && self.conversation() == previous.conversation()
                        && self.peer_account_id() == previous.peer_account_id(),
                    "ticket automation policy chain changes contact identity"
                );
                ensure!(
                    self.generation() == previous.generation().saturating_add(1)
                        && self.content.previous_policy_id == Some(previous.policy_id()?),
                    "ticket automation policy chain is not contiguous"
                );
                ensure!(
                    self.configured_at_unix_seconds() >= previous.configured_at_unix_seconds(),
                    "ticket automation policy chain moves time backwards"
                );
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_policy_id.is_none(),
                "ticket automation policy chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == POLICY_VERSION,
            "unsupported ticket automation policy version"
        );
        ensure!(
            !self.content.conversation.is_empty()
                && self.content.conversation.len() <= 4_096
                && ConversationId::from_label(&self.content.conversation)
                    == self.content.conversation_id,
            "ticket automation policy has an invalid conversation"
        );
        ensure!(
            !self.content.service_base_url.is_empty()
                && self.content.service_base_url.len() <= MAX_SERVICE_URL_BYTES,
            "ticket automation policy has an invalid service URL"
        );
        ensure!(
            (MIN_REFRESH_BEFORE_SECONDS..=MAX_REFRESH_BEFORE_SECONDS)
                .contains(&self.content.refresh_before_seconds)
                && self.content.refresh_before_seconds < self.content.ttl_seconds,
            "ticket automation refresh lead is outside bounds"
        );
        ensure!(
            (1..=MAX_AUTOMATION_RETRY_SECONDS).contains(&self.content.retry_base_seconds)
                && self.content.retry_max_seconds >= self.content.retry_base_seconds
                && self.content.retry_max_seconds <= MAX_AUTOMATION_RETRY_SECONDS,
            "ticket automation retry bounds are invalid"
        );
        self.content
            .local_device_id
            .verify(
                &signing_bytes(POLICY_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify ticket automation policy signature")
    }

    pub fn policy_id(&self) -> Result<TicketAutomationPolicyId> {
        Ok(TicketAutomationPolicyId(
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
    pub fn contact_id(&self) -> RuntimeContactId {
        self.content.contact_id
    }
    pub fn conversation(&self) -> &str {
        &self.content.conversation
    }
    pub fn peer_account_id(&self) -> AccountId {
        self.content.peer_account_id
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
enum TicketAutomationAttemptResult {
    Succeeded {
        publication_generation: u64,
        expires_at_unix_seconds: u64,
    },
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TicketAutomationAttemptContent {
    version: u8,
    local_account_id: AccountId,
    local_device_id: DeviceId,
    contact_id: RuntimeContactId,
    action: TicketAutomationAction,
    generation: u64,
    previous_attempt_id: Option<TicketAutomationAttemptId>,
    policy_generation: u64,
    attempted_at_unix_seconds: u64,
    not_before_unix_seconds: u64,
    consecutive_failures: u32,
    result: TicketAutomationAttemptResult,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedTicketAutomationAttempt {
    content: TicketAutomationAttemptContent,
    signature: Vec<u8>,
}

impl SignedTicketAutomationAttempt {
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        identity: &DeviceIdentity,
        local_account_id: AccountId,
        policy: &SignedTicketAutomationPolicy,
        action: TicketAutomationAction,
        attempted_at_unix_seconds: u64,
        result: Option<(u64, u64)>,
        previous: Option<&Self>,
    ) -> Result<Self> {
        policy.verify_signature()?;
        let generation = previous.map_or(Ok(1), |previous| {
            previous
                .generation()
                .checked_add(1)
                .context("ticket automation attempt generation overflow")
        })?;
        let previous_attempt_id = previous.map(Self::attempt_id).transpose()?;
        let prior_failures = previous
            .filter(|previous| {
                previous.policy_generation() == policy.generation() && !previous.succeeded()
            })
            .map_or(0, Self::consecutive_failures);
        let (result, consecutive_failures, not_before_unix_seconds) = match result {
            Some((publication_generation, expires_at_unix_seconds)) => {
                ensure!(
                    publication_generation != 0
                        && expires_at_unix_seconds > attempted_at_unix_seconds,
                    "ticket automation success metadata is invalid"
                );
                (
                    TicketAutomationAttemptResult::Succeeded {
                        publication_generation,
                        expires_at_unix_seconds,
                    },
                    0,
                    expires_at_unix_seconds
                        .saturating_sub(policy.refresh_before_seconds())
                        .max(attempted_at_unix_seconds.saturating_add(MIN_SUCCESS_RECHECK_SECONDS)),
                )
            }
            None => {
                let consecutive_failures = prior_failures.saturating_add(1);
                let shift = consecutive_failures.saturating_sub(1).min(31);
                let delay = policy
                    .retry_base_seconds()
                    .saturating_mul(1_u64 << shift)
                    .min(policy.retry_max_seconds());
                (
                    TicketAutomationAttemptResult::Failed,
                    consecutive_failures,
                    attempted_at_unix_seconds.saturating_add(delay),
                )
            }
        };
        let content = TicketAutomationAttemptContent {
            version: ATTEMPT_VERSION,
            local_account_id,
            local_device_id: identity.device_id(),
            contact_id: policy.contact_id(),
            action,
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
        postcard::to_allocvec(self).context("encode ticket automation attempt")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= MAX_RUNTIME_RECORD_BYTES,
            "ticket automation attempt is too large"
        );
        let attempt: Self =
            postcard::from_bytes(bytes).context("decode ticket automation attempt")?;
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
                        && self.contact_id() == previous.contact_id()
                        && self.action() == previous.action(),
                    "ticket automation attempt chain changes identity"
                );
                ensure!(
                    self.generation() == previous.generation().saturating_add(1)
                        && self.content.previous_attempt_id == Some(previous.attempt_id()?),
                    "ticket automation attempt chain is not contiguous"
                );
                ensure!(
                    self.attempted_at_unix_seconds() >= previous.attempted_at_unix_seconds(),
                    "ticket automation attempt time moves backwards"
                );
            }
            None => ensure!(
                self.generation() == 1 && self.content.previous_attempt_id.is_none(),
                "ticket automation attempt chain has an invalid first entry"
            ),
        }
        Ok(())
    }

    fn verify_signature(&self) -> Result<()> {
        ensure!(
            self.content.version == ATTEMPT_VERSION,
            "unsupported ticket automation attempt version"
        );
        ensure!(
            self.content.policy_generation != 0,
            "ticket automation attempt has zero policy generation"
        );
        ensure!(
            self.content.not_before_unix_seconds >= self.content.attempted_at_unix_seconds,
            "ticket automation attempt schedules time backwards"
        );
        match self.content.result {
            TicketAutomationAttemptResult::Succeeded {
                publication_generation,
                expires_at_unix_seconds,
            } => ensure!(
                publication_generation != 0
                    && expires_at_unix_seconds > self.content.attempted_at_unix_seconds
                    && self.content.consecutive_failures == 0,
                "ticket automation success state is invalid"
            ),
            TicketAutomationAttemptResult::Failed => ensure!(
                self.content.consecutive_failures != 0,
                "ticket automation failure count is zero"
            ),
        }
        self.content
            .local_device_id
            .verify(
                &signing_bytes(ATTEMPT_SIGNATURE_DOMAIN, &self.content)?,
                &self.signature,
            )
            .context("verify ticket automation attempt signature")
    }

    pub fn attempt_id(&self) -> Result<TicketAutomationAttemptId> {
        Ok(TicketAutomationAttemptId(
            *blake3::hash(&self.encode()?).as_bytes(),
        ))
    }
    pub fn local_account_id(&self) -> AccountId {
        self.content.local_account_id
    }
    pub fn local_device_id(&self) -> DeviceId {
        self.content.local_device_id
    }
    pub fn contact_id(&self) -> RuntimeContactId {
        self.content.contact_id
    }
    pub fn action(&self) -> TicketAutomationAction {
        self.content.action
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
            TicketAutomationAttemptResult::Succeeded { .. }
        )
    }
    pub fn publication_generation(&self) -> Option<u64> {
        match self.content.result {
            TicketAutomationAttemptResult::Succeeded {
                publication_generation,
                ..
            } => Some(publication_generation),
            TicketAutomationAttemptResult::Failed => None,
        }
    }
    pub fn expires_at_unix_seconds(&self) -> Option<u64> {
        match self.content.result {
            TicketAutomationAttemptResult::Succeeded {
                expires_at_unix_seconds,
                ..
            } => Some(expires_at_unix_seconds),
            TicketAutomationAttemptResult::Failed => None,
        }
    }
}

fn signing_bytes<T: Serialize>(domain: &[u8], content: &T) -> Result<Vec<u8>> {
    let mut bytes = Vec::from(domain);
    bytes
        .extend(postcard::to_allocvec(content).context("encode ticket automation signed content")?);
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
    fn policy_and_attempt_chains_are_signed_monotonic_and_bounded() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let account = AccountId::from_bytes([7; 32]);
        let contact = RuntimeContactId::from_bytes([8; 32]);
        let first = SignedTicketAutomationPolicy::sign(
            &identity,
            account,
            contact,
            "automation-test".to_owned(),
            AccountId::from_bytes([9; 32]),
            100,
            true,
            "https://store.example/".to_owned(),
            900,
            300,
            5,
            300,
            true,
            true,
            false,
            false,
            None,
        )?;
        assert!(first.allows_network(RuntimeIpcNetworkClass::Wifi));
        assert!(!first.allows_network(RuntimeIpcNetworkClass::Mobile));
        let failed = SignedTicketAutomationAttempt::sign(
            &identity,
            account,
            &first,
            TicketAutomationAction::Publish,
            110,
            None,
            None,
        )?;
        assert_eq!(failed.not_before_unix_seconds(), 115);
        let failed_again = SignedTicketAutomationAttempt::sign(
            &identity,
            account,
            &first,
            TicketAutomationAction::Publish,
            115,
            None,
            Some(&failed),
        )?;
        assert_eq!(failed_again.not_before_unix_seconds(), 125);
        let success = SignedTicketAutomationAttempt::sign(
            &identity,
            account,
            &first,
            TicketAutomationAction::Publish,
            125,
            Some((1, 1_025)),
            Some(&failed_again),
        )?;
        assert_eq!(success.not_before_unix_seconds(), 725);
        assert_eq!(success.publication_generation(), Some(1));
        let bytes = success.encode()?;
        assert_eq!(SignedTicketAutomationAttempt::decode(&bytes)?, success);

        let disabled = SignedTicketAutomationPolicy::sign(
            &identity,
            account,
            contact,
            "automation-test".to_owned(),
            AccountId::from_bytes([9; 32]),
            126,
            false,
            "https://store.example/".to_owned(),
            900,
            300,
            5,
            300,
            true,
            true,
            false,
            false,
            Some(&first),
        )?;
        assert_eq!(disabled.generation(), 2);
        disabled.verify(Some(&first))?;

        let near_expiry = SignedTicketAutomationAttempt::sign(
            &identity,
            account,
            &disabled,
            TicketAutomationAction::Refresh,
            200,
            Some((2, 210)),
            None,
        )?;
        assert_eq!(near_expiry.not_before_unix_seconds(), 230);
        Ok(())
    }
}
