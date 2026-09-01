use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, DeviceCapability, DeviceCertificate, DeviceId,
    DeviceIdentity, verify_device_authorization_with_snapshot,
};
use kilogram_protocol::{
    ConversationId, HistoryRewrapSas, MAX_HISTORY_REWRAP_ENTRIES, MAX_INVENTORY_EVENT_IDS,
};
use kilogram_transport_iroh::RoutePolicy;
use serde::{Deserialize, Serialize};

use crate::recovery_link::SignedHistoryRecoveryLink;

const HISTORY_RECOVERY_PLAN_VERSION: u8 = 1;
const HISTORY_RECOVERY_PLAN_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-plan-signature:v1\0";
const HISTORY_RECOVERY_PLAN_ID_DOMAIN: &[u8] = b"kilogram:history-recovery-plan-id:v1\0";
const HISTORY_RECOVERY_PLAN_CLOCK_SKEW_SECONDS: u64 = 120;

pub(crate) const DEFAULT_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS: u64 = 24;
pub(crate) const MAX_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS: u64 = 7 * 24;
pub(crate) const MAX_HISTORY_RECOVERY_PLAN_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
pub(crate) enum RecoveryNetworkClass {
    Ethernet,
    Wifi,
    Mobile,
    Unknown,
}

impl RecoveryNetworkClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ethernet => "ethernet",
            Self::Wifi => "wifi",
            Self::Mobile => "mobile",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
pub(crate) enum RecoveryPowerSource {
    External,
    Battery,
    Unknown,
}

impl RecoveryPowerSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::External => "external",
            Self::Battery => "battery",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct RecoveryExecutionPolicy {
    allow_ethernet: bool,
    allow_wifi: bool,
    allow_mobile: bool,
    allow_unknown_network: bool,
    require_external_power: bool,
}

impl RecoveryExecutionPolicy {
    pub(crate) fn new(
        allow_ethernet: bool,
        allow_wifi: bool,
        allow_mobile: bool,
        allow_unknown_network: bool,
        require_external_power: bool,
    ) -> Result<Self> {
        let policy = Self {
            allow_ethernet,
            allow_wifi,
            allow_mobile,
            allow_unknown_network,
            require_external_power,
        };
        policy.validate()?;
        Ok(policy)
    }

    pub(crate) fn allows(self, network: RecoveryNetworkClass, power: RecoveryPowerSource) -> bool {
        let network_allowed = match network {
            RecoveryNetworkClass::Ethernet => self.allow_ethernet,
            RecoveryNetworkClass::Wifi => self.allow_wifi,
            RecoveryNetworkClass::Mobile => self.allow_mobile,
            RecoveryNetworkClass::Unknown => self.allow_unknown_network,
        };
        let power_allowed = !self.require_external_power || power == RecoveryPowerSource::External;
        network_allowed && power_allowed
    }

    pub(crate) fn allow_ethernet(self) -> bool {
        self.allow_ethernet
    }

    pub(crate) fn allow_wifi(self) -> bool {
        self.allow_wifi
    }

    pub(crate) fn allow_mobile(self) -> bool {
        self.allow_mobile
    }

    pub(crate) fn allow_unknown_network(self) -> bool {
        self.allow_unknown_network
    }

    pub(crate) fn require_external_power(self) -> bool {
        self.require_external_power
    }

    fn validate(self) -> Result<()> {
        ensure!(
            self.allow_ethernet
                || self.allow_wifi
                || self.allow_mobile
                || self.allow_unknown_network,
            "history recovery plan blocks every network class"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct HistoryRecoveryPlanContent {
    version: u8,
    account_id: AccountId,
    account_device_list: AccountDeviceListSnapshot,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    conversation_id: ConversationId,
    sas: HistoryRewrapSas,
    approved_range_start: u64,
    approved_range_end: u64,
    page_size: u64,
    route_policy: RoutePolicy,
    execution_policy: RecoveryExecutionPolicy,
    approved_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct SignedHistoryRecoveryPlan {
    content: HistoryRecoveryPlanContent,
    signature: Vec<u8>,
}

pub(crate) struct HistoryRecoveryPlanOptions {
    pub link: SignedHistoryRecoveryLink,
    pub execution_policy: RecoveryExecutionPolicy,
    pub approved_at_unix_seconds: u64,
    pub valid_for_hours: u64,
}

impl SignedHistoryRecoveryPlan {
    pub(crate) fn approve(
        recipient_identity: &DeviceIdentity,
        options: HistoryRecoveryPlanOptions,
    ) -> Result<Self> {
        options.link.verify_at(options.approved_at_unix_seconds)?;
        ensure!(
            recipient_identity.device_id() == options.link.recipient_device_id(),
            "history recovery plan signer is not the link recipient"
        );
        ensure!(
            (1..=MAX_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS).contains(&options.valid_for_hours),
            "history recovery plan validity must be between 1 and {MAX_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS} hours"
        );
        let expires_at_unix_seconds = options
            .approved_at_unix_seconds
            .checked_add(
                options
                    .valid_for_hours
                    .checked_mul(60 * 60)
                    .context("history recovery plan validity overflows")?,
            )
            .context("history recovery plan expiration overflows")?;
        let content = HistoryRecoveryPlanContent {
            version: HISTORY_RECOVERY_PLAN_VERSION,
            account_id: options.link.account_id(),
            account_device_list: options.link.account_device_list().clone(),
            source_device_id: options.link.source_device_id(),
            recipient_device_id: options.link.recipient_device_id(),
            conversation_id: options.link.conversation_id(),
            sas: options.link.sas()?,
            approved_range_start: options.link.approved_range_start(),
            approved_range_end: options.link.approved_range_end(),
            page_size: options.link.page_size(),
            route_policy: options.link.route_policy(),
            execution_policy: options.execution_policy,
            approved_at_unix_seconds: options.approved_at_unix_seconds,
            expires_at_unix_seconds,
        };
        validate_content(&content)?;
        let signature = recipient_identity.sign(&signing_bytes(&content)?).to_vec();
        let plan = Self { content, signature };
        plan.verify()?;
        Ok(plan)
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self).context("encode history recovery plan")?;
        ensure!(
            encoded.len() <= MAX_HISTORY_RECOVERY_PLAN_BYTES,
            "history recovery plan exceeds {MAX_HISTORY_RECOVERY_PLAN_BYTES} bytes"
        );
        Ok(encoded)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_HISTORY_RECOVERY_PLAN_BYTES,
            "history recovery plan file must contain 1..={MAX_HISTORY_RECOVERY_PLAN_BYTES} bytes"
        );
        let plan: Self = postcard::from_bytes(bytes).context("decode history recovery plan")?;
        plan.verify()?;
        Ok(plan)
    }

    pub(crate) fn verify(&self) -> Result<()> {
        validate_content(&self.content)?;
        self.recipient_device_id()
            .verify(&signing_bytes(&self.content)?, &self.signature)
            .context("verify recipient signature on history recovery plan")
    }

    pub(crate) fn verify_at(&self, now_unix_seconds: u64) -> Result<()> {
        self.verify()?;
        ensure!(
            self.content.approved_at_unix_seconds
                <= now_unix_seconds.saturating_add(HISTORY_RECOVERY_PLAN_CLOCK_SKEW_SECONDS),
            "history recovery plan was approved too far in the future"
        );
        ensure!(
            now_unix_seconds
                <= self
                    .content
                    .expires_at_unix_seconds
                    .saturating_add(HISTORY_RECOVERY_PLAN_CLOCK_SKEW_SECONDS),
            "history recovery plan has expired"
        );
        Ok(())
    }

    pub(crate) fn matches_link(&self, link: &SignedHistoryRecoveryLink) -> Result<bool> {
        self.verify()?;
        link.verify()?;
        Ok(link.account_id() == self.account_id()
            && link.account_device_list() == self.account_device_list()
            && link.source_device_id() == self.source_device_id()
            && link.recipient_device_id() == self.recipient_device_id()
            && link.conversation_id() == self.conversation_id()
            && link.sas()? == self.sas()
            && link.approved_range_start() == self.approved_range_start()
            && link.approved_range_end() == self.approved_range_end()
            && link.page_size() == self.page_size()
            && link.route_policy() == self.route_policy())
    }

    pub(crate) fn plan_id(&self) -> Result<[u8; 32]> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self).context("encode history recovery plan ID")?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(HISTORY_RECOVERY_PLAN_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }

    pub(crate) fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub(crate) fn account_device_list(&self) -> &AccountDeviceListSnapshot {
        &self.content.account_device_list
    }

    pub(crate) fn source_device_id(&self) -> DeviceId {
        self.content.source_device_id
    }

    pub(crate) fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub(crate) fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub(crate) fn sas(&self) -> HistoryRewrapSas {
        self.content.sas
    }

    pub(crate) fn approved_range_start(&self) -> u64 {
        self.content.approved_range_start
    }

    pub(crate) fn approved_range_end(&self) -> u64 {
        self.content.approved_range_end
    }

    pub(crate) fn approved_event_count(&self) -> u64 {
        self.content
            .approved_range_end
            .saturating_sub(self.content.approved_range_start)
    }

    pub(crate) fn page_size(&self) -> u64 {
        self.content.page_size
    }

    pub(crate) fn route_policy(&self) -> RoutePolicy {
        self.content.route_policy
    }

    pub(crate) fn execution_policy(&self) -> RecoveryExecutionPolicy {
        self.content.execution_policy
    }

    pub(crate) fn approved_at_unix_seconds(&self) -> u64 {
        self.content.approved_at_unix_seconds
    }

    pub(crate) fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }
}

fn validate_content(content: &HistoryRecoveryPlanContent) -> Result<()> {
    ensure!(
        content.version == HISTORY_RECOVERY_PLAN_VERSION,
        "unsupported history recovery plan version: {}",
        content.version
    );
    content
        .account_device_list
        .verify_for_account(content.account_id)
        .context("verify history recovery plan account device list")?;
    let source_certificate = content
        .account_device_list
        .certificate_for(content.source_device_id)
        .context("history recovery plan source is absent from its device list")?;
    verify_messaging_device(
        content.account_id,
        source_certificate,
        &content.account_device_list,
    )
    .context("verify history recovery plan source authorization")?;
    let recipient_certificate = content
        .account_device_list
        .certificate_for(content.recipient_device_id)
        .context("history recovery plan recipient is absent from its device list")?;
    verify_messaging_device(
        content.account_id,
        recipient_certificate,
        &content.account_device_list,
    )
    .context("verify history recovery plan recipient authorization")?;
    ensure!(
        content.source_device_id != content.recipient_device_id,
        "history recovery plan source and recipient are the same device"
    );
    ensure!(
        HistoryRewrapSas::derive(
            &content.account_device_list,
            content.source_device_id,
            content.recipient_device_id,
        )? == content.sas,
        "history recovery plan SAS does not match its signed device list"
    );
    ensure!(
        content.approved_range_start < content.approved_range_end
            && content.approved_range_end <= MAX_INVENTORY_EVENT_IDS as u64,
        "history recovery plan approved range is invalid"
    );
    ensure!(
        (1..=MAX_HISTORY_REWRAP_ENTRIES as u64).contains(&content.page_size),
        "history recovery plan page size is outside the supported range"
    );
    content.execution_policy.validate()?;
    ensure!(
        content.approved_at_unix_seconds < content.expires_at_unix_seconds,
        "history recovery plan validity window is empty"
    );
    ensure!(
        content
            .expires_at_unix_seconds
            .saturating_sub(content.approved_at_unix_seconds)
            <= MAX_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS * 60 * 60,
        "history recovery plan validity exceeds {MAX_HISTORY_RECOVERY_PLAN_VALIDITY_HOURS} hours"
    );
    Ok(())
}

fn verify_messaging_device(
    account_id: AccountId,
    certificate: &DeviceCertificate,
    device_list: &AccountDeviceListSnapshot,
) -> Result<()> {
    verify_device_authorization_with_snapshot(
        account_id,
        certificate,
        device_list.authority_snapshot(),
        &DeviceCapability::MESSAGING,
    )?;
    Ok(())
}

fn signing_bytes(content: &HistoryRecoveryPlanContent) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode history recovery plan content")?;
    let mut bytes =
        Vec::with_capacity(HISTORY_RECOVERY_PLAN_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(HISTORY_RECOVERY_PLAN_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery_link::{HistoryRecoveryLinkOptions, SignedHistoryRecoveryLink};
    use iroh::{EndpointAddr, SecretKey};
    use kilogram_identity::{AccountRootState, DeviceEncryptionIdentity};

    fn link_fixture() -> Result<(
        SignedHistoryRecoveryLink,
        DeviceIdentity,
        DeviceIdentity,
        DeviceCertificate,
    )> {
        let directory = tempfile::tempdir()?;
        let root = AccountRootState::create(directory.path())?;
        let source = DeviceIdentity::generate()?;
        let recipient = DeviceIdentity::generate()?;
        let source_certificate = root.issue_device_certificate(
            source.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let recipient_certificate = root.issue_device_certificate(
            recipient.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let device_list =
            root.publish_device_list(&[source_certificate.clone(), recipient_certificate.clone()])?;
        let link = SignedHistoryRecoveryLink::sign(
            &source,
            HistoryRecoveryLinkOptions {
                endpoint: EndpointAddr::new(SecretKey::generate().public()),
                source_certificate,
                account_device_list: device_list,
                recipient_device_id: recipient.device_id(),
                conversation_id: ConversationId::from_label("scheduler-test"),
                approved_range_start: 0,
                approved_event_count: 8,
                page_size: 2,
                route_policy: RoutePolicy::Auto,
                issued_at_unix_seconds: 1_000,
                valid_for_seconds: 600,
            },
        )?;
        Ok((link, source, recipient, recipient_certificate))
    }

    #[test]
    fn recipient_signed_plan_round_trips_and_matches_fresh_endpoint() -> Result<()> {
        let (link, source, recipient, _) = link_fixture()?;
        let policy = RecoveryExecutionPolicy::new(true, true, false, false, false)?;
        let plan = SignedHistoryRecoveryPlan::approve(
            &recipient,
            HistoryRecoveryPlanOptions {
                link: link.clone(),
                execution_policy: policy,
                approved_at_unix_seconds: 1_100,
                valid_for_hours: 24,
            },
        )?;
        let decoded = SignedHistoryRecoveryPlan::decode(&plan.encode()?)?;
        decoded.verify_at(2_000)?;
        assert_eq!(decoded, plan);
        assert!(decoded.matches_link(&link)?);
        let fresh_link = SignedHistoryRecoveryLink::sign(
            &source,
            HistoryRecoveryLinkOptions {
                endpoint: EndpointAddr::new(SecretKey::generate().public()),
                source_certificate: link
                    .account_device_list()
                    .certificate_for(source.device_id())
                    .context("source certificate is missing")?
                    .clone(),
                account_device_list: link.account_device_list().clone(),
                recipient_device_id: link.recipient_device_id(),
                conversation_id: link.conversation_id(),
                approved_range_start: usize::try_from(link.approved_range_start())?,
                approved_event_count: usize::try_from(link.approved_event_count())?,
                page_size: usize::try_from(link.page_size())?,
                route_policy: link.route_policy(),
                issued_at_unix_seconds: 1_200,
                valid_for_seconds: 600,
            },
        )?;
        assert_ne!(fresh_link.endpoint(), link.endpoint());
        assert!(decoded.matches_link(&fresh_link)?);
        assert!(policy.allows(RecoveryNetworkClass::Wifi, RecoveryPowerSource::Battery));
        assert!(!policy.allows(RecoveryNetworkClass::Mobile, RecoveryPowerSource::External));
        assert!(decoded.verify_at(100_000).is_err());
        Ok(())
    }

    #[test]
    fn external_power_policy_is_fail_closed_for_battery_and_unknown() -> Result<()> {
        let policy = RecoveryExecutionPolicy::new(true, false, false, false, true)?;
        assert!(policy.allows(
            RecoveryNetworkClass::Ethernet,
            RecoveryPowerSource::External
        ));
        assert!(!policy.allows(RecoveryNetworkClass::Ethernet, RecoveryPowerSource::Battery));
        assert!(!policy.allows(RecoveryNetworkClass::Ethernet, RecoveryPowerSource::Unknown));
        assert!(RecoveryExecutionPolicy::new(false, false, false, false, false).is_err());
        Ok(())
    }
}
