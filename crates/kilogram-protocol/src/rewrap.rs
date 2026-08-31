use std::fmt;

use kilogram_crypto::{DeviceEncryptionIdentity, ENCRYPTION_KEY_BYTES, SealedMessage};
use kilogram_identity::{AccountDeviceListSnapshot, AccountId, DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};

use crate::{
    AuthorizedEvent, ConversationId, EventId, EventPayload, MAX_CIPHERTEXT_BYTES, MAX_TEXT_BYTES,
    ProtocolError, SyncSessionBinding,
};

const HISTORY_REWRAP_VERSION: u8 = 1;
const HISTORY_REWRAP_SIGNATURE_DOMAIN: &[u8] = b"kilogram:history-rewrap-signature:v1\0";
const HISTORY_REWRAP_HPKE_INFO: &[u8] = b"kilogram:history-rewrap-hpke:v1\0";
const HISTORY_REWRAP_AAD_DOMAIN: &[u8] = b"kilogram:history-rewrap-aad:v1\0";
const HISTORY_REWRAP_INVENTORY_DOMAIN: &[u8] = b"kilogram:history-rewrap-inventory:v1\0";
const HISTORY_REWRAP_ID_DOMAIN: &[u8] = b"kilogram:history-rewrap-id:v1\0";
const HISTORY_REWRAP_SAS_DOMAIN: &[u8] = b"kilogram:history-rewrap-sas:v1\0";
const HISTORY_REWRAP_REQUEST_VERSION: u8 = 1;
const HISTORY_REWRAP_REQUEST_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-request-signature:v1\0";
const HISTORY_REWRAP_TRANSFER_VERSION: u8 = 1;
const HISTORY_REWRAP_TRANSFER_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-rewrap-transfer-signature:v1\0";

pub const MAX_HISTORY_REWRAP_ENTRIES: usize = 256;

/// A short authentication string derived from one root-signed account device list.
///
/// The displayed decimal code is only for an out-of-band human comparison. The
/// full digest is carried in and signed by the network request.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct HistoryRewrapSas([u8; 32]);

impl fmt::Display for HistoryRewrapSas {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut prefix = [0_u8; 8];
        prefix.copy_from_slice(&self.0[..8]);
        let digits = u64::from_be_bytes(prefix) % 1_000_000_000_000;
        let code = format!("{digits:012}");
        write!(
            formatter,
            "{}-{}-{}-{}",
            &code[0..3],
            &code[3..6],
            &code[6..9],
            &code[9..12]
        )
    }
}

impl HistoryRewrapSas {
    pub fn derive(
        account_device_list: &AccountDeviceListSnapshot,
        source_device_id: DeviceId,
        recipient_device_id: DeviceId,
    ) -> Result<Self, ProtocolError> {
        account_device_list.verify()?;
        validate_rewrap_devices(account_device_list, source_device_id, recipient_device_id)?;
        let encoded = account_device_list.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(HISTORY_REWRAP_SAS_DOMAIN);
        hasher.update(&encoded);
        hasher.update(source_device_id.as_bytes());
        hasher.update(recipient_device_id.as_bytes());
        Ok(Self(*hasher.finalize().as_bytes()))
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct HistoryRewrapRequestContent {
    version: u8,
    conversation_id: ConversationId,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    session_binding: SyncSessionBinding,
    range_start: u64,
    max_event_count: u64,
    sas: HistoryRewrapSas,
}

/// A recipient-signed, session-bound request for one bounded history range.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedHistoryRewrapRequest {
    content: HistoryRewrapRequestContent,
    signature: Vec<u8>,
}

impl SignedHistoryRewrapRequest {
    pub fn sign(
        recipient_identity: &DeviceIdentity,
        conversation_id: ConversationId,
        source_device_id: DeviceId,
        session_binding: SyncSessionBinding,
        range_start: usize,
        max_event_count: usize,
        sas: HistoryRewrapSas,
    ) -> Result<Self, ProtocolError> {
        let content = HistoryRewrapRequestContent {
            version: HISTORY_REWRAP_REQUEST_VERSION,
            conversation_id,
            source_device_id,
            recipient_device_id: recipient_identity.device_id(),
            session_binding,
            range_start: u64::try_from(range_start)
                .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(range_start))?,
            max_event_count: u64::try_from(max_event_count)
                .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(max_event_count))?,
            sas,
        };
        validate_request_content(&content)?;
        let signature = recipient_identity
            .sign(&history_rewrap_request_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn verify_signature(&self) -> Result<(), ProtocolError> {
        validate_request_content(&self.content)?;
        self.content.recipient_device_id.verify(
            &history_rewrap_request_signing_bytes(&self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn verify_for_session(
        &self,
        expected_session: SyncSessionBinding,
        expected_source: DeviceId,
        expected_recipient: DeviceId,
        expected_sas: HistoryRewrapSas,
    ) -> Result<(), ProtocolError> {
        self.verify_signature()?;
        if self.content.session_binding != expected_session {
            return Err(ProtocolError::HistoryRewrapRequestSessionMismatch);
        }
        if self.content.source_device_id != expected_source {
            return Err(ProtocolError::HistoryRewrapRequestSourceMismatch {
                expected: expected_source,
                actual: self.content.source_device_id,
            });
        }
        if self.content.recipient_device_id != expected_recipient {
            return Err(ProtocolError::HistoryRewrapRecipientMismatch {
                expected: expected_recipient,
                actual: self.content.recipient_device_id,
            });
        }
        if self.content.sas != expected_sas {
            return Err(ProtocolError::HistoryRewrapSasMismatch);
        }
        Ok(())
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

    pub fn session_binding(&self) -> SyncSessionBinding {
        self.content.session_binding
    }

    pub fn range_start(&self) -> u64 {
        self.content.range_start
    }

    pub fn max_event_count(&self) -> u64 {
        self.content.max_event_count
    }

    pub fn sas(&self) -> HistoryRewrapSas {
        self.content.sas
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct HistoryRewrapTransferContent {
    version: u8,
    request: SignedHistoryRewrapRequest,
    bundle: HistoryRewrapBundle,
}

/// A source-signed response that binds the encrypted bundle to the exact request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedHistoryRewrapTransfer {
    content: HistoryRewrapTransferContent,
    signature: Vec<u8>,
}

impl SignedHistoryRewrapTransfer {
    pub fn sign(
        source_identity: &DeviceIdentity,
        request: SignedHistoryRewrapRequest,
        bundle: HistoryRewrapBundle,
    ) -> Result<Self, ProtocolError> {
        let content = HistoryRewrapTransferContent {
            version: HISTORY_REWRAP_TRANSFER_VERSION,
            request,
            bundle,
        };
        validate_transfer_content(&content)?;
        if content.request.source_device_id() != source_identity.device_id() {
            return Err(ProtocolError::HistoryRewrapRequestSourceMismatch {
                expected: source_identity.device_id(),
                actual: content.request.source_device_id(),
            });
        }
        let signature = source_identity
            .sign(&history_rewrap_transfer_signing_bytes(&content)?)
            .to_vec();
        Ok(Self { content, signature })
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.verify_signature()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let transfer: Self = postcard::from_bytes(bytes)?;
        transfer.verify_signature()?;
        Ok(transfer)
    }

    pub fn verify_signature(&self) -> Result<(), ProtocolError> {
        validate_transfer_content(&self.content)?;
        self.content.request.source_device_id().verify(
            &history_rewrap_transfer_signing_bytes(&self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub fn verify_for_request(
        &self,
        expected_request: &SignedHistoryRewrapRequest,
    ) -> Result<(), ProtocolError> {
        self.verify_signature()?;
        if &self.content.request != expected_request {
            return Err(ProtocolError::HistoryRewrapTransferRequestMismatch);
        }
        Ok(())
    }

    pub fn request(&self) -> &SignedHistoryRewrapRequest {
        &self.content.request
    }

    pub fn bundle(&self) -> &HistoryRewrapBundle {
        &self.content.bundle
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HistoryRewrapId([u8; 32]);

impl fmt::Display for HistoryRewrapId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryRewrapManifest {
    version: u8,
    conversation_id: ConversationId,
    account_device_list: Box<AccountDeviceListSnapshot>,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
    inventory_event_count: u64,
    inventory_digest: [u8; 32],
    range_start: u64,
    range_end: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct HistoryRewrapEntryContent {
    inventory_index: u64,
    event: AuthorizedEvent,
    sealed: SealedMessage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryRewrapEntry {
    content: HistoryRewrapEntryContent,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryRewrapBundle {
    manifest: HistoryRewrapManifest,
    entries: Vec<HistoryRewrapEntry>,
}

#[derive(Serialize)]
struct HistoryRewrapSignatureContext<'a> {
    manifest: &'a HistoryRewrapManifest,
    entry: &'a HistoryRewrapEntryContent,
}

#[derive(Serialize)]
struct HistoryRewrapAad {
    manifest: HistoryRewrapManifest,
    inventory_index: u64,
    event_id: EventId,
}

impl HistoryRewrapBundle {
    pub fn seal(
        source_identity: &DeviceIdentity,
        account_device_list: AccountDeviceListSnapshot,
        recipient_device_id: DeviceId,
        conversation_id: ConversationId,
        inventory: &[(AuthorizedEvent, String)],
        range_start: usize,
        range_end: usize,
    ) -> Result<Self, ProtocolError> {
        account_device_list.verify()?;
        let source_device_id = source_identity.device_id();
        validate_rewrap_devices(&account_device_list, source_device_id, recipient_device_id)?;
        let inventory_event_ids = validate_source_inventory(inventory, conversation_id)?;
        validate_range(range_start, range_end, inventory.len())?;
        let inventory_event_count = u64::try_from(inventory.len())
            .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(inventory.len()))?;
        let range_start = u64::try_from(range_start)
            .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(inventory.len()))?;
        let range_end = u64::try_from(range_end)
            .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(inventory.len()))?;
        let manifest = HistoryRewrapManifest {
            version: HISTORY_REWRAP_VERSION,
            conversation_id,
            account_device_list: Box::new(account_device_list),
            source_device_id,
            recipient_device_id,
            inventory_event_count,
            inventory_digest: inventory_digest(conversation_id, &inventory_event_ids),
            range_start,
            range_end,
        };
        validate_manifest(&manifest)?;
        let recipient_public_key = manifest
            .account_device_list
            .certificate_for(recipient_device_id)
            .ok_or(ProtocolError::HistoryRewrapDeviceMissing(
                recipient_device_id,
            ))?
            .encryption_public_key();
        let mut entries = Vec::with_capacity((range_end - range_start) as usize);
        for inventory_index in range_start..range_end {
            let source = &inventory[inventory_index as usize];
            let event_id = source.0.event().event_id()?;
            let aad = history_rewrap_aad(&manifest, inventory_index, event_id)?;
            let content = HistoryRewrapEntryContent {
                inventory_index,
                event: source.0.clone(),
                sealed: recipient_public_key.seal(
                    source.1.as_bytes(),
                    HISTORY_REWRAP_HPKE_INFO,
                    &aad,
                )?,
            };
            let signature = source_identity
                .sign(&history_rewrap_signing_bytes(&manifest, &content)?)
                .to_vec();
            entries.push(HistoryRewrapEntry { content, signature });
        }
        let bundle = Self { manifest, entries };
        bundle.verify()?;
        Ok(bundle)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let bundle: Self = postcard::from_bytes(bytes)?;
        bundle.verify()?;
        Ok(bundle)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.verify()?;
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn verify(&self) -> Result<(), ProtocolError> {
        validate_manifest(&self.manifest)?;
        let expected_entries = usize::try_from(self.manifest.range_end - self.manifest.range_start)
            .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(self.entries.len()))?;
        if self.entries.len() != expected_entries {
            return Err(ProtocolError::HistoryRewrapEntryCountMismatch {
                expected: expected_entries,
                actual: self.entries.len(),
            });
        }
        let mut previous_event_id = None;
        for (offset, entry) in self.entries.iter().enumerate() {
            let expected_index = self.manifest.range_start
                + u64::try_from(offset).map_err(|_| {
                    ProtocolError::HistoryRewrapInventoryTooLarge(self.entries.len())
                })?;
            if entry.content.inventory_index != expected_index {
                return Err(ProtocolError::HistoryRewrapEntryIndexMismatch {
                    expected: expected_index,
                    actual: entry.content.inventory_index,
                });
            }
            entry.verify_for_manifest(&self.manifest)?;
            let event_id = entry.content.event.event().event_id()?;
            if previous_event_id.is_some_and(|previous| previous >= event_id) {
                return Err(ProtocolError::NonCanonicalHistoryRewrapEntries);
            }
            previous_event_id = Some(event_id);
        }
        Ok(())
    }

    pub fn bundle_id(&self) -> Result<HistoryRewrapId, ProtocolError> {
        let encoded = self.encode()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(HISTORY_REWRAP_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(HistoryRewrapId(*hasher.finalize().as_bytes()))
    }

    pub fn open_entry(
        &self,
        entry_offset: usize,
        recipient_device_id: DeviceId,
        recipient_encryption: &DeviceEncryptionIdentity,
    ) -> Result<(AuthorizedEvent, String), ProtocolError> {
        self.verify()?;
        if recipient_device_id != self.manifest.recipient_device_id {
            return Err(ProtocolError::HistoryRewrapRecipientMismatch {
                expected: self.manifest.recipient_device_id,
                actual: recipient_device_id,
            });
        }
        let entry = self.entries.get(entry_offset).ok_or(
            ProtocolError::HistoryRewrapEntryOffsetOutOfRange(entry_offset),
        )?;
        let body =
            entry.open_for_manifest(&self.manifest, recipient_device_id, recipient_encryption)?;
        Ok((entry.content.event.clone(), body))
    }

    pub fn manifest(&self) -> &HistoryRewrapManifest {
        &self.manifest
    }

    pub fn entries(&self) -> &[HistoryRewrapEntry] {
        &self.entries
    }

    pub fn is_complete_source_inventory(&self) -> bool {
        self.manifest.range_start == 0
            && self.manifest.range_end == self.manifest.inventory_event_count
    }
}

impl HistoryRewrapManifest {
    pub fn verify(&self) -> Result<(), ProtocolError> {
        validate_manifest(self)
    }

    pub fn conversation_id(&self) -> ConversationId {
        self.conversation_id
    }

    pub fn account_id(&self) -> AccountId {
        self.account_device_list.account_id()
    }

    pub fn account_device_list(&self) -> &AccountDeviceListSnapshot {
        &self.account_device_list
    }

    pub fn source_device_id(&self) -> DeviceId {
        self.source_device_id
    }

    pub fn recipient_device_id(&self) -> DeviceId {
        self.recipient_device_id
    }

    pub fn inventory_event_count(&self) -> u64 {
        self.inventory_event_count
    }

    pub fn inventory_digest(&self) -> &[u8; 32] {
        &self.inventory_digest
    }

    pub fn range_start(&self) -> u64 {
        self.range_start
    }

    pub fn range_end(&self) -> u64 {
        self.range_end
    }
}

impl HistoryRewrapEntry {
    pub fn inventory_index(&self) -> u64 {
        self.content.inventory_index
    }

    pub fn event(&self) -> &AuthorizedEvent {
        &self.content.event
    }

    pub(crate) fn verify_for_manifest(
        &self,
        manifest: &HistoryRewrapManifest,
    ) -> Result<(), ProtocolError> {
        if self.content.inventory_index < manifest.range_start
            || self.content.inventory_index >= manifest.range_end
        {
            return Err(ProtocolError::HistoryRewrapEntryIndexOutsideRange {
                index: self.content.inventory_index,
                start: manifest.range_start,
                end: manifest.range_end,
            });
        }
        self.content.event.verify_author()?;
        let event = self.content.event.event();
        if event.conversation_id() != manifest.conversation_id {
            return Err(ProtocolError::HistoryRewrapConversationMismatch);
        }
        if !matches!(event.payload(), EventPayload::RatchetText { .. }) {
            return Err(ProtocolError::HistoryRewrapEventIsNotText);
        }
        validate_sealed_message(&self.content.sealed)?;
        manifest.source_device_id.verify(
            &history_rewrap_signing_bytes(manifest, &self.content)?,
            &self.signature,
        )?;
        Ok(())
    }

    pub(crate) fn open_for_manifest(
        &self,
        manifest: &HistoryRewrapManifest,
        recipient_device_id: DeviceId,
        recipient_encryption: &DeviceEncryptionIdentity,
    ) -> Result<String, ProtocolError> {
        manifest.verify()?;
        self.verify_for_manifest(manifest)?;
        if recipient_device_id != manifest.recipient_device_id {
            return Err(ProtocolError::HistoryRewrapRecipientMismatch {
                expected: manifest.recipient_device_id,
                actual: recipient_device_id,
            });
        }
        let event_id = self.content.event.event().event_id()?;
        let aad = history_rewrap_aad(manifest, self.content.inventory_index, event_id)?;
        let plaintext =
            recipient_encryption.open(&self.content.sealed, HISTORY_REWRAP_HPKE_INFO, &aad)?;
        if plaintext.len() > MAX_TEXT_BYTES {
            return Err(ProtocolError::TextTooLarge(plaintext.len()));
        }
        String::from_utf8(plaintext).map_err(ProtocolError::InvalidTextEncoding)
    }
}

fn validate_rewrap_devices(
    account_device_list: &AccountDeviceListSnapshot,
    source_device_id: DeviceId,
    recipient_device_id: DeviceId,
) -> Result<(), ProtocolError> {
    if source_device_id == recipient_device_id {
        return Err(ProtocolError::HistoryRewrapSameDevice(source_device_id));
    }
    for device_id in [source_device_id, recipient_device_id] {
        if account_device_list.certificate_for(device_id).is_none() {
            return Err(ProtocolError::HistoryRewrapDeviceMissing(device_id));
        }
    }
    Ok(())
}

fn validate_source_inventory(
    inventory: &[(AuthorizedEvent, String)],
    conversation_id: ConversationId,
) -> Result<Vec<EventId>, ProtocolError> {
    let mut event_ids = Vec::with_capacity(inventory.len());
    for (event, body) in inventory {
        event.verify_author()?;
        if event.event().conversation_id() != conversation_id {
            return Err(ProtocolError::HistoryRewrapConversationMismatch);
        }
        if !matches!(event.event().payload(), EventPayload::RatchetText { .. }) {
            return Err(ProtocolError::HistoryRewrapEventIsNotText);
        }
        if body.len() > MAX_TEXT_BYTES {
            return Err(ProtocolError::TextTooLarge(body.len()));
        }
        event_ids.push(event.event().event_id()?);
    }
    if event_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ProtocolError::NonCanonicalHistoryRewrapEntries);
    }
    Ok(event_ids)
}

fn validate_range(
    range_start: usize,
    range_end: usize,
    inventory_len: usize,
) -> Result<(), ProtocolError> {
    if range_start >= range_end || range_end > inventory_len {
        return Err(ProtocolError::InvalidHistoryRewrapRange {
            start: range_start,
            end: range_end,
            inventory_len,
        });
    }
    let entry_count = range_end - range_start;
    if entry_count > MAX_HISTORY_REWRAP_ENTRIES {
        return Err(ProtocolError::TooManyHistoryRewrapEntries(entry_count));
    }
    Ok(())
}

fn validate_manifest(manifest: &HistoryRewrapManifest) -> Result<(), ProtocolError> {
    if manifest.version != HISTORY_REWRAP_VERSION {
        return Err(ProtocolError::UnsupportedHistoryRewrapVersion(
            manifest.version,
        ));
    }
    manifest.account_device_list.verify()?;
    validate_rewrap_devices(
        &manifest.account_device_list,
        manifest.source_device_id,
        manifest.recipient_device_id,
    )?;
    let inventory_len = usize::try_from(manifest.inventory_event_count)
        .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(usize::MAX))?;
    let range_start = usize::try_from(manifest.range_start)
        .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(inventory_len))?;
    let range_end = usize::try_from(manifest.range_end)
        .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(inventory_len))?;
    validate_range(range_start, range_end, inventory_len)
}

fn validate_sealed_message(sealed: &SealedMessage) -> Result<(), ProtocolError> {
    if sealed.encapsulated_key.len() != ENCRYPTION_KEY_BYTES {
        return Err(ProtocolError::InvalidEncapsulatedKeyLength(
            sealed.encapsulated_key.len(),
        ));
    }
    if sealed.ciphertext.len() > MAX_CIPHERTEXT_BYTES {
        return Err(ProtocolError::CiphertextTooLarge(sealed.ciphertext.len()));
    }
    Ok(())
}

fn inventory_digest(conversation_id: ConversationId, event_ids: &[EventId]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(HISTORY_REWRAP_INVENTORY_DOMAIN);
    hasher.update(conversation_id.as_bytes());
    for event_id in event_ids {
        hasher.update(event_id.as_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn history_rewrap_signing_bytes(
    manifest: &HistoryRewrapManifest,
    entry: &HistoryRewrapEntryContent,
) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(&HistoryRewrapSignatureContext { manifest, entry })?;
    let mut bytes = Vec::with_capacity(HISTORY_REWRAP_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(HISTORY_REWRAP_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn history_rewrap_aad(
    manifest: &HistoryRewrapManifest,
    inventory_index: u64,
    event_id: EventId,
) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(&HistoryRewrapAad {
        manifest: manifest.clone(),
        inventory_index,
        event_id,
    })?;
    let mut aad = Vec::with_capacity(HISTORY_REWRAP_AAD_DOMAIN.len() + encoded.len());
    aad.extend_from_slice(HISTORY_REWRAP_AAD_DOMAIN);
    aad.extend_from_slice(&encoded);
    Ok(aad)
}

fn validate_request_content(content: &HistoryRewrapRequestContent) -> Result<(), ProtocolError> {
    if content.version != HISTORY_REWRAP_REQUEST_VERSION {
        return Err(ProtocolError::UnsupportedHistoryRewrapRequestVersion(
            content.version,
        ));
    }
    if content.source_device_id == content.recipient_device_id {
        return Err(ProtocolError::HistoryRewrapSameDevice(
            content.source_device_id,
        ));
    }
    let max_event_count = usize::try_from(content.max_event_count)
        .map_err(|_| ProtocolError::HistoryRewrapInventoryTooLarge(usize::MAX))?;
    if max_event_count == 0 || max_event_count > MAX_HISTORY_REWRAP_ENTRIES {
        return Err(ProtocolError::TooManyHistoryRewrapEntries(max_event_count));
    }
    content
        .range_start
        .checked_add(content.max_event_count)
        .ok_or(ProtocolError::HistoryRewrapRequestRangeOverflow)?;
    Ok(())
}

fn validate_transfer_content(content: &HistoryRewrapTransferContent) -> Result<(), ProtocolError> {
    if content.version != HISTORY_REWRAP_TRANSFER_VERSION {
        return Err(ProtocolError::UnsupportedHistoryRewrapTransferVersion(
            content.version,
        ));
    }
    content.request.verify_signature()?;
    content.bundle.verify()?;
    let manifest = content.bundle.manifest();
    if manifest.conversation_id() != content.request.conversation_id() {
        return Err(ProtocolError::HistoryRewrapConversationMismatch);
    }
    if manifest.source_device_id() != content.request.source_device_id() {
        return Err(ProtocolError::HistoryRewrapRequestSourceMismatch {
            expected: content.request.source_device_id(),
            actual: manifest.source_device_id(),
        });
    }
    if manifest.recipient_device_id() != content.request.recipient_device_id() {
        return Err(ProtocolError::HistoryRewrapRecipientMismatch {
            expected: content.request.recipient_device_id(),
            actual: manifest.recipient_device_id(),
        });
    }
    if manifest.range_start() != content.request.range_start() {
        return Err(ProtocolError::HistoryRewrapTransferRangeMismatch);
    }
    let maximum_end = content
        .request
        .range_start()
        .checked_add(content.request.max_event_count())
        .ok_or(ProtocolError::HistoryRewrapRequestRangeOverflow)?;
    if manifest.range_end() > maximum_end {
        return Err(ProtocolError::HistoryRewrapTransferRangeMismatch);
    }
    let expected_sas = HistoryRewrapSas::derive(
        manifest.account_device_list(),
        manifest.source_device_id(),
        manifest.recipient_device_id(),
    )?;
    if content.request.sas() != expected_sas {
        return Err(ProtocolError::HistoryRewrapSasMismatch);
    }
    Ok(())
}

fn history_rewrap_request_signing_bytes(
    content: &HistoryRewrapRequestContent,
) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes =
        Vec::with_capacity(HISTORY_REWRAP_REQUEST_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(HISTORY_REWRAP_REQUEST_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn history_rewrap_transfer_signing_bytes(
    content: &HistoryRewrapTransferContent,
) -> Result<Vec<u8>, ProtocolError> {
    let encoded = postcard::to_allocvec(content)?;
    let mut bytes =
        Vec::with_capacity(HISTORY_REWRAP_TRANSFER_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(HISTORY_REWRAP_TRANSFER_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}
