use std::fmt;

use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};

use crate::{
    ConversationId, HistoryRewrapManifest, HistoryRewrapSas, MAX_HISTORY_REWRAP_ENTRIES,
    ProtocolError,
};

const HISTORY_RECOVERY_CHECKPOINT_VERSION: u8 = 1;
const HISTORY_RECOVERY_CHECKPOINT_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-checkpoint-signature:v1\0";
const HISTORY_RECOVERY_CHECKPOINT_ID_DOMAIN: &[u8] =
    b"kilogram:history-recovery-checkpoint-id:v1\0";
const HISTORY_RECOVERY_ID_DOMAIN: &[u8] = b"kilogram:history-recovery-id:v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HistoryRecoveryId([u8; 32]);

impl fmt::Display for HistoryRecoveryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct HistoryRecoveryCheckpointContent {
    version: u8,
    account_id: AccountId,
    conversation_id: ConversationId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    sas: HistoryRewrapSas,
    approved_range_start: u64,
    approved_range_end: u64,
    page_size: u64,
    inventory_event_count: Option<u64>,
    inventory_digest: Option<[u8; 32]>,
    next_range_start: u64,
    previous_checkpoint_id: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedHistoryRecoveryCheckpoint {
    content: HistoryRecoveryCheckpointContent,
    signature: Vec<u8>,
}

impl SignedHistoryRecoveryCheckpoint {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        recipient_identity: &DeviceIdentity,
        account_id: AccountId,
        conversation_id: ConversationId,
        source_device_id: DeviceId,
        sas: HistoryRewrapSas,
        approved_range_start: usize,
        approved_event_count: usize,
        page_size: usize,
    ) -> Result<Self, ProtocolError> {
        let approved_range_start = u64::try_from(approved_range_start)
            .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(approved_range_start))?;
        let approved_event_count = u64::try_from(approved_event_count)
            .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(approved_event_count))?;
        let approved_range_end = approved_range_start
            .checked_add(approved_event_count)
            .ok_or(ProtocolError::HistoryRecoveryRangeOverflow)?;
        let content = HistoryRecoveryCheckpointContent {
            version: HISTORY_RECOVERY_CHECKPOINT_VERSION,
            account_id,
            conversation_id,
            source_device_id,
            recipient_device_id: recipient_identity.device_id(),
            sas,
            approved_range_start,
            approved_range_end,
            page_size: u64::try_from(page_size)
                .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(page_size))?,
            inventory_event_count: None,
            inventory_digest: None,
            next_range_start: approved_range_start,
            previous_checkpoint_id: None,
        };
        Self::sign_content(recipient_identity, content)
    }

    pub fn advance(
        &self,
        recipient_identity: &DeviceIdentity,
        manifest: &HistoryRewrapManifest,
    ) -> Result<Self, ProtocolError> {
        self.verify_signature()?;
        manifest.verify()?;
        if recipient_identity.device_id() != self.content.recipient_device_id {
            return Err(ProtocolError::HistoryRecoveryCheckpointSignerMismatch {
                expected: self.content.recipient_device_id,
                actual: recipient_identity.device_id(),
            });
        }
        if manifest.account_id() != self.content.account_id
            || manifest.conversation_id() != self.content.conversation_id
            || manifest.source_device_id() != self.content.source_device_id
            || manifest.recipient_device_id() != self.content.recipient_device_id
        {
            return Err(ProtocolError::HistoryRecoveryCheckpointPageMismatch);
        }
        let derived_sas = HistoryRewrapSas::derive(
            manifest.account_device_list(),
            manifest.source_device_id(),
            manifest.recipient_device_id(),
        )?;
        if derived_sas != self.content.sas {
            return Err(ProtocolError::HistoryRewrapSasMismatch);
        }
        if manifest.range_start() != self.content.next_range_start
            || manifest.range_end() > self.content.approved_range_end
            || manifest.range_end().saturating_sub(manifest.range_start()) > self.content.page_size
        {
            return Err(ProtocolError::HistoryRecoveryCheckpointPageMismatch);
        }
        match (
            self.content.inventory_event_count,
            self.content.inventory_digest,
        ) {
            (None, None) => {}
            (Some(count), Some(digest))
                if count == manifest.inventory_event_count()
                    && digest == *manifest.inventory_digest() => {}
            _ => return Err(ProtocolError::HistoryRecoveryCheckpointClaimMismatch),
        }
        let content = HistoryRecoveryCheckpointContent {
            inventory_event_count: Some(manifest.inventory_event_count()),
            inventory_digest: Some(*manifest.inventory_digest()),
            next_range_start: manifest.range_end(),
            previous_checkpoint_id: Some(self.checkpoint_id()?),
            ..self.content.clone()
        };
        Self::sign_content(recipient_identity, content)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.verify_signature()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let checkpoint: Self = postcard::from_bytes(bytes)?;
        checkpoint.verify_signature()?;
        Ok(checkpoint)
    }

    pub fn verify_signature(&self) -> Result<(), ProtocolError> {
        validate_checkpoint_content(&self.content)?;
        self.content
            .recipient_device_id
            .verify(&checkpoint_signing_bytes(&self.content)?, &self.signature)?;
        Ok(())
    }

    pub fn recovery_id(&self) -> Result<HistoryRecoveryId, ProtocolError> {
        self.verify_signature()?;
        #[derive(Serialize)]
        struct RecoveryIdentity {
            account_id: AccountId,
            conversation_id: ConversationId,
            source_device_id: DeviceId,
            recipient_device_id: DeviceId,
            sas: HistoryRewrapSas,
            approved_range_start: u64,
            approved_range_end: u64,
            page_size: u64,
        }
        let encoded = postcard::to_allocvec(&RecoveryIdentity {
            account_id: self.content.account_id,
            conversation_id: self.content.conversation_id,
            source_device_id: self.content.source_device_id,
            recipient_device_id: self.content.recipient_device_id,
            sas: self.content.sas,
            approved_range_start: self.content.approved_range_start,
            approved_range_end: self.content.approved_range_end,
            page_size: self.content.page_size,
        })?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(HISTORY_RECOVERY_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(HistoryRecoveryId(*hasher.finalize().as_bytes()))
    }

    pub fn checkpoint_id(&self) -> Result<[u8; 32], ProtocolError> {
        self.verify_signature()?;
        let encoded = postcard::to_allocvec(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(HISTORY_RECOVERY_CHECKPOINT_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.content.conversation_id
    }

    pub fn source_device_id(&self) -> DeviceId {
        self.content.source_device_id
    }

    pub fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub fn sas(&self) -> HistoryRewrapSas {
        self.content.sas
    }

    pub fn approved_range_start(&self) -> u64 {
        self.content.approved_range_start
    }

    pub fn approved_range_end(&self) -> u64 {
        self.content.approved_range_end
    }

    pub fn page_size(&self) -> u64 {
        self.content.page_size
    }

    pub fn inventory_event_count(&self) -> Option<u64> {
        self.content.inventory_event_count
    }

    pub fn inventory_digest(&self) -> Option<&[u8; 32]> {
        self.content.inventory_digest.as_ref()
    }

    pub fn next_range_start(&self) -> u64 {
        self.content.next_range_start
    }

    pub fn previous_checkpoint_id(&self) -> Option<&[u8; 32]> {
        self.content.previous_checkpoint_id.as_ref()
    }

    pub fn is_complete(&self) -> bool {
        self.content.inventory_event_count.is_some_and(|count| {
            self.content.next_range_start >= self.content.approved_range_end.min(count)
        })
    }

    fn sign_content(
        recipient_identity: &DeviceIdentity,
        content: HistoryRecoveryCheckpointContent,
    ) -> Result<Self, ProtocolError> {
        validate_checkpoint_content(&content)?;
        if recipient_identity.device_id() != content.recipient_device_id {
            return Err(ProtocolError::HistoryRecoveryCheckpointSignerMismatch {
                expected: content.recipient_device_id,
                actual: recipient_identity.device_id(),
            });
        }
        let signature = recipient_identity
            .sign(&checkpoint_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }
}

fn validate_checkpoint_content(
    content: &HistoryRecoveryCheckpointContent,
) -> Result<(), ProtocolError> {
    if content.version != HISTORY_RECOVERY_CHECKPOINT_VERSION {
        return Err(ProtocolError::UnsupportedHistoryRecoveryCheckpointVersion(
            content.version,
        ));
    }
    if content.source_device_id == content.recipient_device_id {
        return Err(ProtocolError::HistoryRewrapSameDevice(
            content.source_device_id,
        ));
    }
    if content.approved_range_start >= content.approved_range_end
        || content.next_range_start < content.approved_range_start
        || content.next_range_start > content.approved_range_end
    {
        return Err(ProtocolError::InvalidHistoryRecoveryCheckpointRange {
            start: content.approved_range_start,
            end: content.approved_range_end,
            next: content.next_range_start,
        });
    }
    let page_size = usize::try_from(content.page_size)
        .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(usize::MAX))?;
    if page_size == 0 || page_size > MAX_HISTORY_REWRAP_ENTRIES {
        return Err(ProtocolError::TooManyHistoryRewrapEntries(page_size));
    }
    match (content.inventory_event_count, content.inventory_digest) {
        (None, None) if content.next_range_start == content.approved_range_start => {}
        (Some(count), Some(_)) if content.next_range_start <= count => {}
        _ => return Err(ProtocolError::HistoryRecoveryCheckpointClaimMismatch),
    }
    Ok(())
}

fn checkpoint_signing_bytes(
    content: &HistoryRecoveryCheckpointContent,
) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes =
        Vec::with_capacity(HISTORY_RECOVERY_CHECKPOINT_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(HISTORY_RECOVERY_CHECKPOINT_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}
