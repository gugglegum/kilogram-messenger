//! Inactive deterministic-CBOR network publication boundary for local runtime IPC.
//!
//! The publish command commits the exact current publication-channel state and
//! recipient directory summary. The runtime creates and encrypts the actual
//! ticket internally, uploads only an opaque envelope, and returns bounded
//! metadata plus the exact write authorization. The refresh command commits the
//! exact local observation head; the runtime fetches, authenticates, decrypts,
//! and atomically installs the ticket internally, then returns a Device-signed
//! observation. Ticket plaintext, HPKE ciphertext, write secrets, and local
//! filesystem paths never cross this IPC boundary.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use url::Url;

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_publication_conflict_v1::{
        CanonicalSignedTicketPublicationObservationV1,
        MAX_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS, MIN_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS,
        decode_and_verify_signed_ticket_publication_observation_v1,
        encode_signed_ticket_publication_observation_v1,
        verify_signed_ticket_publication_observation_v1,
    },
    canonical_runtime_ipc_publication_v1::{
        CanonicalRuntimeIpcPublicationChannelStateV1,
        decode_and_verify_runtime_ipc_publication_channel_state_v1,
        encode_runtime_ipc_publication_channel_state_v1,
    },
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1, verify_signed_runtime_ipc_descriptor_v1,
    },
    canonical_ticket_publication_v1::{
        CanonicalSignedTicketPublicationWriteAuthorizationV1,
        MAX_CANONICAL_TICKET_PUBLICATION_BODY_BYTES,
        decode_and_verify_signed_ticket_publication_write_authorization_v1,
        encode_signed_ticket_publication_write_authorization_v1,
        verify_signed_ticket_publication_write_authorization_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_SERVICE_ORIGIN_BYTES: usize = 512;
pub const MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RECIPIENT_DEVICES: u16 = 32;

pub const CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_SERVICE_ORIGIN_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-network-service-origin-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-network-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-network-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-publication-network-response-id:cbor-v1\0";

const COMMAND_PUBLISH_OWN_TICKET: u64 = 0;
const COMMAND_REFRESH_CONTACT_TICKET: u64 = 1;
const RESULT_OWN_TICKET_PUBLISHED: u64 = 0;
const RESULT_CONTACT_TICKET_REFRESHED: u64 = 1;
const RESULT_ERROR: u64 = 2;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const LOCAL_STATE_COMMITTED: u64 = 0;
const UPLOAD_STATE_CONFIRMED_HTTPS_SUCCESS: u64 = 0;
const PUBLISH_DELIVERY_PENDING_PEER_OBSERVATION: u64 = 0;
const INSTALL_STATE_ATOMICALLY_COMMITTED: u64 = 0;
const NETWORK_STATE_CONFIRMED_HTTPS_SUCCESS: u64 = 0;
const REFRESH_DELIVERY_OBSERVED_BY_LOCAL_DEVICE: u64 = 0;
const RESTART_NOT_REQUIRED: u64 = 0;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationNetworkPublishStateV1 {
    pub channel: CanonicalRuntimeIpcPublicationChannelStateV1,
    pub recipient_device_list_digest: [u8; 32],
    pub recipient_device_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationNetworkRefreshStateV1 {
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub publisher_device_id: [u8; 32],
    pub channel_id: [u8; 32],
    pub latest_observation_id: Option<[u8; 32]>,
    pub latest_observation_generation: u64,
    pub latest_publication_id: Option<[u8; 32]>,
    pub latest_publication_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkCommandV1 {
    PublishOwnTicket {
        expected_current: Box<CanonicalRuntimeIpcPublicationNetworkPublishStateV1>,
        service_origin_utf8: Vec<u8>,
        ttl_seconds: u64,
    },
    RefreshContactTicket {
        expected_current: Box<CanonicalRuntimeIpcPublicationNetworkRefreshStateV1>,
        service_origin_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationNetworkRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcPublicationNetworkCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkLocalStateV1 {
    Committed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkUploadStateV1 {
    ConfirmedHttpsSuccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkPublishDeliveryStateV1 {
    PendingPeerObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkInstallStateV1 {
    AtomicallyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkFetchStateV1 {
    ConfirmedHttpsSuccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkRefreshDeliveryStateV1 {
    ObservedByLocalDevice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkRestartStateV1 {
    NotRequired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnTicketPublicationV1 {
    pub outcome: CanonicalRuntimeIpcPublicationNetworkOutcomeV1,
    pub updated_channel: CanonicalRuntimeIpcPublicationChannelStateV1,
    pub published_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub ticket_digest: [u8; 32],
    pub recipient_device_list_digest: [u8; 32],
    pub recipient_device_count: u16,
    pub opaque_envelope_length: u64,
    pub opaque_envelope_digest: [u8; 32],
    pub write_authorization: CanonicalSignedTicketPublicationWriteAuthorizationV1,
    pub service_origin_utf8: Vec<u8>,
    pub local_state: CanonicalRuntimeIpcPublicationNetworkLocalStateV1,
    pub upload_state: CanonicalRuntimeIpcPublicationNetworkUploadStateV1,
    pub delivery_state: CanonicalRuntimeIpcPublicationNetworkPublishDeliveryStateV1,
    pub restart_state: CanonicalRuntimeIpcPublicationNetworkRestartStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcContactTicketRefreshV1 {
    pub outcome: CanonicalRuntimeIpcPublicationNetworkOutcomeV1,
    pub contact_id: [u8; 32],
    pub peer_account_id: [u8; 32],
    pub publisher_device_id: [u8; 32],
    pub channel_id: [u8; 32],
    pub observation: CanonicalSignedTicketPublicationObservationV1,
    pub publication_expires_at_unix_seconds: u64,
    pub service_origin_utf8: Vec<u8>,
    pub install_state: CanonicalRuntimeIpcPublicationNetworkInstallStateV1,
    pub network_state: CanonicalRuntimeIpcPublicationNetworkFetchStateV1,
    pub delivery_state: CanonicalRuntimeIpcPublicationNetworkRefreshDeliveryStateV1,
    pub restart_state: CanonicalRuntimeIpcPublicationNetworkRestartStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcPublicationNetworkResultV1 {
    OwnTicketPublished(Box<CanonicalRuntimeIpcOwnTicketPublicationV1>),
    ContactTicketRefreshed(Box<CanonicalRuntimeIpcContactTicketRefreshV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcPublicationNetworkResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcPublicationNetworkResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcPublicationNetworkResponseV1 {
    pub content: CanonicalRuntimeIpcPublicationNetworkResponseContentV1,
    pub signature: [u8; 64],
}

pub fn canonical_runtime_ipc_publication_network_service_origin_digest_v1(
    service_origin_utf8: &[u8],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    validate_service_origin(service_origin_utf8, false)?;
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_SERVICE_ORIGIN_DIGEST_DOMAIN,
        service_origin_utf8,
    ))
}

pub fn encode_runtime_ipc_publication_network_request_v1(
    request: &CanonicalRuntimeIpcPublicationNetworkRequestV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_request(request)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 7);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &request.descriptor_id);
    push_bytes(&mut output, &request.request_id);
    push_bytes(&mut output, &request.bearer_token);
    push_unsigned(&mut output, request.issued_at_unix_seconds);
    push_unsigned(&mut output, request.expires_at_unix_seconds);
    encode_command(&mut output, &request.command)?;
    ensure_bound(
        &output,
        "publication network request",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_publication_network_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcPublicationNetworkRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "publication network request",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-publication-network-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-publication-network-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcPublicationNetworkRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-publication-network-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-publication-network-request-v1.request-id")?,
        bearer_token: decoder
            .fixed_bytes("runtime-ipc-publication-network-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-publication-network-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-publication-network-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_publication_network_request_digest_v1(
    request: &CanonicalRuntimeIpcPublicationNetworkRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_publication_network_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_publication_network_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcPublicationNetworkRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_descriptor_at_v1(descriptor, now_unix_seconds)?;
    validate_request(request)?;
    if request.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || request.bearer_token != descriptor.content.bearer_token
        || request.issued_at_unix_seconds < descriptor.content.issued_at_unix_seconds
        || request.expires_at_unix_seconds > descriptor.content.expires_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if request.issued_at_unix_seconds
        > now_unix_seconds.saturating_add(CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS)
        || request.expires_at_unix_seconds < now_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::RequestNotCurrent);
    }
    let peer_account_id = match &request.command {
        CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket {
            expected_current,
            ..
        } => expected_current.channel.peer_account_id,
        CanonicalRuntimeIpcPublicationNetworkCommandV1::RefreshContactTicket {
            expected_current,
            ..
        } => expected_current.peer_account_id,
    };
    if peer_account_id == descriptor.content.account_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

pub fn runtime_ipc_publication_network_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcPublicationNetworkResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_publication_network_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcPublicationNetworkRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcPublicationNetworkResultV1,
) -> Result<CanonicalSignedRuntimeIpcPublicationNetworkResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_publication_network_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&result, request, descriptor)?;
    let content = CanonicalRuntimeIpcPublicationNetworkResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_publication_network_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_publication_network_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcPublicationNetworkResponseV1 { content, signature };
    verify_runtime_ipc_publication_network_response_for_request_v1(&response, request, descriptor)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_publication_network_response_v1(
    response: &CanonicalSignedRuntimeIpcPublicationNetworkResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_publication_network_response_v1(response)?;
    let output = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "publication network response",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_publication_network_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcPublicationNetworkResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "publication network response",
        MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-publication-network-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-publication-network-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-publication-network-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcPublicationNetworkResponseV1 { content, signature };
    verify_signed_runtime_ipc_publication_network_response_v1(&response)?;
    if encode_signed_runtime_ipc_publication_network_response_v1(&response)? != bytes {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(response)
}

pub fn verify_signed_runtime_ipc_publication_network_response_v1(
    response: &CanonicalSignedRuntimeIpcPublicationNetworkResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_publication_network_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_publication_network_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcPublicationNetworkResponseV1,
    request: &CanonicalRuntimeIpcPublicationNetworkRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_publication_network_response_v1(response)?;
    validate_request(request)?;
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest
            != runtime_ipc_publication_network_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&response.content.result, request, descriptor)
}

pub fn runtime_ipc_publication_network_response_id_v1(
    response: &CanonicalSignedRuntimeIpcPublicationNetworkResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_publication_network_response_v1(response)?,
    ))
}

pub fn encode_runtime_ipc_publication_network_publish_state_v1(
    state: &CanonicalRuntimeIpcPublicationNetworkPublishStateV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_publish_state(state)?;
    let mut output = Vec::new();
    encode_publish_state(&mut output, state)?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_publication_network_publish_state_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcPublicationNetworkPublishStateV1, CanonicalRuntimeIpcError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let state = decode_publish_state(&mut decoder)?;
    decoder.finish()?;
    if encode_runtime_ipc_publication_network_publish_state_v1(&state)? != bytes {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(state)
}

pub fn encode_runtime_ipc_publication_network_refresh_state_v1(
    state: &CanonicalRuntimeIpcPublicationNetworkRefreshStateV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_refresh_state(state)?;
    let mut output = Vec::new();
    encode_refresh_state(&mut output, state);
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_publication_network_refresh_state_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcPublicationNetworkRefreshStateV1, CanonicalRuntimeIpcError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let state = decode_refresh_state(&mut decoder)?;
    decoder.finish()?;
    if encode_runtime_ipc_publication_network_refresh_state_v1(&state)? != bytes {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(state)
}

pub fn encode_runtime_ipc_own_ticket_publication_v1(
    publication: &CanonicalRuntimeIpcOwnTicketPublicationV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_own_ticket_publication(publication)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 16);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, encode_outcome(publication.outcome));
    push_bytes(
        &mut output,
        &encode_runtime_ipc_publication_channel_state_v1(&publication.updated_channel)?,
    );
    push_unsigned(&mut output, publication.published_at_unix_seconds);
    push_unsigned(&mut output, publication.expires_at_unix_seconds);
    push_bytes(&mut output, &publication.ticket_digest);
    push_bytes(&mut output, &publication.recipient_device_list_digest);
    push_unsigned(&mut output, u64::from(publication.recipient_device_count));
    push_unsigned(&mut output, publication.opaque_envelope_length);
    push_bytes(&mut output, &publication.opaque_envelope_digest);
    push_bytes(
        &mut output,
        &encode_signed_ticket_publication_write_authorization_v1(&publication.write_authorization)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    push_bytes(&mut output, &publication.service_origin_utf8);
    push_unsigned(&mut output, encode_local_state(publication.local_state));
    push_unsigned(&mut output, encode_upload_state(publication.upload_state));
    push_unsigned(
        &mut output,
        encode_publish_delivery_state(publication.delivery_state),
    );
    push_unsigned(&mut output, encode_restart_state(publication.restart_state));
    Ok(output)
}

pub fn encode_runtime_ipc_contact_ticket_refresh_v1(
    refresh: &CanonicalRuntimeIpcContactTicketRefreshV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_contact_ticket_refresh(refresh)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 13);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_unsigned(&mut output, encode_outcome(refresh.outcome));
    push_bytes(&mut output, &refresh.contact_id);
    push_bytes(&mut output, &refresh.peer_account_id);
    push_bytes(&mut output, &refresh.publisher_device_id);
    push_bytes(&mut output, &refresh.channel_id);
    push_bytes(
        &mut output,
        &encode_signed_ticket_publication_observation_v1(&refresh.observation)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    push_unsigned(&mut output, refresh.publication_expires_at_unix_seconds);
    push_bytes(&mut output, &refresh.service_origin_utf8);
    push_unsigned(&mut output, encode_install_state(refresh.install_state));
    push_unsigned(&mut output, encode_fetch_state(refresh.network_state));
    push_unsigned(
        &mut output,
        encode_refresh_delivery_state(refresh.delivery_state),
    );
    push_unsigned(&mut output, encode_restart_state(refresh.restart_state));
    Ok(output)
}

fn validate_request(
    request: &CanonicalRuntimeIpcPublicationNetworkRequestV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let validity = request
        .expires_at_unix_seconds
        .checked_sub(request.issued_at_unix_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    if request.descriptor_id == [0; 32]
        || request.request_id == [0; 32]
        || request.bearer_token == [0; 32]
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS).contains(&validity)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    match &request.command {
        CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket {
            expected_current,
            service_origin_utf8,
            ttl_seconds,
        } => {
            validate_publish_state(expected_current)?;
            validate_service_origin(service_origin_utf8, false)?;
            if !(MIN_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS
                ..=MAX_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS)
                .contains(ttl_seconds)
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
        CanonicalRuntimeIpcPublicationNetworkCommandV1::RefreshContactTicket {
            expected_current,
            service_origin_utf8,
        } => {
            validate_refresh_state(expected_current)?;
            validate_service_origin(service_origin_utf8, false)?;
        }
    }
    Ok(())
}

fn validate_publish_state(
    state: &CanonicalRuntimeIpcPublicationNetworkPublishStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    encode_runtime_ipc_publication_channel_state_v1(&state.channel)?;
    if state.recipient_device_list_digest == [0; 32]
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RECIPIENT_DEVICES)
            .contains(&state.recipient_device_count)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_refresh_state(
    state: &CanonicalRuntimeIpcPublicationNetworkRefreshStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if state.contact_id == [0; 32]
        || state.peer_account_id == [0; 32]
        || state.publisher_device_id == [0; 32]
        || state.channel_id == [0; 32]
        || option_generation_invalid(
            state.latest_observation_id,
            state.latest_observation_generation,
        )
        || option_generation_invalid(
            state.latest_publication_id,
            state.latest_publication_generation,
        )
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    VerifyingKey::from_bytes(&state.publisher_device_id)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcPublicationNetworkResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcPublicationNetworkResultV1::OwnTicketPublished(publication) => {
            validate_own_ticket_publication(publication)
        }
        CanonicalRuntimeIpcPublicationNetworkResultV1::ContactTicketRefreshed(refresh) => {
            validate_contact_ticket_refresh(refresh)
        }
        CanonicalRuntimeIpcPublicationNetworkResultV1::Error { code, message_utf8 } => {
            if *code == 0
                || message_utf8.is_empty()
                || message_utf8.len() > MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES
                || std::str::from_utf8(message_utf8).is_err()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
    }
}

fn validate_own_ticket_publication(
    publication: &CanonicalRuntimeIpcOwnTicketPublicationV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    encode_runtime_ipc_publication_channel_state_v1(&publication.updated_channel)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    verify_signed_ticket_publication_write_authorization_v1(&publication.write_authorization)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    validate_service_origin(&publication.service_origin_utf8, true)?;
    let lifetime = publication
        .expires_at_unix_seconds
        .checked_sub(publication.published_at_unix_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
    if !(MIN_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS
        ..=MAX_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS)
        .contains(&lifetime)
        || publication.ticket_digest == [0; 32]
        || publication.recipient_device_list_digest == [0; 32]
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RECIPIENT_DEVICES)
            .contains(&publication.recipient_device_count)
        || !(1..=MAX_CANONICAL_TICKET_PUBLICATION_BODY_BYTES)
            .contains(&publication.opaque_envelope_length)
        || publication.opaque_envelope_digest == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let authorization = &publication.write_authorization.content;
    if authorization.capability != publication.updated_channel.capability
        || authorization.generation != publication.updated_channel.latest_publication_generation
        || authorization.body_length != publication.opaque_envelope_length
        || authorization.body_digest != publication.opaque_envelope_digest
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_contact_ticket_refresh(
    refresh: &CanonicalRuntimeIpcContactTicketRefreshV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_service_origin(&refresh.service_origin_utf8, true)?;
    verify_signed_ticket_publication_observation_v1(&refresh.observation)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if refresh.contact_id == [0; 32]
        || refresh.peer_account_id == [0; 32]
        || refresh.publisher_device_id == [0; 32]
        || refresh.channel_id == [0; 32]
        || refresh.observation.content.publisher_account_id != refresh.peer_account_id
        || refresh.observation.content.publisher_device_id != refresh.publisher_device_id
        || refresh.observation.content.channel_id != refresh.channel_id
        || refresh.publication_expires_at_unix_seconds
            <= refresh.observation.content.observed_at_unix_seconds
        || refresh
            .publication_expires_at_unix_seconds
            .saturating_sub(refresh.observation.content.observed_at_unix_seconds)
            > MAX_CANONICAL_TICKET_PUBLICATION_TTL_SECONDS
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_result_for_request(
    result: &CanonicalRuntimeIpcPublicationNetworkResultV1,
    request: &CanonicalRuntimeIpcPublicationNetworkRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match (result, &request.command) {
        (
            CanonicalRuntimeIpcPublicationNetworkResultV1::OwnTicketPublished(publication),
            CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket {
                expected_current,
                service_origin_utf8,
                ttl_seconds,
            },
        ) => {
            validate_own_ticket_publication(publication)?;
            let old = &expected_current.channel;
            let new = &publication.updated_channel;
            let expected_generation = old
                .latest_publication_generation
                .checked_add(1)
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
            if new.contact_id != old.contact_id
                || new.peer_account_id != old.peer_account_id
                || new.peer_scope_digest != old.peer_scope_digest
                || new.channel_epoch != old.channel_epoch
                || new.capability != old.capability
                || new.latest_publication_generation != expected_generation
                || new.latest_publication_id.is_none()
                || new.latest_publication_id == old.latest_publication_id
                || publication.published_at_unix_seconds < request.issued_at_unix_seconds
                || publication.published_at_unix_seconds > request.expires_at_unix_seconds
                || publication.expires_at_unix_seconds
                    != publication
                        .published_at_unix_seconds
                        .checked_add(*ttl_seconds)
                        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?
                || publication.recipient_device_list_digest
                    != expected_current.recipient_device_list_digest
                || publication.recipient_device_count != expected_current.recipient_device_count
                || publication.service_origin_utf8 != *service_origin_utf8
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (
            CanonicalRuntimeIpcPublicationNetworkResultV1::ContactTicketRefreshed(refresh),
            CanonicalRuntimeIpcPublicationNetworkCommandV1::RefreshContactTicket {
                expected_current,
                service_origin_utf8,
            },
        ) => {
            validate_contact_ticket_refresh(refresh)?;
            let observation = &refresh.observation.content;
            let expected_observation_generation = expected_current
                .latest_observation_generation
                .checked_add(1)
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
            if refresh.contact_id != expected_current.contact_id
                || refresh.peer_account_id != expected_current.peer_account_id
                || refresh.publisher_device_id != expected_current.publisher_device_id
                || refresh.channel_id != expected_current.channel_id
                || refresh.service_origin_utf8 != *service_origin_utf8
                || observation.local_account_id != descriptor.content.account_id
                || observation.local_device_id != descriptor.content.device_id
                || observation.observation_generation != expected_observation_generation
                || observation.previous_observation_id != expected_current.latest_observation_id
                || observation.publication_generation
                    <= expected_current.latest_publication_generation
                || Some(observation.publication_id) == expected_current.latest_publication_id
                || observation.observed_at_unix_seconds < request.issued_at_unix_seconds
                || observation.observed_at_unix_seconds > request.expires_at_unix_seconds
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (CanonicalRuntimeIpcPublicationNetworkResultV1::Error { .. }, _) => Ok(()),
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

fn encode_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcPublicationNetworkCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket {
            expected_current,
            service_origin_utf8,
            ttl_seconds,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_PUBLISH_OWN_TICKET);
            push_bytes(
                output,
                &encode_runtime_ipc_publication_network_publish_state_v1(expected_current)?,
            );
            push_bytes(output, service_origin_utf8);
            push_unsigned(output, *ttl_seconds);
        }
        CanonicalRuntimeIpcPublicationNetworkCommandV1::RefreshContactTicket {
            expected_current,
            service_origin_utf8,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_REFRESH_CONTACT_TICKET);
            push_bytes(
                output,
                &encode_runtime_ipc_publication_network_refresh_state_v1(expected_current)?,
            );
            push_bytes(output, service_origin_utf8);
        }
    }
    Ok(())
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationNetworkCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-publication-network-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-publication-network-command-v1.variant")?,
        length,
    ) {
        (COMMAND_PUBLISH_OWN_TICKET, 4) => Ok(
            CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket {
                expected_current: Box::new(
                    decode_and_verify_runtime_ipc_publication_network_publish_state_v1(
                        decoder.bytes(
                            "runtime-ipc-publication-network-command-v1.expected-publish-state",
                        )?,
                    )?,
                ),
                service_origin_utf8: decoder
                    .bytes("runtime-ipc-publication-network-command-v1.service-origin-utf8")?
                    .to_vec(),
                ttl_seconds: decoder
                    .unsigned("runtime-ipc-publication-network-command-v1.ttl-seconds")?,
            },
        ),
        (COMMAND_REFRESH_CONTACT_TICKET, 3) => Ok(
            CanonicalRuntimeIpcPublicationNetworkCommandV1::RefreshContactTicket {
                expected_current: Box::new(
                    decode_and_verify_runtime_ipc_publication_network_refresh_state_v1(
                        decoder.bytes(
                            "runtime-ipc-publication-network-command-v1.expected-refresh-state",
                        )?,
                    )?,
                ),
                service_origin_utf8: decoder
                    .bytes("runtime-ipc-publication-network-command-v1.service-origin-utf8")?
                    .to_vec(),
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_publish_state(
    output: &mut Vec<u8>,
    state: &CanonicalRuntimeIpcPublicationNetworkPublishStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 4);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(
        output,
        &encode_runtime_ipc_publication_channel_state_v1(&state.channel)?,
    );
    push_bytes(output, &state.recipient_device_list_digest);
    push_unsigned(output, u64::from(state.recipient_device_count));
    Ok(())
}

fn decode_publish_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationNetworkPublishStateV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-network-publish-state-v1", 4)?;
    decoder.expect_version("runtime-ipc-publication-network-publish-state-v1.schema-version")?;
    let state =
        CanonicalRuntimeIpcPublicationNetworkPublishStateV1 {
            channel: decode_and_verify_runtime_ipc_publication_channel_state_v1(
                decoder.bytes("runtime-ipc-publication-network-publish-state-v1.channel")?,
            )?,
            recipient_device_list_digest: decoder.fixed_bytes(
                "runtime-ipc-publication-network-publish-state-v1.recipient-device-list-digest",
            )?,
            recipient_device_count: u16::try_from(decoder.unsigned(
                "runtime-ipc-publication-network-publish-state-v1.recipient-device-count",
            )?)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
        };
    validate_publish_state(&state)?;
    Ok(state)
}

fn encode_refresh_state(
    output: &mut Vec<u8>,
    state: &CanonicalRuntimeIpcPublicationNetworkRefreshStateV1,
) {
    push_array_header(output, 9);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &state.contact_id);
    push_bytes(output, &state.peer_account_id);
    push_bytes(output, &state.publisher_device_id);
    push_bytes(output, &state.channel_id);
    encode_optional_digest(output, state.latest_observation_id);
    push_unsigned(output, state.latest_observation_generation);
    encode_optional_digest(output, state.latest_publication_id);
    push_unsigned(output, state.latest_publication_generation);
}

fn decode_refresh_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationNetworkRefreshStateV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-network-refresh-state-v1", 9)?;
    decoder.expect_version("runtime-ipc-publication-network-refresh-state-v1.schema-version")?;
    let state = CanonicalRuntimeIpcPublicationNetworkRefreshStateV1 {
        contact_id: decoder
            .fixed_bytes("runtime-ipc-publication-network-refresh-state-v1.contact-id")?,
        peer_account_id: decoder
            .fixed_bytes("runtime-ipc-publication-network-refresh-state-v1.peer-account-id")?,
        publisher_device_id: decoder
            .fixed_bytes("runtime-ipc-publication-network-refresh-state-v1.publisher-device-id")?,
        channel_id: decoder
            .fixed_bytes("runtime-ipc-publication-network-refresh-state-v1.channel-id")?,
        latest_observation_id: decode_optional_digest(
            decoder,
            "runtime-ipc-publication-network-refresh-state-v1.latest-observation-id",
            CanonicalRuntimeIpcError::InvalidRequest,
        )?,
        latest_observation_generation: decoder.unsigned(
            "runtime-ipc-publication-network-refresh-state-v1.latest-observation-generation",
        )?,
        latest_publication_id: decode_optional_digest(
            decoder,
            "runtime-ipc-publication-network-refresh-state-v1.latest-publication-id",
            CanonicalRuntimeIpcError::InvalidRequest,
        )?,
        latest_publication_generation: decoder.unsigned(
            "runtime-ipc-publication-network-refresh-state-v1.latest-publication-generation",
        )?,
    };
    validate_refresh_state(&state)?;
    Ok(state)
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcPublicationNetworkResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_result(&mut output, &content.result)?;
    Ok(output)
}

fn decode_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationNetworkResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-publication-network-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-publication-network-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcPublicationNetworkResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-publication-network-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-publication-network-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-publication-network-response-content-v1.request-digest")?,
        responder_device_id: decoder.fixed_bytes(
            "runtime-ipc-publication-network-response-content-v1.responder-device-id",
        )?,
        result: decode_result(decoder)?,
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn encode_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcPublicationNetworkResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcPublicationNetworkResultV1::OwnTicketPublished(publication) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_OWN_TICKET_PUBLISHED);
            push_bytes(
                output,
                &encode_runtime_ipc_own_ticket_publication_v1(publication)?,
            );
        }
        CanonicalRuntimeIpcPublicationNetworkResultV1::ContactTicketRefreshed(refresh) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_CONTACT_TICKET_REFRESHED);
            push_bytes(
                output,
                &encode_runtime_ipc_contact_ticket_refresh_v1(refresh)?,
            );
        }
        CanonicalRuntimeIpcPublicationNetworkResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
    Ok(())
}

fn decode_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcPublicationNetworkResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-publication-network-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-publication-network-result-v1.variant")?,
        length,
    ) {
        (RESULT_OWN_TICKET_PUBLISHED, 2) => {
            let bytes = decoder.bytes("runtime-ipc-publication-network-result-v1.publication")?;
            validate_canonical_profile_v1(bytes)?;
            let mut nested = AccountDecoder::new(bytes);
            let publication = decode_own_ticket_publication(&mut nested)?;
            nested.finish()?;
            if encode_runtime_ipc_own_ticket_publication_v1(&publication)? != bytes {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(
                CanonicalRuntimeIpcPublicationNetworkResultV1::OwnTicketPublished(Box::new(
                    publication,
                )),
            )
        }
        (RESULT_CONTACT_TICKET_REFRESHED, 2) => {
            let bytes = decoder.bytes("runtime-ipc-publication-network-result-v1.refresh")?;
            validate_canonical_profile_v1(bytes)?;
            let mut nested = AccountDecoder::new(bytes);
            let refresh = decode_contact_ticket_refresh(&mut nested)?;
            nested.finish()?;
            if encode_runtime_ipc_contact_ticket_refresh_v1(&refresh)? != bytes {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(
                CanonicalRuntimeIpcPublicationNetworkResultV1::ContactTicketRefreshed(Box::new(
                    refresh,
                )),
            )
        }
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcPublicationNetworkResultV1::Error {
            code: u16::try_from(
                decoder.unsigned("runtime-ipc-publication-network-result-v1.error-code")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-publication-network-result-v1.error-message-utf8")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_own_ticket_publication(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnTicketPublicationV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-ticket-publication-v1", 16)?;
    decoder.expect_version("runtime-ipc-own-ticket-publication-v1.schema-version")?;
    let publication = CanonicalRuntimeIpcOwnTicketPublicationV1 {
        outcome: decode_outcome(
            decoder.unsigned("runtime-ipc-own-ticket-publication-v1.outcome")?,
        )?,
        updated_channel: decode_and_verify_runtime_ipc_publication_channel_state_v1(
            decoder.bytes("runtime-ipc-own-ticket-publication-v1.updated-channel")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        published_at_unix_seconds: decoder
            .unsigned("runtime-ipc-own-ticket-publication-v1.published-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-own-ticket-publication-v1.expires-at-unix-seconds")?,
        ticket_digest: decoder
            .fixed_bytes("runtime-ipc-own-ticket-publication-v1.ticket-digest")?,
        recipient_device_list_digest: decoder
            .fixed_bytes("runtime-ipc-own-ticket-publication-v1.recipient-device-list-digest")?,
        recipient_device_count: u16::try_from(
            decoder.unsigned("runtime-ipc-own-ticket-publication-v1.recipient-device-count")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        opaque_envelope_length: decoder
            .unsigned("runtime-ipc-own-ticket-publication-v1.opaque-envelope-length")?,
        opaque_envelope_digest: decoder
            .fixed_bytes("runtime-ipc-own-ticket-publication-v1.opaque-envelope-digest")?,
        write_authorization: decode_and_verify_signed_ticket_publication_write_authorization_v1(
            decoder.bytes("runtime-ipc-own-ticket-publication-v1.write-authorization")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        service_origin_utf8: decoder
            .bytes("runtime-ipc-own-ticket-publication-v1.service-origin-utf8")?
            .to_vec(),
        local_state: decode_local_state(
            decoder.unsigned("runtime-ipc-own-ticket-publication-v1.local-state")?,
        )?,
        upload_state: decode_upload_state(
            decoder.unsigned("runtime-ipc-own-ticket-publication-v1.upload-state")?,
        )?,
        delivery_state: decode_publish_delivery_state(
            decoder.unsigned("runtime-ipc-own-ticket-publication-v1.delivery-state")?,
        )?,
        restart_state: decode_restart_state(
            decoder.unsigned("runtime-ipc-own-ticket-publication-v1.restart-state")?,
        )?,
    };
    validate_own_ticket_publication(&publication)?;
    Ok(publication)
}

fn decode_contact_ticket_refresh(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcContactTicketRefreshV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-contact-ticket-refresh-v1", 13)?;
    decoder.expect_version("runtime-ipc-contact-ticket-refresh-v1.schema-version")?;
    let refresh = CanonicalRuntimeIpcContactTicketRefreshV1 {
        outcome: decode_outcome(
            decoder.unsigned("runtime-ipc-contact-ticket-refresh-v1.outcome")?,
        )?,
        contact_id: decoder.fixed_bytes("runtime-ipc-contact-ticket-refresh-v1.contact-id")?,
        peer_account_id: decoder
            .fixed_bytes("runtime-ipc-contact-ticket-refresh-v1.peer-account-id")?,
        publisher_device_id: decoder
            .fixed_bytes("runtime-ipc-contact-ticket-refresh-v1.publisher-device-id")?,
        channel_id: decoder.fixed_bytes("runtime-ipc-contact-ticket-refresh-v1.channel-id")?,
        observation: decode_and_verify_signed_ticket_publication_observation_v1(
            decoder.bytes("runtime-ipc-contact-ticket-refresh-v1.observation")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        publication_expires_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-contact-ticket-refresh-v1.publication-expires-at-unix-seconds",
        )?,
        service_origin_utf8: decoder
            .bytes("runtime-ipc-contact-ticket-refresh-v1.service-origin-utf8")?
            .to_vec(),
        install_state: decode_install_state(
            decoder.unsigned("runtime-ipc-contact-ticket-refresh-v1.install-state")?,
        )?,
        network_state: decode_fetch_state(
            decoder.unsigned("runtime-ipc-contact-ticket-refresh-v1.network-state")?,
        )?,
        delivery_state: decode_refresh_delivery_state(
            decoder.unsigned("runtime-ipc-contact-ticket-refresh-v1.delivery-state")?,
        )?,
        restart_state: decode_restart_state(
            decoder.unsigned("runtime-ipc-contact-ticket-refresh-v1.restart-state")?,
        )?,
    };
    validate_contact_ticket_refresh(&refresh)?;
    Ok(refresh)
}

fn validate_service_origin(value: &[u8], response: bool) -> Result<(), CanonicalRuntimeIpcError> {
    let invalid = || {
        if response {
            CanonicalRuntimeIpcError::InvalidResponse
        } else {
            CanonicalRuntimeIpcError::InvalidRequest
        }
    };
    if value.is_empty() || value.len() > MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_SERVICE_ORIGIN_BYTES
    {
        return Err(invalid());
    }
    let text = std::str::from_utf8(value).map_err(|_| invalid())?;
    let parsed = Url::parse(text).map_err(|_| invalid())?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
        || parsed.as_str().as_bytes() != value
    {
        return Err(invalid());
    }
    Ok(())
}

fn option_generation_invalid(value: Option<[u8; 32]>, generation: u64) -> bool {
    (value.is_none() && generation != 0)
        || (value.is_some_and(|value| value == [0; 32]))
        || (value.is_some() && generation == 0)
}

fn encode_optional_digest(output: &mut Vec<u8>, value: Option<[u8; 32]>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(output, &value);
        }
    }
}

fn decode_optional_digest(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
    invalid: CanonicalRuntimeIpcError,
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array(field)?;
    match (decoder.unsigned(field)?, length) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(decoder.fixed_bytes(field)?)),
        _ => Err(invalid),
    }
}

fn encode_outcome(value: CanonicalRuntimeIpcPublicationNetworkOutcomeV1) -> u64 {
    match value {
        CanonicalRuntimeIpcPublicationNetworkOutcomeV1::Committed => OUTCOME_COMMITTED,
        CanonicalRuntimeIpcPublicationNetworkOutcomeV1::AlreadyCommitted => {
            OUTCOME_ALREADY_COMMITTED
        }
    }
}

fn decode_outcome(
    value: u64,
) -> Result<CanonicalRuntimeIpcPublicationNetworkOutcomeV1, CanonicalRuntimeIpcError> {
    match value {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcPublicationNetworkOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcPublicationNetworkOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

macro_rules! fixed_state_codec {
    ($encode:ident, $decode:ident, $ty:ty, $variant:path, $wire:expr) => {
        fn $encode(value: $ty) -> u64 {
            match value {
                $variant => $wire,
            }
        }

        fn $decode(value: u64) -> Result<$ty, CanonicalRuntimeIpcError> {
            match value {
                candidate if candidate == $wire => Ok($variant),
                _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
            }
        }
    };
}

fixed_state_codec!(
    encode_local_state,
    decode_local_state,
    CanonicalRuntimeIpcPublicationNetworkLocalStateV1,
    CanonicalRuntimeIpcPublicationNetworkLocalStateV1::Committed,
    LOCAL_STATE_COMMITTED
);
fixed_state_codec!(
    encode_upload_state,
    decode_upload_state,
    CanonicalRuntimeIpcPublicationNetworkUploadStateV1,
    CanonicalRuntimeIpcPublicationNetworkUploadStateV1::ConfirmedHttpsSuccess,
    UPLOAD_STATE_CONFIRMED_HTTPS_SUCCESS
);
fixed_state_codec!(
    encode_publish_delivery_state,
    decode_publish_delivery_state,
    CanonicalRuntimeIpcPublicationNetworkPublishDeliveryStateV1,
    CanonicalRuntimeIpcPublicationNetworkPublishDeliveryStateV1::PendingPeerObservation,
    PUBLISH_DELIVERY_PENDING_PEER_OBSERVATION
);
fixed_state_codec!(
    encode_install_state,
    decode_install_state,
    CanonicalRuntimeIpcPublicationNetworkInstallStateV1,
    CanonicalRuntimeIpcPublicationNetworkInstallStateV1::AtomicallyCommitted,
    INSTALL_STATE_ATOMICALLY_COMMITTED
);
fixed_state_codec!(
    encode_fetch_state,
    decode_fetch_state,
    CanonicalRuntimeIpcPublicationNetworkFetchStateV1,
    CanonicalRuntimeIpcPublicationNetworkFetchStateV1::ConfirmedHttpsSuccess,
    NETWORK_STATE_CONFIRMED_HTTPS_SUCCESS
);
fixed_state_codec!(
    encode_refresh_delivery_state,
    decode_refresh_delivery_state,
    CanonicalRuntimeIpcPublicationNetworkRefreshDeliveryStateV1,
    CanonicalRuntimeIpcPublicationNetworkRefreshDeliveryStateV1::ObservedByLocalDevice,
    REFRESH_DELIVERY_OBSERVED_BY_LOCAL_DEVICE
);
fixed_state_codec!(
    encode_restart_state,
    decode_restart_state,
    CanonicalRuntimeIpcPublicationNetworkRestartStateV1,
    CanonicalRuntimeIpcPublicationNetworkRestartStateV1::NotRequired,
    RESTART_NOT_REQUIRED
);

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalRuntimeIpcError> {
    if bytes.len() > maximum {
        return Err(CanonicalRuntimeIpcError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalRuntimeIpcError> {
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalRuntimeIpcError::InvalidSignature)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut input = Vec::with_capacity(domain.len() + bytes.len());
    input.extend_from_slice(domain);
    input.extend_from_slice(bytes);
    *blake3::hash(&input).as_bytes()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::{Value, json};

    use super::*;
    use crate::{
        canonical_publication_conflict_v1::{
            CanonicalSignedTicketPublicationV1, encode_signed_ticket_publication_v1,
            sign_ticket_publication_observation_v1, sign_ticket_publication_v1,
            ticket_publication_id_v1,
        },
        canonical_runtime_ipc_contact_v1::canonical_runtime_ipc_contact_id_v1,
        canonical_runtime_ipc_publication_v1::canonical_runtime_ipc_publication_peer_scope_digest_v1,
        canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1,
        canonical_ticket_publication_v1::{
            derive_ticket_publication_capability_v1, sign_ticket_publication_write_authorization_v1,
        },
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        device_secret: [u8; 32],
        peer_scope: Vec<u8>,
        service_origin: Vec<u8>,
        opaque_envelope: Vec<u8>,
        publish_inner_publication: CanonicalSignedTicketPublicationV1,
        publish_request: CanonicalRuntimeIpcPublicationNetworkRequestV1,
        publish_result: CanonicalRuntimeIpcOwnTicketPublicationV1,
        refresh_inner_publication: CanonicalSignedTicketPublicationV1,
        refresh_request: CanonicalRuntimeIpcPublicationNetworkRequestV1,
        refresh_result: CanonicalRuntimeIpcContactTicketRefreshV1,
    }

    fn dependencies() -> TestResult<(Value, Value)> {
        Ok((
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json"
            )))?,
        ))
    }

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json"
        )))?)
    }

    fn bytes(document: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        let value = document
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing string at {pointer}"))?;
        if !value.len().is_multiple_of(2) {
            return Err(format!("odd hex length at {pointer}").into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed<const N: usize>(document: &Value, pointer: &str) -> TestResult<[u8; N]> {
        Ok(bytes(document, pointer)?
            .try_into()
            .map_err(|_| format!("wrong byte length at {pointer}"))?)
    }

    fn fixture() -> TestResult<Fixture> {
        let (core, contact) = dependencies()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&bytes(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let device_secret = fixed(&core, "/input/device_secret_key_hex")?;
        let device_key = SigningKey::from_bytes(&device_secret);
        let peer_account_id = fixed(&contact, "/input/peer_account_id_hex")?;
        let peer_scope = b"m1138-publication-network-peer-scope".to_vec();
        let service_origin = b"https://tickets.example/".to_vec();
        let contact_id = canonical_runtime_ipc_contact_id_v1(
            &descriptor.content.account_id,
            &peer_account_id,
            b"m1138-network-publish",
        )?;
        let capability = derive_ticket_publication_capability_v1(device_secret, &peer_scope, 4)?;
        let publish_state = CanonicalRuntimeIpcPublicationNetworkPublishStateV1 {
            channel: CanonicalRuntimeIpcPublicationChannelStateV1 {
                contact_id,
                peer_account_id,
                peer_scope_digest: canonical_runtime_ipc_publication_peer_scope_digest_v1(
                    &peer_scope,
                )?,
                channel_epoch: 4,
                capability: capability.clone(),
                latest_publication_id: None,
                latest_publication_generation: 0,
            },
            recipient_device_list_digest: [0xa4; 32],
            recipient_device_count: 2,
        };
        let publish_request = CanonicalRuntimeIpcPublicationNetworkRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0x94; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: 1_800_002_190,
            expires_at_unix_seconds: 1_800_002_220,
            command: CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket {
                expected_current: Box::new(publish_state.clone()),
                service_origin_utf8: service_origin.clone(),
                ttl_seconds: 900,
            },
        };
        let signed_publication = sign_ticket_publication_v1(
            &device_key,
            capability.channel_id,
            descriptor.content.account_id,
            peer_account_id,
            b"m1138-private-ticket-payload".to_vec(),
            1_800_002_194,
            900,
            None,
        )?;
        let publication_id = ticket_publication_id_v1(&signed_publication)?;
        let opaque_envelope = b"m1138-opaque-hpke-envelope-fixture-only".to_vec();
        let write_authorization = sign_ticket_publication_write_authorization_v1(
            device_secret,
            &peer_scope,
            4,
            1,
            &opaque_envelope,
        )?;
        let mut updated_channel = publish_state.channel;
        updated_channel.latest_publication_id = Some(publication_id);
        updated_channel.latest_publication_generation = 1;
        let publish_result = CanonicalRuntimeIpcOwnTicketPublicationV1 {
            outcome: CanonicalRuntimeIpcPublicationNetworkOutcomeV1::Committed,
            updated_channel,
            published_at_unix_seconds: signed_publication.content.published_at_unix_seconds,
            expires_at_unix_seconds: signed_publication.content.expires_at_unix_seconds,
            ticket_digest: signed_publication.content.ticket_digest,
            recipient_device_list_digest: [0xa4; 32],
            recipient_device_count: 2,
            opaque_envelope_length: u64::try_from(opaque_envelope.len())?,
            opaque_envelope_digest: *blake3::hash(&opaque_envelope).as_bytes(),
            write_authorization,
            service_origin_utf8: service_origin.clone(),
            local_state: CanonicalRuntimeIpcPublicationNetworkLocalStateV1::Committed,
            upload_state: CanonicalRuntimeIpcPublicationNetworkUploadStateV1::ConfirmedHttpsSuccess,
            delivery_state:
                CanonicalRuntimeIpcPublicationNetworkPublishDeliveryStateV1::PendingPeerObservation,
            restart_state: CanonicalRuntimeIpcPublicationNetworkRestartStateV1::NotRequired,
        };

        let peer_device_secret: [u8; 32] = std::array::from_fn(|index| 0x40 + index as u8);
        let peer_device_key = SigningKey::from_bytes(&peer_device_secret);
        let refresh_capability = derive_ticket_publication_capability_v1(
            peer_device_secret,
            b"m1138-refresh-peer-scope",
            2,
        )?;
        let refresh_contact_id = canonical_runtime_ipc_contact_id_v1(
            &descriptor.content.account_id,
            &peer_account_id,
            b"m1138-network-refresh",
        )?;
        let fetched_publication = sign_ticket_publication_v1(
            &peer_device_key,
            refresh_capability.channel_id,
            peer_account_id,
            descriptor.content.account_id,
            b"m1138-peer-private-ticket".to_vec(),
            1_800_002_200,
            900,
            None,
        )?;
        let refresh_request = CanonicalRuntimeIpcPublicationNetworkRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0x95; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: 1_800_002_205,
            expires_at_unix_seconds: 1_800_002_235,
            command: CanonicalRuntimeIpcPublicationNetworkCommandV1::RefreshContactTicket {
                expected_current: Box::new(CanonicalRuntimeIpcPublicationNetworkRefreshStateV1 {
                    contact_id: refresh_contact_id,
                    peer_account_id,
                    publisher_device_id: peer_device_key.verifying_key().to_bytes(),
                    channel_id: refresh_capability.channel_id,
                    latest_observation_id: None,
                    latest_observation_generation: 0,
                    latest_publication_id: None,
                    latest_publication_generation: 0,
                }),
                service_origin_utf8: service_origin.clone(),
            },
        };
        let observation = sign_ticket_publication_observation_v1(
            &device_key,
            descriptor.content.account_id,
            &fetched_publication,
            1_800_002_208,
            None,
        )?;
        let refresh_result = CanonicalRuntimeIpcContactTicketRefreshV1 {
            outcome: CanonicalRuntimeIpcPublicationNetworkOutcomeV1::Committed,
            contact_id: refresh_contact_id,
            peer_account_id,
            publisher_device_id: peer_device_key.verifying_key().to_bytes(),
            channel_id: refresh_capability.channel_id,
            observation,
            publication_expires_at_unix_seconds: fetched_publication
                .content
                .expires_at_unix_seconds,
            service_origin_utf8: service_origin.clone(),
            install_state: CanonicalRuntimeIpcPublicationNetworkInstallStateV1::AtomicallyCommitted,
            network_state: CanonicalRuntimeIpcPublicationNetworkFetchStateV1::ConfirmedHttpsSuccess,
            delivery_state:
                CanonicalRuntimeIpcPublicationNetworkRefreshDeliveryStateV1::ObservedByLocalDevice,
            restart_state: CanonicalRuntimeIpcPublicationNetworkRestartStateV1::NotRequired,
        };

        Ok(Fixture {
            descriptor,
            device_key,
            device_secret,
            peer_scope,
            service_origin,
            opaque_envelope,
            publish_inner_publication: signed_publication,
            publish_request,
            publish_result,
            refresh_inner_publication: fetched_publication,
            refresh_request,
            refresh_result,
        })
    }

    fn publish_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcPublicationNetworkResponseV1, CanonicalRuntimeIpcError>
    {
        sign_runtime_ipc_publication_network_response_v1(
            &fixture.device_key,
            &fixture.publish_request,
            &fixture.descriptor,
            fixture.publish_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcPublicationNetworkResultV1::OwnTicketPublished(Box::new(
                fixture.publish_result.clone(),
            )),
        )
    }

    fn refresh_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcPublicationNetworkResponseV1, CanonicalRuntimeIpcError>
    {
        sign_runtime_ipc_publication_network_response_v1(
            &fixture.device_key,
            &fixture.refresh_request,
            &fixture.descriptor,
            fixture.refresh_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcPublicationNetworkResultV1::ContactTicketRefreshed(Box::new(
                fixture.refresh_result.clone(),
            )),
        )
    }

    #[test]
    fn canonical_runtime_ipc_publication_network_round_trips() -> TestResult {
        let fixture = fixture()?;
        for request in [&fixture.publish_request, &fixture.refresh_request] {
            verify_runtime_ipc_publication_network_request_for_descriptor_v1(
                request,
                &fixture.descriptor,
                request.issued_at_unix_seconds,
            )?;
            let encoded = encode_runtime_ipc_publication_network_request_v1(request)?;
            assert_eq!(
                decode_and_verify_runtime_ipc_publication_network_request_v1(&encoded)?,
                *request
            );
        }
        for response in [publish_response(&fixture)?, refresh_response(&fixture)?] {
            let encoded = encode_signed_runtime_ipc_publication_network_response_v1(&response)?;
            assert_eq!(
                decode_and_verify_signed_runtime_ipc_publication_network_response_v1(&encoded)?,
                response
            );
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_publication_network_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let vectors = vectors()?;
        let publish_response = publish_response(&fixture)?;
        let refresh_response = refresh_response(&fixture)?;
        assert_eq!(
            encode_runtime_ipc_publication_network_request_v1(&fixture.publish_request)?,
            bytes(&vectors, "/expected/publish/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_network_request_digest_v1(&fixture.publish_request)?,
            fixed(&vectors, "/expected/publish/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_publication_network_response_v1(&publish_response)?,
            bytes(&vectors, "/expected/publish/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_network_response_id_v1(&publish_response)?,
            fixed(&vectors, "/expected/publish/response/object_id_hex")?
        );
        assert_eq!(
            encode_runtime_ipc_publication_network_request_v1(&fixture.refresh_request)?,
            bytes(&vectors, "/expected/refresh/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_network_request_digest_v1(&fixture.refresh_request)?,
            fixed(&vectors, "/expected/refresh/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_publication_network_response_v1(&refresh_response)?,
            bytes(&vectors, "/expected/refresh/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_publication_network_response_id_v1(&refresh_response)?,
            fixed(&vectors, "/expected/refresh/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_publication_network_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let publish_response = publish_response(&fixture)?;
        let refresh_response = refresh_response(&fixture)?;

        for invalid in [
            b"http://tickets.example/".as_slice(),
            b"https://tickets.example/path".as_slice(),
            b"https://tickets.example/?query=1".as_slice(),
            b"https://TICKETS.example/".as_slice(),
        ] {
            let mut request = fixture.publish_request.clone();
            let CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket {
                service_origin_utf8,
                ..
            } = &mut request.command
            else {
                unreachable!();
            };
            *service_origin_utf8 = invalid.to_vec();
            assert!(encode_runtime_ipc_publication_network_request_v1(&request).is_err());
        }

        let mut changed_request = fixture.publish_request.clone();
        changed_request.request_id[0] ^= 1;
        assert!(
            verify_runtime_ipc_publication_network_response_for_request_v1(
                &publish_response,
                &changed_request,
                &fixture.descriptor,
            )
            .is_err()
        );

        let mut stale_head = fixture.publish_result.clone();
        stale_head.updated_channel.latest_publication_generation = 2;
        assert!(
            sign_runtime_ipc_publication_network_response_v1(
                &fixture.device_key,
                &fixture.publish_request,
                &fixture.descriptor,
                fixture.publish_request.issued_at_unix_seconds,
                CanonicalRuntimeIpcPublicationNetworkResultV1::OwnTicketPublished(Box::new(
                    stale_head,
                )),
            )
            .is_err()
        );

        let mut wrong_observation = fixture.refresh_result.clone();
        wrong_observation.observation.content.local_account_id[0] ^= 1;
        assert!(
            sign_runtime_ipc_publication_network_response_v1(
                &fixture.device_key,
                &fixture.refresh_request,
                &fixture.descriptor,
                fixture.refresh_request.issued_at_unix_seconds,
                CanonicalRuntimeIpcPublicationNetworkResultV1::ContactTicketRefreshed(Box::new(
                    wrong_observation,
                )),
            )
            .is_err()
        );

        assert!(
            verify_runtime_ipc_publication_network_response_for_request_v1(
                &refresh_response,
                &fixture.publish_request,
                &fixture.descriptor,
            )
            .is_err()
        );
        Ok(())
    }

    fn lower_hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(DIGITS[usize::from(byte >> 4)]));
            output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        output
    }

    #[test]
    #[ignore = "explicit deterministic vector regeneration"]
    fn regenerate_canonical_runtime_ipc_publication_network_vectors() -> TestResult {
        let fixture = fixture()?;
        let publish_request =
            encode_runtime_ipc_publication_network_request_v1(&fixture.publish_request)?;
        let publish_response = publish_response(&fixture)?;
        let publish_response_content = encode_response_content(&publish_response.content)?;
        let publish_response_bytes =
            encode_signed_runtime_ipc_publication_network_response_v1(&publish_response)?;
        let refresh_request =
            encode_runtime_ipc_publication_network_request_v1(&fixture.refresh_request)?;
        let refresh_response = refresh_response(&fixture)?;
        let refresh_response_content = encode_response_content(&refresh_response.content)?;
        let refresh_response_bytes =
            encode_signed_runtime_ipc_publication_network_response_v1(&refresh_response)?;
        let document = json!({
            "schema_version": 1,
            "profile": "kilogram-deterministic-cbor-v1",
            "activation_status": "not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json",
                "CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json",
                "CANONICAL-TICKET-PUBLICATION-VECTORS.json",
                "CANONICAL-PUBLICATION-CONFLICT-VECTORS.json"
            ],
            "input": {
                "device_secret_hex": lower_hex(&fixture.device_secret),
                "peer_scope_hex": lower_hex(&fixture.peer_scope),
                "service_origin_utf8_hex": lower_hex(&fixture.service_origin),
                "opaque_envelope_fixture_hex": lower_hex(&fixture.opaque_envelope),
                "publish_request_id_hex": lower_hex(&fixture.publish_request.request_id),
                "publish_request_issued_at_unix_seconds": fixture.publish_request.issued_at_unix_seconds,
                "publish_request_expires_at_unix_seconds": fixture.publish_request.expires_at_unix_seconds,
                "publish_contact_id_hex": lower_hex(&fixture.publish_result.updated_channel.contact_id),
                "peer_account_id_hex": lower_hex(&fixture.publish_result.updated_channel.peer_account_id),
                "channel_epoch": fixture.publish_result.updated_channel.channel_epoch,
                "recipient_device_list_digest_hex": lower_hex(&fixture.publish_result.recipient_device_list_digest),
                "recipient_device_count": fixture.publish_result.recipient_device_count,
                "publish_ticket_utf8_hex": lower_hex(b"m1138-private-ticket-payload"),
                "publish_at_unix_seconds": fixture.publish_result.published_at_unix_seconds,
                "publish_ttl_seconds": fixture.publish_result.expires_at_unix_seconds - fixture.publish_result.published_at_unix_seconds,
                "refresh_request_id_hex": lower_hex(&fixture.refresh_request.request_id),
                "refresh_request_issued_at_unix_seconds": fixture.refresh_request.issued_at_unix_seconds,
                "refresh_request_expires_at_unix_seconds": fixture.refresh_request.expires_at_unix_seconds,
                "refresh_contact_id_hex": lower_hex(&fixture.refresh_result.contact_id),
                "peer_device_secret_hex": lower_hex(&(0x40_u8..=0x5f).collect::<Vec<_>>()),
                "refresh_peer_scope_hex": lower_hex(b"m1138-refresh-peer-scope"),
                "refresh_channel_epoch": 2,
                "refresh_ticket_utf8_hex": lower_hex(b"m1138-peer-private-ticket"),
                "refresh_publication_at_unix_seconds": 1_800_002_200_u64,
                "refresh_publication_ttl_seconds": 900,
                "refresh_observed_at_unix_seconds": fixture.refresh_result.observation.content.observed_at_unix_seconds
            },
            "expected": {
                "account_id_hex": lower_hex(&fixture.descriptor.content.account_id),
                "device_id_hex": lower_hex(&fixture.descriptor.content.device_id),
                "service_origin_digest_hex": lower_hex(&canonical_runtime_ipc_publication_network_service_origin_digest_v1(&fixture.service_origin)?),
                "publish": {
                    "inner_publication_fixture_signed_cbor_hex": lower_hex(&encode_signed_ticket_publication_v1(&fixture.publish_inner_publication)?),
                    "state_cbor_hex": lower_hex(&encode_runtime_ipc_publication_network_publish_state_v1(match &fixture.publish_request.command {
                        CanonicalRuntimeIpcPublicationNetworkCommandV1::PublishOwnTicket { expected_current, .. } => expected_current,
                        _ => unreachable!(),
                    })?),
                    "result_cbor_hex": lower_hex(&encode_runtime_ipc_own_ticket_publication_v1(&fixture.publish_result)?),
                    "request": {
                        "cbor_hex": lower_hex(&publish_request),
                        "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_DIGEST_DOMAIN),
                        "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_DIGEST_DOMAIN, &publish_request)),
                        "object_id_hex": lower_hex(&runtime_ipc_publication_network_request_digest_v1(&fixture.publish_request)?)
                    },
                    "response": {
                        "content_cbor_hex": lower_hex(&publish_response_content),
                        "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_SIGNATURE_DOMAIN),
                        "signing_preimage_hex": lower_hex(&runtime_ipc_publication_network_response_signing_bytes_v1(&publish_response.content)?),
                        "signature_hex": lower_hex(&publish_response.signature),
                        "signed_cbor_hex": lower_hex(&publish_response_bytes),
                        "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_ID_DOMAIN, &publish_response_bytes)),
                        "object_id_hex": lower_hex(&runtime_ipc_publication_network_response_id_v1(&publish_response)?)
                    }
                },
                "refresh": {
                    "inner_publication_fixture_signed_cbor_hex": lower_hex(&encode_signed_ticket_publication_v1(&fixture.refresh_inner_publication)?),
                    "state_cbor_hex": lower_hex(&encode_runtime_ipc_publication_network_refresh_state_v1(match &fixture.refresh_request.command {
                        CanonicalRuntimeIpcPublicationNetworkCommandV1::RefreshContactTicket { expected_current, .. } => expected_current,
                        _ => unreachable!(),
                    })?),
                    "result_cbor_hex": lower_hex(&encode_runtime_ipc_contact_ticket_refresh_v1(&fixture.refresh_result)?),
                    "request": {
                        "cbor_hex": lower_hex(&refresh_request),
                        "digest_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_DIGEST_DOMAIN),
                        "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_DIGEST_DOMAIN, &refresh_request)),
                        "object_id_hex": lower_hex(&runtime_ipc_publication_network_request_digest_v1(&fixture.refresh_request)?)
                    },
                    "response": {
                        "content_cbor_hex": lower_hex(&refresh_response_content),
                        "signature_domain_hex": lower_hex(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_SIGNATURE_DOMAIN),
                        "signing_preimage_hex": lower_hex(&runtime_ipc_publication_network_response_signing_bytes_v1(&refresh_response.content)?),
                        "signature_hex": lower_hex(&refresh_response.signature),
                        "signed_cbor_hex": lower_hex(&refresh_response_bytes),
                        "id_preimage_hex": lower_hex(&prefixed(CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_ID_DOMAIN, &refresh_response_bytes)),
                        "object_id_hex": lower_hex(&runtime_ipc_publication_network_response_id_v1(&refresh_response)?)
                    }
                },
                "context": {
                    "active_runtime_changed": false,
                    "network_io_executed": false,
                    "hpke_envelope_fixture_only": true,
                    "ticket_plaintext_in_ipc": false,
                    "hpke_ciphertext_in_ipc": false,
                    "write_secret_in_ipc": false,
                    "https_success_is_device_attestation_not_remote_durability_proof": true,
                    "publish_delivery_requires_separate_peer_observation": true,
                    "refresh_install_is_atomic_before_success": true
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
