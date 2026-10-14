//! Candidate deterministic CBOR transcripts for future cross-language protocol use.
//!
//! This module is deliberately not connected to the active Postcard wire codec.
//! Its constants and vectors reserve an explicit migration target; activating it
//! requires a separately versioned protocol transition.

use thiserror::Error;

use crate::{MAX_INVENTORY_EVENT_IDS, MAX_PARENTS};

pub const CANONICAL_PROFILE_VERSION: u64 = 1;
pub const ACKNOWLEDGEMENT_PAYLOAD_VARIANT: u64 = 1;
pub const CANONICAL_EVENT_SIGNATURE_DOMAIN: &[u8] = b"kilogram:event-signature:cbor-v1\0";
pub const CANONICAL_EVENT_ID_DOMAIN: &[u8] = b"kilogram:event-id:cbor-v1\0";
pub const CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:sync-inventory-signature:cbor-v1\0";

const MAX_CANONICAL_DEPTH: usize = 8;
const MAX_CANONICAL_ITEMS: usize = 8192;
const MAX_CANONICAL_BYTE_STRING_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct AcknowledgementEventContentV1<'a> {
    pub conversation_id: &'a [u8; 32],
    pub author_device_id: &'a [u8; 32],
    pub author_sequence: u64,
    pub parents: &'a [[u8; 32]],
    pub acknowledged_event_id: &'a [u8; 32],
}

#[derive(Clone, Copy, Debug)]
pub struct SyncInventoryContentV1<'a> {
    pub conversation_id: &'a [u8; 32],
    pub requester_device_id: &'a [u8; 32],
    pub session_binding: &'a [u8; 32],
    pub event_ids: &'a [[u8; 32]],
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CanonicalWireError {
    #[error("{field} contains {actual} items; maximum is {maximum}")]
    TooManyItems {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },

    #[error("{field} must be strictly ordered by unsigned bytewise lexicographic order")]
    NonCanonicalOrder { field: &'static str },

    #[error("canonical CBOR ended before an item was complete")]
    UnexpectedEnd,

    #[error("canonical CBOR uses a non-minimal integer or length argument")]
    NonMinimalArgument,

    #[error("canonical CBOR profile forbids indefinite-length items")]
    IndefiniteLength,

    #[error("canonical CBOR uses reserved additional information {0}")]
    ReservedAdditionalInformation(u8),

    #[error("canonical CBOR profile does not support major type {0}")]
    UnsupportedMajorType(u8),

    #[error("canonical CBOR nesting exceeds {MAX_CANONICAL_DEPTH}")]
    DepthLimit,

    #[error("canonical CBOR item count exceeds {MAX_CANONICAL_ITEMS}")]
    ItemLimit,

    #[error("canonical CBOR byte string exceeds {MAX_CANONICAL_BYTE_STRING_BYTES} bytes")]
    ByteStringLimit,

    #[error("canonical CBOR length does not fit this platform")]
    LengthOverflow,

    #[error("canonical CBOR contains trailing bytes")]
    TrailingData,
}

pub fn encode_acknowledgement_event_content_v1(
    content: AcknowledgementEventContentV1<'_>,
) -> Result<Vec<u8>, CanonicalWireError> {
    validate_ordered_ids("parents", content.parents, MAX_PARENTS)?;

    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 6);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, content.conversation_id);
    push_bytes(&mut encoded, content.author_device_id);
    push_unsigned(&mut encoded, content.author_sequence);
    push_array_header(&mut encoded, content.parents.len() as u64);
    for parent in content.parents {
        push_bytes(&mut encoded, parent);
    }
    push_array_header(&mut encoded, 2);
    push_unsigned(&mut encoded, ACKNOWLEDGEMENT_PAYLOAD_VARIANT);
    push_bytes(&mut encoded, content.acknowledged_event_id);
    Ok(encoded)
}

pub fn acknowledgement_event_signing_bytes_v1(
    content: AcknowledgementEventContentV1<'_>,
) -> Result<Vec<u8>, CanonicalWireError> {
    let encoded = encode_acknowledgement_event_content_v1(content)?;
    Ok(prefixed(CANONICAL_EVENT_SIGNATURE_DOMAIN, &encoded))
}

pub fn encode_signed_acknowledgement_event_v1(
    content: AcknowledgementEventContentV1<'_>,
    signature: &[u8; 64],
) -> Result<Vec<u8>, CanonicalWireError> {
    let content = encode_acknowledgement_event_content_v1(content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&content);
    push_bytes(&mut encoded, signature);
    Ok(encoded)
}

pub fn acknowledgement_event_id_v1(
    content: AcknowledgementEventContentV1<'_>,
    signature: &[u8; 64],
) -> Result<[u8; 32], CanonicalWireError> {
    let event = encode_signed_acknowledgement_event_v1(content, signature)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_EVENT_ID_DOMAIN);
    hasher.update(&event);
    Ok(*hasher.finalize().as_bytes())
}

pub fn encode_sync_inventory_content_v1(
    content: SyncInventoryContentV1<'_>,
) -> Result<Vec<u8>, CanonicalWireError> {
    validate_ordered_ids("event_ids", content.event_ids, MAX_INVENTORY_EVENT_IDS)?;

    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 5);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut encoded, content.conversation_id);
    push_bytes(&mut encoded, content.requester_device_id);
    push_bytes(&mut encoded, content.session_binding);
    push_array_header(&mut encoded, content.event_ids.len() as u64);
    for event_id in content.event_ids {
        push_bytes(&mut encoded, event_id);
    }
    Ok(encoded)
}

pub fn sync_inventory_signing_bytes_v1(
    content: SyncInventoryContentV1<'_>,
) -> Result<Vec<u8>, CanonicalWireError> {
    let encoded = encode_sync_inventory_content_v1(content)?;
    Ok(prefixed(
        CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN,
        &encoded,
    ))
}

pub fn encode_signed_sync_inventory_v1(
    content: SyncInventoryContentV1<'_>,
    signature: &[u8; 64],
) -> Result<Vec<u8>, CanonicalWireError> {
    let content = encode_sync_inventory_content_v1(content)?;
    let mut encoded = Vec::new();
    push_array_header(&mut encoded, 3);
    push_unsigned(&mut encoded, CANONICAL_PROFILE_VERSION);
    encoded.extend_from_slice(&content);
    push_bytes(&mut encoded, signature);
    Ok(encoded)
}

/// Validates the narrow Kilogram CBOR profile, not an application schema.
///
/// Only unsigned integers, byte strings and definite-length arrays are
/// accepted. Callers must additionally validate the exact positional schema.
pub fn validate_canonical_profile_v1(bytes: &[u8]) -> Result<(), CanonicalWireError> {
    let mut cursor = 0;
    let mut item_count = 0;
    validate_item(bytes, &mut cursor, 0, &mut item_count)?;
    if cursor != bytes.len() {
        return Err(CanonicalWireError::TrailingData);
    }
    Ok(())
}

fn validate_ordered_ids(
    field: &'static str,
    values: &[[u8; 32]],
    maximum: usize,
) -> Result<(), CanonicalWireError> {
    if values.len() > maximum {
        return Err(CanonicalWireError::TooManyItems {
            field,
            actual: values.len(),
            maximum,
        });
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CanonicalWireError::NonCanonicalOrder { field });
    }
    Ok(())
}

fn prefixed(domain: &[u8], encoded: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(encoded);
    bytes
}

fn push_unsigned(output: &mut Vec<u8>, value: u64) {
    push_head(output, 0, value);
}

fn push_bytes(output: &mut Vec<u8>, value: &[u8]) {
    push_head(output, 2, value.len() as u64);
    output.extend_from_slice(value);
}

fn push_array_header(output: &mut Vec<u8>, length: u64) {
    push_head(output, 4, length);
}

fn push_head(output: &mut Vec<u8>, major_type: u8, argument: u64) {
    let major = major_type << 5;
    match argument {
        0..=23 => output.push(major | argument as u8),
        24..=255 => {
            output.push(major | 24);
            output.push(argument as u8);
        }
        256..=65_535 => {
            output.push(major | 25);
            output.extend_from_slice(&(argument as u16).to_be_bytes());
        }
        65_536..=4_294_967_295 => {
            output.push(major | 26);
            output.extend_from_slice(&(argument as u32).to_be_bytes());
        }
        _ => {
            output.push(major | 27);
            output.extend_from_slice(&argument.to_be_bytes());
        }
    }
}

fn validate_item(
    bytes: &[u8],
    cursor: &mut usize,
    depth: usize,
    item_count: &mut usize,
) -> Result<(), CanonicalWireError> {
    if depth > MAX_CANONICAL_DEPTH {
        return Err(CanonicalWireError::DepthLimit);
    }
    *item_count = item_count
        .checked_add(1)
        .ok_or(CanonicalWireError::ItemLimit)?;
    if *item_count > MAX_CANONICAL_ITEMS {
        return Err(CanonicalWireError::ItemLimit);
    }

    let initial = take_byte(bytes, cursor)?;
    let major_type = initial >> 5;
    let additional = initial & 0x1f;
    match major_type {
        0 => {
            read_argument(bytes, cursor, additional)?;
            Ok(())
        }
        2 => {
            let length = argument_as_usize(read_argument(bytes, cursor, additional)?)?;
            if length > MAX_CANONICAL_BYTE_STRING_BYTES {
                return Err(CanonicalWireError::ByteStringLimit);
            }
            let end = cursor
                .checked_add(length)
                .ok_or(CanonicalWireError::LengthOverflow)?;
            if end > bytes.len() {
                return Err(CanonicalWireError::UnexpectedEnd);
            }
            *cursor = end;
            Ok(())
        }
        4 => {
            let length = argument_as_usize(read_argument(bytes, cursor, additional)?)?;
            if length > MAX_CANONICAL_ITEMS {
                return Err(CanonicalWireError::ItemLimit);
            }
            for _ in 0..length {
                validate_item(bytes, cursor, depth + 1, item_count)?;
            }
            Ok(())
        }
        _ => Err(CanonicalWireError::UnsupportedMajorType(major_type)),
    }
}

fn read_argument(
    bytes: &[u8],
    cursor: &mut usize,
    additional: u8,
) -> Result<u64, CanonicalWireError> {
    match additional {
        value @ 0..=23 => Ok(u64::from(value)),
        24 => {
            let value = u64::from(take_byte(bytes, cursor)?);
            if value < 24 {
                return Err(CanonicalWireError::NonMinimalArgument);
            }
            Ok(value)
        }
        25 => {
            let value = u64::from(u16::from_be_bytes(take_array(bytes, cursor)?));
            if value <= u64::from(u8::MAX) {
                return Err(CanonicalWireError::NonMinimalArgument);
            }
            Ok(value)
        }
        26 => {
            let value = u64::from(u32::from_be_bytes(take_array(bytes, cursor)?));
            if value <= u64::from(u16::MAX) {
                return Err(CanonicalWireError::NonMinimalArgument);
            }
            Ok(value)
        }
        27 => {
            let value = u64::from_be_bytes(take_array(bytes, cursor)?);
            if value <= u64::from(u32::MAX) {
                return Err(CanonicalWireError::NonMinimalArgument);
            }
            Ok(value)
        }
        31 => Err(CanonicalWireError::IndefiniteLength),
        value => Err(CanonicalWireError::ReservedAdditionalInformation(value)),
    }
}

fn take_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, CanonicalWireError> {
    let byte = bytes
        .get(*cursor)
        .copied()
        .ok_or(CanonicalWireError::UnexpectedEnd)?;
    *cursor += 1;
    Ok(byte)
}

fn take_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], CanonicalWireError> {
    let end = cursor
        .checked_add(N)
        .ok_or(CanonicalWireError::LengthOverflow)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(CanonicalWireError::UnexpectedEnd)?;
    *cursor = end;
    value
        .try_into()
        .map_err(|_| CanonicalWireError::UnexpectedEnd)
}

fn argument_as_usize(value: u64) -> Result<usize, CanonicalWireError> {
    usize::try_from(value).map_err(|_| CanonicalWireError::LengthOverflow)
}

#[cfg(test)]
mod tests {
    use kilogram_identity::DeviceIdentity;
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct GoldenVectors {
        schema_version: u64,
        profile: String,
        activation_status: String,
        acknowledgement_event: AcknowledgementVector,
        sync_inventory: SyncInventoryVector,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AcknowledgementVector {
        input: AcknowledgementInput,
        expected: AcknowledgementExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AcknowledgementInput {
        author_secret_key_hex: String,
        conversation_id_hex: String,
        author_sequence: u64,
        parents_hex: Vec<String>,
        acknowledged_event_id_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AcknowledgementExpected {
        author_device_id_hex: String,
        content_cbor_hex: String,
        signature_domain_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_event_cbor_hex: String,
        event_id_domain_hex: String,
        event_id_hex: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SyncInventoryVector {
        input: SyncInventoryInput,
        expected: SyncInventoryExpected,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SyncInventoryInput {
        author_secret_key_hex: String,
        conversation_id_hex: String,
        session_binding_hex: String,
        event_ids_hex: Vec<String>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SyncInventoryExpected {
        requester_device_id_hex: String,
        content_cbor_hex: String,
        signature_domain_hex: String,
        signing_preimage_hex: String,
        signature_hex: String,
        signed_inventory_cbor_hex: String,
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn invalid_data(message: impl Into<String>) -> Box<dyn std::error::Error> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        ))
    }

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid_data("golden vector contains invalid hexadecimal"));
        }
        let mut decoded = Vec::with_capacity(value.len() / 2);
        for offset in (0..value.len()).step_by(2) {
            decoded.push(u8::from_str_radix(&value[offset..offset + 2], 16)?);
        }
        Ok(decoded)
    }

    fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], Box<dyn std::error::Error>> {
        let bytes = decode_hex(value)?;
        bytes.try_into().map_err(|value: Vec<u8>| {
            invalid_data(format!(
                "golden vector contains {} bytes; expected {N}",
                value.len()
            ))
        })
    }

    fn load_vectors() -> Result<GoldenVectors, Box<dyn std::error::Error>> {
        let vectors: GoldenVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-WIRE-VECTORS.json"
        )))?;
        if vectors.schema_version != CANONICAL_PROFILE_VERSION
            || vectors.profile != "kilogram-deterministic-cbor-v1"
            || vectors.activation_status != "not-active"
        {
            return Err(invalid_data(
                "golden vector header does not match profile v1",
            ));
        }
        Ok(vectors)
    }

    #[test]
    fn profile_rejects_non_deterministic_or_unsupported_cbor() {
        assert_eq!(
            validate_canonical_profile_v1(&[0x18, 0x01]),
            Err(CanonicalWireError::NonMinimalArgument)
        );
        assert_eq!(
            validate_canonical_profile_v1(&[0x9f, 0x01, 0xff]),
            Err(CanonicalWireError::IndefiniteLength)
        );
        assert_eq!(
            validate_canonical_profile_v1(&[0x61, b'a']),
            Err(CanonicalWireError::UnsupportedMajorType(3))
        );
        assert_eq!(
            validate_canonical_profile_v1(&[0x01, 0x02]),
            Err(CanonicalWireError::TrailingData)
        );
    }

    #[test]
    fn profile_accepts_shortest_unsigned_boundaries() {
        let mut encoded = Vec::new();
        for value in [0, 23, 24, 255, 256, 65_535, 65_536, u64::MAX] {
            push_unsigned(&mut encoded, value);
        }
        let mut wrapped = Vec::new();
        push_array_header(&mut wrapped, 8);
        wrapped.extend_from_slice(&encoded);
        assert_eq!(validate_canonical_profile_v1(&wrapped), Ok(()));
    }

    #[test]
    fn content_rejects_unsorted_or_duplicate_identifiers() {
        let zero = [0_u8; 32];
        let one = [1_u8; 32];
        let duplicate = [one, one];
        let unsorted = [one, zero];
        let base = AcknowledgementEventContentV1 {
            conversation_id: &zero,
            author_device_id: &one,
            author_sequence: 0,
            parents: &duplicate,
            acknowledged_event_id: &zero,
        };
        assert_eq!(
            encode_acknowledgement_event_content_v1(base),
            Err(CanonicalWireError::NonCanonicalOrder { field: "parents" })
        );
        assert_eq!(
            encode_sync_inventory_content_v1(SyncInventoryContentV1 {
                conversation_id: &zero,
                requester_device_id: &one,
                session_binding: &zero,
                event_ids: &unsorted,
            }),
            Err(CanonicalWireError::NonCanonicalOrder { field: "event_ids" })
        );
    }

    #[test]
    fn acknowledgement_vector_matches_bytes_signature_and_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let vector = load_vectors()?.acknowledgement_event;
        let author_secret = decode_fixed(&vector.input.author_secret_key_hex)?;
        let identity = DeviceIdentity::from_secret_bytes(author_secret);
        let author_device_id = decode_fixed(&vector.expected.author_device_id_hex)?;
        assert_eq!(*identity.device_id().as_bytes(), author_device_id);
        let conversation_id = decode_fixed(&vector.input.conversation_id_hex)?;
        let parents = vector
            .input
            .parents_hex
            .iter()
            .map(|value| decode_fixed(value))
            .collect::<Result<Vec<[u8; 32]>, _>>()?;
        let acknowledged_event_id = decode_fixed(&vector.input.acknowledged_event_id_hex)?;
        let event_content = AcknowledgementEventContentV1 {
            conversation_id: &conversation_id,
            author_device_id: &author_device_id,
            author_sequence: vector.input.author_sequence,
            parents: &parents,
            acknowledged_event_id: &acknowledged_event_id,
        };
        let event_content_bytes = encode_acknowledgement_event_content_v1(event_content)?;
        let event_signing_bytes = acknowledgement_event_signing_bytes_v1(event_content)?;
        let event_signature = identity.sign(&event_signing_bytes);
        let signed_event = encode_signed_acknowledgement_event_v1(event_content, &event_signature)?;
        let event_id = acknowledgement_event_id_v1(event_content, &event_signature)?;
        assert_eq!(
            to_hex(&event_content_bytes),
            vector.expected.content_cbor_hex
        );
        assert_eq!(
            to_hex(CANONICAL_EVENT_SIGNATURE_DOMAIN),
            vector.expected.signature_domain_hex
        );
        assert_eq!(
            to_hex(&event_signing_bytes),
            vector.expected.signing_preimage_hex
        );
        assert_eq!(to_hex(&event_signature), vector.expected.signature_hex);
        assert_eq!(to_hex(&signed_event), vector.expected.signed_event_cbor_hex);
        assert_eq!(
            to_hex(CANONICAL_EVENT_ID_DOMAIN),
            vector.expected.event_id_domain_hex
        );
        assert_eq!(to_hex(&event_id), vector.expected.event_id_hex);
        identity
            .device_id()
            .verify(&event_signing_bytes, &event_signature)?;
        validate_canonical_profile_v1(&event_content_bytes)?;
        validate_canonical_profile_v1(&signed_event)?;
        Ok(())
    }

    #[test]
    fn sync_inventory_vector_matches_bytes_and_signature() -> Result<(), Box<dyn std::error::Error>>
    {
        let vector = load_vectors()?.sync_inventory;
        let author_secret = decode_fixed(&vector.input.author_secret_key_hex)?;
        let identity = DeviceIdentity::from_secret_bytes(author_secret);
        let requester_device_id = decode_fixed(&vector.expected.requester_device_id_hex)?;
        assert_eq!(*identity.device_id().as_bytes(), requester_device_id);
        let conversation_id = decode_fixed(&vector.input.conversation_id_hex)?;
        let session_binding = decode_fixed(&vector.input.session_binding_hex)?;
        let event_ids = vector
            .input
            .event_ids_hex
            .iter()
            .map(|value| decode_fixed(value))
            .collect::<Result<Vec<[u8; 32]>, _>>()?;
        let inventory_content = SyncInventoryContentV1 {
            conversation_id: &conversation_id,
            requester_device_id: &requester_device_id,
            session_binding: &session_binding,
            event_ids: &event_ids,
        };
        let inventory_content_bytes = encode_sync_inventory_content_v1(inventory_content)?;
        let inventory_signing_bytes = sync_inventory_signing_bytes_v1(inventory_content)?;
        let inventory_signature = identity.sign(&inventory_signing_bytes);
        let signed_inventory =
            encode_signed_sync_inventory_v1(inventory_content, &inventory_signature)?;
        assert_eq!(
            to_hex(&inventory_content_bytes),
            vector.expected.content_cbor_hex
        );
        assert_eq!(
            to_hex(CANONICAL_SYNC_INVENTORY_SIGNATURE_DOMAIN),
            vector.expected.signature_domain_hex
        );
        assert_eq!(
            to_hex(&inventory_signing_bytes),
            vector.expected.signing_preimage_hex
        );
        assert_eq!(to_hex(&inventory_signature), vector.expected.signature_hex);
        assert_eq!(
            to_hex(&signed_inventory),
            vector.expected.signed_inventory_cbor_hex
        );
        identity
            .device_id()
            .verify(&inventory_signing_bytes, &inventory_signature)?;
        validate_canonical_profile_v1(&inventory_content_bytes)?;
        validate_canonical_profile_v1(&signed_inventory)?;
        Ok(())
    }
}
