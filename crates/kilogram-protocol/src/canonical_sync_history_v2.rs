//! Inactive history-compatible event identity and carrier for canonical sync v2.
//!
//! The active Postcard and native canonical event formats use independent
//! Event-ID domains. A bare 32-byte identifier is therefore ambiguous once a
//! sync session can carry both. This module gives every identifier an explicit
//! namespace and closes the event carrier over exactly two verified variants:
//! exact active Postcard history and native canonical CBOR events.

use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_event_v1::{
        CanonicalAuthorizedEventV1, CanonicalEventError, decode_and_verify_authorized_event_v1,
        encode_authorized_event_v1, event_id_v1,
    },
    canonical_sync_legacy_event_v1::{
        CanonicalSyncLegacyEventBridgeError, CanonicalSyncLegacyEventBridgeV1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_SYNC_HISTORY_V2_OBJECT_BYTES: usize = 8 * 1024 * 1024;
pub const LEGACY_POSTCARD_V6_EVENT_NAMESPACE: u64 = 1;
pub const CANONICAL_CBOR_V1_EVENT_NAMESPACE: u64 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum CanonicalSyncEventNamespaceV2 {
    LegacyPostcardV6 = LEGACY_POSTCARD_V6_EVENT_NAMESPACE,
    CanonicalCborV1 = CANONICAL_CBOR_V1_EVENT_NAMESPACE,
}

impl CanonicalSyncEventNamespaceV2 {
    pub const fn code(self) -> u64 {
        self as u64
    }

    fn from_code(code: u64) -> Result<Self, CanonicalSyncHistoryV2Error> {
        match code {
            LEGACY_POSTCARD_V6_EVENT_NAMESPACE => Ok(Self::LegacyPostcardV6),
            CANONICAL_CBOR_V1_EVENT_NAMESPACE => Ok(Self::CanonicalCborV1),
            other => Err(CanonicalSyncHistoryV2Error::UnknownEventNamespace(other)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalSyncEventKeyV2 {
    pub namespace: CanonicalSyncEventNamespaceV2,
    pub event_id: [u8; 32],
}

impl CanonicalSyncEventKeyV2 {
    pub const fn new(namespace: CanonicalSyncEventNamespaceV2, event_id: [u8; 32]) -> Self {
        Self {
            namespace,
            event_id,
        }
    }

    pub fn encode(self) -> Result<Vec<u8>, CanonicalSyncHistoryV2Error> {
        let mut output = Vec::new();
        push_array_header(&mut output, 3);
        push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
        push_unsigned(&mut output, self.namespace.code());
        push_bytes(&mut output, &self.event_id);
        bounded(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CanonicalSyncHistoryV2Error> {
        validate_input(bytes)?;
        let mut decoder = AccountDecoder::new(bytes);
        decoder.expect_array("sync-event-key-v2", 3)?;
        decoder.expect_version("sync-event-key-v2.schema_version")?;
        let namespace = CanonicalSyncEventNamespaceV2::from_code(
            decoder.unsigned("sync-event-key-v2.namespace")?,
        )?;
        let event_id = decoder.fixed_bytes("sync-event-key-v2.event_id")?;
        decoder.finish()?;
        Ok(Self {
            namespace,
            event_id,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSyncEventCarrierV2 {
    LegacyPostcardV6(CanonicalSyncLegacyEventBridgeV1),
    CanonicalCborV1(CanonicalAuthorizedEventV1),
}

impl CanonicalSyncEventCarrierV2 {
    pub fn key(&self) -> Result<CanonicalSyncEventKeyV2, CanonicalSyncHistoryV2Error> {
        match self {
            Self::LegacyPostcardV6(value) => Ok(CanonicalSyncEventKeyV2::new(
                CanonicalSyncEventNamespaceV2::LegacyPostcardV6,
                *value.event_id(),
            )),
            Self::CanonicalCborV1(value) => Ok(CanonicalSyncEventKeyV2::new(
                CanonicalSyncEventNamespaceV2::CanonicalCborV1,
                event_id_v1(&value.event)?,
            )),
        }
    }

    pub fn conversation_id(&self) -> &[u8; 32] {
        match self {
            Self::LegacyPostcardV6(value) => value.conversation_id(),
            Self::CanonicalCborV1(value) => &value.event.content.conversation_id,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, CanonicalSyncHistoryV2Error> {
        let (namespace, payload) = match self {
            Self::LegacyPostcardV6(value) => (
                CanonicalSyncEventNamespaceV2::LegacyPostcardV6,
                value.encode()?,
            ),
            Self::CanonicalCborV1(value) => (
                CanonicalSyncEventNamespaceV2::CanonicalCborV1,
                encode_authorized_event_v1(value)?,
            ),
        };
        ensure_bound(&payload)?;
        let mut output = Vec::new();
        push_array_header(&mut output, 3);
        push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
        push_unsigned(&mut output, namespace.code());
        push_bytes(&mut output, &payload);
        bounded(output)
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, CanonicalSyncHistoryV2Error> {
        validate_input(bytes)?;
        let mut decoder = AccountDecoder::new(bytes);
        decoder.expect_array("sync-event-carrier-v2", 3)?;
        decoder.expect_version("sync-event-carrier-v2.schema_version")?;
        let namespace = CanonicalSyncEventNamespaceV2::from_code(
            decoder.unsigned("sync-event-carrier-v2.namespace")?,
        )?;
        let payload = decoder.bytes("sync-event-carrier-v2.payload")?;
        ensure_bound(payload)?;
        decoder.finish()?;
        match namespace {
            CanonicalSyncEventNamespaceV2::LegacyPostcardV6 => Ok(Self::LegacyPostcardV6(
                CanonicalSyncLegacyEventBridgeV1::decode_and_verify(payload)?,
            )),
            CanonicalSyncEventNamespaceV2::CanonicalCborV1 => Ok(Self::CanonicalCborV1(
                decode_and_verify_authorized_event_v1(payload)?,
            )),
        }
    }
}

#[derive(Debug, Error)]
pub enum CanonicalSyncHistoryV2Error {
    #[error(transparent)]
    Account(#[from] CanonicalAccountError),

    #[error(transparent)]
    LegacyEvent(#[from] CanonicalSyncLegacyEventBridgeError),

    #[error(transparent)]
    CanonicalEvent(#[from] CanonicalEventError),

    #[error(
        "canonical sync history-v2 object exceeds {MAX_CANONICAL_SYNC_HISTORY_V2_OBJECT_BYTES} bytes"
    )]
    ObjectTooLarge,

    #[error("unknown canonical sync event namespace {0}")]
    UnknownEventNamespace(u64),
}

fn validate_input(bytes: &[u8]) -> Result<(), CanonicalSyncHistoryV2Error> {
    ensure_bound(bytes)?;
    validate_canonical_profile_v1(bytes).map_err(CanonicalAccountError::from)?;
    Ok(())
}

fn ensure_bound(bytes: &[u8]) -> Result<(), CanonicalSyncHistoryV2Error> {
    if bytes.len() > MAX_CANONICAL_SYNC_HISTORY_V2_OBJECT_BYTES {
        Err(CanonicalSyncHistoryV2Error::ObjectTooLarge)
    } else {
        Ok(())
    }
}

fn bounded(bytes: Vec<u8>) -> Result<Vec<u8>, CanonicalSyncHistoryV2Error> {
    ensure_bound(&bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct HistoryVectors {
        schema_version: u64,
        milestone: String,
        activation_status: String,
        expected: HistoryExpected,
    }

    #[derive(Deserialize)]
    struct HistoryExpected {
        legacy_key_cbor_hex: String,
        canonical_key_cbor_hex: String,
        legacy_carrier_cbor_hex: String,
        canonical_carrier_cbor_hex: String,
    }

    #[derive(Deserialize)]
    struct LegacyVectors {
        expected: LegacyExpected,
    }

    #[derive(Deserialize)]
    struct LegacyExpected {
        bridge_cbor_hex: String,
        event_id_hex: String,
    }

    #[derive(Deserialize)]
    struct EventVectors {
        expected: EventExpected,
    }

    #[derive(Deserialize)]
    struct EventExpected {
        authorized_text_event_cbor_hex: String,
        text_event: EventIdExpected,
    }

    #[derive(Deserialize)]
    struct EventIdExpected {
        event_id_hex: String,
    }

    fn parse<T: for<'de> Deserialize<'de>>(value: &str) -> Result<T, serde_json::Error> {
        serde_json::from_str(value)
    }

    fn from_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2)
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid lowercase hexadecimal",
            )
            .into());
        }
        (0..value.len())
            .step_by(2)
            .map(|offset| Ok(u8::from_str_radix(&value[offset..offset + 2], 16)?))
            .collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn typed_keys_and_closed_carriers_match_vectors() -> Result<(), Box<dyn std::error::Error>> {
        let vectors: HistoryVectors = parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
        )))?;
        let legacy: LegacyVectors = parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-LEGACY-EVENT-VECTORS.json"
        )))?;
        let canonical: EventVectors = parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-EVENT-VECTORS.json"
        )))?;
        assert_eq!(vectors.schema_version, CANONICAL_PROFILE_VERSION);
        assert_eq!(vectors.milestone, "M1.1.53");
        assert_eq!(vectors.activation_status, "not-active");

        let legacy_bridge = CanonicalSyncLegacyEventBridgeV1::decode_and_verify(&from_hex(
            &legacy.expected.bridge_cbor_hex,
        )?)?;
        let canonical_event = decode_and_verify_authorized_event_v1(&from_hex(
            &canonical.expected.authorized_text_event_cbor_hex,
        )?)?;
        let legacy_carrier = CanonicalSyncEventCarrierV2::LegacyPostcardV6(legacy_bridge);
        let canonical_carrier = CanonicalSyncEventCarrierV2::CanonicalCborV1(canonical_event);
        let legacy_key = legacy_carrier.key()?;
        let canonical_key = canonical_carrier.key()?;

        assert_eq!(
            to_hex(&legacy_key.encode()?),
            vectors.expected.legacy_key_cbor_hex
        );
        assert_eq!(
            to_hex(&canonical_key.encode()?),
            vectors.expected.canonical_key_cbor_hex
        );
        assert_eq!(
            to_hex(&legacy_carrier.encode()?),
            vectors.expected.legacy_carrier_cbor_hex
        );
        assert_eq!(
            to_hex(&canonical_carrier.encode()?),
            vectors.expected.canonical_carrier_cbor_hex
        );
        assert_eq!(
            CanonicalSyncEventKeyV2::decode(&legacy_key.encode()?)?,
            legacy_key
        );
        assert_eq!(
            CanonicalSyncEventKeyV2::decode(&canonical_key.encode()?)?,
            canonical_key
        );
        assert_eq!(
            CanonicalSyncEventCarrierV2::decode_and_verify(&legacy_carrier.encode()?)?,
            legacy_carrier
        );
        assert_eq!(
            CanonicalSyncEventCarrierV2::decode_and_verify(&canonical_carrier.encode()?)?,
            canonical_carrier
        );
        assert_eq!(to_hex(&legacy_key.event_id), legacy.expected.event_id_hex);
        assert_eq!(
            to_hex(&canonical_key.event_id),
            canonical.expected.text_event.event_id_hex
        );
        Ok(())
    }

    #[test]
    fn namespaces_prevent_raw_id_collision_and_unknown_variants_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let raw_id = [7_u8; 32];
        let legacy =
            CanonicalSyncEventKeyV2::new(CanonicalSyncEventNamespaceV2::LegacyPostcardV6, raw_id);
        let canonical =
            CanonicalSyncEventKeyV2::new(CanonicalSyncEventNamespaceV2::CanonicalCborV1, raw_id);
        assert_ne!(legacy, canonical);
        assert!(legacy < canonical);

        let mut unknown_key = legacy.encode()?;
        unknown_key[2] = 3;
        assert!(matches!(
            CanonicalSyncEventKeyV2::decode(&unknown_key),
            Err(CanonicalSyncHistoryV2Error::UnknownEventNamespace(3))
        ));

        let vectors: HistoryVectors = parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-SYNC-HISTORY-V2-VECTORS.json"
        )))?;
        let mut unknown_carrier = from_hex(&vectors.expected.legacy_carrier_cbor_hex)?;
        unknown_carrier[2] = 3;
        assert!(matches!(
            CanonicalSyncEventCarrierV2::decode_and_verify(&unknown_carrier),
            Err(CanonicalSyncHistoryV2Error::UnknownEventNamespace(3))
        ));

        let mut trailing = from_hex(&vectors.expected.canonical_carrier_cbor_hex)?;
        trailing.push(0);
        assert!(CanonicalSyncEventCarrierV2::decode_and_verify(&trailing).is_err());

        let mut wrong_variant = from_hex(&vectors.expected.legacy_carrier_cbor_hex)?;
        wrong_variant[2] = CANONICAL_CBOR_V1_EVENT_NAMESPACE as u8;
        assert!(CanonicalSyncEventCarrierV2::decode_and_verify(&wrong_variant).is_err());
        Ok(())
    }
}
