//! Inactive deterministic-CBOR roster-wide own-device automation boundary.
//!
//! One local Device-signed parent policy projects the exact Root-signed roster
//! into the pairwise discovery policies defined by M1.1.41. Active sibling
//! Devices receive enabled children; permanently revoked Devices retain a
//! disabled auditable tombstone. The full projection commits atomically and is
//! processed in Device-ID order with at most one foreground workflow. Private
//! capabilities, ticket plaintext and filesystem paths never cross this IPC
//! surface. Network execution and OS background services are not activated by
//! this candidate.

use crate::{
    canonical_account_v1::{
        AccountDecoder, CanonicalSignedAccountDeviceListV1,
        decode_and_verify_signed_account_device_list_v1, encode_signed_account_device_list_v1,
        signed_envelope, verify_signed_account_device_list_v1,
    },
    canonical_runtime_ipc_directory_v1::runtime_ipc_device_list_digest_v1,
    canonical_runtime_ipc_own_device_discovery_v1::{
        CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1,
        CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1,
        CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1,
        decode_network as decode_discovery_network,
        decode_policy_status as decode_discovery_policy_status,
        encode_network as encode_discovery_network,
        encode_policy_status as encode_discovery_policy_status,
        runtime_ipc_own_device_discovery_policy_id_v1,
        validate_configuration as validate_discovery_configuration,
        validate_policy_status_shape as validate_discovery_policy_status_shape,
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
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_REQUEST_BYTES: usize = 768 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_BYTES: usize = 768 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_CHILDREN: usize = 64;
pub const MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_PARALLEL_WORKFLOWS: u8 = 1;

pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_POLICY_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-roster-policy-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_SNAPSHOT_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-roster-snapshot-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-roster-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-roster-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-own-device-roster-response-id:cbor-v1\0";

const COMMAND_CONFIGURE: u64 = 0;
const COMMAND_STATUS: u64 = 1;
const RESULT_CONFIGURED: u64 = 0;
const RESULT_STATUS: u64 = 1;
const RESULT_ERROR: u64 = 2;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const CHILD_ACTIVE: u64 = 0;
const CHILD_RETIRED: u64 = 1;
const RESTART_NOT_REQUIRED: u64 = 0;
const EXECUTION_FOREGROUND_RUNTIME: u64 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterPreviousPolicyV1 {
    pub generation: u64,
    pub policy_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1 {
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

pub type CanonicalRuntimeIpcOwnDeviceRosterNetworkV1 =
    CanonicalRuntimeIpcOwnDeviceDiscoveryNetworkV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1 {
    pub configuration: CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1,
    pub generation: u64,
    pub previous_policy_id: Option<[u8; 32]>,
    pub policy_id: [u8; 32],
    pub configured_at_unix_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1 {
    Active,
    Retired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1 {
    pub lifecycle: CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1,
    pub discovery: CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceRosterCommandV1 {
    ConfigureOwnDeviceRosterAutomation {
        current_device_list: Box<CanonicalSignedAccountDeviceListV1>,
        expected_previous: Option<CanonicalRuntimeIpcOwnDeviceRosterPreviousPolicyV1>,
        configuration: CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1,
    },
    OwnDeviceRosterAutomationStatus {
        current_device_list: Box<CanonicalSignedAccountDeviceListV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcOwnDeviceRosterCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceRosterRestartStateV1 {
    NotRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceRosterExecutionScopeV1 {
    ForegroundRuntimeOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterConfiguredV1 {
    pub outcome: CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1,
    pub authority_revision: u64,
    pub device_list_digest: [u8; 32],
    pub current_network: CanonicalRuntimeIpcOwnDeviceRosterNetworkV1,
    pub policy: CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
    pub snapshot_id: [u8; 32],
    pub children: Vec<CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1>,
    pub active_children: u8,
    pub retired_children: u8,
    pub max_parallel_workflows: u8,
    pub restart_state: CanonicalRuntimeIpcOwnDeviceRosterRestartStateV1,
    pub execution_scope: CanonicalRuntimeIpcOwnDeviceRosterExecutionScopeV1,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterStatusV1 {
    pub authority_revision: u64,
    pub device_list_digest: [u8; 32],
    pub current_network: CanonicalRuntimeIpcOwnDeviceRosterNetworkV1,
    pub policy: CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
    pub snapshot_id: [u8; 32],
    pub children: Vec<CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1>,
    pub active_children: u8,
    pub retired_children: u8,
    pub max_parallel_workflows: u8,
    pub execution_scope: CanonicalRuntimeIpcOwnDeviceRosterExecutionScopeV1,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcOwnDeviceRosterResultV1 {
    OwnDeviceRosterAutomationConfigured(Box<CanonicalRuntimeIpcOwnDeviceRosterConfiguredV1>),
    OwnDeviceRosterAutomationStatus(Box<CanonicalRuntimeIpcOwnDeviceRosterStatusV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcOwnDeviceRosterResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1 {
    pub content: CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_own_device_roster_request_v1(
    request: &CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
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
        "own-device roster request",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_own_device_roster_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "own-device roster request",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-own-device-roster-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-own-device-roster-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcOwnDeviceRosterRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-own-device-roster-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-own-device-roster-request-v1.request-id")?,
        bearer_token: decoder
            .fixed_bytes("runtime-ipc-own-device-roster-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_own_device_roster_request_digest_v1(
    request: &CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_own_device_roster_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_own_device_roster_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
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
    Ok(())
}

pub fn runtime_ipc_own_device_roster_policy_id_v1(
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    authority_revision: u64,
    device_list_digest: &[u8; 32],
    policy: &CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    validate_configuration(
        &policy.configuration,
        CanonicalRuntimeIpcError::InvalidResponse,
    )?;
    if *account_id == [0; 32]
        || *local_device_id == [0; 32]
        || authority_revision == 0
        || *device_list_digest == [0; 32]
        || policy.generation == 0
        || policy.configured_at_unix_seconds == 0
        || (policy.generation == 1) != policy.previous_policy_id.is_none()
        || policy.previous_policy_id == Some([0; 32])
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut content = Vec::new();
    push_array_header(&mut content, 9);
    push_unsigned(&mut content, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut content, account_id);
    push_bytes(&mut content, local_device_id);
    push_unsigned(&mut content, authority_revision);
    push_bytes(&mut content, device_list_digest);
    push_unsigned(&mut content, policy.generation);
    encode_optional_id(&mut content, policy.previous_policy_id);
    push_unsigned(&mut content, policy.configured_at_unix_seconds);
    encode_configuration(&mut content, &policy.configuration);
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_POLICY_ID_DOMAIN,
        &content,
    ))
}

pub fn runtime_ipc_own_device_roster_snapshot_id_v1(
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    authority_revision: u64,
    device_list_digest: &[u8; 32],
    current_network: CanonicalRuntimeIpcOwnDeviceRosterNetworkV1,
    policy: &CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
    children: &[CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    if *account_id == [0; 32]
        || *local_device_id == [0; 32]
        || authority_revision == 0
        || *device_list_digest == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    validate_parent_policy_shape(policy)?;
    validate_child_order(children, CanonicalRuntimeIpcError::InvalidResponse)?;
    for child in children {
        validate_child_shape(child, current_network)?;
    }
    let mut content = Vec::new();
    push_array_header(&mut content, 8);
    push_unsigned(&mut content, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut content, account_id);
    push_bytes(&mut content, local_device_id);
    push_unsigned(&mut content, authority_revision);
    push_bytes(&mut content, device_list_digest);
    encode_discovery_network(&mut content, current_network);
    encode_parent_policy_status(&mut content, policy);
    push_array_header(&mut content, children.len() as u64);
    for child in children {
        encode_child_status(&mut content, child);
    }
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_SNAPSHOT_ID_DOMAIN,
        &content,
    ))
}

pub fn runtime_ipc_own_device_roster_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_own_device_roster_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcOwnDeviceRosterResultV1,
) -> Result<CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_own_device_roster_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&result, request, descriptor)?;
    let content = CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_own_device_roster_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_own_device_roster_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1 { content, signature };
    verify_signed_runtime_ipc_own_device_roster_response_v1(&response)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_own_device_roster_response_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_own_device_roster_response_v1(response)?;
    let encoded = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &encoded,
        "own-device roster response",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_runtime_ipc_own_device_roster_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "own-device roster response",
        MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-own-device-roster-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-own-device-roster-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-own-device-roster-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1 { content, signature };
    verify_signed_runtime_ipc_own_device_roster_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_own_device_roster_response_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_own_device_roster_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_own_device_roster_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1,
    request: &CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_own_device_roster_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    verify_signed_runtime_ipc_own_device_roster_response_v1(response)?;
    if response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest
            != runtime_ipc_own_device_roster_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&response.content.result, request, descriptor)
}

pub fn runtime_ipc_own_device_roster_response_id_v1(
    response: &CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_own_device_roster_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
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
        CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
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
        CanonicalRuntimeIpcOwnDeviceRosterCommandV1::OwnDeviceRosterAutomationStatus {
            current_device_list,
        } => verify_signed_account_device_list_v1(current_device_list)?,
    }
    Ok(())
}

fn validate_configuration(
    configuration: &CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1,
    error: CanonicalRuntimeIpcError,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_discovery_configuration(
        &derived_discovery_configuration(
            configuration,
            [1; 32],
            CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active,
        ),
        error,
    )
}

fn derived_discovery_configuration(
    configuration: &CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1,
    recipient_device_id: [u8; 32],
    lifecycle: CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1,
) -> CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1 {
    CanonicalRuntimeIpcOwnDeviceDiscoveryConfigurationV1 {
        recipient_device_id,
        enabled: configuration.enabled
            && lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active,
        service_origin_utf8: configuration.service_origin_utf8.clone(),
        publication_ttl_seconds: configuration.publication_ttl_seconds,
        refresh_before_seconds: configuration.refresh_before_seconds,
        announcement_interval_seconds: configuration.announcement_interval_seconds,
        announcement_validity_seconds: configuration.announcement_validity_seconds,
        retry_base_seconds: configuration.retry_base_seconds,
        retry_max_seconds: configuration.retry_max_seconds,
        allowed_network_mask: configuration.allowed_network_mask,
    }
}

fn validate_parent_policy(
    policy: &CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    authority_revision: u64,
    device_list_digest: &[u8; 32],
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_parent_policy_shape(policy)?;
    if policy.policy_id
        != runtime_ipc_own_device_roster_policy_id_v1(
            account_id,
            local_device_id,
            authority_revision,
            device_list_digest,
            policy,
        )?
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_parent_policy_shape(
    policy: &CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_configuration(
        &policy.configuration,
        CanonicalRuntimeIpcError::InvalidResponse,
    )?;
    if policy.generation == 0
        || policy.configured_at_unix_seconds == 0
        || (policy.generation == 1) != policy.previous_policy_id.is_none()
        || policy.previous_policy_id == Some([0; 32])
        || policy.policy_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_child_shape(
    child: &CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1,
    current_network: CanonicalRuntimeIpcOwnDeviceRosterNetworkV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_discovery_policy_status_shape(&child.discovery, current_network)?;
    if child.lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Retired
        && child.discovery.configuration.enabled
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_child_order(
    children: &[CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1],
    error: CanonicalRuntimeIpcError,
) -> Result<(), CanonicalRuntimeIpcError> {
    if children.len() > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_CHILDREN
        || children.windows(2).any(|pair| {
            pair[0].discovery.configuration.recipient_device_id
                >= pair[1].discovery.configuration.recipient_device_id
        })
    {
        return Err(error);
    }
    Ok(())
}

fn expected_child_lifecycles(
    list: &CanonicalSignedAccountDeviceListV1,
    local_device_id: &[u8; 32],
) -> Result<
    Vec<([u8; 32], CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1)>,
    CanonicalRuntimeIpcError,
> {
    let mut expected = Vec::new();
    for certificate in &list.content.devices {
        if certificate.content.device_id != *local_device_id {
            expected.push((
                certificate.content.device_id,
                CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active,
            ));
        }
    }
    for revocation in &list.content.authority_snapshot.content.revocations {
        if revocation.content.device_id != *local_device_id {
            expected.push((
                revocation.content.device_id,
                CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Retired,
            ));
        }
    }
    expected.sort_by_key(|(device_id, _)| *device_id);
    if expected.len() > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_CHILDREN
        || expected.windows(2).any(|pair| pair[0].0 == pair[1].0)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(expected)
}

#[allow(clippy::too_many_arguments)]
fn validate_projection(
    list: &CanonicalSignedAccountDeviceListV1,
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    authority_revision: u64,
    device_list_digest: &[u8; 32],
    current_network: CanonicalRuntimeIpcOwnDeviceRosterNetworkV1,
    policy: &CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
    snapshot_id: &[u8; 32],
    children: &[CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1],
    active_children: u8,
    retired_children: u8,
    max_parallel_workflows: u8,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_parent_policy(
        policy,
        account_id,
        local_device_id,
        authority_revision,
        device_list_digest,
    )?;
    validate_child_order(children, CanonicalRuntimeIpcError::InvalidResponse)?;
    let expected = expected_child_lifecycles(list, local_device_id)?;
    if children.len() != expected.len()
        || max_parallel_workflows != MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_PARALLEL_WORKFLOWS
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let mut active = 0usize;
    let mut retired = 0usize;
    for (child, (recipient_device_id, lifecycle)) in children.iter().zip(expected) {
        validate_child_shape(child, current_network)?;
        if child.lifecycle != lifecycle
            || child.discovery.configuration
                != derived_discovery_configuration(
                    &policy.configuration,
                    recipient_device_id,
                    lifecycle,
                )
            || child.discovery.configured_at_unix_seconds != policy.configured_at_unix_seconds
            || child.discovery.policy_id
                != runtime_ipc_own_device_discovery_policy_id_v1(
                    account_id,
                    local_device_id,
                    authority_revision,
                    device_list_digest,
                    child.discovery.generation,
                    child.discovery.previous_policy_id,
                    child.discovery.configured_at_unix_seconds,
                    &child.discovery.configuration,
                    &child.discovery.outbound_channel_id,
                    &child.discovery.inbound_channel_id,
                )?
        {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
        match lifecycle {
            CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active => active += 1,
            CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Retired => retired += 1,
        }
    }
    if usize::from(active_children) != active
        || usize::from(retired_children) != retired
        || active + retired != children.len()
        || *snapshot_id
            != runtime_ipc_own_device_roster_snapshot_id_v1(
                account_id,
                local_device_id,
                authority_revision,
                device_list_digest,
                current_network,
                policy,
                children,
            )?
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationConfigured(
            configured,
        ) => {
            if configured.authority_revision == 0
                || configured.device_list_digest == [0; 32]
                || configured.snapshot_id == [0; 32]
                || configured.os_background_service_enabled
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_parent_policy_shape(&configured.policy)?;
            validate_child_order(
                &configured.children,
                CanonicalRuntimeIpcError::InvalidResponse,
            )?;
            for child in &configured.children {
                validate_child_shape(child, configured.current_network)?;
            }
            validate_declared_child_summary(
                &configured.children,
                configured.active_children,
                configured.retired_children,
                configured.max_parallel_workflows,
            )?;
        }
        CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationStatus(status) => {
            if status.authority_revision == 0
                || status.device_list_digest == [0; 32]
                || status.snapshot_id == [0; 32]
                || status.os_background_service_enabled
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_parent_policy_shape(&status.policy)?;
            validate_child_order(&status.children, CanonicalRuntimeIpcError::InvalidResponse)?;
            for child in &status.children {
                validate_child_shape(child, status.current_network)?;
            }
            validate_declared_child_summary(
                &status.children,
                status.active_children,
                status.retired_children,
                status.max_parallel_workflows,
            )?;
        }
        CanonicalRuntimeIpcOwnDeviceRosterResultV1::Error { code, message_utf8 } => {
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

fn validate_declared_child_summary(
    children: &[CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1],
    active_children: u8,
    retired_children: u8,
    max_parallel_workflows: u8,
) -> Result<(), CanonicalRuntimeIpcError> {
    let active = children
        .iter()
        .filter(|child| {
            child.lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active
        })
        .count();
    let retired = children.len().saturating_sub(active);
    if usize::from(active_children) != active
        || usize::from(retired_children) != retired
        || max_parallel_workflows != MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_PARALLEL_WORKFLOWS
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_result_for_request(
    result: &CanonicalRuntimeIpcOwnDeviceRosterResultV1,
    request: &CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_own_device_roster_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result: result.clone(),
    })?;
    let list = command_device_list(&request.command);
    let revision = list.content.authority_snapshot.content.revision;
    let digest = runtime_ipc_device_list_digest_v1(list)?;
    match (&request.command, result) {
        (
            CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
                expected_previous,
                configuration,
                ..
            },
            CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationConfigured(
                configured,
            ),
        ) => {
            if configured.authority_revision != revision
                || configured.device_list_digest != digest
                || &configured.policy.configuration != configuration
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            match configured.outcome {
                CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::Committed => {
                    let expected_generation = expected_previous
                        .map_or(Some(1), |previous| previous.generation.checked_add(1))
                        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
                    if configured.policy.generation != expected_generation
                        || configured.policy.previous_policy_id
                            != expected_previous.map(|previous| previous.policy_id)
                        || configured.policy.configured_at_unix_seconds
                            != request.issued_at_unix_seconds
                    {
                        return Err(CanonicalRuntimeIpcError::ContextMismatch);
                    }
                }
                CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::AlreadyCommitted => {
                    let Some(previous) = expected_previous else {
                        return Err(CanonicalRuntimeIpcError::ContextMismatch);
                    };
                    if configured.policy.generation != previous.generation
                        || configured.policy.policy_id != previous.policy_id
                        || configured.policy.configured_at_unix_seconds
                            > request.issued_at_unix_seconds
                    {
                        return Err(CanonicalRuntimeIpcError::ContextMismatch);
                    }
                }
            }
            validate_projection(
                list,
                &descriptor.content.account_id,
                &descriptor.content.device_id,
                revision,
                &digest,
                configured.current_network,
                &configured.policy,
                &configured.snapshot_id,
                &configured.children,
                configured.active_children,
                configured.retired_children,
                configured.max_parallel_workflows,
            )?;
        }
        (
            CanonicalRuntimeIpcOwnDeviceRosterCommandV1::OwnDeviceRosterAutomationStatus { .. },
            CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationStatus(status),
        ) => {
            if status.authority_revision != revision || status.device_list_digest != digest {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            validate_projection(
                list,
                &descriptor.content.account_id,
                &descriptor.content.device_id,
                revision,
                &digest,
                status.current_network,
                &status.policy,
                &status.snapshot_id,
                &status.children,
                status.active_children,
                status.retired_children,
                status.max_parallel_workflows,
            )?;
        }
        (_, CanonicalRuntimeIpcOwnDeviceRosterResultV1::Error { .. }) => {}
        _ => return Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
    Ok(())
}

fn command_device_list(
    command: &CanonicalRuntimeIpcOwnDeviceRosterCommandV1,
) -> &CanonicalSignedAccountDeviceListV1 {
    match command {
        CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
            current_device_list,
            ..
        }
        | CanonicalRuntimeIpcOwnDeviceRosterCommandV1::OwnDeviceRosterAutomationStatus {
            current_device_list,
        } => current_device_list,
    }
}

fn encode_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcOwnDeviceRosterCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
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
        CanonicalRuntimeIpcOwnDeviceRosterCommandV1::OwnDeviceRosterAutomationStatus {
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
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-roster-command-v1")?;
    let variant = decoder.unsigned("runtime-ipc-own-device-roster-command-v1.variant")?;
    match variant {
        COMMAND_CONFIGURE if length == 4 => Ok(
            CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
                current_device_list: Box::new(decode_and_verify_signed_account_device_list_v1(
                    decoder
                        .bytes("runtime-ipc-own-device-roster-command-v1.current-device-list")?,
                )?),
                expected_previous: decode_optional_previous(decoder)?,
                configuration: decode_configuration(decoder)?,
            },
        ),
        COMMAND_STATUS if length == 2 => Ok(
            CanonicalRuntimeIpcOwnDeviceRosterCommandV1::OwnDeviceRosterAutomationStatus {
                current_device_list: Box::new(decode_and_verify_signed_account_device_list_v1(
                    decoder
                        .bytes("runtime-ipc-own-device-roster-command-v1.current-device-list")?,
                )?),
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_configuration(
    output: &mut Vec<u8>,
    configuration: &CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1,
) {
    push_array_header(output, 10);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
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
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-roster-configuration-v1", 10)?;
    decoder.expect_version("runtime-ipc-own-device-roster-configuration-v1.schema-version")?;
    let configuration = CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1 {
        enabled: decode_bool(
            decoder,
            "runtime-ipc-own-device-roster-configuration-v1.enabled",
            false,
        )?,
        service_origin_utf8: decoder
            .bytes("runtime-ipc-own-device-roster-configuration-v1.service-origin-utf8")?
            .to_vec(),
        publication_ttl_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.publication-ttl-seconds")?,
        refresh_before_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.refresh-before-seconds")?,
        announcement_interval_seconds: decoder.unsigned(
            "runtime-ipc-own-device-roster-configuration-v1.announcement-interval-seconds",
        )?,
        announcement_validity_seconds: decoder.unsigned(
            "runtime-ipc-own-device-roster-configuration-v1.announcement-validity-seconds",
        )?,
        retry_base_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.retry-base-seconds")?,
        retry_max_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.retry-max-seconds")?,
        allowed_network_mask: u8::try_from(
            decoder
                .unsigned("runtime-ipc-own-device-roster-configuration-v1.allowed-network-mask")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
    };
    validate_configuration(&configuration, CanonicalRuntimeIpcError::InvalidRequest)?;
    Ok(configuration)
}

fn encode_optional_previous(
    output: &mut Vec<u8>,
    value: Option<CanonicalRuntimeIpcOwnDeviceRosterPreviousPolicyV1>,
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
) -> Result<Option<CanonicalRuntimeIpcOwnDeviceRosterPreviousPolicyV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-roster-previous-policy-v1")?;
    match decoder.unsigned("runtime-ipc-own-device-roster-previous-policy-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 3 => {
            Ok(Some(CanonicalRuntimeIpcOwnDeviceRosterPreviousPolicyV1 {
                generation: decoder
                    .unsigned("runtime-ipc-own-device-roster-previous-policy-v1.generation")?,
                policy_id: decoder
                    .fixed_bytes("runtime-ipc-own-device-roster-previous-policy-v1.policy-id")?,
            }))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_parent_policy_status(
    output: &mut Vec<u8>,
    policy: &CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1,
) {
    push_array_header(output, 6);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    encode_configuration(output, &policy.configuration);
    push_unsigned(output, policy.generation);
    encode_optional_id(output, policy.previous_policy_id);
    push_bytes(output, &policy.policy_id);
    push_unsigned(output, policy.configured_at_unix_seconds);
}

fn decode_parent_policy_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-roster-policy-status-v1", 6)?;
    decoder.expect_version("runtime-ipc-own-device-roster-policy-status-v1.schema-version")?;
    let policy = CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1 {
        configuration: decode_configuration_response(decoder)?,
        generation: decoder
            .unsigned("runtime-ipc-own-device-roster-policy-status-v1.generation")?,
        previous_policy_id: decode_optional_id(decoder, false)?,
        policy_id: decoder
            .fixed_bytes("runtime-ipc-own-device-roster-policy-status-v1.policy-id")?,
        configured_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-own-device-roster-policy-status-v1.configured-at-unix-seconds",
        )?,
    };
    validate_parent_policy_shape(&policy)?;
    Ok(policy)
}

fn encode_child_status(
    output: &mut Vec<u8>,
    child: &CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1,
) {
    push_array_header(output, 3);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    encode_child_lifecycle(output, child.lifecycle);
    let mut discovery = Vec::new();
    encode_discovery_policy_status(&mut discovery, &child.discovery);
    push_bytes(output, &discovery);
}

fn decode_child_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-roster-child-status-v1", 3)?;
    decoder.expect_version("runtime-ipc-own-device-roster-child-status-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1 {
        lifecycle: decode_child_lifecycle(decoder)?,
        discovery: decode_embedded_discovery_policy_status(decoder.bytes(
            "runtime-ipc-own-device-roster-child-status-v1.discovery-policy-status-cbor",
        )?)?,
    })
}

fn decode_embedded_discovery_policy_status(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1, CanonicalRuntimeIpcError> {
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let status = decode_discovery_policy_status(&mut decoder)?;
    decoder.finish()?;
    Ok(status)
}

fn encode_child_lifecycle(
    output: &mut Vec<u8>,
    lifecycle: CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1,
) {
    push_unsigned(
        output,
        match lifecycle {
            CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active => CHILD_ACTIVE,
            CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Retired => CHILD_RETIRED,
        },
    );
}

fn decode_child_lifecycle(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-roster-child-lifecycle-v1")? {
        CHILD_ACTIVE => Ok(CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active),
        CHILD_RETIRED => Ok(CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Retired),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_configuration_response(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-roster-configuration-v1", 10)?;
    decoder.expect_version("runtime-ipc-own-device-roster-configuration-v1.schema-version")?;
    let configuration = CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1 {
        enabled: decode_bool(
            decoder,
            "runtime-ipc-own-device-roster-configuration-v1.enabled",
            true,
        )?,
        service_origin_utf8: decoder
            .bytes("runtime-ipc-own-device-roster-configuration-v1.service-origin-utf8")?
            .to_vec(),
        publication_ttl_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.publication-ttl-seconds")?,
        refresh_before_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.refresh-before-seconds")?,
        announcement_interval_seconds: decoder.unsigned(
            "runtime-ipc-own-device-roster-configuration-v1.announcement-interval-seconds",
        )?,
        announcement_validity_seconds: decoder.unsigned(
            "runtime-ipc-own-device-roster-configuration-v1.announcement-validity-seconds",
        )?,
        retry_base_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.retry-base-seconds")?,
        retry_max_seconds: decoder
            .unsigned("runtime-ipc-own-device-roster-configuration-v1.retry-max-seconds")?,
        allowed_network_mask: u8::try_from(
            decoder
                .unsigned("runtime-ipc-own-device-roster-configuration-v1.allowed-network-mask")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    };
    validate_configuration(&configuration, CanonicalRuntimeIpcError::InvalidResponse)?;
    Ok(configuration)
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1,
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
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-own-device-roster-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-own-device-roster-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcOwnDeviceRosterResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-own-device-roster-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-own-device-roster-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-own-device-roster-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-own-device-roster-response-content-v1.responder-device-id")?,
        result: decode_result(decoder)?,
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn encode_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcOwnDeviceRosterResultV1) {
    match result {
        CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationConfigured(
            configured,
        ) => {
            push_array_header(output, 14);
            push_unsigned(output, RESULT_CONFIGURED);
            encode_outcome(output, configured.outcome);
            push_unsigned(output, configured.authority_revision);
            push_bytes(output, &configured.device_list_digest);
            encode_discovery_network(output, configured.current_network);
            encode_parent_policy_status(output, &configured.policy);
            push_bytes(output, &configured.snapshot_id);
            encode_children(output, &configured.children);
            push_unsigned(output, u64::from(configured.active_children));
            push_unsigned(output, u64::from(configured.retired_children));
            push_unsigned(output, u64::from(configured.max_parallel_workflows));
            push_unsigned(output, RESTART_NOT_REQUIRED);
            push_unsigned(output, EXECUTION_FOREGROUND_RUNTIME);
            encode_bool(output, configured.os_background_service_enabled);
        }
        CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationStatus(status) => {
            push_array_header(output, 12);
            push_unsigned(output, RESULT_STATUS);
            push_unsigned(output, status.authority_revision);
            push_bytes(output, &status.device_list_digest);
            encode_discovery_network(output, status.current_network);
            encode_parent_policy_status(output, &status.policy);
            push_bytes(output, &status.snapshot_id);
            encode_children(output, &status.children);
            push_unsigned(output, u64::from(status.active_children));
            push_unsigned(output, u64::from(status.retired_children));
            push_unsigned(output, u64::from(status.max_parallel_workflows));
            push_unsigned(output, EXECUTION_FOREGROUND_RUNTIME);
            encode_bool(output, status.os_background_service_enabled);
        }
        CanonicalRuntimeIpcOwnDeviceRosterResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn encode_children(
    output: &mut Vec<u8>,
    children: &[CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1],
) {
    push_array_header(output, children.len() as u64);
    for child in children {
        encode_child_status(output, child);
    }
}

fn decode_children(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Vec<CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1>, CanonicalRuntimeIpcError> {
    let count = decoder.array("runtime-ipc-own-device-roster-children-v1")?;
    if count > MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_CHILDREN {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut children = Vec::with_capacity(count);
    for _ in 0..count {
        children.push(decode_child_status(decoder)?);
    }
    Ok(children)
}

fn decode_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-own-device-roster-result-v1")?;
    let variant = decoder.unsigned("runtime-ipc-own-device-roster-result-v1.variant")?;
    match variant {
        RESULT_CONFIGURED if length == 14 => Ok(
            CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationConfigured(
                Box::new(CanonicalRuntimeIpcOwnDeviceRosterConfiguredV1 {
                    outcome: decode_outcome(decoder)?,
                    authority_revision: decoder.unsigned(
                        "runtime-ipc-own-device-roster-configured-v1.authority-revision",
                    )?,
                    device_list_digest: decoder.fixed_bytes(
                        "runtime-ipc-own-device-roster-configured-v1.device-list-digest",
                    )?,
                    current_network: decode_discovery_network(decoder)?,
                    policy: decode_parent_policy_status(decoder)?,
                    snapshot_id: decoder
                        .fixed_bytes("runtime-ipc-own-device-roster-configured-v1.snapshot-id")?,
                    children: decode_children(decoder)?,
                    active_children: u8::try_from(
                        decoder.unsigned(
                            "runtime-ipc-own-device-roster-configured-v1.active-children",
                        )?,
                    )
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
                    retired_children: u8::try_from(decoder.unsigned(
                        "runtime-ipc-own-device-roster-configured-v1.retired-children",
                    )?)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
                    max_parallel_workflows: u8::try_from(decoder.unsigned(
                        "runtime-ipc-own-device-roster-configured-v1.max-parallel-workflows",
                    )?)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
                    restart_state: decode_restart(decoder)?,
                    execution_scope: decode_execution(decoder)?,
                    os_background_service_enabled: decode_bool(
                        decoder,
                        "runtime-ipc-own-device-roster-configured-v1.os-background-service-enabled",
                        true,
                    )?,
                }),
            ),
        ),
        RESULT_STATUS if length == 12 => {
            let authority_revision =
                decoder.unsigned("runtime-ipc-own-device-roster-status-v1.authority-revision")?;
            let device_list_digest = decoder
                .fixed_bytes("runtime-ipc-own-device-roster-status-v1.device-list-digest")?;
            let current_network = decode_discovery_network(decoder)?;
            let policy = decode_parent_policy_status(decoder)?;
            let snapshot_id =
                decoder.fixed_bytes("runtime-ipc-own-device-roster-status-v1.snapshot-id")?;
            let children = decode_children(decoder)?;
            let active_children = u8::try_from(
                decoder.unsigned("runtime-ipc-own-device-roster-status-v1.active-children")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            let retired_children = u8::try_from(
                decoder.unsigned("runtime-ipc-own-device-roster-status-v1.retired-children")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            let max_parallel_workflows = u8::try_from(
                decoder
                    .unsigned("runtime-ipc-own-device-roster-status-v1.max-parallel-workflows")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            let execution_scope = decode_execution(decoder)?;
            let os_background_service_enabled = decode_bool(
                decoder,
                "runtime-ipc-own-device-roster-status-v1.os-background-service-enabled",
                true,
            )?;
            Ok(
                CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationStatus(
                    Box::new(CanonicalRuntimeIpcOwnDeviceRosterStatusV1 {
                        authority_revision,
                        device_list_digest,
                        current_network,
                        policy,
                        snapshot_id,
                        children,
                        active_children,
                        retired_children,
                        max_parallel_workflows,
                        execution_scope,
                        os_background_service_enabled,
                    }),
                ),
            )
        }
        RESULT_ERROR if length == 3 => Ok(CanonicalRuntimeIpcOwnDeviceRosterResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-own-device-roster-error-v1.code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-own-device-roster-error-v1.message")?
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
    let length = decoder.array("runtime-ipc-own-device-roster-optional-id-v1")?;
    match decoder.unsigned("runtime-ipc-own-device-roster-optional-id-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 2 => Ok(Some(
            decoder.fixed_bytes("runtime-ipc-own-device-roster-optional-id-v1.value")?,
        )),
        _ if request => Err(CanonicalRuntimeIpcError::InvalidRequest),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_outcome(output: &mut Vec<u8>, value: CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::Committed => OUTCOME_COMMITTED,
            CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::AlreadyCommitted => {
                OUTCOME_ALREADY_COMMITTED
            }
        },
    );
}

fn decode_outcome(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-roster-outcome-v1")? {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_restart(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterRestartStateV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-roster-restart-v1")? {
        RESTART_NOT_REQUIRED => Ok(CanonicalRuntimeIpcOwnDeviceRosterRestartStateV1::NotRequired),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_execution(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcOwnDeviceRosterExecutionScopeV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-own-device-roster-execution-scope-v1")? {
        EXECUTION_FOREGROUND_RUNTIME => {
            Ok(CanonicalRuntimeIpcOwnDeviceRosterExecutionScopeV1::ForegroundRuntimeOnly)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
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
    use crate::{
        canonical_runtime_ipc_own_device_announcement_v1::runtime_ipc_own_device_announcement_artifact_digest_v1,
        canonical_runtime_ipc_own_device_discovery_v1::{
            CanonicalRuntimeIpcOwnDeviceDiscoveryArtifactV1,
            CanonicalRuntimeIpcOwnDeviceDiscoveryInstalledLocatorV1,
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1,
            CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1,
        },
        canonical_runtime_ipc_v1::decode_and_verify_signed_runtime_ipc_descriptor_v1,
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        device_key: SigningKey,
        current: CanonicalSignedAccountDeviceListV1,
        ticket: Vec<u8>,
        configure_request: CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
        configure_result: CanonicalRuntimeIpcOwnDeviceRosterConfiguredV1,
        status_request: CanonicalRuntimeIpcOwnDeviceRosterRequestV1,
        status_result: CanonicalRuntimeIpcOwnDeviceRosterStatusV1,
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
            "/../../CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json"
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
        let issued_at = 1_800_003_000;
        let configuration = CanonicalRuntimeIpcOwnDeviceRosterConfigurationV1 {
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
        let configure_request = CanonicalRuntimeIpcOwnDeviceRosterRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0xda; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: issued_at,
            expires_at_unix_seconds: issued_at + 30,
            command:
                CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
                    current_device_list: Box::new(current.clone()),
                    expected_previous: None,
                    configuration: configuration.clone(),
                },
        };
        let revision = current.content.authority_snapshot.content.revision;
        let digest = runtime_ipc_device_list_digest_v1(&current)?;
        let mut parent = CanonicalRuntimeIpcOwnDeviceRosterPolicyStatusV1 {
            configuration,
            generation: 1,
            previous_policy_id: None,
            policy_id: [0; 32],
            configured_at_unix_seconds: issued_at,
        };
        parent.policy_id = runtime_ipc_own_device_roster_policy_id_v1(
            &descriptor.content.account_id,
            &descriptor.content.device_id,
            revision,
            &digest,
            &parent,
        )?;
        let expected = expected_child_lifecycles(&current, &descriptor.content.device_id)?;
        if expected.len() < 2
            || !expected.iter().any(|(_, lifecycle)| {
                *lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Retired
            })
        {
            return Err("fixture must contain active and retired sibling Devices".into());
        }
        let mut children = Vec::new();
        for (index, (recipient_device_id, lifecycle)) in expected.into_iter().enumerate() {
            let index = u8::try_from(index)?;
            let discovery_configuration = derived_discovery_configuration(
                &parent.configuration,
                recipient_device_id,
                lifecycle,
            );
            let mut discovery = CanonicalRuntimeIpcOwnDeviceDiscoveryPolicyStatusV1 {
                configuration: discovery_configuration,
                generation: 1,
                previous_policy_id: None,
                policy_id: [0; 32],
                configured_at_unix_seconds: issued_at,
                outbound_channel_id: [0xd0u8.saturating_add(index * 2); 32],
                inbound_channel_id: [0xd1u8.saturating_add(index * 2); 32],
                outbound_publication_generation: None,
                inbound_locator: None,
                announcement_policy_id: None,
                workflow_state: if lifecycle
                    == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active
                {
                    CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::AwaitingPublish
                } else {
                    CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Disabled
                },
                last_attempt_unix_seconds: None,
                last_success_unix_seconds: None,
                next_attempt_unix_seconds: None,
                consecutive_failures: 0,
                last_bundle_id: None,
                last_transport: None,
            };
            discovery.policy_id = runtime_ipc_own_device_discovery_policy_id_v1(
                &descriptor.content.account_id,
                &descriptor.content.device_id,
                revision,
                &digest,
                discovery.generation,
                discovery.previous_policy_id,
                discovery.configured_at_unix_seconds,
                &discovery.configuration,
                &discovery.outbound_channel_id,
                &discovery.inbound_channel_id,
            )?;
            children.push(CanonicalRuntimeIpcOwnDeviceRosterChildStatusV1 {
                lifecycle,
                discovery,
            });
        }
        let active_children = u8::try_from(
            children
                .iter()
                .filter(|child| {
                    child.lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active
                })
                .count(),
        )?;
        let retired_children = u8::try_from(children.len())? - active_children;
        let current_network = CanonicalRuntimeIpcOwnDeviceRosterNetworkV1::Wifi;
        let configure_snapshot_id = runtime_ipc_own_device_roster_snapshot_id_v1(
            &descriptor.content.account_id,
            &descriptor.content.device_id,
            revision,
            &digest,
            current_network,
            &parent,
            &children,
        )?;
        let configure_result = CanonicalRuntimeIpcOwnDeviceRosterConfiguredV1 {
            outcome: CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::Committed,
            authority_revision: revision,
            device_list_digest: digest,
            current_network,
            policy: parent.clone(),
            snapshot_id: configure_snapshot_id,
            children: children.clone(),
            active_children,
            retired_children,
            max_parallel_workflows: MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_PARALLEL_WORKFLOWS,
            restart_state: CanonicalRuntimeIpcOwnDeviceRosterRestartStateV1::NotRequired,
            execution_scope:
                CanonicalRuntimeIpcOwnDeviceRosterExecutionScopeV1::ForegroundRuntimeOnly,
            os_background_service_enabled: false,
        };
        let status_request = CanonicalRuntimeIpcOwnDeviceRosterRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [0xdb; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: issued_at + 1,
            expires_at_unix_seconds: issued_at + 31,
            command: CanonicalRuntimeIpcOwnDeviceRosterCommandV1::OwnDeviceRosterAutomationStatus {
                current_device_list: Box::new(current.clone()),
            },
        };
        let ticket = b"kilogram-roster-ticket-candidate-v1".to_vec();
        let active_child = children
            .iter_mut()
            .find(|child| {
                child.lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active
            })
            .ok_or_else(|| std::io::Error::other("fixture has no active child"))?;
        active_child.discovery.outbound_publication_generation = Some(3);
        active_child.discovery.inbound_locator =
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
        active_child.discovery.announcement_policy_id = Some([0xe1; 32]);
        active_child.discovery.workflow_state =
            CanonicalRuntimeIpcOwnDeviceDiscoverySchedulerStateV1::Synchronized;
        active_child.discovery.last_attempt_unix_seconds = Some(issued_at - 4);
        active_child.discovery.last_success_unix_seconds = Some(issued_at - 4);
        active_child.discovery.next_attempt_unix_seconds = Some(issued_at + 296);
        active_child.discovery.last_bundle_id = Some([0xe2; 32]);
        active_child.discovery.last_transport =
            Some(CanonicalRuntimeIpcOwnDeviceDiscoveryTransportV1::Relay);
        let snapshot_id = runtime_ipc_own_device_roster_snapshot_id_v1(
            &descriptor.content.account_id,
            &descriptor.content.device_id,
            revision,
            &digest,
            current_network,
            &parent,
            &children,
        )?;
        let status_result = CanonicalRuntimeIpcOwnDeviceRosterStatusV1 {
            authority_revision: revision,
            device_list_digest: digest,
            current_network,
            policy: parent,
            snapshot_id,
            children,
            active_children,
            retired_children,
            max_parallel_workflows: MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_PARALLEL_WORKFLOWS,
            execution_scope:
                CanonicalRuntimeIpcOwnDeviceRosterExecutionScopeV1::ForegroundRuntimeOnly,
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
    ) -> TestResult<CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1> {
        Ok(sign_runtime_ipc_own_device_roster_response_v1(
            &fixture.device_key,
            &fixture.configure_request,
            &fixture.descriptor,
            fixture.configure_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationConfigured(
                Box::new(fixture.configure_result.clone()),
            ),
        )?)
    }

    fn status_response(
        fixture: &Fixture,
    ) -> TestResult<CanonicalSignedRuntimeIpcOwnDeviceRosterResponseV1> {
        Ok(sign_runtime_ipc_own_device_roster_response_v1(
            &fixture.device_key,
            &fixture.status_request,
            &fixture.descriptor,
            fixture.status_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationStatus(Box::new(
                fixture.status_result.clone(),
            )),
        )?)
    }

    #[test]
    fn canonical_runtime_ipc_own_device_roster_round_trips() -> TestResult {
        let fixture = fixture()?;
        for request in [&fixture.configure_request, &fixture.status_request] {
            let bytes = encode_runtime_ipc_own_device_roster_request_v1(request)?;
            assert_eq!(
                decode_and_verify_runtime_ipc_own_device_roster_request_v1(&bytes)?,
                *request
            );
            verify_runtime_ipc_own_device_roster_request_for_descriptor_v1(
                request,
                &fixture.descriptor,
                request.issued_at_unix_seconds,
            )?;
        }
        for (request, response) in [
            (&fixture.configure_request, configure_response(&fixture)?),
            (&fixture.status_request, status_response(&fixture)?),
        ] {
            let bytes = encode_signed_runtime_ipc_own_device_roster_response_v1(&response)?;
            assert_eq!(
                decode_and_verify_signed_runtime_ipc_own_device_roster_response_v1(&bytes)?,
                response
            );
            verify_runtime_ipc_own_device_roster_response_for_request_v1(
                &response,
                request,
                &fixture.descriptor,
                request.issued_at_unix_seconds,
            )?;
        }

        let mut repeat_request = fixture.configure_request.clone();
        repeat_request.request_id = [0xdc; 32];
        repeat_request.issued_at_unix_seconds += 2;
        repeat_request.expires_at_unix_seconds += 2;
        let CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
            expected_previous,
            ..
        } = &mut repeat_request.command
        else {
            unreachable!()
        };
        *expected_previous = Some(CanonicalRuntimeIpcOwnDeviceRosterPreviousPolicyV1 {
            generation: fixture.configure_result.policy.generation,
            policy_id: fixture.configure_result.policy.policy_id,
        });
        let mut repeat_result = fixture.configure_result.clone();
        repeat_result.outcome = CanonicalRuntimeIpcOwnDeviceRosterOutcomeV1::AlreadyCommitted;
        let repeat_response = sign_runtime_ipc_own_device_roster_response_v1(
            &fixture.device_key,
            &repeat_request,
            &fixture.descriptor,
            repeat_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationConfigured(
                Box::new(repeat_result),
            ),
        )?;
        verify_runtime_ipc_own_device_roster_response_for_request_v1(
            &repeat_response,
            &repeat_request,
            &fixture.descriptor,
            repeat_request.issued_at_unix_seconds,
        )?;
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_own_device_roster_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let vectors = vectors()?;
        let configure_response = configure_response(&fixture)?;
        let status_response = status_response(&fixture)?;
        let configure_request_bytes =
            encode_runtime_ipc_own_device_roster_request_v1(&fixture.configure_request)?;
        let configure_response_bytes =
            encode_signed_runtime_ipc_own_device_roster_response_v1(&configure_response)?;
        let status_request_bytes =
            encode_runtime_ipc_own_device_roster_request_v1(&fixture.status_request)?;
        let status_response_bytes =
            encode_signed_runtime_ipc_own_device_roster_response_v1(&status_response)?;
        assert_eq!(
            configure_request_bytes,
            bytes(&vectors, "/expected/configure/request/cbor_hex")?
        );
        assert_eq!(
            configure_response_bytes,
            bytes(&vectors, "/expected/configure/response/signed_cbor_hex")?
        );
        assert_eq!(
            status_request_bytes,
            bytes(&vectors, "/expected/status/request/cbor_hex")?
        );
        assert_eq!(
            status_response_bytes,
            bytes(&vectors, "/expected/status/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_own_device_roster_response_id_v1(&configure_response)?,
            fixed(&vectors, "/expected/configure/response/id_hex")?
        );
        assert_eq!(
            runtime_ipc_own_device_roster_response_id_v1(&status_response)?,
            fixed(&vectors, "/expected/status/response/id_hex")?
        );
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_own_device_roster_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let configure_response = configure_response(&fixture)?;
        let mut changed = fixture.configure_request.clone();
        let CanonicalRuntimeIpcOwnDeviceRosterCommandV1::ConfigureOwnDeviceRosterAutomation {
            configuration,
            ..
        } = &mut changed.command
        else {
            unreachable!()
        };
        configuration.allowed_network_mask ^= 0b0100;
        assert!(
            verify_runtime_ipc_own_device_roster_response_for_request_v1(
                &configure_response,
                &changed,
                &fixture.descriptor,
                changed.issued_at_unix_seconds,
            )
            .is_err()
        );

        let mut changed_response = status_response(&fixture)?;
        let CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationStatus(status) =
            &mut changed_response.content.result
        else {
            unreachable!()
        };
        status.children.swap(0, 1);
        assert!(
            encode_signed_runtime_ipc_own_device_roster_response_v1(&changed_response).is_err()
        );

        let mut forbidden_parallelism = fixture.configure_result.clone();
        forbidden_parallelism.max_parallel_workflows = 2;
        assert!(
            sign_runtime_ipc_own_device_roster_response_v1(
                &fixture.device_key,
                &fixture.configure_request,
                &fixture.descriptor,
                fixture.configure_request.issued_at_unix_seconds,
                CanonicalRuntimeIpcOwnDeviceRosterResultV1::OwnDeviceRosterAutomationConfigured(
                    Box::new(forbidden_parallelism),
                ),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    #[ignore = "explicit golden-vector regeneration"]
    fn regenerate_canonical_runtime_ipc_own_device_roster_vectors() -> TestResult {
        let fixture = fixture()?;
        let configure_response = configure_response(&fixture)?;
        let status_response = status_response(&fixture)?;
        let configure_request_bytes =
            encode_runtime_ipc_own_device_roster_request_v1(&fixture.configure_request)?;
        let configure_response_bytes =
            encode_signed_runtime_ipc_own_device_roster_response_v1(&configure_response)?;
        let status_request_bytes =
            encode_runtime_ipc_own_device_roster_request_v1(&fixture.status_request)?;
        let status_response_bytes =
            encode_signed_runtime_ipc_own_device_roster_response_v1(&status_response)?;
        let current_bytes = encode_signed_account_device_list_v1(&fixture.current)?;
        let active = fixture
            .status_result
            .children
            .iter()
            .find(|child| {
                child.lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Active
            })
            .ok_or_else(|| std::io::Error::other("fixture has no active child"))?;
        let retired = fixture
            .status_result
            .children
            .iter()
            .find(|child| {
                child.lifecycle == CanonicalRuntimeIpcOwnDeviceRosterChildLifecycleV1::Retired
            })
            .ok_or_else(|| std::io::Error::other("fixture has no retired child"))?;
        let inbound_locator = active
            .discovery
            .inbound_locator
            .as_ref()
            .ok_or_else(|| std::io::Error::other("active child has no installed locator"))?;
        let input = json!({
            "device_secret_key_hex": hex(&fixture.device_key.to_bytes()),
            "descriptor_id_hex": hex(&fixture.configure_request.descriptor_id),
            "bearer_token_hex": hex(&fixture.configure_request.bearer_token),
            "account_id_hex": hex(&fixture.descriptor.content.account_id),
            "local_device_id_hex": hex(&fixture.descriptor.content.device_id),
            "current_device_list_signed_cbor_hex": hex(&current_bytes),
            "authority_revision": fixture.current.content.authority_snapshot.content.revision,
            "device_list_digest_hex": hex(&fixture.configure_result.device_list_digest),
            "service_origin_utf8": String::from_utf8(fixture.configure_result.policy.configuration.service_origin_utf8.clone())?,
            "enabled": fixture.configure_result.policy.configuration.enabled,
            "publication_ttl_seconds": fixture.configure_result.policy.configuration.publication_ttl_seconds,
            "refresh_before_seconds": fixture.configure_result.policy.configuration.refresh_before_seconds,
            "announcement_interval_seconds": fixture.configure_result.policy.configuration.announcement_interval_seconds,
            "announcement_validity_seconds": fixture.configure_result.policy.configuration.announcement_validity_seconds,
            "retry_base_seconds": fixture.configure_result.policy.configuration.retry_base_seconds,
            "retry_max_seconds": fixture.configure_result.policy.configuration.retry_max_seconds,
            "allowed_network_mask": fixture.configure_result.policy.configuration.allowed_network_mask,
            "parent_generation": fixture.configure_result.policy.generation,
            "parent_policy_id_hex": hex(&fixture.configure_result.policy.policy_id),
            "active_recipient_device_id_hex": hex(&active.discovery.configuration.recipient_device_id),
            "active_discovery_policy_id_hex": hex(&active.discovery.policy_id),
            "active_outbound_channel_id_hex": hex(&active.discovery.outbound_channel_id),
            "active_inbound_channel_id_hex": hex(&active.discovery.inbound_channel_id),
            "retired_recipient_device_id_hex": hex(&retired.discovery.configuration.recipient_device_id),
            "retired_discovery_policy_id_hex": hex(&retired.discovery.policy_id),
            "recipient_ticket_hex": hex(&fixture.ticket),
            "recipient_ticket_digest_hex": hex(&inbound_locator.recipient_ticket.digest),
            "configure_request_id_hex": hex(&fixture.configure_request.request_id),
            "configure_issued_at_unix_seconds": fixture.configure_request.issued_at_unix_seconds,
            "configure_expires_at_unix_seconds": fixture.configure_request.expires_at_unix_seconds,
            "status_request_id_hex": hex(&fixture.status_request.request_id),
            "status_issued_at_unix_seconds": fixture.status_request.issued_at_unix_seconds,
            "status_expires_at_unix_seconds": fixture.status_request.expires_at_unix_seconds
        });
        let expected = json!({
            "configure": {
                "request": {
                    "cbor_hex": hex(&configure_request_bytes),
                    "digest_hex": hex(&runtime_ipc_own_device_roster_request_digest_v1(&fixture.configure_request)?)
                },
                "response": {
                    "content_cbor_hex": hex(&encode_response_content(&configure_response.content)?),
                    "signing_preimage_hex": hex(&runtime_ipc_own_device_roster_response_signing_bytes_v1(&configure_response.content)?),
                    "signature_hex": hex(&configure_response.signature),
                    "signed_cbor_hex": hex(&configure_response_bytes),
                    "id_hex": hex(&runtime_ipc_own_device_roster_response_id_v1(&configure_response)?),
                    "parent_policy_id_hex": hex(&fixture.configure_result.policy.policy_id),
                    "snapshot_id_hex": hex(&fixture.configure_result.snapshot_id)
                }
            },
            "status": {
                "request": {
                    "cbor_hex": hex(&status_request_bytes),
                    "digest_hex": hex(&runtime_ipc_own_device_roster_request_digest_v1(&fixture.status_request)?)
                },
                "response": {
                    "content_cbor_hex": hex(&encode_response_content(&status_response.content)?),
                    "signing_preimage_hex": hex(&runtime_ipc_own_device_roster_response_signing_bytes_v1(&status_response.content)?),
                    "signature_hex": hex(&status_response.signature),
                    "signed_cbor_hex": hex(&status_response_bytes),
                    "id_hex": hex(&runtime_ipc_own_device_roster_response_id_v1(&status_response)?),
                    "snapshot_id_hex": hex(&fixture.status_result.snapshot_id),
                    "active_children": fixture.status_result.active_children,
                    "retired_children": fixture.status_result.retired_children,
                    "max_parallel_workflows": fixture.status_result.max_parallel_workflows
                }
            }
        });
        let document = json!({
            "schema_version": 1,
            "milestone": "M1.1.42",
            "status": "candidate-not-active",
            "changes_active_runtime_ipc": false,
            "encoding": "kilogram-deterministic-cbor-v1",
            "domains": {
                "policy_id": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_POLICY_ID_DOMAIN),
                "snapshot_id": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_SNAPSHOT_ID_DOMAIN),
                "request_digest": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_REQUEST_DIGEST_DOMAIN),
                "response_signature": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_SIGNATURE_DOMAIN),
                "response_id": domain(CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_ID_DOMAIN)
            },
            "input": input,
            "expected": expected
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
