//! Inactive deterministic-CBOR bridge for active Postcard-authorized events.
//!
//! Existing event, certificate and authority signatures commit to their
//! original Postcard preimages. They cannot be translated to the native
//! canonical event model without the original authors' private keys. This
//! bridge therefore preserves the exact active bytes and verifies them with
//! the active decoder before exposing the event to a future canonical sync
//! session. It is deliberately not connected to an active ALPN.

use thiserror::Error;

use crate::{
    AuthorizedEvent, ProtocolError,
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_SYNC_LEGACY_EVENT_BRIDGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ACTIVE_AUTHORIZED_EVENT_POSTCARD_BYTES: usize =
    MAX_CANONICAL_SYNC_LEGACY_EVENT_BRIDGE_BYTES - 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncLegacyEventBridgeV1 {
    conversation_id: [u8; 32],
    event_id: [u8; 32],
    author_account_id: [u8; 32],
    author_device_id: [u8; 32],
    active_authorized_event_postcard: Vec<u8>,
    active_authorized_event: AuthorizedEvent,
}

#[derive(Debug, Error)]
pub enum CanonicalSyncLegacyEventBridgeError {
    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    ActiveEvent(#[from] ProtocolError),

    #[error(
        "canonical legacy-event bridge exceeds {MAX_CANONICAL_SYNC_LEGACY_EVENT_BRIDGE_BYTES} bytes"
    )]
    BridgeTooLarge,

    #[error("active AuthorizedEvent exceeds {MAX_ACTIVE_AUTHORIZED_EVENT_POSTCARD_BYTES} bytes")]
    ActiveEventTooLarge,

    #[error("active AuthorizedEvent Postcard bytes are not the exact verified encoding")]
    NonCanonicalActiveEncoding,

    #[error("legacy-event bridge conversation ID does not match the active event")]
    ConversationMismatch,

    #[error("legacy-event bridge Event ID does not match the active event")]
    EventIdMismatch,

    #[error("legacy-event bridge author Account ID does not match the active authorization")]
    AuthorAccountMismatch,

    #[error("legacy-event bridge author Device ID does not match the active event")]
    AuthorDeviceMismatch,
}

impl CanonicalSyncLegacyEventBridgeV1 {
    pub fn from_active_event(
        event: &AuthorizedEvent,
    ) -> Result<Self, CanonicalSyncLegacyEventBridgeError> {
        event.verify_author()?;
        let active_authorized_event_postcard = event.encode()?;
        ensure_active_event_bound(&active_authorized_event_postcard)?;
        let value = Self {
            conversation_id: *event.event().conversation_id().as_bytes(),
            event_id: *event.event().event_id()?.as_bytes(),
            author_account_id: *event.author_account_id().as_bytes(),
            author_device_id: *event.event().author_device_id().as_bytes(),
            active_authorized_event_postcard,
            active_authorized_event: event.clone(),
        };
        value.verify_bindings()?;
        Ok(value)
    }

    pub fn encode(&self) -> Result<Vec<u8>, CanonicalSyncLegacyEventBridgeError> {
        self.verify_bindings()?;
        let mut output = Vec::new();
        push_array_header(&mut output, 6);
        push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
        push_bytes(&mut output, &self.conversation_id);
        push_bytes(&mut output, &self.event_id);
        push_bytes(&mut output, &self.author_account_id);
        push_bytes(&mut output, &self.author_device_id);
        push_bytes(&mut output, &self.active_authorized_event_postcard);
        if output.len() > MAX_CANONICAL_SYNC_LEGACY_EVENT_BRIDGE_BYTES {
            return Err(CanonicalSyncLegacyEventBridgeError::BridgeTooLarge);
        }
        Ok(output)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, CanonicalSyncLegacyEventBridgeError> {
        if bytes.len() > MAX_CANONICAL_SYNC_LEGACY_EVENT_BRIDGE_BYTES {
            return Err(CanonicalSyncLegacyEventBridgeError::BridgeTooLarge);
        }
        validate_canonical_profile_v1(bytes).map_err(CanonicalAccountError::from)?;
        let mut decoder = AccountDecoder::new(bytes);
        decoder.expect_array("sync-legacy-event-bridge-v1", 6)?;
        decoder.expect_version("sync-legacy-event-bridge-v1.schema_version")?;
        let conversation_id = decoder.fixed_bytes("sync-legacy-event-bridge-v1.conversation_id")?;
        let event_id = decoder.fixed_bytes("sync-legacy-event-bridge-v1.event_id")?;
        let author_account_id =
            decoder.fixed_bytes("sync-legacy-event-bridge-v1.author_account_id")?;
        let author_device_id =
            decoder.fixed_bytes("sync-legacy-event-bridge-v1.author_device_id")?;
        let active_authorized_event_postcard = decoder
            .bytes("sync-legacy-event-bridge-v1.active_authorized_event_postcard")?
            .to_vec();
        decoder.finish()?;
        ensure_active_event_bound(&active_authorized_event_postcard)?;
        let active_authorized_event =
            AuthorizedEvent::decode_and_verify_author(&active_authorized_event_postcard)?;
        let value = Self {
            conversation_id,
            event_id,
            author_account_id,
            author_device_id,
            active_authorized_event_postcard,
            active_authorized_event,
        };
        value.verify_bindings()?;
        Ok(value)
    }

    pub fn conversation_id(&self) -> &[u8; 32] {
        &self.conversation_id
    }

    pub fn event_id(&self) -> &[u8; 32] {
        &self.event_id
    }

    pub fn author_account_id(&self) -> &[u8; 32] {
        &self.author_account_id
    }

    pub fn author_device_id(&self) -> &[u8; 32] {
        &self.author_device_id
    }

    pub fn active_authorized_event_postcard(&self) -> &[u8] {
        &self.active_authorized_event_postcard
    }

    pub fn active_authorized_event(&self) -> &AuthorizedEvent {
        &self.active_authorized_event
    }

    pub fn into_active_authorized_event(self) -> AuthorizedEvent {
        self.active_authorized_event
    }

    fn verify_bindings(&self) -> Result<(), CanonicalSyncLegacyEventBridgeError> {
        self.active_authorized_event.verify_author()?;
        ensure_active_event_bound(&self.active_authorized_event_postcard)?;
        if self.active_authorized_event.encode()? != self.active_authorized_event_postcard {
            return Err(CanonicalSyncLegacyEventBridgeError::NonCanonicalActiveEncoding);
        }
        if self.conversation_id
            != *self
                .active_authorized_event
                .event()
                .conversation_id()
                .as_bytes()
        {
            return Err(CanonicalSyncLegacyEventBridgeError::ConversationMismatch);
        }
        if self.event_id != *self.active_authorized_event.event().event_id()?.as_bytes() {
            return Err(CanonicalSyncLegacyEventBridgeError::EventIdMismatch);
        }
        if self.author_account_id != *self.active_authorized_event.author_account_id().as_bytes() {
            return Err(CanonicalSyncLegacyEventBridgeError::AuthorAccountMismatch);
        }
        if self.author_device_id
            != *self
                .active_authorized_event
                .event()
                .author_device_id()
                .as_bytes()
        {
            return Err(CanonicalSyncLegacyEventBridgeError::AuthorDeviceMismatch);
        }
        Ok(())
    }
}

fn ensure_active_event_bound(bytes: &[u8]) -> Result<(), CanonicalSyncLegacyEventBridgeError> {
    if bytes.len() > MAX_ACTIVE_AUTHORIZED_EVENT_POSTCARD_BYTES {
        return Err(CanonicalSyncLegacyEventBridgeError::ActiveEventTooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct BridgeVectors {
        schema_version: u64,
        milestone: String,
        profile: String,
        activation_status: String,
        input: BridgeInput,
        expected: BridgeExpected,
    }

    #[derive(Deserialize)]
    struct BridgeInput {
        active_authorized_event_postcard_hex: String,
    }

    #[derive(Deserialize)]
    struct BridgeExpected {
        conversation_id_hex: String,
        event_id_hex: String,
        author_account_id_hex: String,
        author_device_id_hex: String,
        bridge_cbor_hex: String,
    }

    fn vectors() -> Result<BridgeVectors, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-LEGACY-EVENT-VECTORS.json"
        )))?)
    }

    fn invalid_data(message: impl Into<String>) -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        ))
    }

    fn from_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2)
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(invalid_data("bridge vector contains invalid hexadecimal"));
        }
        (0..value.len())
            .step_by(2)
            .map(|offset| Ok(u8::from_str_radix(&value[offset..offset + 2], 16)?))
            .collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn encode_unchecked(value: &CanonicalSyncLegacyEventBridgeV1) -> Vec<u8> {
        let mut output = Vec::new();
        push_array_header(&mut output, 6);
        push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
        push_bytes(&mut output, &value.conversation_id);
        push_bytes(&mut output, &value.event_id);
        push_bytes(&mut output, &value.author_account_id);
        push_bytes(&mut output, &value.author_device_id);
        push_bytes(&mut output, &value.active_authorized_event_postcard);
        output
    }

    #[test]
    fn bridge_vector_preserves_and_verifies_exact_active_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        assert_eq!(vectors.schema_version, CANONICAL_PROFILE_VERSION);
        assert_eq!(vectors.milestone, "M1.1.52");
        assert_eq!(vectors.profile, "kilogram-deterministic-cbor-v1");
        assert_eq!(vectors.activation_status, "not-active");
        let active_bytes = from_hex(&vectors.input.active_authorized_event_postcard_hex)?;
        let active = AuthorizedEvent::decode_and_verify_author(&active_bytes)?;
        let bridge = CanonicalSyncLegacyEventBridgeV1::from_active_event(&active)?;
        assert_eq!(
            to_hex(bridge.conversation_id()),
            vectors.expected.conversation_id_hex
        );
        assert_eq!(to_hex(bridge.event_id()), vectors.expected.event_id_hex);
        assert_eq!(
            to_hex(bridge.author_account_id()),
            vectors.expected.author_account_id_hex
        );
        assert_eq!(
            to_hex(bridge.author_device_id()),
            vectors.expected.author_device_id_hex
        );
        assert_eq!(
            bridge.active_authorized_event_postcard(),
            active_bytes.as_slice()
        );
        assert_eq!(to_hex(&bridge.encode()?), vectors.expected.bridge_cbor_hex);
        assert_eq!(
            CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&bridge.encode()?)?,
            bridge
        );
        Ok(())
    }

    #[test]
    fn bridge_fails_closed_on_metadata_payload_and_cbor_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors = vectors()?;
        let encoded = from_hex(&vectors.expected.bridge_cbor_hex)?;
        let bridge = CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&encoded)?;

        let mut changed_conversation = bridge.clone();
        changed_conversation.conversation_id[0] ^= 1;
        assert!(matches!(
            CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&encode_unchecked(
                &changed_conversation
            )),
            Err(CanonicalSyncLegacyEventBridgeError::ConversationMismatch)
        ));

        let mut changed_event_id = bridge.clone();
        changed_event_id.event_id[0] ^= 1;
        assert!(matches!(
            CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&encode_unchecked(
                &changed_event_id
            )),
            Err(CanonicalSyncLegacyEventBridgeError::EventIdMismatch)
        ));

        let mut changed_account = bridge.clone();
        changed_account.author_account_id[0] ^= 1;
        assert!(matches!(
            CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&encode_unchecked(
                &changed_account
            )),
            Err(CanonicalSyncLegacyEventBridgeError::AuthorAccountMismatch)
        ));

        let mut changed_device = bridge.clone();
        changed_device.author_device_id[0] ^= 1;
        assert!(matches!(
            CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&encode_unchecked(&changed_device)),
            Err(CanonicalSyncLegacyEventBridgeError::AuthorDeviceMismatch)
        ));

        let mut changed_active = bridge;
        changed_active.active_authorized_event_postcard[10] ^= 1;
        assert!(
            CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&encode_unchecked(&changed_active))
                .is_err()
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&trailing).is_err());

        let mut non_minimal = Vec::with_capacity(encoded.len() + 1);
        non_minimal.extend_from_slice(&[0x98, 0x06]);
        non_minimal.extend_from_slice(&encoded[1..]);
        assert!(CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&non_minimal).is_err());
        Ok(())
    }
}
