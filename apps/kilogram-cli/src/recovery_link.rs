use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::EndpointAddr;
use kilogram_identity::{
    AccountDeviceListSnapshot, AccountId, DeviceCapability, DeviceCertificate, DeviceId,
    DeviceIdentity, verify_device_authorization_with_snapshot,
};
use kilogram_protocol::{
    ConversationId, HistoryRewrapSas, MAX_HISTORY_REWRAP_ENTRIES, MAX_INVENTORY_EVENT_IDS,
};
use kilogram_transport_iroh::RoutePolicy;
use serde::{Deserialize, Serialize};

const HISTORY_RECOVERY_LINK_VERSION: u8 = 1;
const HISTORY_RECOVERY_LINK_PREFIX: &str = "kilogram://history-recovery/v1/";
const HISTORY_RECOVERY_LINK_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-link-signature:v1\0";
const HISTORY_RECOVERY_LINK_ID_DOMAIN: &[u8] = b"kilogram:history-recovery-link-id:v1\0";
const HISTORY_RECOVERY_LINK_CLOCK_SKEW_SECONDS: u64 = 120;

pub(crate) const DEFAULT_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS: u64 = 10 * 60;
pub(crate) const MAX_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS: u64 = 60 * 60;
pub(crate) const MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES: usize = 2_953;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct HistoryRecoveryLinkContent {
    version: u8,
    endpoint: EndpointAddr,
    source_certificate: DeviceCertificate,
    account_device_list: AccountDeviceListSnapshot,
    recipient_device_id: DeviceId,
    conversation_id: ConversationId,
    approved_range_start: u64,
    approved_range_end: u64,
    page_size: u64,
    route_policy: RoutePolicy,
    issued_at_unix_seconds: u64,
    expires_at_unix_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct SignedHistoryRecoveryLink {
    content: HistoryRecoveryLinkContent,
    signature: Vec<u8>,
}

pub(crate) struct HistoryRecoveryLinkOptions {
    pub endpoint: EndpointAddr,
    pub source_certificate: DeviceCertificate,
    pub account_device_list: AccountDeviceListSnapshot,
    pub recipient_device_id: DeviceId,
    pub conversation_id: ConversationId,
    pub approved_range_start: usize,
    pub approved_event_count: usize,
    pub page_size: usize,
    pub route_policy: RoutePolicy,
    pub issued_at_unix_seconds: u64,
    pub valid_for_seconds: u64,
}

impl SignedHistoryRecoveryLink {
    pub(crate) fn sign(
        source_identity: &DeviceIdentity,
        options: HistoryRecoveryLinkOptions,
    ) -> Result<Self> {
        let approved_range_start = u64::try_from(options.approved_range_start)
            .context("history recovery link range start cannot be represented")?;
        let approved_event_count = u64::try_from(options.approved_event_count)
            .context("history recovery link event count cannot be represented")?;
        let approved_range_end = approved_range_start
            .checked_add(approved_event_count)
            .context("history recovery link approved range overflows")?;
        let expires_at_unix_seconds = options
            .issued_at_unix_seconds
            .checked_add(options.valid_for_seconds)
            .context("history recovery link expiration overflows")?;
        let content = HistoryRecoveryLinkContent {
            version: HISTORY_RECOVERY_LINK_VERSION,
            endpoint: options.endpoint,
            source_certificate: options.source_certificate,
            account_device_list: options.account_device_list,
            recipient_device_id: options.recipient_device_id,
            conversation_id: options.conversation_id,
            approved_range_start,
            approved_range_end,
            page_size: u64::try_from(options.page_size)
                .context("history recovery link page size cannot be represented")?,
            route_policy: options.route_policy,
            issued_at_unix_seconds: options.issued_at_unix_seconds,
            expires_at_unix_seconds,
        };
        validate_content(&content)?;
        ensure!(
            source_identity.device_id() == content.source_certificate.device_id(),
            "history recovery link signer is not the certified source device"
        );
        let signature = source_identity.sign(&signing_bytes(&content)?).to_vec();
        let link = Self { content, signature };
        link.verify()?;
        Ok(link)
    }

    pub(crate) fn encode_text(&self) -> Result<String> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self).context("encode history recovery link")?;
        let text = format!(
            "{HISTORY_RECOVERY_LINK_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(encoded)
        );
        ensure!(
            text.len() <= MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES,
            "history recovery link has {} bytes; QR-ready limit is {MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES}",
            text.len()
        );
        Ok(text)
    }

    pub(crate) fn decode_text(text: &str) -> Result<Self> {
        let text = text.trim();
        ensure!(
            text.len() <= MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES,
            "history recovery link exceeds the QR-ready size limit"
        );
        let payload = text
            .strip_prefix(HISTORY_RECOVERY_LINK_PREFIX)
            .context("history recovery link has an unsupported URI prefix")?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .context("decode history recovery link as base64url")?;
        let link: Self =
            postcard::from_bytes(&bytes).context("decode history recovery link payload")?;
        link.verify()?;
        Ok(link)
    }

    pub(crate) fn verify(&self) -> Result<()> {
        validate_content(&self.content)?;
        self.source_device_id()
            .verify(&signing_bytes(&self.content)?, &self.signature)
            .context("verify history recovery link source signature")
    }

    pub(crate) fn verify_at(&self, now_unix_seconds: u64) -> Result<()> {
        self.verify()?;
        ensure!(
            self.content.issued_at_unix_seconds
                <= now_unix_seconds.saturating_add(HISTORY_RECOVERY_LINK_CLOCK_SKEW_SECONDS),
            "history recovery link was issued too far in the future"
        );
        ensure!(
            now_unix_seconds
                <= self
                    .content
                    .expires_at_unix_seconds
                    .saturating_add(HISTORY_RECOVERY_LINK_CLOCK_SKEW_SECONDS),
            "history recovery link has expired"
        );
        Ok(())
    }

    pub(crate) fn verify_for_recipient(
        &self,
        now_unix_seconds: u64,
        recipient_certificate: &DeviceCertificate,
    ) -> Result<()> {
        self.verify_at(now_unix_seconds)?;
        ensure!(
            recipient_certificate.account_id() == self.account_id(),
            "history recovery link belongs to a different account"
        );
        ensure!(
            recipient_certificate.device_id() == self.recipient_device_id(),
            "history recovery link addresses a different recipient device"
        );
        ensure!(
            self.account_device_list()
                .certificate_for(recipient_certificate.device_id())
                == Some(recipient_certificate),
            "recipient certificate is not present exactly in the link device list"
        );
        Ok(())
    }

    pub(crate) fn link_id(&self) -> Result<[u8; 32]> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self).context("encode history recovery link ID")?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(HISTORY_RECOVERY_LINK_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }

    pub(crate) fn endpoint(&self) -> &EndpointAddr {
        &self.content.endpoint
    }

    pub(crate) fn account_device_list(&self) -> &AccountDeviceListSnapshot {
        &self.content.account_device_list
    }

    pub(crate) fn account_id(&self) -> AccountId {
        self.content.source_certificate.account_id()
    }

    pub(crate) fn source_device_id(&self) -> DeviceId {
        self.content.source_certificate.device_id()
    }

    pub(crate) fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub(crate) fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
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

    pub(crate) fn issued_at_unix_seconds(&self) -> u64 {
        self.content.issued_at_unix_seconds
    }

    pub(crate) fn expires_at_unix_seconds(&self) -> u64 {
        self.content.expires_at_unix_seconds
    }

    pub(crate) fn sas(&self) -> Result<HistoryRewrapSas> {
        HistoryRewrapSas::derive(
            self.account_device_list(),
            self.source_device_id(),
            self.recipient_device_id(),
        )
        .context("derive history recovery link SAS")
    }
}

fn validate_content(content: &HistoryRecoveryLinkContent) -> Result<()> {
    ensure!(
        content.version == HISTORY_RECOVERY_LINK_VERSION,
        "unsupported history recovery link version: {}",
        content.version
    );
    content
        .account_device_list
        .verify_for_account(content.source_certificate.account_id())
        .context("verify history recovery link account device list")?;
    verify_device_authorization_with_snapshot(
        content.source_certificate.account_id(),
        &content.source_certificate,
        content.account_device_list.authority_snapshot(),
        &DeviceCapability::MESSAGING,
    )
    .context("verify history recovery link source authorization")?;
    ensure!(
        content
            .account_device_list
            .certificate_for(content.source_certificate.device_id())
            == Some(&content.source_certificate),
        "history recovery link source certificate is absent from its exact device list"
    );
    ensure!(
        content
            .account_device_list
            .certificate_for(content.recipient_device_id)
            .is_some(),
        "history recovery link recipient is absent from the signed device list"
    );
    ensure!(
        content.source_certificate.device_id() != content.recipient_device_id,
        "history recovery link source and recipient are the same device"
    );
    ensure!(
        content.approved_range_start < content.approved_range_end,
        "history recovery link approved range is empty"
    );
    ensure!(
        content.approved_range_end <= MAX_INVENTORY_EVENT_IDS as u64,
        "history recovery link range exceeds the bounded inventory limit"
    );
    ensure!(
        (1..=MAX_HISTORY_REWRAP_ENTRIES as u64).contains(&content.page_size),
        "history recovery link page size is outside the supported range"
    );
    ensure!(
        content.issued_at_unix_seconds < content.expires_at_unix_seconds,
        "history recovery link validity window is empty"
    );
    ensure!(
        content
            .expires_at_unix_seconds
            .saturating_sub(content.issued_at_unix_seconds)
            <= MAX_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS,
        "history recovery link validity exceeds {MAX_HISTORY_RECOVERY_LINK_VALIDITY_SECONDS} seconds"
    );
    Ok(())
}

fn signing_bytes(content: &HistoryRecoveryLinkContent) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode history recovery link content")?;
    let mut bytes =
        Vec::with_capacity(HISTORY_RECOVERY_LINK_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(HISTORY_RECOVERY_LINK_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;
    use kilogram_identity::{AccountRootState, DeviceEncryptionIdentity};

    #[test]
    fn signed_recovery_link_round_trips_and_binds_time_roles_and_payload() -> Result<()> {
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
                conversation_id: ConversationId::from_label("link-test"),
                approved_range_start: 0,
                approved_event_count: 4_096,
                page_size: 64,
                route_policy: RoutePolicy::Auto,
                issued_at_unix_seconds: 1_000,
                valid_for_seconds: 600,
            },
        )?;
        let encoded = link.encode_text()?;
        assert!(encoded.len() <= MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES);
        let decoded = SignedHistoryRecoveryLink::decode_text(&encoded)?;
        assert_eq!(decoded, link);
        decoded.verify_for_recipient(1_500, &recipient_certificate)?;
        assert_eq!(decoded.sas()?, link.sas()?);
        assert!(decoded.verify_at(2_000).is_err());

        let outsider = DeviceIdentity::generate()?;
        let outsider_certificate = root.issue_device_certificate(
            outsider.device_id(),
            DeviceEncryptionIdentity::generate()?.public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        assert!(
            decoded
                .verify_for_recipient(1_500, &outsider_certificate)
                .is_err()
        );

        let mut tampered = encoded.into_bytes();
        let last = tampered
            .last_mut()
            .context("encoded recovery link is unexpectedly empty")?;
        *last = if *last == b'A' { b'B' } else { b'A' };
        assert!(SignedHistoryRecoveryLink::decode_text(std::str::from_utf8(&tampered)?).is_err());
        Ok(())
    }
}
