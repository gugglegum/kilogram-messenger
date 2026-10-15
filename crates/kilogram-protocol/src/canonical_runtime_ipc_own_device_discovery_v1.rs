//! Inactive deterministic-CBOR pairwise own-device ticket discovery boundary.
//!
//! This boundary configures and inspects a foreground runtime workflow that
//! derives directional publication capabilities internally, publishes an
//! opaque locator through one canonical HTTPS origin, fetches its sibling's
//! locator and atomically hands the verified content-addressed ticket to the
//! sibling announcement policy. Private capabilities, ticket plaintext and
//! filesystem paths never cross this IPC surface. Network execution and OS
//! background services are not activated by this candidate.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use url::Url;

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalSignedAccountDeviceListV1,
        decode_and_verify_signed_account_device_list_v1, encode_signed_account_device_list_v1,
        signed_envelope, verify_signed_account_device_list_v1,
    },
    canonical_runtime_ipc_directory_v1::runtime_ipc_device_list_digest_v1,
    canonical_runtime_ipc_own_device_announcement_v1::{
        CanonicalRuntimeIpcOwnDeviceAnnouncementArtifactV1,
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ANNOUNCEMENT_TICKET_BYTES,
    },
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

#[cfg(test)]
use crate::canonical_runtime_ipc_own_device_announcement_v1::runtime_ipc_own_device_announcement_artifact_digest_v1;

pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REQUEST_BYTES: usize = 768 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_BYTES: usize = 512 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_POLICIES: usize = 32;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_SERVICE_ORIGIN_BYTES: usize = 512;
pub const MIN_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_PUBLICATION_TTL_SECONDS: u64 = 30;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_PUBLICATION_TTL_SECONDS: u64 = 60 * 60;
pub const MIN_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REFRESH_BEFORE_SECONDS: u64 = 10;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REFRESH_BEFORE_SECONDS: u64 = 30 * 60;
pub const MIN_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_INTERVAL_SECONDS: u64 = 30;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_VALIDITY_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RETRY_SECONDS: u64 = 60 * 60;

pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_POLICY_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-discovery-policy-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_SNAPSHOT_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-discovery-snapshot-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-discovery-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-discovery-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-discovery-response-id:cbor-v1\0";

const COMMAND_CONFIGURE: u64 = 0;
const COMMAND_STATUS: u64 = 1;
const RESULT_CONFIGURED: u64 = 0;
const RESULT_STATUS: u64 = 1;
const RESULT_ERROR: u64 = 2;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const NETWORK_ETHERNET: u64 = 0;
const NETWORK_WIFI: u64 = 1;
const NETWORK_MOBILE: u64 = 2;
const NETWORK_UNKNOWN: u64 = 3;
const STATE_DISABLED: u64 = 0;
const STATE_NETWORK_BLOCKED: u64 = 1;
const STATE_AWAITING_PUBLISH: u64 = 2;
const STATE_AWAITING_FETCH: u64 = 3;
const STATE_AWAITING_ANNOUNCEMENT: u64 = 4;
const STATE_BACKOFF: u64 = 5;
const STATE_SYNCHRONIZED: u64 = 6;
const TRANSPORT_DIRECT: u64 = 0;
const TRANSPORT_RELAY: u64 = 1;
const RESTART_NOT_REQUIRED: u64 = 0;
const EXECUTION_FOREGROUND_RUNTIME: u64 = 0;
const NETWORK_MASK_ALL: u8 = 0b1111;

pub type CanonicalRuntimeIpcOwnDeviceDiscoveryArtifactV1 =
    CanonicalRuntimeIpcOwnDeviceAnnouncementArtifactV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryPreviousPolicyV1 {
    pub generation: u64,
    pub policy_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1 {
    pub recipient_device_id: [u8; 32],
    pub enabled: bool,
    pub service_origin_utf8: Vec<u8>,
    pub publication_ttl_seconds: u64,
    pub refresh_before_seconds: u64,
    pub announcement_interval_seconds: u64,
    pub announcement_validity_seconds: u64,
    pub retry_base_seconds: u64,
    pub retry_max_seconds: u64,
    /// bit 0 Ethernet, bit 1 Wi-Fi, bit 2 mobile, bit 3 unknown.
    pub allowed_network_mask: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1 {
    Ethernet,
    Wifi,
    Mobile,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1 {
    Disabled,
    NetworkBlocked,
    AwaitingPublish,
    AwaitingFetch,
    AwaitingAnnouncement,
    Backoff,
    Synchronized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1 {
    Direct,
    Relay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1 {
    pub generation: u64,
    pub published_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub recipient_ticket: CanonicalRuntimeIpcOwnDeviceDiscoveryArtifactV1,
    pub observation_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1 {
    pub configuration: CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1,
    pub generation: u64,
    pub previous_policy_id: Option<[u8; 32]>,
    pub policy_id: [u8; 32],
    pub configured_at_unix_seconds: u64,
    pub outbound_channel_id: [u8; 32],
    pub inbound_channel_id: [u8; 32],
    pub outbound_publication_generation: Option<u64>,
    pub inbound_locator: Option<CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1>,
    pub announcement_policy_id: Option<[u8; 32]>,
    pub workflow_state: CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1,
    pub last_attempt_unix_seconds: Option<u64>,
    pub last_success_unix_seconds: Option<u64>,
    pub next_attempt_unix_seconds: Option<u64>,
    pub consecutive_failures: u32,
    pub last_bundle_id: Option<[u8; 32]>,
    pub last_transport: Option<CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1 {
    ConfigureOwnDeviceTicketDiscovery {
        current_device_list: Box<CanonicalSignedAccountDeviceListV1>,
        expected_previous: Option<CanonicalRuntimeIpcOwnDeviceDiscoveryPreviousPolicyV1>,
        configuration: CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1,
    },
    OwnDeviceTicketDiscoveryStatus {
        current_device_list: Box<CanonicalSignedAccountDeviceListV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoveryRestartStateV1 {
    NotRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoveryExecutionScopeV1 {
    ForegroundRuntimeOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryConfiguredV1 {
    pub outcome: CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1,
    pub authority_revision: u64,
    pub device_list_digest: [u8; 32],
    pub current_network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1,
    pub status: CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1,
    pub restart_state: CanonicalRuntimeIpcOwnDeviceDiscoveryRestartStateV1,
    pub execution_scope: CanonicalRuntimeIpcOwnDeviceDiscoveryExecutionScopeV1,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryStatusV1 {
    pub authority_revision: u64,
    pub device_list_digest: [u8; 32],
    pub current_network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1,
    pub snapshot_id: [u8; 32],
    pub policies: Vec<CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1>,
    pub execution_scope: CanonicalRuntimeIpcOwnDeviceDiscoveryExecutionScopeV1,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1 {
    OwnDeviceTicketDiscoveryConfigured(Box<CanonicalRuntimeIpcOwnDeviceDiscoveryConfiguredV1>),
    OwnDeviceTicketDiscoveryStatus(Box<CanonicalRuntimeIpcOwnDeviceDiscoveryStatusV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1 {
    pub content: CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_own_device_discovery_request_v1(
    request: &CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
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
        "own-device discovery request",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_own_device_discovery_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "own-device discovery request",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-own-device-discovery-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-own-device-discovery-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-request-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-request-v1.request-id")?,
        bearer_token: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_own_device_discovery_request_digest_v1(
    request: &CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_own_device_discovery_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_own_device_discovery_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
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
    let list = command_device_list(&request.command);
    if list.content.authority_snapshot.content.account_id != descriptor.content.account_id
        || !list
            .content
            .devices
            .iter()
            .any(|certificate| certificate.content.device_id == descriptor.content.device_id)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    if let CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
        configuration,
        ..
    } = &request.command
        && (configuration.recipient_device_id == descriptor.content.device_id
            || !list.content.devices.iter().any(|certificate| {
                certificate.content.device_id == configuration.recipient_device_id
            }))
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_ipc_own_device_discovery_policy_id_v1(
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    authority_revision: u64,
    device_list_digest: &[u8; 32],
    generation: u64,
    previous_policy_id: Option<[u8; 32]>,
    configured_at_unix_seconds: u64,
    configuration: &CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1,
    outbound_channel_id: &[u8; 32],
    inbound_channel_id: &[u8; 32],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    validate_configuration(configuration, CanonicalRuntimeIpcError::InvalidResponse)?;
    if *account_id == [0; 32]
        || *local_device_id == [0; 32]
        || authority_revision == 0
        || *device_list_digest == [0; 32]
        || generation == 0
        || configured_at_unix_seconds == 0
        || *outbound_channel_id == [0; 32]
        || *inbound_channel_id == [0; 32]
        || outbound_channel_id == inbound_channel_id
        || (generation == 1) != previous_policy_id.is_none()
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut content = Vec::new();
    push_array_header(&mut content, 11);
    push_unsigned(&mut content, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut content, account_id);
    push_bytes(&mut content, local_device_id);
    push_unsigned(&mut content, authority_revision);
    push_bytes(&mut content, device_list_digest);
    push_unsigned(&mut content, generation);
    encode_optional_id(&mut content, previous_policy_id);
    push_unsigned(&mut content, configured_at_unix_seconds);
    encode_configuration(&mut content, configuration);
    push_bytes(&mut content, outbound_channel_id);
    push_bytes(&mut content, inbound_channel_id);
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_POLICY_ID_DOMAIN,
        &content,
    ))
}

pub fn runtime_ipc_own_device_discovery_snapshot_id_v1(
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    authority_revision: u64,
    device_list_digest: &[u8; 32],
    current_network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1,
    policies: &[CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    validate_policy_order(policies, CanonicalRuntimeIpcError::InvalidResponse)?;
    let mut content = Vec::new();
    push_array_header(&mut content, 7);
    push_unsigned(&mut content, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut content, account_id);
    push_bytes(&mut content, local_device_id);
    push_unsigned(&mut content, authority_revision);
    push_bytes(&mut content, device_list_digest);
    encode_network(&mut content, current_network);
    push_array_header(&mut content, policies.len() as u64);
    for policy in policies {
        encode_policy_status(&mut content, policy);
    }
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_SNAPSHOT_ID_DOMAIN,
        &content,
    ))
}

pub fn runtime_ipc_own_device_discovery_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_own_device_discovery_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1,
) -> Result<CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_own_device_discovery_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&result, request, descriptor)?;
    let content = CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_own_device_discovery_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_own_device_discovery_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1 { content, signature };
    verify_signed_runtime_ipc_own_device_discovery_response_v1(&response)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_own_device_discovery_response_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_own_device_discovery_response_v1(response)?;
    let encoded = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &encoded,
        "own-device discovery response",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_runtime_ipc_own_device_discovery_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "own-device discovery response",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-own-device-discovery-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-own-device-discovery-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-own-device-discovery-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1 { content, signature };
    verify_signed_runtime_ipc_own_device_discovery_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_own_device_discovery_response_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_own_device_discovery_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_own_device_discovery_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1,
    request: &CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_own_device_discovery_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    verify_signed_runtime_ipc_own_device_discovery_response_v1(response)?;
    if response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest
            != runtime_ipc_own_device_discovery_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&response.content.result, request, descriptor)
}

pub fn runtime_ipc_own_device_discovery_response_id_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_own_device_discovery_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let validity = request
        .expires_at_unix_seconds
        .checked_sub(request.issued_at_unix_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    if request.descriptor_id == [0; 32]
        || request.request_id == [0; 32]
        || request.bearer_token == [0; 32]
        || request.issued_at_unix_seconds == 0
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS).contains(&validity)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    match &request.command {
        CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
            current_device_list,
            expected_previous,
            configuration,
        } => {
            verify_signed_account_device_list_v1(current_device_list)?;
            validate_configuration(configuration, CanonicalRuntimeIpcError::InvalidRequest)?;
            if let Some(previous) = expected_previous
                && (previous.generation == 0 || previous.policy_id == [0; 32])
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
        CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::OwnDeviceTicketDiscoveryStatus {
            current_device_list,
        } => verify_signed_account_device_list_v1(current_device_list)?,
    }
    Ok(())
}

pub(crate) fn validate_configuration(
    configuration: &CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1,
    error: CanonicalRuntimeIpcError,
) -> Result<(), CanonicalRuntimeIpcError> {
    if configuration.recipient_device_id == [0; 32]
        || !service_origin_is_valid(&configuration.service_origin_utf8)
        || !(MIN_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_PUBLICATION_TTL_SECONDS
            ..=MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_PUBLICATION_TTL_SECONDS)
            .contains(&configuration.publication_ttl_seconds)
        || !(MIN_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REFRESH_BEFORE_SECONDS
            ..=MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REFRESH_BEFORE_SECONDS)
            .contains(&configuration.refresh_before_seconds)
        || configuration.refresh_before_seconds >= configuration.publication_ttl_seconds
        || !(MIN_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_INTERVAL_SECONDS
            ..=MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_INTERVAL_SECONDS)
            .contains(&configuration.announcement_interval_seconds)
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_VALIDITY_SECONDS)
            .contains(&configuration.announcement_validity_seconds)
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RETRY_SECONDS)
            .contains(&configuration.retry_base_seconds)
        || configuration.retry_max_seconds < configuration.retry_base_seconds
        || configuration.retry_max_seconds
            > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RETRY_SECONDS
        || configuration.allowed_network_mask & !NETWORK_MASK_ALL != 0
    {
        return Err(error);
    }
    Ok(())
}

fn validate_policy_status(
    status: &CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1,
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    authority_revision: u64,
    device_list_digest: &[u8; 32],
    current_network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_policy_status_shape(status, current_network)?;
    if status.policy_id
        != runtime_ipc_own_device_discovery_policy_id_v1(
            account_id,
            local_device_id,
            authority_revision,
            device_list_digest,
            status.generation,
            status.previous_policy_id,
            status.configured_at_unix_seconds,
            &status.configuration,
            &status.outbound_channel_id,
            &status.inbound_channel_id,
        )?
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

pub(crate) fn validate_policy_status_shape(
    status: &CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1,
    current_network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_configuration(
        &status.configuration,
        CanonicalRuntimeIpcError::InvalidResponse,
    )?;
    if status.generation == 0
        || status.configured_at_unix_seconds == 0
        || (status.generation == 1) != status.previous_policy_id.is_none()
        || status.previous_policy_id == Some([0; 32])
        || status.policy_id == [0; 32]
        || status.outbound_channel_id == [0; 32]
        || status.inbound_channel_id == [0; 32]
        || status.outbound_channel_id == status.inbound_channel_id
        || status.outbound_publication_generation == Some(0)
        || status.announcement_policy_id == Some([0; 32])
        || status.last_attempt_unix_seconds == Some(0)
        || status.last_success_unix_seconds == Some(0)
        || status.next_attempt_unix_seconds == Some(0)
        || status.last_bundle_id == Some([0; 32])
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    if status.inbound_locator.is_some() != status.announcement_policy_id.is_some() {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    if let Some(locator) = status.inbound_locator {
        validate_installed_locator(&locator)?;
    }
    let allowed = network_allowed(status.configuration.allowed_network_mask, current_network);
    let expected_state = if !status.configuration.enabled {
        CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Disabled
    } else if !allowed {
        CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::NetworkBlocked
    } else if status.consecutive_failures != 0 {
        CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Backoff
    } else if status.outbound_publication_generation.is_none() {
        CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingPublish
    } else if status.inbound_locator.is_none() {
        CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingFetch
    } else if status.last_success_unix_seconds.is_none() {
        CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingAnnouncement
    } else {
        CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Synchronized
    };
    if status.workflow_state != expected_state
        || status.last_attempt_unix_seconds.is_none() != status.next_attempt_unix_seconds.is_none()
        || status.last_success_unix_seconds.is_none() != status.last_bundle_id.is_none()
        || status.last_success_unix_seconds.is_none() != status.last_transport.is_none()
        || (status.last_success_unix_seconds.is_some()
            && status.last_attempt_unix_seconds.is_none())
        || (status.last_success_unix_seconds.is_some() && status.inbound_locator.is_none())
        || status
            .last_success_unix_seconds
            .zip(status.last_attempt_unix_seconds)
            .is_some_and(|(success, attempt)| success > attempt)
        || status
            .last_attempt_unix_seconds
            .zip(status.next_attempt_unix_seconds)
            .is_some_and(|(attempt, next)| next <= attempt)
        || (status.consecutive_failures != 0 && status.last_attempt_unix_seconds.is_none())
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_installed_locator(
    locator: &CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if locator.generation == 0
        || locator.published_at_unix_seconds == 0
        || locator.expires_at_unix_seconds <= locator.published_at_unix_seconds
        || locator.recipient_ticket.bytes == 0
        || locator.recipient_ticket.bytes
            > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ANNOUNCEMENT_TICKET_BYTES as u64
        || locator.recipient_ticket.digest == [0; 32]
        || locator.observation_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_policy_order(
    policies: &[CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1],
    error: CanonicalRuntimeIpcError,
) -> Result<(), CanonicalRuntimeIpcError> {
    if policies.len() > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_POLICIES
        || policies.windows(2).any(|pair| {
            pair[0].configuration.recipient_device_id >= pair[1].configuration.recipient_device_id
        })
    {
        return Err(error);
    }
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryConfigured(
            configured,
        ) => {
            if configured.authority_revision == 0
                || configured.device_list_digest == [0; 32]
                || configured.os_background_service_enabled
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_policy_status_shape(&configured.status, configured.current_network)?;
        }
        CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryStatus(status) => {
            if status.authority_revision == 0
                || status.device_list_digest == [0; 32]
                || status.snapshot_id == [0; 32]
                || status.os_background_service_enabled
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_policy_order(&status.policies, CanonicalRuntimeIpcError::InvalidResponse)?;
            for policy in &status.policies {
                validate_policy_status_shape(policy, status.current_network)?;
            }
        }
        CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::Error { code, message_utf8 } => {
            if *code == 0
                || message_utf8.is_empty()
                || message_utf8.len() > MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES
                || std::str::from_utf8(message_utf8).is_err()
                || message_utf8.contains(&0)
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
    }
    Ok(())
}

fn verify_result_for_request(
    result: &CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1,
    request: &CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_own_device_discovery_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result: result.clone(),
    })?;
    let list = command_device_list(&request.command);
    let revision = list.content.authority_snapshot.content.revision;
    let digest = runtime_ipc_device_list_digest_v1(list)?;
    match (&request.command, result) {
        (
            CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
                expected_previous,
                configuration,
                ..
            },
            CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryConfigured(
                configured,
            ),
        ) => {
            let expected_generation = expected_previous
                .map_or(Some(1), |previous| previous.generation.checked_add(1))
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
            if configured.authority_revision != revision
                || configured.device_list_digest != digest
                || &configured.status.configuration != configuration
                || configured.status.generation != expected_generation
                || configured.status.previous_policy_id
                    != expected_previous.map(|previous| previous.policy_id)
                || configured.status.configured_at_unix_seconds != request.issued_at_unix_seconds
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            validate_policy_status(
                &configured.status,
                &descriptor.content.account_id,
                &descriptor.content.device_id,
                revision,
                &digest,
                configured.current_network,
            )?;
        }
        (
            CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::OwnDeviceTicketDiscoveryStatus {
                ..
            },
            CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryStatus(status),
        ) => {
            if status.authority_revision != revision || status.device_list_digest != digest {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            for policy in &status.policies {
                if policy.configuration.recipient_device_id == descriptor.content.device_id
                    || !list.content.devices.iter().any(|certificate| {
                        certificate.content.device_id == policy.configuration.recipient_device_id
                    })
                {
                    return Err(CanonicalRuntimeIpcError::ContextMismatch);
                }
                validate_policy_status(
                    policy,
                    &descriptor.content.account_id,
                    &descriptor.content.device_id,
                    revision,
                    &digest,
                    status.current_network,
                )?;
            }
            if status.snapshot_id
                != runtime_ipc_own_device_discovery_snapshot_id_v1(
                    &descriptor.content.account_id,
                    &descriptor.content.device_id,
                    revision,
                    &digest,
                    status.current_network,
                    &status.policies,
                )?
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
        }
        (_, CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::Error { .. }) => {}
        _ => return Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
    Ok(())
}

fn command_device_list(
    command: &CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1,
) -> &CanonicalSignedAccountDeviceListV1 {
    match command {
        CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
            current_device_list,
            ..
        }
        | CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::OwnDeviceTicketDiscoveryStatus {
            current_device_list,
        } => current_device_list,
    }
}

fn encode_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
            current_device_list,
            expected_previous,
            configuration,
        } => {
            push_array_header(output, 4);
            push_unsigned(output, COMMAND_CONFIGURE);
            push_bytes(
                output,
                &encode_signed_account_device_list_v1(current_device_list)?,
            );
            encode_optional_previous(output, *expected_previous);
            encode_configuration(output, configuration);
        }
        CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::OwnDeviceTicketDiscoveryStatus {
            current_device_list,
        } => {
            push_array_header(output, 2);
            push_unsigned(output, COMMAND_STATUS);
            push_bytes(
                output,
                &encode_signed_account_device_list_v1(current_device_list)?,
            );
        }
    }
    Ok(())
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-discovery-command-v1")?;
    let variant = decoder.unsigned("runtime-ipc-own-device-discovery-command-v1.variant")?;
    match variant {
        COMMAND_CONFIGURE if length == 4 => Ok(
            CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
                current_device_list: Box::new(decode_and_verify_signed_account_device_list_v1(
                    decoder
                        .bytes("runtime-ipc-own-device-discovery-command-v1.current-device-list")?,
                )?),
                expected_previous: decode_optional_previous(decoder)?,
                configuration: decode_configuration(decoder)?,
            },
        ),
        COMMAND_STATUS if length == 2 => Ok(
            CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::OwnDeviceTicketDiscoveryStatus {
                current_device_list: Box::new(decode_and_verify_signed_account_device_list_v1(
                    decoder
                        .bytes("runtime-ipc-own-device-discovery-command-v1.current-device-list")?,
                )?),
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_configuration(
    output: &mut Vec<u8>,
    configuration: &CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1,
) {
    push_array_header(output, 11);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &configuration.recipient_device_id);
    encode_bool(output, configuration.enabled);
    push_bytes(output, &configuration.service_origin_utf8);
    push_unsigned(output, configuration.publication_ttl_seconds);
    push_unsigned(output, configuration.refresh_before_seconds);
    push_unsigned(output, configuration.announcement_interval_seconds);
    push_unsigned(output, configuration.announcement_validity_seconds);
    push_unsigned(output, configuration.retry_base_seconds);
    push_unsigned(output, configuration.retry_max_seconds);
    push_unsigned(output, u64::from(configuration.allowed_network_mask));
}

fn decode_configuration(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-discovery-configuration-v1", 11)?;
    decoder.expect_version("runtime-ipc-own-device-discovery-configuration-v1.schema-version")?;
    let configuration = CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1 {
        recipient_device_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-configuration-v1.recipient-device-id")?,
        enabled: decode_bool(
            decoder,
            "runtime-ipc-own-device-discovery-configuration-v1.enabled",
            false,
        )?,
        service_origin_utf8: decoder
            .bytes("runtime-ipc-own-device-discovery-configuration-v1.service-origin-utf8")?
            .to_vec(),
        publication_ttl_seconds: decoder.unsigned(
            "runtime-ipc-own-device-discovery-configuration-v1.publication-ttl-seconds",
        )?,
        refresh_before_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-configuration-v1.refresh-before-seconds")?,
        announcement_interval_seconds: decoder.unsigned(
            "runtime-ipc-own-device-discovery-configuration-v1.announcement-interval-seconds",
        )?,
        announcement_validity_seconds: decoder.unsigned(
            "runtime-ipc-own-device-discovery-configuration-v1.announcement-validity-seconds",
        )?,
        retry_base_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-configuration-v1.retry-base-seconds")?,
        retry_max_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-configuration-v1.retry-max-seconds")?,
        allowed_network_mask: u8::try_from(
            decoder.unsigned(
                "runtime-ipc-own-device-discovery-configuration-v1.allowed-network-mask",
            )?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
    };
    validate_configuration(&configuration, CanonicalRuntimeIpcError::InvalidRequest)?;
    Ok(configuration)
}

fn encode_artifact(
    output: &mut Vec<u8>,
    artifact: CanonicalRuntimeIpcOwnDeviceDiscoveryArtifactV1,
) {
    push_array_header(output, 2);
    push_unsigned(output, artifact.bytes);
    push_bytes(output, &artifact.digest);
}

fn decode_artifact(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryArtifactV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-discovery-artifact-v1", 2)?;
    Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryArtifactV1 {
        bytes: decoder.unsigned("runtime-ipc-own-device-discovery-artifact-v1.bytes")?,
        digest: decoder.fixed_bytes("runtime-ipc-own-device-discovery-artifact-v1.digest")?,
    })
}

fn encode_optional_locator(
    output: &mut Vec<u8>,
    value: Option<CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1>,
) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(locator) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_array_header(output, 6);
            push_unsigned(output, CANONICAL_PROFILE_VERSION);
            push_unsigned(output, locator.generation);
            push_unsigned(output, locator.published_at_unix_seconds);
            push_unsigned(output, locator.expires_at_unix_seconds);
            encode_artifact(output, locator.recipient_ticket);
            push_bytes(output, &locator.observation_id);
        }
    }
}

fn decode_optional_locator(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1>, CanonicalRuntimeIpcError>
{
    let length = decoder.array("runtime-ipc-own-device-discovery-optional-locator-v1")?;
    match decoder.unsigned("runtime-ipc-own-device-discovery-optional-locator-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 2 => {
            decoder.expect_array("runtime-ipc-own-device-discovery-installed-locator-v1", 6)?;
            decoder.expect_version(
                "runtime-ipc-own-device-discovery-installed-locator-v1.schema-version",
            )?;
            let locator = CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1 {
                generation: decoder
                    .unsigned("runtime-ipc-own-device-discovery-installed-locator-v1.generation")?,
                published_at_unix_seconds: decoder.unsigned(
                    "runtime-ipc-own-device-discovery-installed-locator-v1.published-at-unix-seconds",
                )?,
                expires_at_unix_seconds: decoder.unsigned(
                    "runtime-ipc-own-device-discovery-installed-locator-v1.expires-at-unix-seconds",
                )?,
                recipient_ticket: decode_artifact(decoder)?,
                observation_id: decoder.fixed_bytes(
                    "runtime-ipc-own-device-discovery-installed-locator-v1.observation-id",
                )?,
            };
            validate_installed_locator(&locator)?;
            Ok(Some(locator))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_optional_previous(
    output: &mut Vec<u8>,
    value: Option<CanonicalRuntimeIpcOwnDeviceDiscoveryPreviousPolicyV1>,
) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(previous) => {
            push_array_header(output, 3);
            push_unsigned(output, OPTION_SOME);
            push_unsigned(output, previous.generation);
            push_bytes(output, &previous.policy_id);
        }
    }
}

fn decode_optional_previous(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcOwnDeviceDiscoveryPreviousPolicyV1>, CanonicalRuntimeIpcError>
{
    let length = decoder.array("runtime-ipc-own-device-discovery-previous-policy-v1")?;
    match decoder.unsigned("runtime-ipc-own-device-discovery-previous-policy-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 3 => Ok(Some(
            CanonicalRuntimeIpcOwnDeviceDiscoveryPreviousPolicyV1 {
                generation: decoder
                    .unsigned("runtime-ipc-own-device-discovery-previous-policy-v1.generation")?,
                policy_id: decoder
                    .fixed_bytes("runtime-ipc-own-device-discovery-previous-policy-v1.policy-id")?,
            },
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

pub(crate) fn encode_policy_status(
    output: &mut Vec<u8>,
    status: &CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1,
) {
    push_array_header(output, 18);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    encode_configuration(output, &status.configuration);
    push_unsigned(output, status.generation);
    encode_optional_id(output, status.previous_policy_id);
    push_bytes(output, &status.policy_id);
    push_unsigned(output, status.configured_at_unix_seconds);
    push_bytes(output, &status.outbound_channel_id);
    push_bytes(output, &status.inbound_channel_id);
    encode_optional_unsigned(output, status.outbound_publication_generation);
    encode_optional_locator(output, status.inbound_locator);
    encode_optional_id(output, status.announcement_policy_id);
    encode_scheduler_state(output, status.workflow_state);
    encode_optional_unsigned(output, status.last_attempt_unix_seconds);
    encode_optional_unsigned(output, status.last_success_unix_seconds);
    encode_optional_unsigned(output, status.next_attempt_unix_seconds);
    push_unsigned(output, u64::from(status.consecutive_failures));
    encode_optional_id(output, status.last_bundle_id);
    encode_optional_transport(output, status.last_transport);
}

pub(crate) fn decode_policy_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-discovery-policy-status-v1", 18)?;
    decoder.expect_version("runtime-ipc-own-device-discovery-policy-status-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1 {
        configuration: decode_configuration_response(decoder)?,
        generation: decoder
            .unsigned("runtime-ipc-own-device-discovery-policy-status-v1.generation")?,
        previous_policy_id: decode_optional_id(decoder, false)?,
        policy_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-policy-status-v1.policy-id")?,
        configured_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-own-device-discovery-policy-status-v1.configured-at-unix-seconds",
        )?,
        outbound_channel_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-policy-status-v1.outbound-channel-id")?,
        inbound_channel_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-policy-status-v1.inbound-channel-id")?,
        outbound_publication_generation: decode_optional_unsigned(decoder, false)?,
        inbound_locator: decode_optional_locator(decoder)?,
        announcement_policy_id: decode_optional_id(decoder, false)?,
        workflow_state: decode_scheduler_state(decoder)?,
        last_attempt_unix_seconds: decode_optional_unsigned(decoder, false)?,
        last_success_unix_seconds: decode_optional_unsigned(decoder, false)?,
        next_attempt_unix_seconds: decode_optional_unsigned(decoder, false)?,
        consecutive_failures: u32::try_from(
            decoder.unsigned(
                "runtime-ipc-own-device-discovery-policy-status-v1.consecutive-failures",
            )?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        last_bundle_id: decode_optional_id(decoder, false)?,
        last_transport: decode_optional_transport(decoder)?,
    })
}

fn decode_configuration_response(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-discovery-configuration-v1", 11)?;
    decoder.expect_version("runtime-ipc-own-device-discovery-configuration-v1.schema-version")?;
    let configuration = CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1 {
        recipient_device_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-configuration-v1.recipient-device-id")?,
        enabled: decode_bool(
            decoder,
            "runtime-ipc-own-device-discovery-configuration-v1.enabled",
            true,
        )?,
        service_origin_utf8: decoder
            .bytes("runtime-ipc-own-device-discovery-configuration-v1.service-origin-utf8")?
            .to_vec(),
        publication_ttl_seconds: decoder.unsigned(
            "runtime-ipc-own-device-discovery-configuration-v1.publication-ttl-seconds",
        )?,
        refresh_before_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-configuration-v1.refresh-before-seconds")?,
        announcement_interval_seconds: decoder.unsigned(
            "runtime-ipc-own-device-discovery-configuration-v1.announcement-interval-seconds",
        )?,
        announcement_validity_seconds: decoder.unsigned(
            "runtime-ipc-own-device-discovery-configuration-v1.announcement-validity-seconds",
        )?,
        retry_base_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-configuration-v1.retry-base-seconds")?,
        retry_max_seconds: decoder
            .unsigned("runtime-ipc-own-device-discovery-configuration-v1.retry-max-seconds")?,
        allowed_network_mask: u8::try_from(
            decoder.unsigned(
                "runtime-ipc-own-device-discovery-configuration-v1.allowed-network-mask",
            )?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    };
    validate_configuration(&configuration, CanonicalRuntimeIpcError::InvalidResponse)?;
    Ok(configuration)
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_response_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_result(&mut output, &content.result);
    Ok(output)
}

fn decode_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-discovery-response-content-v1", 6)?;
    decoder
        .expect_version("runtime-ipc-own-device-discovery-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcOwnDeviceDiscoveryResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-own-device-discovery-response-content-v1.request-digest")?,
        responder_device_id: decoder.fixed_bytes(
            "runtime-ipc-own-device-discovery-response-content-v1.responder-device-id",
        )?,
        result: decode_result(decoder)?,
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn encode_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1) {
    match result {
        CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryConfigured(
            configured,
        ) => {
            push_array_header(output, 9);
            push_unsigned(output, RESULT_CONFIGURED);
            encode_outcome(output, configured.outcome);
            push_unsigned(output, configured.authority_revision);
            push_bytes(output, &configured.device_list_digest);
            encode_network(output, configured.current_network);
            encode_policy_status(output, &configured.status);
            push_unsigned(output, RESTART_NOT_REQUIRED);
            push_unsigned(output, EXECUTION_FOREGROUND_RUNTIME);
            encode_bool(output, configured.os_background_service_enabled);
        }
        CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryStatus(status) => {
            push_array_header(output, 9);
            push_unsigned(output, RESULT_STATUS);
            push_unsigned(output, status.authority_revision);
            push_bytes(output, &status.device_list_digest);
            encode_network(output, status.current_network);
            push_bytes(output, &status.snapshot_id);
            push_array_header(output, status.policies.len() as u64);
            for policy in &status.policies {
                encode_policy_status(output, policy);
            }
            push_unsigned(output, EXECUTION_FOREGROUND_RUNTIME);
            encode_bool(output, status.os_background_service_enabled);
            push_unsigned(output, status.policies.len() as u64);
        }
        CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-discovery-result-v1")?;
    let variant = decoder.unsigned("runtime-ipc-own-device-discovery-result-v1.variant")?;
    match variant {
        RESULT_CONFIGURED if length == 9 => Ok(
            CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryConfigured(
                Box::new(CanonicalRuntimeIpcOwnDeviceDiscoveryConfiguredV1 {
                    outcome: decode_outcome(decoder)?,
                    authority_revision: decoder.unsigned(
                        "runtime-ipc-own-device-discovery-configured-v1.authority-revision",
                    )?,
                    device_list_digest: decoder.fixed_bytes(
                        "runtime-ipc-own-device-discovery-configured-v1.device-list-digest",
                    )?,
                    current_network: decode_network(decoder)?,
                    status: decode_policy_status(decoder)?,
                    restart_state: decode_restart(decoder)?,
                    execution_scope: decode_execution(decoder)?,
                    os_background_service_enabled: decode_bool(
                        decoder,
                        "runtime-ipc-own-device-discovery-configured-v1.os-background-service-enabled",
                        true,
                    )?,
                }),
            ),
        ),
        RESULT_STATUS if length == 9 => {
            let authority_revision = decoder
                .unsigned("runtime-ipc-own-device-discovery-status-v1.authority-revision")?;
            let device_list_digest = decoder
                .fixed_bytes("runtime-ipc-own-device-discovery-status-v1.device-list-digest")?;
            let current_network = decode_network(decoder)?;
            let snapshot_id =
                decoder.fixed_bytes("runtime-ipc-own-device-discovery-status-v1.snapshot-id")?;
            let count = decoder.array("runtime-ipc-own-device-discovery-status-v1.policies")?;
            if count > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_POLICIES {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            let mut policies = Vec::with_capacity(count);
            for _ in 0..count {
                policies.push(decode_policy_status(decoder)?);
            }
            let execution_scope = decode_execution(decoder)?;
            let os_background_service_enabled = decode_bool(
                decoder,
                "runtime-ipc-own-device-discovery-status-v1.os-background-service-enabled",
                true,
            )?;
            let declared_count = usize::try_from(
                decoder.unsigned("runtime-ipc-own-device-discovery-status-v1.policy-count")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            if declared_count != policies.len() {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(
                CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryStatus(
                    Box::new(CanonicalRuntimeIpcOwnDeviceDiscoveryStatusV1 {
                        authority_revision,
                        device_list_digest,
                        current_network,
                        snapshot_id,
                        policies,
                        execution_scope,
                        os_background_service_enabled,
                    }),
                ),
            )
        }
        RESULT_ERROR if length == 3 => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::Error {
            code: u16::try_from(
                decoder.unsigned("runtime-ipc-own-device-discovery-error-v1.code")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-own-device-discovery-error-v1.message")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_optional_id(output: &mut Vec<u8>, value: Option<[u8; 32]>) {
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

fn decode_optional_id(
    decoder: &mut AccountDecoder<'_>,
    request: bool,
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-discovery-optional-id-v1")?;
    match decoder.unsigned("runtime-ipc-own-device-discovery-optional-id-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 2 => Ok(Some(
            decoder.fixed_bytes("runtime-ipc-own-device-discovery-optional-id-v1.value")?,
        )),
        _ if request => Err(CanonicalRuntimeIpcError::InvalidRequest),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_optional_unsigned(output: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_unsigned(output, value);
        }
    }
}

fn decode_optional_unsigned(
    decoder: &mut AccountDecoder<'_>,
    request: bool,
) -> Result<Option<u64>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-discovery-optional-unsigned-v1")?;
    match decoder.unsigned("runtime-ipc-own-device-discovery-optional-unsigned-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 2 => Ok(Some(
            decoder.unsigned("runtime-ipc-own-device-discovery-optional-unsigned-v1.value")?,
        )),
        _ if request => Err(CanonicalRuntimeIpcError::InvalidRequest),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_optional_transport(
    output: &mut Vec<u8>,
    value: Option<CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1>,
) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            encode_transport(output, value);
        }
    }
}

fn decode_optional_transport(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-discovery-optional-transport-v1")?;
    match decoder.unsigned("runtime-ipc-own-device-discovery-optional-transport-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 2 => Ok(Some(decode_transport(decoder)?)),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

pub(crate) fn encode_network(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Ethernet => NETWORK_ETHERNET,
            CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Wifi => NETWORK_WIFI,
            CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Mobile => NETWORK_MOBILE,
            CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Unknown => NETWORK_UNKNOWN,
        },
    );
}

pub(crate) fn decode_network(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-discovery-network-v1")? {
        NETWORK_ETHERNET => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Ethernet),
        NETWORK_WIFI => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Wifi),
        NETWORK_MOBILE => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Mobile),
        NETWORK_UNKNOWN => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Unknown),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_scheduler_state(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Disabled => STATE_DISABLED,
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::NetworkBlocked => {
                STATE_NETWORK_BLOCKED
            }
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingPublish => {
                STATE_AWAITING_PUBLISH
            }
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingFetch => {
                STATE_AWAITING_FETCH
            }
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingAnnouncement => {
                STATE_AWAITING_ANNOUNCEMENT
            }
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Backoff => STATE_BACKOFF,
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Synchronized => {
                STATE_SYNCHRONIZED
            }
        },
    );
}

fn decode_scheduler_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-discovery-scheduler-state-v1")? {
        STATE_DISABLED => Ok(CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Disabled),
        STATE_NETWORK_BLOCKED => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::NetworkBlocked)
        }
        STATE_AWAITING_PUBLISH => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingPublish)
        }
        STATE_AWAITING_FETCH => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingFetch)
        }
        STATE_AWAITING_ANNOUNCEMENT => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingAnnouncement)
        }
        STATE_BACKOFF => Ok(CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Backoff),
        STATE_SYNCHRONIZED => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Synchronized)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_transport(output: &mut Vec<u8>, value: CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1::Direct => TRANSPORT_DIRECT,
            CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1::Relay => TRANSPORT_RELAY,
        },
    );
}

fn decode_transport(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-discovery-transport-v1")? {
        TRANSPORT_DIRECT => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1::Direct),
        TRANSPORT_RELAY => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1::Relay),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_outcome(output: &mut Vec<u8>, value: CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1::Committed => OUTCOME_COMMITTED,
            CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1::AlreadyCommitted => {
                OUTCOME_ALREADY_COMMITTED
            }
        },
    );
}

fn decode_outcome(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-discovery-outcome-v1")? {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_restart(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryRestartStateV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-discovery-restart-v1")? {
        RESTART_NOT_REQUIRED => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryRestartStateV1::NotRequired)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_execution(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryExecutionScopeV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-discovery-execution-scope-v1")? {
        EXECUTION_FOREGROUND_RUNTIME => {
            Ok(CanonicalRuntimeIpcOwnDeviceDiscoveryExecutionScopeV1::ForegroundRuntimeOnly)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn network_allowed(mask: u8, network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1) -> bool {
    let bit = match network {
        CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Ethernet => 0,
        CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Wifi => 1,
        CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Mobile => 2,
        CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Unknown => 3,
    };
    mask & (1 << bit) != 0
}

fn service_origin_is_valid(value: &[u8]) -> bool {
    if value.is_empty()
        || value.len() > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_SERVICE_ORIGIN_BYTES
    {
        return false;
    }
    let Ok(text) = std::str::from_utf8(value) else {
        return false;
    };
    let Ok(parsed) = Url::parse(text) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.host_str().is_some()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && parsed.path() == "/"
        && parsed.as_str().as_bytes() == value
}

fn encode_bool(output: &mut Vec<u8>, value: bool) {
    push_unsigned(output, u64::from(value));
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
    response: bool,
) -> Result<bool, CanonicalRuntimeIpcError> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        _ if response => Err(CanonicalRuntimeIpcError::InvalidResponse),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn ensure_bound(
    bytes: &[u8],
    object: &'static str,
    maximum: usize,
) -> Result<(), CanonicalRuntimeIpcError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(CanonicalRuntimeIpcError::ObjectTooLarge { object, maximum });
    }
    Ok(())
}

fn verify_ed25519(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CanonicalRuntimeIpcError> {
    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    verifying_key
        .verify_strict(message, &Signature::from_bytes(signature))
        .map_err(|_| CanonicalRuntimeIpcError::InvalidSignature)
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::{Value, json};

    use super::*;
    use crate::canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        current: CanonicalSignedAccountDeviceListV1,
        ticket: Vec<u8>,
        configure_request: CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
        configure_result: CanonicalRuntimeIpcOwnDeviceDiscoveryConfiguredV1,
        status_request: CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1,
        status_result: CanonicalRuntimeIpcOwnDeviceDiscoveryStatusV1,
    }

    fn documents() -> TestResult<(Value, Value)> {
        Ok((
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-RUNTIME-IPC-VECTORS.json"
            )))?,
            serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json"
            )))?,
        ))
    }

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json"
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

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn domain(bytes: &[u8]) -> String {
        assert_eq!(bytes.last(), Some(&0));
        String::from_utf8_lossy(&bytes[..bytes.len() - 1]).into_owned()
    }

    fn fixture() -> TestResult<Fixture> {
        let (core, recovery) = documents()?;
        let descriptor = decode_and_verify_signed_runtime_ipc_descriptor_v1(&bytes(
            &core,
            "/expected/descriptor/signed_cbor_hex",
        )?)?;
        let device_key = SigningKey::from_bytes(&fixed(&core, "/input/device_secret_key_hex")?);
        let current = decode_and_verify_signed_account_device_list_v1(&bytes(
            &recovery,
            "/expected/dependencies/new_account_device_list/signed_cbor_hex",
        )?)?;
        let recipient_device_id = current.content.devices[1].content.device_id;
        let ticket = b"kilogram-own-device-ticket-candidate-v1".to_vec();
        let configuration = CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1 {
            recipient_device_id,
            enabled: true,
            service_origin_utf8: b"https://tickets.example/".to_vec(),
            publication_ttl_seconds: 900,
            refresh_before_seconds: 120,
            announcement_interval_seconds: 300,
            announcement_validity_seconds: 900,
            retry_base_seconds: 5,
            retry_max_seconds: 300,
            allowed_network_mask: 0b0011,
        };
        let issued_at = 1_800_002_040;
        let configure_request = CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0xca; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: issued_at,
            expires_at_unix_seconds: issued_at + 30,
            command:
                CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
                    current_device_list: Box::new(current.clone()),
                    expected_previous: None,
                    configuration: configuration.clone(),
                },
        };
        let revision = current.content.authority_snapshot.content.revision;
        let digest = runtime_ipc_device_list_digest_v1(&current)?;
        let mut policy = CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1 {
            configuration,
            generation: 1,
            previous_policy_id: None,
            policy_id: [0; 32],
            configured_at_unix_seconds: issued_at,
            outbound_channel_id: [0xd1; 32],
            inbound_channel_id: [0xd2; 32],
            outbound_publication_generation: None,
            inbound_locator: None,
            announcement_policy_id: None,
            workflow_state: CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingPublish,
            last_attempt_unix_seconds: None,
            last_success_unix_seconds: None,
            next_attempt_unix_seconds: None,
            consecutive_failures: 0,
            last_bundle_id: None,
            last_transport: None,
        };
        policy.policy_id = runtime_ipc_own_device_discovery_policy_id_v1(
            &descriptor.content.account_id,
            &descriptor.content.device_id,
            revision,
            &digest,
            policy.generation,
            policy.previous_policy_id,
            policy.configured_at_unix_seconds,
            &policy.configuration,
            &policy.outbound_channel_id,
            &policy.inbound_channel_id,
        )?;
        let configure_result = CanonicalRuntimeIpcOwnDeviceDiscoveryConfiguredV1 {
            outcome: CanonicalRuntimeIpcOwnDeviceDiscoveryOutcomeV1::Committed,
            authority_revision: revision,
            device_list_digest: digest,
            current_network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Wifi,
            status: policy.clone(),
            restart_state: CanonicalRuntimeIpcOwnDeviceDiscoveryRestartStateV1::NotRequired,
            execution_scope:
                CanonicalRuntimeIpcOwnDeviceDiscoveryExecutionScopeV1::ForegroundRuntimeOnly,
            os_background_service_enabled: false,
        };
        let status_request = CanonicalRuntimeIpcOwnDeviceDiscoveryRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0xcb; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: issued_at + 1,
            expires_at_unix_seconds: issued_at + 31,
            command:
                CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::OwnDeviceTicketDiscoveryStatus {
                    current_device_list: Box::new(current.clone()),
                },
        };
        let mut synchronized = policy;
        synchronized.outbound_publication_generation = Some(3);
        synchronized.inbound_locator =
            Some(CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1 {
                generation: 4,
                published_at_unix_seconds: issued_at - 20,
                expires_at_unix_seconds: issued_at + 880,
                recipient_ticket: CanonicalRuntimeIpcOwnDeviceDiscoveryArtifactV1 {
                    bytes: u64::try_from(ticket.len())?,
                    digest: runtime_ipc_own_device_announcement_artifact_digest_v1(&ticket),
                },
                observation_id: [0xe0; 32],
            });
        synchronized.announcement_policy_id = Some([0xe1; 32]);
        synchronized.workflow_state =
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Synchronized;
        synchronized.last_attempt_unix_seconds = Some(issued_at - 4);
        synchronized.last_success_unix_seconds = Some(issued_at - 4);
        synchronized.next_attempt_unix_seconds = Some(issued_at + 296);
        synchronized.last_bundle_id = Some([0xe2; 32]);
        synchronized.last_transport = Some(CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1::Relay);
        let policies = vec![synchronized];
        let snapshot_id = runtime_ipc_own_device_discovery_snapshot_id_v1(
            &descriptor.content.account_id,
            &descriptor.content.device_id,
            revision,
            &digest,
            CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Wifi,
            &policies,
        )?;
        let status_result = CanonicalRuntimeIpcOwnDeviceDiscoveryStatusV1 {
            authority_revision: revision,
            device_list_digest: digest,
            current_network: CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1::Wifi,
            snapshot_id,
            policies,
            execution_scope:
                CanonicalRuntimeIpcOwnDeviceDiscoveryExecutionScopeV1::ForegroundRuntimeOnly,
            os_background_service_enabled: false,
        };
        Ok(Fixture {
            descriptor,
            device_key,
            current,
            ticket,
            configure_request,
            configure_result,
            status_request,
            status_result,
        })
    }

    fn configure_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1, CanonicalRuntimeIpcError>
    {
        sign_runtime_ipc_own_device_discovery_response_v1(
            &fixture.device_key,
            &fixture.configure_request,
            &fixture.descriptor,
            fixture.configure_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryConfigured(
                Box::new(fixture.configure_result.clone()),
            ),
        )
    }

    fn status_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcOwnDeviceDiscoveryResponseV1, CanonicalRuntimeIpcError>
    {
        sign_runtime_ipc_own_device_discovery_response_v1(
            &fixture.device_key,
            &fixture.status_request,
            &fixture.descriptor,
            fixture.status_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryStatus(
                Box::new(fixture.status_result.clone()),
            ),
        )
    }

    #[test]
    fn canonical_runtime_ipc_own_device_discovery_round_trips() -> TestResult {
        let fixture = fixture()?;
        for request in [&fixture.configure_request, &fixture.status_request] {
            verify_runtime_ipc_own_device_discovery_request_for_descriptor_v1(
                request,
                &fixture.descriptor,
                request.issued_at_unix_seconds,
            )?;
            let encoded = encode_runtime_ipc_own_device_discovery_request_v1(request)?;
            assert_eq!(
                decode_and_verify_runtime_ipc_own_device_discovery_request_v1(&encoded)?,
                *request
            );
        }
        for (request, response) in [
            (&fixture.configure_request, configure_response(&fixture)?),
            (&fixture.status_request, status_response(&fixture)?),
        ] {
            let encoded = encode_signed_runtime_ipc_own_device_discovery_response_v1(&response)?;
            assert_eq!(
                decode_and_verify_signed_runtime_ipc_own_device_discovery_response_v1(&encoded)?,
                response
            );
            verify_runtime_ipc_own_device_discovery_response_for_request_v1(
                &response,
                request,
                &fixture.descriptor,
                request.issued_at_unix_seconds,
            )?;
        }
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_own_device_discovery_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let vectors = vectors()?;
        let configure_response = configure_response(&fixture)?;
        let status_response = status_response(&fixture)?;
        assert_eq!(
            encode_runtime_ipc_own_device_discovery_request_v1(&fixture.configure_request)?,
            bytes(&vectors, "/expected/configure/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_own_device_discovery_request_digest_v1(&fixture.configure_request)?,
            fixed(&vectors, "/expected/configure/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_own_device_discovery_response_v1(&configure_response)?,
            bytes(&vectors, "/expected/configure/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_own_device_discovery_response_id_v1(&configure_response)?,
            fixed(&vectors, "/expected/configure/response/object_id_hex")?
        );
        assert_eq!(
            encode_runtime_ipc_own_device_discovery_request_v1(&fixture.status_request)?,
            bytes(&vectors, "/expected/status/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_own_device_discovery_request_digest_v1(&fixture.status_request)?,
            fixed(&vectors, "/expected/status/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_own_device_discovery_response_v1(&status_response)?,
            bytes(&vectors, "/expected/status/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_own_device_discovery_response_id_v1(&status_response)?,
            fixed(&vectors, "/expected/status/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_own_device_discovery_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let configure_response = configure_response(&fixture)?;
        let mut changed = fixture.configure_request.clone();
        let CanonicalRuntimeIpcOwnDeviceDiscoveryCommandV1::ConfigureOwnDeviceTicketDiscovery {
            configuration,
            ..
        } = &mut changed.command
        else {
            unreachable!()
        };
        configuration.allowed_network_mask ^= 0b0100;
        assert!(
            verify_runtime_ipc_own_device_discovery_response_for_request_v1(
                &configure_response,
                &changed,
                &fixture.descriptor,
                changed.issued_at_unix_seconds,
            )
            .is_err()
        );

        let mut changed_response = status_response(&fixture)?;
        let CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryStatus(status) =
            &mut changed_response.content.result
        else {
            unreachable!()
        };
        let inbound_locator = status.policies[0]
            .inbound_locator
            .as_mut()
            .ok_or_else(|| std::io::Error::other("fixture has no installed locator"))?;
        inbound_locator.recipient_ticket.digest[0] ^= 1;
        assert!(
            encode_signed_runtime_ipc_own_device_discovery_response_v1(&changed_response).is_err()
        );

        let mut forbidden_background = fixture.configure_result.clone();
        forbidden_background.os_background_service_enabled = true;
        assert!(
            sign_runtime_ipc_own_device_discovery_response_v1(
                &fixture.device_key,
                &fixture.configure_request,
                &fixture.descriptor,
                fixture.configure_request.issued_at_unix_seconds,
                CanonicalRuntimeIpcOwnDeviceDiscoveryResultV1::OwnDeviceTicketDiscoveryConfigured(
                    Box::new(forbidden_background),
                ),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    #[ignore = "explicit golden-vector regeneration"]
    fn regenerate_canonical_runtime_ipc_own_device_discovery_vectors() -> TestResult {
        let fixture = fixture()?;
        let configure_response = configure_response(&fixture)?;
        let status_response = status_response(&fixture)?;
        let configure_request_bytes =
            encode_runtime_ipc_own_device_discovery_request_v1(&fixture.configure_request)?;
        let configure_response_bytes =
            encode_signed_runtime_ipc_own_device_discovery_response_v1(&configure_response)?;
        let status_request_bytes =
            encode_runtime_ipc_own_device_discovery_request_v1(&fixture.status_request)?;
        let status_response_bytes =
            encode_signed_runtime_ipc_own_device_discovery_response_v1(&status_response)?;
        let current_bytes = encode_signed_account_device_list_v1(&fixture.current)?;
        let status_policy =
            fixture.status_result.policies.first().ok_or_else(|| {
                std::io::Error::other("fixture has no own-device discovery policy")
            })?;
        let inbound_locator = status_policy
            .inbound_locator
            .as_ref()
            .ok_or_else(|| std::io::Error::other("fixture has no installed locator"))?;
        let announcement_policy_id = status_policy
            .announcement_policy_id
            .ok_or_else(|| std::io::Error::other("fixture has no announcement policy"))?;
        let last_attempt_unix_seconds = status_policy
            .last_attempt_unix_seconds
            .ok_or_else(|| std::io::Error::other("fixture has no last attempt"))?;
        let last_success_unix_seconds = status_policy
            .last_success_unix_seconds
            .ok_or_else(|| std::io::Error::other("fixture has no last success"))?;
        let next_attempt_unix_seconds = status_policy
            .next_attempt_unix_seconds
            .ok_or_else(|| std::io::Error::other("fixture has no next attempt"))?;
        let last_bundle_id = status_policy
            .last_bundle_id
            .ok_or_else(|| std::io::Error::other("fixture has no last bundle"))?;
        let input = json!({
                "device_secret_key_hex": hex(&fixture.device_key.to_bytes()),
                "descriptor_id_hex": hex(&fixture.configure_request.descriptor_id),
                "bearer_token_hex": hex(&fixture.configure_request.bearer_token),
                "account_id_hex": hex(&fixture.descriptor.content.account_id),
                "local_device_id_hex": hex(&fixture.descriptor.content.device_id),
                "current_device_list_signed_cbor_hex": hex(&current_bytes),
                "authority_revision": fixture.current.content.authority_snapshot.content.revision,
                "device_list_digest_hex": hex(&fixture.configure_result.device_list_digest),
                "recipient_device_id_hex": hex(&fixture.configure_result.status.configuration.recipient_device_id),
                "service_origin_utf8": String::from_utf8(fixture.configure_result.status.configuration.service_origin_utf8.clone())?,
                "publication_ttl_seconds": 900,
                "refresh_before_seconds": 120,
                "announcement_interval_seconds": 300,
                "announcement_validity_seconds": 900,
                "recipient_ticket_hex": hex(&fixture.ticket),
                "recipient_ticket_bytes": fixture.ticket.len(),
                "recipient_ticket_digest_hex": hex(&inbound_locator.recipient_ticket.digest),
                "outbound_channel_id_hex": hex(&fixture.configure_result.status.outbound_channel_id),
                "inbound_channel_id_hex": hex(&fixture.configure_result.status.inbound_channel_id),
                "outbound_publication_generation": 3,
                "inbound_publication_generation": 4,
                "observation_id_hex": hex(&inbound_locator.observation_id),
                "inbound_published_at_unix_seconds": inbound_locator.published_at_unix_seconds,
                "inbound_expires_at_unix_seconds": inbound_locator.expires_at_unix_seconds,
                "announcement_policy_id_hex": hex(&announcement_policy_id),
                "last_attempt_unix_seconds": last_attempt_unix_seconds,
                "last_success_unix_seconds": last_success_unix_seconds,
                "next_attempt_unix_seconds": next_attempt_unix_seconds,
                "last_bundle_id_hex": hex(&last_bundle_id),
                "last_transport": "relay",
                "enabled": true,
                "retry_base_seconds": 5,
                "retry_max_seconds": 300,
                "allowed_network_mask": 3,
                "current_network": "wifi",
                "configure_request_id_hex": hex(&fixture.configure_request.request_id),
                "configure_issued_at_unix_seconds": fixture.configure_request.issued_at_unix_seconds,
                "configure_expires_at_unix_seconds": fixture.configure_request.expires_at_unix_seconds,
                "status_request_id_hex": hex(&fixture.status_request.request_id),
                "status_issued_at_unix_seconds": fixture.status_request.issued_at_unix_seconds,
                "status_expires_at_unix_seconds": fixture.status_request.expires_at_unix_seconds
        });
        let expected = json!({
                "policy_id_hex": hex(&fixture.configure_result.status.policy_id),
                "snapshot_id_hex": hex(&fixture.status_result.snapshot_id),
                "configure": {
                    "request": {
                        "cbor_hex": hex(&configure_request_bytes),
                        "object_id_hex": hex(&runtime_ipc_own_device_discovery_request_digest_v1(&fixture.configure_request)?)
                    },
                    "response": {
                        "content_cbor_hex": hex(&encode_response_content(&configure_response.content)?),
                        "signing_bytes_hex": hex(&runtime_ipc_own_device_discovery_response_signing_bytes_v1(&configure_response.content)?),
                        "signature_hex": hex(&configure_response.signature),
                        "signed_cbor_hex": hex(&configure_response_bytes),
                        "object_id_hex": hex(&runtime_ipc_own_device_discovery_response_id_v1(&configure_response)?)
                    }
                },
                "status": {
                    "request": {
                        "cbor_hex": hex(&status_request_bytes),
                        "object_id_hex": hex(&runtime_ipc_own_device_discovery_request_digest_v1(&fixture.status_request)?)
                    },
                    "response": {
                        "content_cbor_hex": hex(&encode_response_content(&status_response.content)?),
                        "signing_bytes_hex": hex(&runtime_ipc_own_device_discovery_response_signing_bytes_v1(&status_response.content)?),
                        "signature_hex": hex(&status_response.signature),
                        "signed_cbor_hex": hex(&status_response_bytes),
                        "object_id_hex": hex(&runtime_ipc_own_device_discovery_response_id_v1(&status_response)?)
                    }
                }
        });
        let document = json!({
            "schema_version": 1,
            "milestone": "M1.1.41",
            "status": "candidate-not-active",
            "dependencies": [
                "CANONICAL-RUNTIME-IPC-VECTORS.json",
                "CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json"
            ],
            "domains": {
                "policy_id": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_POLICY_ID_DOMAIN),
                "snapshot_id": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_SNAPSHOT_ID_DOMAIN),
                "request_digest": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REQUEST_DIGEST_DOMAIN),
                "response_signature": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_SIGNATURE_DOMAIN),
                "response_id": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_ID_DOMAIN)
            },
            "input": input,
            "expected": expected
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
