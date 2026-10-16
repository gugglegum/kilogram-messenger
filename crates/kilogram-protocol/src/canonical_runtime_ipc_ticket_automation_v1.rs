//! Inactive deterministic-CBOR ticket-automation policy and status boundary.
//!
//! The contract controls only the foreground runtime scheduler for already
//! enrolled contacts. It neither registers an OS service nor carries ticket
//! plaintext, publication capabilities, filesystem paths, IP addresses or
//! network names.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use url::Url;

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_runtime_ipc_contact_v1::canonical_runtime_ipc_contact_id_v1,
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

pub const MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REQUEST_BYTES: usize = 128 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_BYTES: usize = 512 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_POLICIES: usize = 256;
pub const MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SERVICE_ORIGIN_BYTES: usize = 512;
pub const MIN_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_TTL_SECONDS: u64 = 30;
pub const MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_TTL_SECONDS: u64 = 60 * 60;
pub const MIN_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REFRESH_BEFORE_SECONDS: u64 = 10;
pub const MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REFRESH_BEFORE_SECONDS: u64 = 30 * 60;
pub const MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RETRY_SECONDS: u64 = 60 * 60;
pub const MIN_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SUCCESS_RECHECK_SECONDS: u64 = 30;

pub const CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_POLICY_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-ticket-automation-policy-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SNAPSHOT_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-ticket-automation-snapshot-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-ticket-automation-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-ticket-automation-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-ticket-automation-response-id:cbor-v1\0";

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
const ACTION_PUBLISH: u64 = 0;
const ACTION_REFRESH: u64 = 1;
const STATE_DISABLED: u64 = 0;
const STATE_NETWORK_BLOCKED: u64 = 1;
const STATE_PUBLICATION_CONFLICT_QUARANTINED: u64 = 2;
const STATE_DUE: u64 = 3;
const STATE_FRESH: u64 = 4;
const STATE_BACKOFF: u64 = 5;
const LAST_RESULT_NEVER: u64 = 0;
const LAST_RESULT_SUCCEEDED: u64 = 1;
const LAST_RESULT_FAILED: u64 = 2;
const RESTART_NOT_REQUIRED: u64 = 0;
const EXECUTION_FOREGROUND_RUNTIME: u64 = 0;
const NETWORK_MASK_ALL: u8 = 0b1111;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationPreviousPolicyV1 {
    pub generation: u64,
    pub policy_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationConfigurationV1 {
    pub contact_id: [u8; 32],
    pub conversation_label_utf8: Vec<u8>,
    pub peer_account_id: [u8; 32],
    pub enabled: bool,
    pub service_origin_utf8: Vec<u8>,
    pub ttl_seconds: u64,
    pub refresh_before_seconds: u64,
    pub retry_base_seconds: u64,
    pub retry_max_seconds: u64,
    /// bit 0 Ethernet, bit 1 Wi-Fi, bit 2 mobile, bit 3 unknown.
    pub allowed_network_mask: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationNetworkV1 {
    Ethernet,
    Wifi,
    Mobile,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationActionV1 {
    Publish,
    Refresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationSchedulerStateV1 {
    Disabled,
    NetworkBlocked,
    PublicationConflictQuarantined,
    Due,
    Fresh,
    Backoff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationLastResultV1 {
    Never,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationActionStatusV1 {
    pub action: CanonicalRuntimeIpcTicketAutomationActionV1,
    pub scheduler_state: CanonicalRuntimeIpcTicketAutomationSchedulerStateV1,
    pub last_attempt_unix_seconds: Option<u64>,
    pub last_success_unix_seconds: Option<u64>,
    pub next_attempt_unix_seconds: Option<u64>,
    pub consecutive_failures: u32,
    pub publication_generation: Option<u64>,
    pub expires_at_unix_seconds: Option<u64>,
    pub last_result: CanonicalRuntimeIpcTicketAutomationLastResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationPolicyStatusV1 {
    pub configuration: CanonicalRuntimeIpcTicketAutomationConfigurationV1,
    pub generation: u64,
    pub previous_policy_id: Option<[u8; 32]>,
    pub policy_id: [u8; 32],
    pub configured_at_unix_seconds: u64,
    pub publish: CanonicalRuntimeIpcTicketAutomationActionStatusV1,
    pub refresh: CanonicalRuntimeIpcTicketAutomationActionStatusV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationCommandV1 {
    ConfigureTicketAutomation {
        expected_previous: Option<CanonicalRuntimeIpcTicketAutomationPreviousPolicyV1>,
        configuration: CanonicalRuntimeIpcTicketAutomationConfigurationV1,
    },
    TicketAutomationStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcTicketAutomationCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationRestartStateV1 {
    NotRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationExecutionScopeV1 {
    ForegroundRuntimeOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationConfiguredV1 {
    pub outcome: CanonicalRuntimeIpcTicketAutomationOutcomeV1,
    pub observed_at_unix_seconds: u64,
    pub current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1,
    pub status: CanonicalRuntimeIpcTicketAutomationPolicyStatusV1,
    pub restart_state: CanonicalRuntimeIpcTicketAutomationRestartStateV1,
    pub execution_scope: CanonicalRuntimeIpcTicketAutomationExecutionScopeV1,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationStatusV1 {
    pub observed_at_unix_seconds: u64,
    pub current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1,
    pub snapshot_id: [u8; 32],
    pub policies: Vec<CanonicalRuntimeIpcTicketAutomationPolicyStatusV1>,
    pub execution_scope: CanonicalRuntimeIpcTicketAutomationExecutionScopeV1,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcTicketAutomationResultV1 {
    TicketAutomationConfigured(Box<CanonicalRuntimeIpcTicketAutomationConfiguredV1>),
    TicketAutomationStatus(Box<CanonicalRuntimeIpcTicketAutomationStatusV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcTicketAutomationResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcTicketAutomationResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcTicketAutomationResponseV1 {
    pub content: CanonicalRuntimeIpcTicketAutomationResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_ticket_automation_request_v1(
    request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
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
    encode_command(&mut output, &request.command);
    ensure_bound(
        &output,
        "ticket automation request",
        MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_ticket_automation_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcTicketAutomationRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "ticket automation request",
        MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-ticket-automation-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-ticket-automation-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcTicketAutomationRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-ticket-automation-request-v1.request-id")?,
        bearer_token: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-ticket-automation-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-ticket-automation-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_ticket_automation_request_digest_v1(
    request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_ticket_automation_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_ticket_automation_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
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
    if let CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
        configuration,
        ..
    } = &request.command
    {
        let expected_contact_id = canonical_runtime_ipc_contact_id_v1(
            &descriptor.content.account_id,
            &configuration.peer_account_id,
            &configuration.conversation_label_utf8,
        )
        .map_err(|_| CanonicalRuntimeIpcError::ContextMismatch)?;
        if configuration.contact_id != expected_contact_id {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_ipc_ticket_automation_policy_id_v1(
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    generation: u64,
    previous_policy_id: Option<[u8; 32]>,
    configured_at_unix_seconds: u64,
    configuration: &CanonicalRuntimeIpcTicketAutomationConfigurationV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    validate_configuration(configuration, true)?;
    let expected_contact_id = canonical_runtime_ipc_contact_id_v1(
        account_id,
        &configuration.peer_account_id,
        &configuration.conversation_label_utf8,
    )
    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if *account_id == [0; 32]
        || *local_device_id == [0; 32]
        || configuration.contact_id != expected_contact_id
        || generation == 0
        || configured_at_unix_seconds == 0
        || (generation == 1) != previous_policy_id.is_none()
        || previous_policy_id == Some([0; 32])
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let mut content = Vec::new();
    push_array_header(&mut content, 7);
    push_unsigned(&mut content, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut content, account_id);
    push_bytes(&mut content, local_device_id);
    push_unsigned(&mut content, generation);
    encode_optional_id(&mut content, previous_policy_id);
    push_unsigned(&mut content, configured_at_unix_seconds);
    encode_configuration(&mut content, configuration);
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_POLICY_ID_DOMAIN,
        &content,
    ))
}

pub fn runtime_ipc_ticket_automation_snapshot_id_v1(
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    observed_at_unix_seconds: u64,
    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1,
    policies: &[CanonicalRuntimeIpcTicketAutomationPolicyStatusV1],
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    if *account_id == [0; 32] || *local_device_id == [0; 32] || observed_at_unix_seconds == 0 {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    validate_policy_order(policies, CanonicalRuntimeIpcError::InvalidResponse)?;
    for policy in policies {
        validate_policy_status(
            policy,
            account_id,
            local_device_id,
            current_network,
            observed_at_unix_seconds,
        )?;
    }
    let mut content = Vec::new();
    push_array_header(&mut content, 6);
    push_unsigned(&mut content, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut content, account_id);
    push_bytes(&mut content, local_device_id);
    push_unsigned(&mut content, observed_at_unix_seconds);
    encode_network(&mut content, current_network);
    push_array_header(&mut content, policies.len() as u64);
    for policy in policies {
        encode_policy_status(&mut content, policy);
    }
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SNAPSHOT_ID_DOMAIN,
        &content,
    ))
}

pub fn runtime_ipc_ticket_automation_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcTicketAutomationResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content(content)?,
    ))
}

pub fn sign_runtime_ipc_ticket_automation_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcTicketAutomationResultV1,
) -> Result<CanonicalSignedRuntimeIpcTicketAutomationResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_ticket_automation_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&result, request, descriptor)?;
    let content = CanonicalRuntimeIpcTicketAutomationResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_ticket_automation_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_ticket_automation_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    let response = CanonicalSignedRuntimeIpcTicketAutomationResponseV1 { content, signature };
    verify_signed_runtime_ipc_ticket_automation_response_v1(&response)?;
    Ok(response)
}

pub fn encode_signed_runtime_ipc_ticket_automation_response_v1(
    response: &CanonicalSignedRuntimeIpcTicketAutomationResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_ticket_automation_response_v1(response)?;
    let encoded = signed_envelope(
        &encode_response_content(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &encoded,
        "ticket automation response",
        MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_BYTES,
    )?;
    Ok(encoded)
}

pub fn decode_and_verify_signed_runtime_ipc_ticket_automation_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcTicketAutomationResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "ticket automation response",
        MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-ticket-automation-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-ticket-automation-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-ticket-automation-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcTicketAutomationResponseV1 { content, signature };
    verify_signed_runtime_ipc_ticket_automation_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_ticket_automation_response_v1(
    response: &CanonicalSignedRuntimeIpcTicketAutomationResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &runtime_ipc_ticket_automation_response_signing_bytes_v1(&response.content)?,
        &response.signature,
    )
}

pub fn verify_runtime_ipc_ticket_automation_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcTicketAutomationResponseV1,
    request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_ticket_automation_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    verify_signed_runtime_ipc_ticket_automation_response_v1(response)?;
    if response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest
            != runtime_ipc_ticket_automation_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&response.content.result, request, descriptor)
}

pub fn runtime_ipc_ticket_automation_response_id_v1(
    response: &CanonicalSignedRuntimeIpcTicketAutomationResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_ticket_automation_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
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
    if let CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
        expected_previous,
        configuration,
    } = &request.command
    {
        validate_configuration(configuration, false)?;
        if expected_previous
            .is_some_and(|previous| previous.generation == 0 || previous.policy_id == [0; 32])
        {
            return Err(CanonicalRuntimeIpcError::InvalidRequest);
        }
    }
    Ok(())
}

fn invalid_configuration(response: bool) -> CanonicalRuntimeIpcError {
    if response {
        CanonicalRuntimeIpcError::InvalidResponse
    } else {
        CanonicalRuntimeIpcError::InvalidRequest
    }
}

fn validate_configuration(
    configuration: &CanonicalRuntimeIpcTicketAutomationConfigurationV1,
    response: bool,
) -> Result<(), CanonicalRuntimeIpcError> {
    if configuration.contact_id == [0; 32]
        || configuration.peer_account_id == [0; 32]
        || configuration.conversation_label_utf8.is_empty()
        || configuration.conversation_label_utf8.len() > 4096
        || std::str::from_utf8(&configuration.conversation_label_utf8).is_err()
        || configuration.conversation_label_utf8.contains(&0)
        || !(MIN_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_TTL_SECONDS
            ..=MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_TTL_SECONDS)
            .contains(&configuration.ttl_seconds)
        || !(MIN_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REFRESH_BEFORE_SECONDS
            ..=MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REFRESH_BEFORE_SECONDS)
            .contains(&configuration.refresh_before_seconds)
        || configuration.refresh_before_seconds >= configuration.ttl_seconds
        || !(1..=MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RETRY_SECONDS)
            .contains(&configuration.retry_base_seconds)
        || configuration.retry_max_seconds < configuration.retry_base_seconds
        || configuration.retry_max_seconds
            > MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RETRY_SECONDS
        || configuration.allowed_network_mask & !NETWORK_MASK_ALL != 0
    {
        return Err(invalid_configuration(response));
    }
    validate_service_origin(&configuration.service_origin_utf8, response)
}

fn validate_service_origin(value: &[u8], response: bool) -> Result<(), CanonicalRuntimeIpcError> {
    if value.is_empty()
        || value.len() > MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SERVICE_ORIGIN_BYTES
    {
        return Err(invalid_configuration(response));
    }
    let text = std::str::from_utf8(value).map_err(|_| invalid_configuration(response))?;
    let parsed = Url::parse(text).map_err(|_| invalid_configuration(response))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
        || parsed.as_str().as_bytes() != value
    {
        return Err(invalid_configuration(response));
    }
    Ok(())
}

fn validate_action_status(
    status: &CanonicalRuntimeIpcTicketAutomationActionStatusV1,
    configuration: &CanonicalRuntimeIpcTicketAutomationConfigurationV1,
    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1,
    observed_at_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    if observed_at_unix_seconds == 0
        || status.last_attempt_unix_seconds == Some(0)
        || status.last_success_unix_seconds == Some(0)
        || status.next_attempt_unix_seconds == Some(0)
        || status.publication_generation == Some(0)
        || status.expires_at_unix_seconds == Some(0)
        || status
            .last_attempt_unix_seconds
            .is_some_and(|attempt| attempt > observed_at_unix_seconds)
        || status
            .last_success_unix_seconds
            .is_some_and(|success| success > observed_at_unix_seconds)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let success_tuple = status.last_success_unix_seconds.is_some()
        == status.publication_generation.is_some()
        && status.last_success_unix_seconds.is_some() == status.expires_at_unix_seconds.is_some();
    if !success_tuple {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match status.last_result {
        CanonicalRuntimeIpcTicketAutomationLastResultV1::Never => {
            if status.last_attempt_unix_seconds.is_some()
                || status.last_success_unix_seconds.is_some()
                || status.next_attempt_unix_seconds.is_some()
                || status.consecutive_failures != 0
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalRuntimeIpcTicketAutomationLastResultV1::Succeeded => {
            let Some(attempted_at_unix_seconds) = status.last_attempt_unix_seconds else {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            };
            let Some(expires_at_unix_seconds) = status.expires_at_unix_seconds else {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            };
            let expected_next_attempt = expires_at_unix_seconds
                .saturating_sub(configuration.refresh_before_seconds)
                .max(attempted_at_unix_seconds.saturating_add(
                    MIN_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SUCCESS_RECHECK_SECONDS,
                ));
            if status.last_success_unix_seconds != status.last_attempt_unix_seconds
                || status.next_attempt_unix_seconds != Some(expected_next_attempt)
                || status.consecutive_failures != 0
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalRuntimeIpcTicketAutomationLastResultV1::Failed => {
            let quarantined = status.scheduler_state
                == CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::PublicationConflictQuarantined;
            let Some(attempted_at_unix_seconds) = status.last_attempt_unix_seconds else {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            };
            if status.consecutive_failures == 0
                || (quarantined && status.next_attempt_unix_seconds.is_some())
                || (!quarantined && status.next_attempt_unix_seconds.is_none())
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            if !quarantined {
                let shift = status.consecutive_failures.saturating_sub(1).min(31);
                let delay = configuration
                    .retry_base_seconds
                    .saturating_mul(1_u64 << shift)
                    .min(configuration.retry_max_seconds);
                let expected_next_attempt = attempted_at_unix_seconds.saturating_add(delay);
                if status.next_attempt_unix_seconds != Some(expected_next_attempt) {
                    return Err(CanonicalRuntimeIpcError::InvalidResponse);
                }
            }
        }
    }
    if status
        .last_success_unix_seconds
        .zip(status.last_attempt_unix_seconds)
        .is_some_and(|(success, attempt)| success > attempt)
        || status
            .last_success_unix_seconds
            .zip(status.expires_at_unix_seconds)
            .is_some_and(|(success, expiry)| expiry <= success)
        || status
            .last_attempt_unix_seconds
            .zip(status.next_attempt_unix_seconds)
            .is_some_and(|(attempt, next)| next <= attempt)
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    let allowed = network_allowed(configuration.allowed_network_mask, current_network);
    let expected = if !configuration.enabled {
        CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Disabled
    } else if !allowed {
        CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::NetworkBlocked
    } else if status.scheduler_state
        == CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::PublicationConflictQuarantined
    {
        if status.action != CanonicalRuntimeIpcTicketAutomationActionV1::Refresh
            || status.last_result != CanonicalRuntimeIpcTicketAutomationLastResultV1::Failed
            || status.next_attempt_unix_seconds.is_some()
        {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        }
        CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::PublicationConflictQuarantined
    } else {
        match status.last_result {
            CanonicalRuntimeIpcTicketAutomationLastResultV1::Never => {
                CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Due
            }
            CanonicalRuntimeIpcTicketAutomationLastResultV1::Succeeded => {
                if status
                    .next_attempt_unix_seconds
                    .is_some_and(|next| next > observed_at_unix_seconds)
                {
                    CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Fresh
                } else {
                    CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Due
                }
            }
            CanonicalRuntimeIpcTicketAutomationLastResultV1::Failed => {
                if status
                    .next_attempt_unix_seconds
                    .is_some_and(|next| next > observed_at_unix_seconds)
                {
                    CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Backoff
                } else {
                    CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Due
                }
            }
        }
    };
    if status.scheduler_state != expected {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_policy_status_shape(
    status: &CanonicalRuntimeIpcTicketAutomationPolicyStatusV1,
    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1,
    observed_at_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_configuration(&status.configuration, true)?;
    if status.generation == 0
        || status.configured_at_unix_seconds == 0
        || status.configured_at_unix_seconds > observed_at_unix_seconds
        || (status.generation == 1) != status.previous_policy_id.is_none()
        || status.previous_policy_id == Some([0; 32])
        || status.policy_id == [0; 32]
        || status.publish.action != CanonicalRuntimeIpcTicketAutomationActionV1::Publish
        || status.refresh.action != CanonicalRuntimeIpcTicketAutomationActionV1::Refresh
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    validate_action_status(
        &status.publish,
        &status.configuration,
        current_network,
        observed_at_unix_seconds,
    )?;
    validate_action_status(
        &status.refresh,
        &status.configuration,
        current_network,
        observed_at_unix_seconds,
    )
}

fn validate_policy_status(
    status: &CanonicalRuntimeIpcTicketAutomationPolicyStatusV1,
    account_id: &[u8; 32],
    local_device_id: &[u8; 32],
    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1,
    observed_at_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_policy_status_shape(status, current_network, observed_at_unix_seconds)?;
    let expected_contact_id = canonical_runtime_ipc_contact_id_v1(
        account_id,
        &status.configuration.peer_account_id,
        &status.configuration.conversation_label_utf8,
    )
    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if status.configuration.contact_id != expected_contact_id
        || status.policy_id
            != runtime_ipc_ticket_automation_policy_id_v1(
                account_id,
                local_device_id,
                status.generation,
                status.previous_policy_id,
                status.configured_at_unix_seconds,
                &status.configuration,
            )?
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_policy_order(
    policies: &[CanonicalRuntimeIpcTicketAutomationPolicyStatusV1],
    error: CanonicalRuntimeIpcError,
) -> Result<(), CanonicalRuntimeIpcError> {
    if policies.len() > MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_POLICIES
        || policies
            .windows(2)
            .any(|pair| pair[0].configuration.contact_id >= pair[1].configuration.contact_id)
    {
        return Err(error);
    }
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcTicketAutomationResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationConfigured(configured) => {
            if configured.observed_at_unix_seconds == 0 || configured.os_background_service_enabled
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_policy_status_shape(
                &configured.status,
                configured.current_network,
                configured.observed_at_unix_seconds,
            )?;
        }
        CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationStatus(status) => {
            if status.observed_at_unix_seconds == 0
                || status.snapshot_id == [0; 32]
                || status.os_background_service_enabled
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            validate_policy_order(&status.policies, CanonicalRuntimeIpcError::InvalidResponse)?;
            for policy in &status.policies {
                validate_policy_status_shape(
                    policy,
                    status.current_network,
                    status.observed_at_unix_seconds,
                )?;
            }
        }
        CanonicalRuntimeIpcTicketAutomationResultV1::Error { code, message_utf8 } => {
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
    result: &CanonicalRuntimeIpcTicketAutomationResultV1,
    request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match (&request.command, result) {
        (
            CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
                expected_previous,
                configuration,
            },
            CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationConfigured(configured),
        ) => {
            let expected_generation = expected_previous
                .map_or(Some(1), |previous| previous.generation.checked_add(1))
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
            if configured.observed_at_unix_seconds < request.issued_at_unix_seconds
                || configured.observed_at_unix_seconds > request.expires_at_unix_seconds
                || configured.status.configuration != *configuration
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
                configured.current_network,
                configured.observed_at_unix_seconds,
            )?;
        }
        (
            CanonicalRuntimeIpcTicketAutomationCommandV1::TicketAutomationStatus,
            CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationStatus(status),
        ) => {
            if status.observed_at_unix_seconds < request.issued_at_unix_seconds
                || status.observed_at_unix_seconds > request.expires_at_unix_seconds
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            for policy in &status.policies {
                validate_policy_status(
                    policy,
                    &descriptor.content.account_id,
                    &descriptor.content.device_id,
                    status.current_network,
                    status.observed_at_unix_seconds,
                )?;
            }
            if status.snapshot_id
                != runtime_ipc_ticket_automation_snapshot_id_v1(
                    &descriptor.content.account_id,
                    &descriptor.content.device_id,
                    status.observed_at_unix_seconds,
                    status.current_network,
                    &status.policies,
                )?
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        (_, CanonicalRuntimeIpcTicketAutomationResultV1::Error { .. }) => {}
        _ => return Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
    Ok(())
}

fn encode_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcTicketAutomationCommandV1) {
    match command {
        CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
            expected_previous,
            configuration,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_CONFIGURE);
            encode_optional_previous(output, *expected_previous);
            encode_configuration(output, configuration);
        }
        CanonicalRuntimeIpcTicketAutomationCommandV1::TicketAutomationStatus => {
            push_array_header(output, 1);
            push_unsigned(output, COMMAND_STATUS);
        }
    }
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-ticket-automation-command-v1")?;
    match decoder.unsigned("runtime-ipc-ticket-automation-command-v1.variant")? {
        COMMAND_CONFIGURE if length == 3 => Ok(
            CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
                expected_previous: decode_optional_previous(decoder)?,
                configuration: decode_configuration(decoder, false)?,
            },
        ),
        COMMAND_STATUS if length == 1 => {
            Ok(CanonicalRuntimeIpcTicketAutomationCommandV1::TicketAutomationStatus)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_configuration(
    output: &mut Vec<u8>,
    configuration: &CanonicalRuntimeIpcTicketAutomationConfigurationV1,
) {
    push_array_header(output, 11);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &configuration.contact_id);
    push_bytes(output, &configuration.conversation_label_utf8);
    push_bytes(output, &configuration.peer_account_id);
    encode_bool(output, configuration.enabled);
    push_bytes(output, &configuration.service_origin_utf8);
    push_unsigned(output, configuration.ttl_seconds);
    push_unsigned(output, configuration.refresh_before_seconds);
    push_unsigned(output, configuration.retry_base_seconds);
    push_unsigned(output, configuration.retry_max_seconds);
    push_unsigned(output, u64::from(configuration.allowed_network_mask));
}

fn decode_configuration(
    decoder: &mut AccountDecoder<'_>,
    response: bool,
) -> Result<CanonicalRuntimeIpcTicketAutomationConfigurationV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-ticket-automation-configuration-v1", 11)?;
    decoder.expect_version("runtime-ipc-ticket-automation-configuration-v1.schema-version")?;
    let configuration = CanonicalRuntimeIpcTicketAutomationConfigurationV1 {
        contact_id: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-configuration-v1.contact-id")?,
        conversation_label_utf8: decoder
            .bytes("runtime-ipc-ticket-automation-configuration-v1.conversation-label")?
            .to_vec(),
        peer_account_id: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-configuration-v1.peer-account-id")?,
        enabled: decode_bool(
            decoder,
            "runtime-ipc-ticket-automation-configuration-v1.enabled",
            response,
        )?,
        service_origin_utf8: decoder
            .bytes("runtime-ipc-ticket-automation-configuration-v1.service-origin")?
            .to_vec(),
        ttl_seconds: decoder
            .unsigned("runtime-ipc-ticket-automation-configuration-v1.ttl-seconds")?,
        refresh_before_seconds: decoder
            .unsigned("runtime-ipc-ticket-automation-configuration-v1.refresh-before-seconds")?,
        retry_base_seconds: decoder
            .unsigned("runtime-ipc-ticket-automation-configuration-v1.retry-base-seconds")?,
        retry_max_seconds: decoder
            .unsigned("runtime-ipc-ticket-automation-configuration-v1.retry-max-seconds")?,
        allowed_network_mask: u8::try_from(
            decoder
                .unsigned("runtime-ipc-ticket-automation-configuration-v1.allowed-network-mask")?,
        )
        .map_err(|_| invalid_configuration(response))?,
    };
    validate_configuration(&configuration, response)?;
    Ok(configuration)
}

fn encode_policy_status(
    output: &mut Vec<u8>,
    status: &CanonicalRuntimeIpcTicketAutomationPolicyStatusV1,
) {
    push_array_header(output, 8);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    encode_configuration(output, &status.configuration);
    push_unsigned(output, status.generation);
    encode_optional_id(output, status.previous_policy_id);
    push_bytes(output, &status.policy_id);
    push_unsigned(output, status.configured_at_unix_seconds);
    encode_action_status(output, &status.publish);
    encode_action_status(output, &status.refresh);
}

fn decode_policy_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationPolicyStatusV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-ticket-automation-policy-status-v1", 8)?;
    decoder.expect_version("runtime-ipc-ticket-automation-policy-status-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcTicketAutomationPolicyStatusV1 {
        configuration: decode_configuration(decoder, true)?,
        generation: decoder
            .unsigned("runtime-ipc-ticket-automation-policy-status-v1.generation")?,
        previous_policy_id: decode_optional_id(decoder)?,
        policy_id: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-policy-status-v1.policy-id")?,
        configured_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-ticket-automation-policy-status-v1.configured-at-unix-seconds",
        )?,
        publish: decode_action_status(decoder)?,
        refresh: decode_action_status(decoder)?,
    })
}

fn encode_action_status(
    output: &mut Vec<u8>,
    status: &CanonicalRuntimeIpcTicketAutomationActionStatusV1,
) {
    push_array_header(output, 10);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    encode_action(output, status.action);
    encode_scheduler_state(output, status.scheduler_state);
    encode_optional_unsigned(output, status.last_attempt_unix_seconds);
    encode_optional_unsigned(output, status.last_success_unix_seconds);
    encode_optional_unsigned(output, status.next_attempt_unix_seconds);
    push_unsigned(output, u64::from(status.consecutive_failures));
    encode_optional_unsigned(output, status.publication_generation);
    encode_optional_unsigned(output, status.expires_at_unix_seconds);
    encode_last_result(output, status.last_result);
}

fn decode_action_status(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationActionStatusV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-ticket-automation-action-status-v1", 10)?;
    decoder.expect_version("runtime-ipc-ticket-automation-action-status-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcTicketAutomationActionStatusV1 {
        action: decode_action(decoder)?,
        scheduler_state: decode_scheduler_state(decoder)?,
        last_attempt_unix_seconds: decode_optional_unsigned(decoder)?,
        last_success_unix_seconds: decode_optional_unsigned(decoder)?,
        next_attempt_unix_seconds: decode_optional_unsigned(decoder)?,
        consecutive_failures: u32::try_from(
            decoder
                .unsigned("runtime-ipc-ticket-automation-action-status-v1.consecutive-failures")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        publication_generation: decode_optional_unsigned(decoder)?,
        expires_at_unix_seconds: decode_optional_unsigned(decoder)?,
        last_result: decode_last_result(decoder)?,
    })
}

fn encode_response_content(
    content: &CanonicalRuntimeIpcTicketAutomationResponseContentV1,
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
) -> Result<CanonicalRuntimeIpcTicketAutomationResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-ticket-automation-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-ticket-automation-response-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcTicketAutomationResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-ticket-automation-response-content-v1.responder-device-id")?,
        result: decode_result(decoder)?,
    };
    validate_response_content(&content)?;
    Ok(content)
}

fn encode_result(output: &mut Vec<u8>, result: &CanonicalRuntimeIpcTicketAutomationResultV1) {
    match result {
        CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationConfigured(configured) => {
            push_array_header(output, 8);
            push_unsigned(output, RESULT_CONFIGURED);
            encode_outcome(output, configured.outcome);
            push_unsigned(output, configured.observed_at_unix_seconds);
            encode_network(output, configured.current_network);
            encode_policy_status(output, &configured.status);
            push_unsigned(output, RESTART_NOT_REQUIRED);
            push_unsigned(output, EXECUTION_FOREGROUND_RUNTIME);
            encode_bool(output, configured.os_background_service_enabled);
        }
        CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationStatus(status) => {
            push_array_header(output, 8);
            push_unsigned(output, RESULT_STATUS);
            push_unsigned(output, status.observed_at_unix_seconds);
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
        CanonicalRuntimeIpcTicketAutomationResultV1::Error { code, message_utf8 } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_ERROR);
            push_unsigned(output, u64::from(*code));
            push_bytes(output, message_utf8);
        }
    }
}

fn decode_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-ticket-automation-result-v1")?;
    match decoder.unsigned("runtime-ipc-ticket-automation-result-v1.variant")? {
        RESULT_CONFIGURED if length == 8 => Ok(
            CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationConfigured(Box::new(
                CanonicalRuntimeIpcTicketAutomationConfiguredV1 {
                    outcome: decode_outcome(decoder)?,
                    observed_at_unix_seconds: decoder.unsigned(
                        "runtime-ipc-ticket-automation-configured-v1.observed-at-unix-seconds",
                    )?,
                    current_network: decode_network(decoder)?,
                    status: decode_policy_status(decoder)?,
                    restart_state: decode_restart(decoder)?,
                    execution_scope: decode_execution(decoder)?,
                    os_background_service_enabled: decode_bool(
                        decoder,
                        "runtime-ipc-ticket-automation-configured-v1.os-background-service-enabled",
                        true,
                    )?,
                },
            )),
        ),
        RESULT_STATUS if length == 8 => {
            let observed_at_unix_seconds = decoder
                .unsigned("runtime-ipc-ticket-automation-status-v1.observed-at-unix-seconds")?;
            let current_network = decode_network(decoder)?;
            let snapshot_id =
                decoder.fixed_bytes("runtime-ipc-ticket-automation-status-v1.snapshot-id")?;
            let count = decoder.array("runtime-ipc-ticket-automation-status-v1.policies")?;
            if count > MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_POLICIES {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            let mut policies = Vec::with_capacity(count);
            for _ in 0..count {
                policies.push(decode_policy_status(decoder)?);
            }
            let execution_scope = decode_execution(decoder)?;
            let os_background_service_enabled = decode_bool(
                decoder,
                "runtime-ipc-ticket-automation-status-v1.os-background-service-enabled",
                true,
            )?;
            let declared = usize::try_from(
                decoder.unsigned("runtime-ipc-ticket-automation-status-v1.policy-count")?,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            if declared != policies.len() {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(
                CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationStatus(Box::new(
                    CanonicalRuntimeIpcTicketAutomationStatusV1 {
                        observed_at_unix_seconds,
                        current_network,
                        snapshot_id,
                        policies,
                        execution_scope,
                        os_background_service_enabled,
                    },
                )),
            )
        }
        RESULT_ERROR if length == 3 => Ok(CanonicalRuntimeIpcTicketAutomationResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-ticket-automation-error-v1.code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-ticket-automation-error-v1.message")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_optional_previous(
    output: &mut Vec<u8>,
    value: Option<CanonicalRuntimeIpcTicketAutomationPreviousPolicyV1>,
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
) -> Result<Option<CanonicalRuntimeIpcTicketAutomationPreviousPolicyV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-ticket-automation-previous-policy-v1")?;
    match decoder.unsigned("runtime-ipc-ticket-automation-previous-policy-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 3 => {
            Ok(Some(CanonicalRuntimeIpcTicketAutomationPreviousPolicyV1 {
                generation: decoder
                    .unsigned("runtime-ipc-ticket-automation-previous-policy-v1.generation")?,
                policy_id: decoder
                    .fixed_bytes("runtime-ipc-ticket-automation-previous-policy-v1.policy-id")?,
            }))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
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
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-ticket-automation-optional-id-v1")?;
    match decoder.unsigned("runtime-ipc-ticket-automation-optional-id-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 2 => Ok(Some(
            decoder.fixed_bytes("runtime-ipc-ticket-automation-optional-id-v1.value")?,
        )),
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
) -> Result<Option<u64>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-ticket-automation-optional-unsigned-v1")?;
    match decoder.unsigned("runtime-ipc-ticket-automation-optional-unsigned-v1.variant")? {
        OPTION_NONE if length == 1 => Ok(None),
        OPTION_SOME if length == 2 => Ok(Some(
            decoder.unsigned("runtime-ipc-ticket-automation-optional-unsigned-v1.value")?,
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_network(output: &mut Vec<u8>, value: CanonicalRuntimeIpcTicketAutomationNetworkV1) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcTicketAutomationNetworkV1::Ethernet => NETWORK_ETHERNET,
            CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi => NETWORK_WIFI,
            CanonicalRuntimeIpcTicketAutomationNetworkV1::Mobile => NETWORK_MOBILE,
            CanonicalRuntimeIpcTicketAutomationNetworkV1::Unknown => NETWORK_UNKNOWN,
        },
    );
}

fn decode_network(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationNetworkV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-ticket-automation-network-v1")? {
        NETWORK_ETHERNET => Ok(CanonicalRuntimeIpcTicketAutomationNetworkV1::Ethernet),
        NETWORK_WIFI => Ok(CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi),
        NETWORK_MOBILE => Ok(CanonicalRuntimeIpcTicketAutomationNetworkV1::Mobile),
        NETWORK_UNKNOWN => Ok(CanonicalRuntimeIpcTicketAutomationNetworkV1::Unknown),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_action(output: &mut Vec<u8>, value: CanonicalRuntimeIpcTicketAutomationActionV1) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcTicketAutomationActionV1::Publish => ACTION_PUBLISH,
            CanonicalRuntimeIpcTicketAutomationActionV1::Refresh => ACTION_REFRESH,
        },
    );
}

fn decode_action(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationActionV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-ticket-automation-action-v1")? {
        ACTION_PUBLISH => Ok(CanonicalRuntimeIpcTicketAutomationActionV1::Publish),
        ACTION_REFRESH => Ok(CanonicalRuntimeIpcTicketAutomationActionV1::Refresh),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_scheduler_state(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcTicketAutomationSchedulerStateV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Disabled => STATE_DISABLED,
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::NetworkBlocked => {
                STATE_NETWORK_BLOCKED
            }
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::PublicationConflictQuarantined => {
                STATE_PUBLICATION_CONFLICT_QUARANTINED
            }
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Due => STATE_DUE,
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Fresh => STATE_FRESH,
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Backoff => STATE_BACKOFF,
        },
    );
}

fn decode_scheduler_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationSchedulerStateV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-ticket-automation-scheduler-state-v1")? {
        STATE_DISABLED => Ok(CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Disabled),
        STATE_NETWORK_BLOCKED => {
            Ok(CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::NetworkBlocked)
        }
        STATE_PUBLICATION_CONFLICT_QUARANTINED => {
            Ok(CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::PublicationConflictQuarantined)
        }
        STATE_DUE => Ok(CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Due),
        STATE_FRESH => Ok(CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Fresh),
        STATE_BACKOFF => Ok(CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Backoff),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_last_result(
    output: &mut Vec<u8>,
    value: CanonicalRuntimeIpcTicketAutomationLastResultV1,
) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcTicketAutomationLastResultV1::Never => LAST_RESULT_NEVER,
            CanonicalRuntimeIpcTicketAutomationLastResultV1::Succeeded => LAST_RESULT_SUCCEEDED,
            CanonicalRuntimeIpcTicketAutomationLastResultV1::Failed => LAST_RESULT_FAILED,
        },
    );
}

fn decode_last_result(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationLastResultV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-ticket-automation-last-result-v1")? {
        LAST_RESULT_NEVER => Ok(CanonicalRuntimeIpcTicketAutomationLastResultV1::Never),
        LAST_RESULT_SUCCEEDED => Ok(CanonicalRuntimeIpcTicketAutomationLastResultV1::Succeeded),
        LAST_RESULT_FAILED => Ok(CanonicalRuntimeIpcTicketAutomationLastResultV1::Failed),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_outcome(output: &mut Vec<u8>, value: CanonicalRuntimeIpcTicketAutomationOutcomeV1) {
    push_unsigned(
        output,
        match value {
            CanonicalRuntimeIpcTicketAutomationOutcomeV1::Committed => OUTCOME_COMMITTED,
            CanonicalRuntimeIpcTicketAutomationOutcomeV1::AlreadyCommitted => {
                OUTCOME_ALREADY_COMMITTED
            }
        },
    );
}

fn decode_outcome(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationOutcomeV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-ticket-automation-outcome-v1")? {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcTicketAutomationOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcTicketAutomationOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_restart(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationRestartStateV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-ticket-automation-restart-v1")? {
        RESTART_NOT_REQUIRED => Ok(CanonicalRuntimeIpcTicketAutomationRestartStateV1::NotRequired),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_execution(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcTicketAutomationExecutionScopeV1, CanonicalRuntimeIpcError> {
    match decoder.unsigned("runtime-ipc-ticket-automation-execution-v1")? {
        EXECUTION_FOREGROUND_RUNTIME => {
            Ok(CanonicalRuntimeIpcTicketAutomationExecutionScopeV1::ForegroundRuntimeOnly)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn network_allowed(mask: u8, network: CanonicalRuntimeIpcTicketAutomationNetworkV1) -> bool {
    let bit = match network {
        CanonicalRuntimeIpcTicketAutomationNetworkV1::Ethernet => 0,
        CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi => 1,
        CanonicalRuntimeIpcTicketAutomationNetworkV1::Mobile => 2,
        CanonicalRuntimeIpcTicketAutomationNetworkV1::Unknown => 3,
    };
    mask & (1 << bit) != 0
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
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidPublicKey)?;
    key.verify_strict(message, &Signature::from_bytes(signature))
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
    use super::*;
    use crate::canonical_runtime_ipc_v1::{
        CanonicalRuntimeIpcDescriptorContentV1, CanonicalRuntimeIpcLoopbackAddressV1,
        encode_signed_runtime_ipc_descriptor_v1, sign_runtime_ipc_descriptor_v1,
    };
    use serde_json::{Value, json};
    use std::{fs, path::PathBuf};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn fixture() -> Result<
        (
            SigningKey,
            CanonicalSignedRuntimeIpcDescriptorV1,
            CanonicalRuntimeIpcTicketAutomationRequestV1,
            CanonicalRuntimeIpcTicketAutomationPolicyStatusV1,
        ),
        CanonicalRuntimeIpcError,
    > {
        let key = SigningKey::from_bytes(&[9; 32]);
        let descriptor = sign_runtime_ipc_descriptor_v1(
            &key,
            CanonicalRuntimeIpcDescriptorContentV1 {
                address: CanonicalRuntimeIpcLoopbackAddressV1::Ipv4([127, 0, 0, 1]),
                port: 43125,
                bearer_token: [2; 32],
                account_id: [3; 32],
                device_id: key.verifying_key().to_bytes(),
                server_instance_id: [4; 32],
                issued_at_unix_seconds: 100,
                expires_at_unix_seconds: 1000,
            },
        )?;
        let conversation_label_utf8 = b"ticket-automation".to_vec();
        let peer_account_id = [6; 32];
        let contact_id = canonical_runtime_ipc_contact_id_v1(
            &[3; 32],
            &peer_account_id,
            &conversation_label_utf8,
        )?;
        let configuration = CanonicalRuntimeIpcTicketAutomationConfigurationV1 {
            contact_id,
            conversation_label_utf8,
            peer_account_id,
            enabled: true,
            service_origin_utf8: b"https://tickets.example/".to_vec(),
            ttl_seconds: 900,
            refresh_before_seconds: 300,
            retry_base_seconds: 5,
            retry_max_seconds: 300,
            allowed_network_mask: 0b0011,
        };
        let request = CanonicalRuntimeIpcTicketAutomationRequestV1 {
            descriptor_id: runtime_ipc_descriptor_id_v1(&descriptor)?,
            request_id: [7; 32],
            bearer_token: [2; 32],
            issued_at_unix_seconds: 200,
            expires_at_unix_seconds: 230,
            command: CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
                expected_previous: None,
                configuration: configuration.clone(),
            },
        };
        let policy_id = runtime_ipc_ticket_automation_policy_id_v1(
            &[3; 32],
            &key.verifying_key().to_bytes(),
            1,
            None,
            200,
            &configuration,
        )?;
        let policy = CanonicalRuntimeIpcTicketAutomationPolicyStatusV1 {
            configuration,
            generation: 1,
            previous_policy_id: None,
            policy_id,
            configured_at_unix_seconds: 200,
            publish: CanonicalRuntimeIpcTicketAutomationActionStatusV1 {
                action: CanonicalRuntimeIpcTicketAutomationActionV1::Publish,
                scheduler_state: CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Fresh,
                last_attempt_unix_seconds: Some(205),
                last_success_unix_seconds: Some(205),
                next_attempt_unix_seconds: Some(805),
                consecutive_failures: 0,
                publication_generation: Some(1),
                expires_at_unix_seconds: Some(1105),
                last_result: CanonicalRuntimeIpcTicketAutomationLastResultV1::Succeeded,
            },
            refresh: CanonicalRuntimeIpcTicketAutomationActionStatusV1 {
                action: CanonicalRuntimeIpcTicketAutomationActionV1::Refresh,
                scheduler_state: CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Backoff,
                last_attempt_unix_seconds: Some(206),
                last_success_unix_seconds: None,
                next_attempt_unix_seconds: Some(211),
                consecutive_failures: 1,
                publication_generation: None,
                expires_at_unix_seconds: None,
                last_result: CanonicalRuntimeIpcTicketAutomationLastResultV1::Failed,
            },
        };
        Ok((key, descriptor, request, policy))
    }

    fn responses(
        key: &SigningKey,
        descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
        request: &CanonicalRuntimeIpcTicketAutomationRequestV1,
        policy: &CanonicalRuntimeIpcTicketAutomationPolicyStatusV1,
    ) -> Result<
        (
            CanonicalSignedRuntimeIpcTicketAutomationResponseV1,
            CanonicalRuntimeIpcTicketAutomationRequestV1,
            CanonicalSignedRuntimeIpcTicketAutomationResponseV1,
        ),
        CanonicalRuntimeIpcError,
    > {
        let configure_response = sign_runtime_ipc_ticket_automation_response_v1(
            key,
            request,
            descriptor,
            210,
            CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationConfigured(Box::new(
                CanonicalRuntimeIpcTicketAutomationConfiguredV1 {
                    outcome: CanonicalRuntimeIpcTicketAutomationOutcomeV1::Committed,
                    observed_at_unix_seconds: 210,
                    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                    status: policy.clone(),
                    restart_state: CanonicalRuntimeIpcTicketAutomationRestartStateV1::NotRequired,
                    execution_scope:
                        CanonicalRuntimeIpcTicketAutomationExecutionScopeV1::ForegroundRuntimeOnly,
                    os_background_service_enabled: false,
                },
            )),
        )?;
        let status_request = CanonicalRuntimeIpcTicketAutomationRequestV1 {
            descriptor_id: request.descriptor_id,
            request_id: [8; 32],
            bearer_token: request.bearer_token,
            issued_at_unix_seconds: 210,
            expires_at_unix_seconds: 230,
            command: CanonicalRuntimeIpcTicketAutomationCommandV1::TicketAutomationStatus,
        };
        let snapshot_id = runtime_ipc_ticket_automation_snapshot_id_v1(
            &descriptor.content.account_id,
            &descriptor.content.device_id,
            210,
            CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
            std::slice::from_ref(policy),
        )?;
        let status_response = sign_runtime_ipc_ticket_automation_response_v1(
            key,
            &status_request,
            descriptor,
            210,
            CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationStatus(Box::new(
                CanonicalRuntimeIpcTicketAutomationStatusV1 {
                    observed_at_unix_seconds: 210,
                    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                    snapshot_id,
                    policies: vec![policy.clone()],
                    execution_scope:
                        CanonicalRuntimeIpcTicketAutomationExecutionScopeV1::ForegroundRuntimeOnly,
                    os_background_service_enabled: false,
                },
            )),
        )?;
        Ok((configure_response, status_request, status_response))
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn domain(bytes: &[u8]) -> String {
        assert_eq!(bytes.last(), Some(&0));
        String::from_utf8_lossy(&bytes[..bytes.len() - 1]).into_owned()
    }

    fn vectors() -> Result<Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json"
        )))?)
    }

    fn vector_bytes(
        document: &Value,
        pointer: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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

    fn vector_fixed<const N: usize>(
        document: &Value,
        pointer: &str,
    ) -> Result<[u8; N], Box<dyn std::error::Error>> {
        Ok(vector_bytes(document, pointer)?
            .try_into()
            .map_err(|_| format!("wrong byte length at {pointer}"))?)
    }

    #[test]
    fn configure_and_status_are_exact_signed_and_round_trip() -> TestResult {
        let (key, descriptor, request, policy) = fixture()?;
        let encoded = encode_runtime_ipc_ticket_automation_request_v1(&request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_ticket_automation_request_v1(&encoded)?,
            request
        );
        let (response, status_request, status_response) =
            responses(&key, &descriptor, &request, &policy)?;
        let response_bytes = encode_signed_runtime_ipc_ticket_automation_response_v1(&response)?;
        assert_eq!(
            decode_and_verify_signed_runtime_ipc_ticket_automation_response_v1(&response_bytes)?,
            response
        );
        verify_runtime_ipc_ticket_automation_response_for_request_v1(
            &response,
            &request,
            &descriptor,
            210,
        )?;
        verify_runtime_ipc_ticket_automation_response_for_request_v1(
            &status_response,
            &status_request,
            &descriptor,
            210,
        )?;
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_ticket_automation_vectors_match() -> TestResult {
        let (key, descriptor, configure_request, policy) = fixture()?;
        let (configure_response, status_request, status_response) =
            responses(&key, &descriptor, &configure_request, &policy)?;
        let vectors = vectors()?;
        assert_eq!(
            encode_runtime_ipc_ticket_automation_request_v1(&configure_request)?,
            vector_bytes(&vectors, "/expected/configure/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_ticket_automation_request_digest_v1(&configure_request)?,
            vector_fixed(&vectors, "/expected/configure/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_ticket_automation_response_v1(&configure_response)?,
            vector_bytes(&vectors, "/expected/configure/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_ticket_automation_response_id_v1(&configure_response)?,
            vector_fixed(&vectors, "/expected/configure/response/object_id_hex")?
        );
        assert_eq!(
            encode_runtime_ipc_ticket_automation_request_v1(&status_request)?,
            vector_bytes(&vectors, "/expected/status/request/cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_ticket_automation_request_digest_v1(&status_request)?,
            vector_fixed(&vectors, "/expected/status/request/object_id_hex")?
        );
        assert_eq!(
            encode_signed_runtime_ipc_ticket_automation_response_v1(&status_response)?,
            vector_bytes(&vectors, "/expected/status/response/signed_cbor_hex")?
        );
        assert_eq!(
            runtime_ipc_ticket_automation_response_id_v1(&status_response)?,
            vector_fixed(&vectors, "/expected/status/response/object_id_hex")?
        );
        Ok(())
    }

    #[test]
    fn conflict_quarantine_is_terminal_until_reconfigured() -> TestResult {
        let (_, _, _, mut policy) = fixture()?;
        policy.refresh.scheduler_state =
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::PublicationConflictQuarantined;
        policy.refresh.next_attempt_unix_seconds = None;
        validate_action_status(
            &policy.refresh,
            &policy.configuration,
            CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
            210,
        )?;

        let mut never = policy.refresh.clone();
        never.last_result = CanonicalRuntimeIpcTicketAutomationLastResultV1::Never;
        never.last_attempt_unix_seconds = None;
        never.consecutive_failures = 0;
        assert!(matches!(
            validate_action_status(
                &never,
                &policy.configuration,
                CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                210,
            ),
            Err(CanonicalRuntimeIpcError::InvalidResponse)
        ));

        let mut future_attempt = policy.refresh.clone();
        future_attempt.last_attempt_unix_seconds = Some(211);
        assert!(matches!(
            validate_action_status(
                &future_attempt,
                &policy.configuration,
                CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                210,
            ),
            Err(CanonicalRuntimeIpcError::InvalidResponse)
        ));
        Ok(())
    }

    #[test]
    #[ignore = "explicit golden-vector regeneration"]
    fn regenerate_canonical_runtime_ipc_ticket_automation_vectors() -> TestResult {
        let (key, descriptor, configure_request, policy) = fixture()?;
        let (configure_response, status_request, status_response) =
            responses(&key, &descriptor, &configure_request, &policy)?;
        let descriptor_bytes = encode_signed_runtime_ipc_descriptor_v1(&descriptor)?;
        let configure_request_bytes =
            encode_runtime_ipc_ticket_automation_request_v1(&configure_request)?;
        let configure_response_bytes =
            encode_signed_runtime_ipc_ticket_automation_response_v1(&configure_response)?;
        let status_request_bytes =
            encode_runtime_ipc_ticket_automation_request_v1(&status_request)?;
        let status_response_bytes =
            encode_signed_runtime_ipc_ticket_automation_response_v1(&status_response)?;
        let CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationStatus(status) =
            &status_response.content.result
        else {
            unreachable!()
        };
        let document = json!({
            "schema_version": 1,
            "milestone": "M1.1.47",
            "status": "candidate-not-active",
            "domains": {
                "policy_id": domain(CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_POLICY_ID_DOMAIN),
                "snapshot_id": domain(CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SNAPSHOT_ID_DOMAIN),
                "request_digest": domain(CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REQUEST_DIGEST_DOMAIN),
                "response_signature": domain(CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_SIGNATURE_DOMAIN),
                "response_id": domain(CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_ID_DOMAIN)
            },
            "input": {
                "device_secret_key_hex": hex(&key.to_bytes()),
                "descriptor_signed_cbor_hex": hex(&descriptor_bytes),
                "descriptor_id_hex": hex(&runtime_ipc_descriptor_id_v1(&descriptor)?),
                "bearer_token_hex": hex(&descriptor.content.bearer_token),
                "account_id_hex": hex(&descriptor.content.account_id),
                "device_id_hex": hex(&descriptor.content.device_id),
                "contact_id_hex": hex(&policy.configuration.contact_id),
                "conversation_label_utf8": String::from_utf8(policy.configuration.conversation_label_utf8.clone())?,
                "peer_account_id_hex": hex(&policy.configuration.peer_account_id),
                "enabled": policy.configuration.enabled,
                "service_origin_utf8": String::from_utf8(policy.configuration.service_origin_utf8.clone())?,
                "ttl_seconds": policy.configuration.ttl_seconds,
                "refresh_before_seconds": policy.configuration.refresh_before_seconds,
                "retry_base_seconds": policy.configuration.retry_base_seconds,
                "retry_max_seconds": policy.configuration.retry_max_seconds,
                "allowed_network_mask": policy.configuration.allowed_network_mask,
                "configured_at_unix_seconds": policy.configured_at_unix_seconds,
                "observed_at_unix_seconds": 210,
                "current_network": "wifi"
            },
            "expected": {
                "policy_id_hex": hex(&policy.policy_id),
                "snapshot_id_hex": hex(&status.snapshot_id),
                "configure": {
                    "request": {
                        "cbor_hex": hex(&configure_request_bytes),
                        "object_id_hex": hex(&runtime_ipc_ticket_automation_request_digest_v1(&configure_request)?)
                    },
                    "response": {
                        "content_cbor_hex": hex(&encode_response_content(&configure_response.content)?),
                        "signing_bytes_hex": hex(&runtime_ipc_ticket_automation_response_signing_bytes_v1(&configure_response.content)?),
                        "signature_hex": hex(&configure_response.signature),
                        "signed_cbor_hex": hex(&configure_response_bytes),
                        "object_id_hex": hex(&runtime_ipc_ticket_automation_response_id_v1(&configure_response)?)
                    }
                },
                "status": {
                    "request": {
                        "cbor_hex": hex(&status_request_bytes),
                        "object_id_hex": hex(&runtime_ipc_ticket_automation_request_digest_v1(&status_request)?)
                    },
                    "response": {
                        "content_cbor_hex": hex(&encode_response_content(&status_response.content)?),
                        "signing_bytes_hex": hex(&runtime_ipc_ticket_automation_response_signing_bytes_v1(&status_response.content)?),
                        "signature_hex": hex(&status_response.signature),
                        "signed_cbor_hex": hex(&status_response_bytes),
                        "object_id_hex": hex(&runtime_ipc_ticket_automation_response_id_v1(&status_response)?)
                    }
                }
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }

    #[test]
    fn inconsistent_scheduler_state_and_context_fail_closed() -> TestResult {
        let (key, descriptor, request, policy) = fixture()?;
        let mut inconsistent_policy = policy.clone();
        inconsistent_policy.publish.scheduler_state =
            CanonicalRuntimeIpcTicketAutomationSchedulerStateV1::Due;
        assert!(sign_runtime_ipc_ticket_automation_response_v1(
            &key,
            &request,
            &descriptor,
            210,
            CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationConfigured(Box::new(
                CanonicalRuntimeIpcTicketAutomationConfiguredV1 {
                    outcome: CanonicalRuntimeIpcTicketAutomationOutcomeV1::Committed,
                    observed_at_unix_seconds: 210,
                    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                    status: inconsistent_policy,
                    restart_state: CanonicalRuntimeIpcTicketAutomationRestartStateV1::NotRequired,
                    execution_scope: CanonicalRuntimeIpcTicketAutomationExecutionScopeV1::ForegroundRuntimeOnly,
                    os_background_service_enabled: false,
                },
            )),
        ).is_err());
        let mut inconsistent_success_deadline = policy.clone();
        inconsistent_success_deadline
            .publish
            .next_attempt_unix_seconds = Some(804);
        assert!(
            validate_policy_status(
                &inconsistent_success_deadline,
                &descriptor.content.account_id,
                &descriptor.content.device_id,
                CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                210,
            )
            .is_err()
        );
        let mut inconsistent_retry_deadline = policy.clone();
        inconsistent_retry_deadline
            .refresh
            .next_attempt_unix_seconds = Some(212);
        assert!(
            validate_policy_status(
                &inconsistent_retry_deadline,
                &descriptor.content.account_id,
                &descriptor.content.device_id,
                CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                210,
            )
            .is_err()
        );
        assert!(sign_runtime_ipc_ticket_automation_response_v1(
            &key,
            &request,
            &descriptor,
            210,
            CanonicalRuntimeIpcTicketAutomationResultV1::TicketAutomationConfigured(Box::new(
                CanonicalRuntimeIpcTicketAutomationConfiguredV1 {
                    outcome: CanonicalRuntimeIpcTicketAutomationOutcomeV1::Committed,
                    observed_at_unix_seconds: request.expires_at_unix_seconds + 1,
                    current_network: CanonicalRuntimeIpcTicketAutomationNetworkV1::Wifi,
                    status: policy.clone(),
                    restart_state: CanonicalRuntimeIpcTicketAutomationRestartStateV1::NotRequired,
                    execution_scope:
                        CanonicalRuntimeIpcTicketAutomationExecutionScopeV1::ForegroundRuntimeOnly,
                    os_background_service_enabled: false,
                },
            )),
        )
        .is_err());
        let mut default_port_request = request.clone();
        let CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
            configuration,
            ..
        } = &mut default_port_request.command
        else {
            unreachable!()
        };
        configuration.service_origin_utf8 = b"https://tickets.example:443/".to_vec();
        assert!(
            verify_runtime_ipc_ticket_automation_request_for_descriptor_v1(
                &default_port_request,
                &descriptor,
                210,
            )
            .is_err()
        );
        let mut wrong_contact_request = request.clone();
        let CanonicalRuntimeIpcTicketAutomationCommandV1::ConfigureTicketAutomation {
            configuration,
            ..
        } = &mut wrong_contact_request.command
        else {
            unreachable!()
        };
        configuration.contact_id = [5; 32];
        assert!(
            runtime_ipc_ticket_automation_policy_id_v1(
                &descriptor.content.account_id,
                &descriptor.content.device_id,
                1,
                None,
                request.issued_at_unix_seconds,
                configuration,
            )
            .is_err()
        );
        assert!(
            verify_runtime_ipc_ticket_automation_request_for_descriptor_v1(
                &wrong_contact_request,
                &descriptor,
                210,
            )
            .is_err()
        );
        let mut wrong_request = request;
        wrong_request.bearer_token = [99; 32];
        assert!(
            verify_runtime_ipc_ticket_automation_request_for_descriptor_v1(
                &wrong_request,
                &descriptor,
                210
            )
            .is_err()
        );
        Ok(())
    }
}
