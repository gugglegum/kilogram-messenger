//! Inactive deterministic-CBOR blind-mailbox request/response transport.
//!
//! The active mailbox wire remains Postcard over the existing Iroh ALPN. This
//! module composes the canonical mailbox capability objects into strict,
//! bounded request, response, peer-forwarding, and rejection frames. It is a
//! migration target only and is not imported by the active mailbox crates.

use thiserror::Error;

use crate::{
    canonical_account_v1::{AccountDecoder, CanonicalAccountError},
    canonical_mailbox_v1::{
        CanonicalMailboxAddressV1, CanonicalMailboxEnvelopeV1, CanonicalMailboxError,
        CanonicalMailboxReadOperationV1, CanonicalSignedMailboxDeleteReceiptV1,
        CanonicalSignedMailboxReadAuthorizationV1, CanonicalSignedMailboxStoredReceiptV1,
        CanonicalSignedMailboxWriteAuthorizationV1, decode_and_verify_mailbox_address_v1,
        decode_and_verify_mailbox_envelope_v1, decode_and_verify_signed_mailbox_delete_receipt_v1,
        decode_and_verify_signed_mailbox_read_authorization_v1,
        decode_and_verify_signed_mailbox_stored_receipt_v1,
        decode_and_verify_signed_mailbox_write_authorization_v1, encode_mailbox_address_v1,
        encode_mailbox_envelope_v1, encode_signed_mailbox_delete_receipt_v1,
        encode_signed_mailbox_read_authorization_v1, encode_signed_mailbox_stored_receipt_v1,
        encode_signed_mailbox_write_authorization_v1, mailbox_id_v1,
        verify_mailbox_read_authorization_for_v1, verify_mailbox_stored_receipt_for_v1,
        verify_mailbox_write_authorization_for_v1, verify_signed_mailbox_delete_receipt_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, CanonicalWireError, push_array_header, push_bytes,
        push_unsigned, validate_canonical_profile_v1,
    },
};

pub const CANONICAL_MAILBOX_TRANSPORT_OVERHEAD_BYTES: usize = 4 * 1024;
pub const MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES: usize =
    1024 * 1024 + CANONICAL_MAILBOX_TRANSPORT_OVERHEAD_BYTES;
pub const MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES: usize =
    8 * MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES + CANONICAL_MAILBOX_TRANSPORT_OVERHEAD_BYTES;
pub const MAX_CANONICAL_MAILBOX_PEER_REQUEST_BYTES: usize =
    MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES + CANONICAL_MAILBOX_TRANSPORT_OVERHEAD_BYTES;
pub const MAX_CANONICAL_MAILBOX_PEER_RESPONSE_BYTES: usize =
    MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES + CANONICAL_MAILBOX_TRANSPORT_OVERHEAD_BYTES;
pub const MAX_CANONICAL_MAILBOX_TRANSPORT_PAGE_ITEMS: usize = 8;

pub const CANONICAL_MAILBOX_PEER_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:blind-mailbox-peer-request:cbor-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalMailboxPeerOperationV1 {
    Put = 1,
    List = 2,
    Delete = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum CanonicalMailboxPeerRejectionV1 {
    RateLimited = 1,
    TransferBudgetExhausted = 2,
    InvalidCapability = 3,
    StoreUnavailable = 4,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxPutRequestV1 {
    pub address: CanonicalMailboxAddressV1,
    pub authorization: CanonicalSignedMailboxWriteAuthorizationV1,
    pub envelope: CanonicalMailboxEnvelopeV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxListRequestV1 {
    pub address: CanonicalMailboxAddressV1,
    pub authorization: CanonicalSignedMailboxReadAuthorizationV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxDeleteRequestV1 {
    pub address: CanonicalMailboxAddressV1,
    pub authorization: CanonicalSignedMailboxReadAuthorizationV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalMailboxRequestV1 {
    Put(CanonicalMailboxPutRequestV1),
    List(CanonicalMailboxListRequestV1),
    Delete(CanonicalMailboxDeleteRequestV1),
}

impl CanonicalMailboxRequestV1 {
    pub fn operation(&self) -> CanonicalMailboxPeerOperationV1 {
        match self {
            Self::Put(_) => CanonicalMailboxPeerOperationV1::Put,
            Self::List(_) => CanonicalMailboxPeerOperationV1::List,
            Self::Delete(_) => CanonicalMailboxPeerOperationV1::Delete,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxPeerRequestV1 {
    pub operation: CanonicalMailboxPeerOperationV1,
    pub request: CanonicalMailboxRequestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalMailboxPutResponseV1 {
    Stored {
        created: bool,
        receipt: Box<CanonicalSignedMailboxStoredReceiptV1>,
    },
    Conflict,
    Tombstoned,
    CapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalStoredMailboxItemV1 {
    pub item_id: [u8; 32],
    pub expires_at_unix_seconds: u64,
    pub envelope: CanonicalMailboxEnvelopeV1,
    pub receipt: CanonicalSignedMailboxStoredReceiptV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxListResponseV1 {
    pub items: Vec<CanonicalStoredMailboxItemV1>,
    pub next_after_item_id: Option<[u8; 32]>,
    pub more_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalMailboxDeleteResponseV1 {
    Deleted {
        newly_deleted: bool,
        receipt: Box<CanonicalSignedMailboxDeleteReceiptV1>,
    },
    Absent,
    ReceiptMismatch,
    TombstoneCapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalMailboxResponseV1 {
    Put(CanonicalMailboxPutResponseV1),
    List(CanonicalMailboxListResponseV1),
    Delete(CanonicalMailboxDeleteResponseV1),
}

impl CanonicalMailboxResponseV1 {
    pub fn operation(&self) -> CanonicalMailboxPeerOperationV1 {
        match self {
            Self::Put(_) => CanonicalMailboxPeerOperationV1::Put,
            Self::List(_) => CanonicalMailboxPeerOperationV1::List,
            Self::Delete(_) => CanonicalMailboxPeerOperationV1::Delete,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalMailboxPeerOutcomeV1 {
    Success(Box<CanonicalMailboxResponseV1>),
    Rejected(CanonicalMailboxPeerRejectionV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMailboxPeerResponseV1 {
    pub operation: CanonicalMailboxPeerOperationV1,
    pub request_digest: [u8; 32],
    pub outcome: CanonicalMailboxPeerOutcomeV1,
}

#[derive(Debug, Error)]
pub enum CanonicalMailboxTransportError {
    #[error(transparent)]
    Wire(#[from] CanonicalWireError),
    #[error(transparent)]
    Decode(#[from] CanonicalAccountError),
    #[error(transparent)]
    Mailbox(#[from] CanonicalMailboxError),
    #[error("canonical mailbox transport {object} exceeds {maximum} bytes")]
    ObjectTooLarge {
        object: &'static str,
        maximum: usize,
    },
    #[error("unknown canonical mailbox peer operation {0}")]
    UnknownOperation(u64),
    #[error("unknown canonical mailbox put response {0}")]
    UnknownPutResponse(u64),
    #[error("unknown canonical mailbox delete response {0}")]
    UnknownDeleteResponse(u64),
    #[error("unknown canonical mailbox peer outcome {0}")]
    UnknownOutcome(u64),
    #[error("unknown canonical mailbox peer rejection {0}")]
    UnknownRejection(u64),
    #[error("canonical mailbox transport boolean must be zero or one")]
    InvalidBoolean,
    #[error("canonical mailbox transport operation and payload disagree")]
    OperationMismatch,
    #[error("canonical mailbox transport request is malformed")]
    InvalidRequest,
    #[error("canonical mailbox transport response does not match the request")]
    ResponseMismatch,
    #[error("canonical mailbox list page is malformed")]
    InvalidPage,
}

pub fn encode_mailbox_put_request_v1(
    request: &CanonicalMailboxPutRequestV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    verify_put_request(request)?;
    let address = encode_mailbox_address_v1(&request.address)?;
    let authorization = encode_signed_mailbox_write_authorization_v1(&request.authorization)?;
    let envelope = encode_mailbox_envelope_v1(&request.envelope)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &address);
    push_bytes(&mut output, &authorization);
    push_bytes(&mut output, &envelope);
    ensure_bound(
        &output,
        "put request",
        MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_put_request_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxPutRequestV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "put request",
        MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let request = decode_put_request_from(&mut decoder)?;
    decoder.finish()?;
    verify_put_request(&request)?;
    Ok(request)
}

pub fn encode_mailbox_list_request_v1(
    request: &CanonicalMailboxListRequestV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    verify_list_request(request)?;
    let address = encode_mailbox_address_v1(&request.address)?;
    let authorization = encode_signed_mailbox_read_authorization_v1(&request.authorization)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &address);
    push_bytes(&mut output, &authorization);
    ensure_bound(
        &output,
        "list request",
        MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_list_request_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxListRequestV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "list request",
        MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let request = decode_list_request_from(&mut decoder)?;
    decoder.finish()?;
    verify_list_request(&request)?;
    Ok(request)
}

pub fn encode_mailbox_delete_request_v1(
    request: &CanonicalMailboxDeleteRequestV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    verify_delete_request(request)?;
    let address = encode_mailbox_address_v1(&request.address)?;
    let authorization = encode_signed_mailbox_read_authorization_v1(&request.authorization)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &address);
    push_bytes(&mut output, &authorization);
    ensure_bound(
        &output,
        "delete request",
        MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_delete_request_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxDeleteRequestV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "delete request",
        MAX_CANONICAL_MAILBOX_TRANSPORT_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let request = decode_delete_request_from(&mut decoder)?;
    decoder.finish()?;
    verify_delete_request(&request)?;
    Ok(request)
}

pub fn encode_mailbox_peer_request_v1(
    request: &CanonicalMailboxPeerRequestV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    if request.operation != request.request.operation() {
        return Err(CanonicalMailboxTransportError::OperationMismatch);
    }
    let payload = encode_request(&request.request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 3);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, request.operation as u64);
    push_bytes(&mut output, &payload);
    ensure_bound(
        &output,
        "peer request",
        MAX_CANONICAL_MAILBOX_PEER_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_peer_request_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxPeerRequestV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "peer request",
        MAX_CANONICAL_MAILBOX_PEER_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-peer-request-v1", 3)?;
    decoder.expect_version("mailbox-peer-request-v1.schema_version")?;
    let operation = decode_operation(decoder.unsigned("mailbox-peer-request-v1.operation")?)?;
    let payload = decoder.bytes("mailbox-peer-request-v1.payload")?;
    decoder.finish()?;
    let request = decode_request(operation, payload)?;
    Ok(CanonicalMailboxPeerRequestV1 { operation, request })
}

pub fn mailbox_peer_request_digest_v1(
    request: &CanonicalMailboxPeerRequestV1,
) -> Result<[u8; 32], CanonicalMailboxTransportError> {
    let encoded = encode_mailbox_peer_request_v1(request)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_MAILBOX_PEER_REQUEST_DIGEST_DOMAIN);
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

pub fn encode_mailbox_put_response_v1(
    response: &CanonicalMailboxPutResponseV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    let mut output = Vec::new();
    encode_put_response_into(response, &mut output)?;
    ensure_bound(
        &output,
        "put response",
        MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_put_response_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxPutResponseV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "put response",
        MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let response = decode_put_response_from(&mut decoder)?;
    decoder.finish()?;
    Ok(response)
}

pub fn verify_mailbox_put_response_for_v1(
    response: &CanonicalMailboxPutResponseV1,
    request: &CanonicalMailboxPutRequestV1,
    expected_store_key: &[u8; 32],
) -> Result<(), CanonicalMailboxTransportError> {
    verify_put_request(request)?;
    if let CanonicalMailboxPutResponseV1::Stored { receipt, .. } = response {
        verify_mailbox_stored_receipt_for_v1(receipt, &request.envelope)?;
        if &receipt.content.store_key != expected_store_key {
            return Err(CanonicalMailboxTransportError::ResponseMismatch);
        }
    }
    Ok(())
}

pub fn encode_mailbox_list_response_v1(
    response: &CanonicalMailboxListResponseV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    validate_list_response(response)?;
    let mut output = Vec::new();
    encode_list_response_into(response, &mut output)?;
    ensure_bound(
        &output,
        "list response",
        MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_list_response_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxListResponseV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "list response",
        MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let response = decode_list_response_from(&mut decoder)?;
    decoder.finish()?;
    validate_list_response(&response)?;
    Ok(response)
}

pub fn verify_mailbox_list_response_for_v1(
    response: &CanonicalMailboxListResponseV1,
    request: &CanonicalMailboxListRequestV1,
    expected_store_key: &[u8; 32],
) -> Result<(), CanonicalMailboxTransportError> {
    verify_list_request(request)?;
    validate_list_response(response)?;
    let (after_item_id, limit) = match request.authorization.content.operation {
        CanonicalMailboxReadOperationV1::List {
            after_item_id,
            limit,
            ..
        } => (after_item_id, limit),
        CanonicalMailboxReadOperationV1::Delete { .. } => {
            return Err(CanonicalMailboxTransportError::OperationMismatch);
        }
    };
    if response.items.len() > limit as usize {
        return Err(CanonicalMailboxTransportError::InvalidPage);
    }
    let expected_mailbox_id = mailbox_id_v1(&request.address)?;
    for item in &response.items {
        if item.receipt.content.store_key != *expected_store_key
            || item.receipt.content.mailbox_id != expected_mailbox_id
            || after_item_id.is_some_and(|after| item.item_id <= after)
        {
            return Err(CanonicalMailboxTransportError::ResponseMismatch);
        }
    }
    Ok(())
}

pub fn encode_mailbox_delete_response_v1(
    response: &CanonicalMailboxDeleteResponseV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    let mut output = Vec::new();
    encode_delete_response_into(response, &mut output)?;
    ensure_bound(
        &output,
        "delete response",
        MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_delete_response_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxDeleteResponseV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "delete response",
        MAX_CANONICAL_MAILBOX_TRANSPORT_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let response = decode_delete_response_from(&mut decoder)?;
    decoder.finish()?;
    Ok(response)
}

pub fn verify_mailbox_delete_response_for_v1(
    response: &CanonicalMailboxDeleteResponseV1,
    request: &CanonicalMailboxDeleteRequestV1,
    expected_store_key: &[u8; 32],
) -> Result<(), CanonicalMailboxTransportError> {
    verify_delete_request(request)?;
    if let CanonicalMailboxDeleteResponseV1::Deleted { receipt, .. } = response {
        let (item_id, receipt_id) = match request.authorization.content.operation {
            CanonicalMailboxReadOperationV1::Delete {
                item_id,
                receipt_id,
            } => (item_id, receipt_id),
            CanonicalMailboxReadOperationV1::List { .. } => {
                return Err(CanonicalMailboxTransportError::OperationMismatch);
            }
        };
        if receipt.content.store_key != *expected_store_key
            || receipt.content.mailbox_id != mailbox_id_v1(&request.address)?
            || receipt.content.item_id != item_id
            || receipt.content.stored_receipt_id != receipt_id
        {
            return Err(CanonicalMailboxTransportError::ResponseMismatch);
        }
    }
    Ok(())
}

pub fn encode_mailbox_peer_response_v1(
    response: &CanonicalMailboxPeerResponseV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    if let CanonicalMailboxPeerOutcomeV1::Success(payload) = &response.outcome
        && response.operation != payload.operation()
    {
        return Err(CanonicalMailboxTransportError::OperationMismatch);
    }
    let mut output = Vec::new();
    push_array_header(&mut output, 4);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, response.operation as u64);
    push_bytes(&mut output, &response.request_digest);
    match &response.outcome {
        CanonicalMailboxPeerOutcomeV1::Success(payload) => {
            push_array_header(&mut output, 3);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 1);
            encode_response_into(payload, &mut output)?;
        }
        CanonicalMailboxPeerOutcomeV1::Rejected(reason) => {
            push_array_header(&mut output, 3);
            push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
            push_unsigned(&mut output, 2);
            push_unsigned(&mut output, *reason as u64);
        }
    }
    ensure_bound(
        &output,
        "peer response",
        MAX_CANONICAL_MAILBOX_PEER_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_mailbox_peer_response_v1(
    bytes: &[u8],
) -> Result<CanonicalMailboxPeerResponseV1, CanonicalMailboxTransportError> {
    ensure_bound(
        bytes,
        "peer response",
        MAX_CANONICAL_MAILBOX_PEER_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("mailbox-peer-response-v1", 4)?;
    decoder.expect_version("mailbox-peer-response-v1.schema_version")?;
    let operation = decode_operation(decoder.unsigned("mailbox-peer-response-v1.operation")?)?;
    let request_digest = decoder.fixed_bytes("mailbox-peer-response-v1.request_digest")?;
    decoder.expect_array("mailbox-peer-outcome-v1", 3)?;
    decoder.expect_version("mailbox-peer-outcome-v1.schema_version")?;
    let outcome = match decoder.unsigned("mailbox-peer-outcome-v1.variant")? {
        1 => CanonicalMailboxPeerOutcomeV1::Success(Box::new(decode_response_from(
            operation,
            &mut decoder,
        )?)),
        2 => CanonicalMailboxPeerOutcomeV1::Rejected(decode_rejection(
            decoder.unsigned("mailbox-peer-outcome-v1.rejection")?,
        )?),
        value => return Err(CanonicalMailboxTransportError::UnknownOutcome(value)),
    };
    decoder.finish()?;
    Ok(CanonicalMailboxPeerResponseV1 {
        operation,
        request_digest,
        outcome,
    })
}

pub fn verify_mailbox_peer_response_for_v1(
    response: &CanonicalMailboxPeerResponseV1,
    request: &CanonicalMailboxPeerRequestV1,
    expected_store_key: &[u8; 32],
) -> Result<(), CanonicalMailboxTransportError> {
    if response.operation != request.operation
        || response.request_digest != mailbox_peer_request_digest_v1(request)?
    {
        return Err(CanonicalMailboxTransportError::ResponseMismatch);
    }
    let CanonicalMailboxPeerOutcomeV1::Success(payload) = &response.outcome else {
        return Ok(());
    };
    match (&request.request, payload.as_ref()) {
        (CanonicalMailboxRequestV1::Put(request), CanonicalMailboxResponseV1::Put(response)) => {
            verify_mailbox_put_response_for_v1(response, request, expected_store_key)
        }
        (CanonicalMailboxRequestV1::List(request), CanonicalMailboxResponseV1::List(response)) => {
            verify_mailbox_list_response_for_v1(response, request, expected_store_key)
        }
        (
            CanonicalMailboxRequestV1::Delete(request),
            CanonicalMailboxResponseV1::Delete(response),
        ) => verify_mailbox_delete_response_for_v1(response, request, expected_store_key),
        _ => Err(CanonicalMailboxTransportError::OperationMismatch),
    }
}

fn encode_request(
    request: &CanonicalMailboxRequestV1,
) -> Result<Vec<u8>, CanonicalMailboxTransportError> {
    match request {
        CanonicalMailboxRequestV1::Put(value) => encode_mailbox_put_request_v1(value),
        CanonicalMailboxRequestV1::List(value) => encode_mailbox_list_request_v1(value),
        CanonicalMailboxRequestV1::Delete(value) => encode_mailbox_delete_request_v1(value),
    }
}

fn decode_request(
    operation: CanonicalMailboxPeerOperationV1,
    bytes: &[u8],
) -> Result<CanonicalMailboxRequestV1, CanonicalMailboxTransportError> {
    match operation {
        CanonicalMailboxPeerOperationV1::Put => Ok(CanonicalMailboxRequestV1::Put(
            decode_and_verify_mailbox_put_request_v1(bytes)?,
        )),
        CanonicalMailboxPeerOperationV1::List => Ok(CanonicalMailboxRequestV1::List(
            decode_and_verify_mailbox_list_request_v1(bytes)?,
        )),
        CanonicalMailboxPeerOperationV1::Delete => Ok(CanonicalMailboxRequestV1::Delete(
            decode_and_verify_mailbox_delete_request_v1(bytes)?,
        )),
    }
}

fn decode_put_request_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxPutRequestV1, CanonicalMailboxTransportError> {
    decoder.expect_array("mailbox-put-request-v1", 4)?;
    decoder.expect_version("mailbox-put-request-v1.schema_version")?;
    let address =
        decode_and_verify_mailbox_address_v1(decoder.bytes("mailbox-put-request-v1.address")?)?;
    let authorization = decode_and_verify_signed_mailbox_write_authorization_v1(
        decoder.bytes("mailbox-put-request-v1.authorization")?,
    )?;
    let envelope =
        decode_and_verify_mailbox_envelope_v1(decoder.bytes("mailbox-put-request-v1.envelope")?)?;
    Ok(CanonicalMailboxPutRequestV1 {
        address,
        authorization,
        envelope,
    })
}

fn decode_list_request_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxListRequestV1, CanonicalMailboxTransportError> {
    decoder.expect_array("mailbox-list-request-v1", 3)?;
    decoder.expect_version("mailbox-list-request-v1.schema_version")?;
    let address =
        decode_and_verify_mailbox_address_v1(decoder.bytes("mailbox-list-request-v1.address")?)?;
    let authorization = decode_and_verify_signed_mailbox_read_authorization_v1(
        decoder.bytes("mailbox-list-request-v1.authorization")?,
    )?;
    Ok(CanonicalMailboxListRequestV1 {
        address,
        authorization,
    })
}

fn decode_delete_request_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxDeleteRequestV1, CanonicalMailboxTransportError> {
    decoder.expect_array("mailbox-delete-request-v1", 3)?;
    decoder.expect_version("mailbox-delete-request-v1.schema_version")?;
    let address =
        decode_and_verify_mailbox_address_v1(decoder.bytes("mailbox-delete-request-v1.address")?)?;
    let authorization = decode_and_verify_signed_mailbox_read_authorization_v1(
        decoder.bytes("mailbox-delete-request-v1.authorization")?,
    )?;
    Ok(CanonicalMailboxDeleteRequestV1 {
        address,
        authorization,
    })
}

fn verify_put_request(
    request: &CanonicalMailboxPutRequestV1,
) -> Result<(), CanonicalMailboxTransportError> {
    verify_mailbox_write_authorization_for_v1(
        &request.authorization,
        &request.address,
        &request.envelope,
    )?;
    Ok(())
}

fn verify_list_request(
    request: &CanonicalMailboxListRequestV1,
) -> Result<(), CanonicalMailboxTransportError> {
    verify_mailbox_read_authorization_for_v1(&request.authorization, &request.address)?;
    if !matches!(
        request.authorization.content.operation,
        CanonicalMailboxReadOperationV1::List { .. }
    ) {
        return Err(CanonicalMailboxTransportError::OperationMismatch);
    }
    Ok(())
}

fn verify_delete_request(
    request: &CanonicalMailboxDeleteRequestV1,
) -> Result<(), CanonicalMailboxTransportError> {
    verify_mailbox_read_authorization_for_v1(&request.authorization, &request.address)?;
    if !matches!(
        request.authorization.content.operation,
        CanonicalMailboxReadOperationV1::Delete { .. }
    ) {
        return Err(CanonicalMailboxTransportError::OperationMismatch);
    }
    Ok(())
}

fn encode_put_response_into(
    response: &CanonicalMailboxPutResponseV1,
    output: &mut Vec<u8>,
) -> Result<(), CanonicalMailboxTransportError> {
    match response {
        CanonicalMailboxPutResponseV1::Stored { created, receipt } => {
            let receipt = encode_signed_mailbox_stored_receipt_v1(receipt)?;
            push_array_header(output, 4);
            push_unsigned(output, CANONICAL_PROFILE_VERSION);
            push_unsigned(output, 1);
            push_bool(output, *created);
            push_bytes(output, &receipt);
        }
        CanonicalMailboxPutResponseV1::Conflict => encode_status(output, 2),
        CanonicalMailboxPutResponseV1::Tombstoned => encode_status(output, 3),
        CanonicalMailboxPutResponseV1::CapacityExceeded => encode_status(output, 4),
    }
    Ok(())
}

fn decode_put_response_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxPutResponseV1, CanonicalMailboxTransportError> {
    let length = decoder.array("mailbox-put-response-v1")?;
    decoder.expect_version("mailbox-put-response-v1.schema_version")?;
    let variant = decoder.unsigned("mailbox-put-response-v1.variant")?;
    match (variant, length) {
        (1, 4) => Ok(CanonicalMailboxPutResponseV1::Stored {
            created: decode_bool(decoder.unsigned("mailbox-put-response-v1.created")?)?,
            receipt: Box::new(decode_and_verify_signed_mailbox_stored_receipt_v1(
                decoder.bytes("mailbox-put-response-v1.receipt")?,
            )?),
        }),
        (2, 2) => Ok(CanonicalMailboxPutResponseV1::Conflict),
        (3, 2) => Ok(CanonicalMailboxPutResponseV1::Tombstoned),
        (4, 2) => Ok(CanonicalMailboxPutResponseV1::CapacityExceeded),
        (1..=4, _) => Err(CanonicalMailboxTransportError::InvalidRequest),
        _ => Err(CanonicalMailboxTransportError::UnknownPutResponse(variant)),
    }
}

fn encode_list_response_into(
    response: &CanonicalMailboxListResponseV1,
    output: &mut Vec<u8>,
) -> Result<(), CanonicalMailboxTransportError> {
    push_array_header(output, 4);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_array_header(output, response.items.len() as u64);
    for item in &response.items {
        let envelope = encode_mailbox_envelope_v1(&item.envelope)?;
        let receipt = encode_signed_mailbox_stored_receipt_v1(&item.receipt)?;
        push_array_header(output, 5);
        push_unsigned(output, CANONICAL_PROFILE_VERSION);
        push_bytes(output, &item.item_id);
        push_unsigned(output, item.expires_at_unix_seconds);
        push_bytes(output, &envelope);
        push_bytes(output, &receipt);
    }
    encode_item_option(response.next_after_item_id, output);
    push_bool(output, response.more_available);
    Ok(())
}

fn decode_list_response_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxListResponseV1, CanonicalMailboxTransportError> {
    decoder.expect_array("mailbox-list-response-v1", 4)?;
    decoder.expect_version("mailbox-list-response-v1.schema_version")?;
    let item_count = decoder.array("mailbox-list-response-v1.items")?;
    if item_count > MAX_CANONICAL_MAILBOX_TRANSPORT_PAGE_ITEMS {
        return Err(CanonicalMailboxTransportError::InvalidPage);
    }
    let mut items = Vec::with_capacity(item_count);
    for _ in 0..item_count {
        decoder.expect_array("stored-mailbox-item-v1", 5)?;
        decoder.expect_version("stored-mailbox-item-v1.schema_version")?;
        items.push(CanonicalStoredMailboxItemV1 {
            item_id: decoder.fixed_bytes("stored-mailbox-item-v1.item_id")?,
            expires_at_unix_seconds: decoder.unsigned("stored-mailbox-item-v1.expires_at")?,
            envelope: decode_and_verify_mailbox_envelope_v1(
                decoder.bytes("stored-mailbox-item-v1.envelope")?,
            )?,
            receipt: decode_and_verify_signed_mailbox_stored_receipt_v1(
                decoder.bytes("stored-mailbox-item-v1.receipt")?,
            )?,
        });
    }
    let next_after_item_id = decode_item_option(decoder)?;
    let more_available = decode_bool(decoder.unsigned("mailbox-list-response-v1.more")?)?;
    Ok(CanonicalMailboxListResponseV1 {
        items,
        next_after_item_id,
        more_available,
    })
}

fn validate_list_response(
    response: &CanonicalMailboxListResponseV1,
) -> Result<(), CanonicalMailboxTransportError> {
    if response.items.len() > MAX_CANONICAL_MAILBOX_TRANSPORT_PAGE_ITEMS
        || response.more_available != response.next_after_item_id.is_some()
        || (response.more_available && response.items.is_empty())
        || response
            .items
            .windows(2)
            .any(|pair| pair[0].item_id >= pair[1].item_id)
        || response.next_after_item_id
            != response
                .items
                .last()
                .map(|item| item.item_id)
                .filter(|_| response.more_available)
    {
        return Err(CanonicalMailboxTransportError::InvalidPage);
    }
    for item in &response.items {
        verify_mailbox_stored_receipt_for_v1(&item.receipt, &item.envelope)?;
        if item.item_id != item.envelope.context.item_id
            || item.item_id != item.receipt.content.item_id
            || item.expires_at_unix_seconds != item.envelope.context.expires_at_unix_seconds
            || item.expires_at_unix_seconds != item.receipt.content.expires_at_unix_seconds
        {
            return Err(CanonicalMailboxTransportError::InvalidPage);
        }
    }
    Ok(())
}

fn encode_delete_response_into(
    response: &CanonicalMailboxDeleteResponseV1,
    output: &mut Vec<u8>,
) -> Result<(), CanonicalMailboxTransportError> {
    match response {
        CanonicalMailboxDeleteResponseV1::Deleted {
            newly_deleted,
            receipt,
        } => {
            verify_signed_mailbox_delete_receipt_v1(receipt)?;
            let receipt = encode_signed_mailbox_delete_receipt_v1(receipt)?;
            push_array_header(output, 4);
            push_unsigned(output, CANONICAL_PROFILE_VERSION);
            push_unsigned(output, 1);
            push_bool(output, *newly_deleted);
            push_bytes(output, &receipt);
        }
        CanonicalMailboxDeleteResponseV1::Absent => encode_status(output, 2),
        CanonicalMailboxDeleteResponseV1::ReceiptMismatch => encode_status(output, 3),
        CanonicalMailboxDeleteResponseV1::TombstoneCapacityExceeded => encode_status(output, 4),
    }
    Ok(())
}

fn decode_delete_response_from(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxDeleteResponseV1, CanonicalMailboxTransportError> {
    let length = decoder.array("mailbox-delete-response-v1")?;
    decoder.expect_version("mailbox-delete-response-v1.schema_version")?;
    let variant = decoder.unsigned("mailbox-delete-response-v1.variant")?;
    match (variant, length) {
        (1, 4) => Ok(CanonicalMailboxDeleteResponseV1::Deleted {
            newly_deleted: decode_bool(
                decoder.unsigned("mailbox-delete-response-v1.newly_deleted")?,
            )?,
            receipt: Box::new(decode_and_verify_signed_mailbox_delete_receipt_v1(
                decoder.bytes("mailbox-delete-response-v1.receipt")?,
            )?),
        }),
        (2, 2) => Ok(CanonicalMailboxDeleteResponseV1::Absent),
        (3, 2) => Ok(CanonicalMailboxDeleteResponseV1::ReceiptMismatch),
        (4, 2) => Ok(CanonicalMailboxDeleteResponseV1::TombstoneCapacityExceeded),
        (1..=4, _) => Err(CanonicalMailboxTransportError::InvalidRequest),
        _ => Err(CanonicalMailboxTransportError::UnknownDeleteResponse(
            variant,
        )),
    }
}

fn encode_response_into(
    response: &CanonicalMailboxResponseV1,
    output: &mut Vec<u8>,
) -> Result<(), CanonicalMailboxTransportError> {
    match response {
        CanonicalMailboxResponseV1::Put(value) => encode_put_response_into(value, output),
        CanonicalMailboxResponseV1::List(value) => encode_list_response_into(value, output),
        CanonicalMailboxResponseV1::Delete(value) => encode_delete_response_into(value, output),
    }
}

fn decode_response_from(
    operation: CanonicalMailboxPeerOperationV1,
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalMailboxResponseV1, CanonicalMailboxTransportError> {
    match operation {
        CanonicalMailboxPeerOperationV1::Put => Ok(CanonicalMailboxResponseV1::Put(
            decode_put_response_from(decoder)?,
        )),
        CanonicalMailboxPeerOperationV1::List => Ok(CanonicalMailboxResponseV1::List(
            decode_list_response_from(decoder)?,
        )),
        CanonicalMailboxPeerOperationV1::Delete => Ok(CanonicalMailboxResponseV1::Delete(
            decode_delete_response_from(decoder)?,
        )),
    }
}

fn encode_item_option(value: Option<[u8; 32]>, output: &mut Vec<u8>) {
    if let Some(value) = value {
        push_array_header(output, 1);
        push_bytes(output, &value);
    } else {
        push_array_header(output, 0);
    }
}

fn decode_item_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<[u8; 32]>, CanonicalMailboxTransportError> {
    match decoder.array("mailbox-item-option-v1")? {
        0 => Ok(None),
        1 => Ok(Some(decoder.fixed_bytes("mailbox-item-option-v1.item_id")?)),
        _ => Err(CanonicalMailboxTransportError::InvalidPage),
    }
}

fn encode_status(output: &mut Vec<u8>, variant: u64) {
    push_array_header(output, 2);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_unsigned(output, variant);
}

fn push_bool(output: &mut Vec<u8>, value: bool) {
    push_unsigned(output, u64::from(value));
}

fn decode_bool(value: u64) -> Result<bool, CanonicalMailboxTransportError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CanonicalMailboxTransportError::InvalidBoolean),
    }
}

fn decode_operation(
    value: u64,
) -> Result<CanonicalMailboxPeerOperationV1, CanonicalMailboxTransportError> {
    match value {
        1 => Ok(CanonicalMailboxPeerOperationV1::Put),
        2 => Ok(CanonicalMailboxPeerOperationV1::List),
        3 => Ok(CanonicalMailboxPeerOperationV1::Delete),
        _ => Err(CanonicalMailboxTransportError::UnknownOperation(value)),
    }
}

fn decode_rejection(
    value: u64,
) -> Result<CanonicalMailboxPeerRejectionV1, CanonicalMailboxTransportError> {
    match value {
        1 => Ok(CanonicalMailboxPeerRejectionV1::RateLimited),
        2 => Ok(CanonicalMailboxPeerRejectionV1::TransferBudgetExhausted),
        3 => Ok(CanonicalMailboxPeerRejectionV1::InvalidCapability),
        4 => Ok(CanonicalMailboxPeerRejectionV1::StoreUnavailable),
        _ => Err(CanonicalMailboxTransportError::UnknownRejection(value)),
    }
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalMailboxTransportError> {
    if bytes.len() > maximum {
        return Err(CanonicalMailboxTransportError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if !value.len().is_multiple_of(2) {
            return Err("hex value has odd length".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn pointer_hex(document: &Value, pointer: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        decode_hex(
            document
                .pointer(pointer)
                .and_then(Value::as_str)
                .ok_or("vector JSON pointer is missing")?,
        )
    }

    fn fixed_hex<const N: usize>(
        document: &Value,
        pointer: &str,
    ) -> Result<[u8; N], Box<dyn std::error::Error>> {
        let value = pointer_hex(document, pointer)?;
        value
            .try_into()
            .map_err(|_| format!("{pointer} has the wrong length").into())
    }

    #[test]
    fn golden_transport_frames_round_trip_and_bind_requests()
    -> Result<(), Box<dyn std::error::Error>> {
        let vectors: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-TRANSPORT-VECTORS.json"
        )))?;
        let store_key = fixed_hex(&vectors, "/input/store_key_hex")?;

        let put_peer = decode_and_verify_mailbox_peer_request_v1(&pointer_hex(
            &vectors,
            "/expected/peer_requests/put/cbor_hex",
        )?)?;
        let list_peer = decode_and_verify_mailbox_peer_request_v1(&pointer_hex(
            &vectors,
            "/expected/peer_requests/list/cbor_hex",
        )?)?;
        let delete_peer = decode_and_verify_mailbox_peer_request_v1(&pointer_hex(
            &vectors,
            "/expected/peer_requests/delete/cbor_hex",
        )?)?;
        for (request, pointer) in [
            (&put_peer, "/expected/peer_responses/put_success_cbor_hex"),
            (&list_peer, "/expected/peer_responses/list_success_cbor_hex"),
            (
                &delete_peer,
                "/expected/peer_responses/delete_success_cbor_hex",
            ),
        ] {
            let response =
                decode_and_verify_mailbox_peer_response_v1(&pointer_hex(&vectors, pointer)?)?;
            verify_mailbox_peer_response_for_v1(&response, request, &store_key)?;
            assert_eq!(
                encode_mailbox_peer_response_v1(&response)?,
                pointer_hex(&vectors, pointer)?
            );
        }

        let rejected = decode_and_verify_mailbox_peer_response_v1(&pointer_hex(
            &vectors,
            "/expected/peer_responses/put_rejected_cbor_hex",
        )?)?;
        verify_mailbox_peer_response_for_v1(&rejected, &put_peer, &store_key)?;
        Ok(())
    }

    #[test]
    fn mismatched_operation_digest_and_page_fail_closed() -> Result<(), Box<dyn std::error::Error>>
    {
        let vectors: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-MAILBOX-TRANSPORT-VECTORS.json"
        )))?;
        let store_key = fixed_hex(&vectors, "/input/store_key_hex")?;
        let put_peer = decode_and_verify_mailbox_peer_request_v1(&pointer_hex(
            &vectors,
            "/expected/peer_requests/put/cbor_hex",
        )?)?;
        let mut response = decode_and_verify_mailbox_peer_response_v1(&pointer_hex(
            &vectors,
            "/expected/peer_responses/put_success_cbor_hex",
        )?)?;
        response.request_digest[0] ^= 1;
        assert!(verify_mailbox_peer_response_for_v1(&response, &put_peer, &store_key).is_err());

        let mut unknown = pointer_hex(&vectors, "/expected/peer_requests/put/cbor_hex")?;
        unknown[2] = 4;
        assert!(decode_and_verify_mailbox_peer_request_v1(&unknown).is_err());

        let mut page = decode_and_verify_mailbox_list_response_v1(&pointer_hex(
            &vectors,
            "/expected/list_response_cbor_hex",
        )?)?;
        page.more_available = true;
        assert!(encode_mailbox_list_response_v1(&page).is_err());
        Ok(())
    }
}
