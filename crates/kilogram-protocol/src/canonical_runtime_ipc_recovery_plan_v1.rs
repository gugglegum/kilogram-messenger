//! Inactive deterministic-CBOR history-recovery plan custody boundary.
//!
//! The foreground runtime verifies one source-signed recovery link, compares
//! the full SAS digest confirmed by the local caller, signs the exact bounded
//! plan with its Device key and retains a content-addressed record. Cancellation
//! is a separate immutable Device-signed tombstone. This candidate deliberately
//! excludes network execution, OS background registration, filesystem paths and
//! every private recovery capability. The active Postcard runtime IPC remains
//! unchanged.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_recovery_exchange_v1::{
        CanonicalHistoryRecoveryPlanContentV1, CanonicalRecoveryExecutionPolicyV1,
        CanonicalSignedHistoryRecoveryLinkV1, CanonicalSignedHistoryRecoveryPlanV1,
        MAX_CANONICAL_RECOVERY_PLAN_VALIDITY_SECONDS,
        decode_and_verify_signed_history_recovery_link_v1,
        decode_and_verify_signed_history_recovery_plan_v1, encode_recovery_execution_policy_v1,
        encode_signed_history_recovery_link_v1, encode_signed_history_recovery_plan_v1,
        history_recovery_plan_id_v1, history_recovery_plan_signing_bytes_v1,
        history_recovery_sas_v1, verify_signed_history_recovery_link_v1,
        verify_signed_history_recovery_plan_v1,
    },
    canonical_runtime_ipc_v1::{
        CANONICAL_RUNTIME_IPC_CLOCK_SKEW_SECONDS, CanonicalRuntimeIpcError,
        CanonicalSignedRuntimeIpcDescriptorV1, MAX_CANONICAL_RUNTIME_IPC_ERROR_BYTES,
        MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS, runtime_ipc_descriptor_id_v1,
        verify_runtime_ipc_descriptor_at_v1, verify_signed_runtime_ipc_descriptor_v1,
    },
    canonical_v1::{
        CANONICAL_PROFILE_VERSION, prefixed, push_array_header, push_bytes, push_unsigned,
        validate_canonical_profile_v1,
    },
};

pub const MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_REQUEST_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_CANCELLATION_BYTES: usize = 1024;
pub const MIN_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_VALIDITY_SECONDS: u64 = 60 * 60;

pub const CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_CANCELLATION_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-plan-cancellation-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RECORD_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-plan-record-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-plan-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-plan-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-plan-response-id:cbor-v1\0";

const COMMAND_APPROVE: u64 = 0;
const COMMAND_STATUS: u64 = 1;
const COMMAND_CANCEL: u64 = 2;
const RESULT_APPROVED: u64 = 0;
const RESULT_STATUS: u64 = 1;
const RESULT_CANCELLED: u64 = 2;
const RESULT_ERROR: u64 = 3;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const LIFECYCLE_APPROVED: u64 = 0;
const LIFECYCLE_CANCELLED: u64 = 1;
const CANCELLATION_USER_REQUESTED: u64 = 0;
const RESTART_NOT_REQUIRED: u64 = 0;
const EXECUTION_FOREGROUND_RUNTIME: u64 = 0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPlanCommandV1 {
    ApproveHistoryRecoveryPlan {
        signed_link: Box<CanonicalSignedHistoryRecoveryLinkV1>,
        confirmed_sas: [u8; 32],
        execution_policy: CanonicalRecoveryExecutionPolicyV1,
        valid_for_seconds: u64,
    },
    HistoryRecoveryPlanStatus {
        plan_id: [u8; 32],
    },
    CancelHistoryRecoveryPlan {
        plan_id: [u8; 32],
        expected_record_id: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryPlanRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcRecoveryPlanCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPlanCancellationReasonV1 {
    UserRequested,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryPlanCancellationContentV1 {
    pub plan_id: [u8; 32],
    pub account_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub cancelled_at_unix_seconds: u64,
    pub reason: CanonicalRuntimeIpcRecoveryPlanCancellationReasonV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1 {
    pub content: CanonicalRuntimeIpcRecoveryPlanCancellationContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPlanLifecycleV1 {
    Approved,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryPlanStateV1 {
    pub record_id: [u8; 32],
    pub lifecycle: CanonicalRuntimeIpcRecoveryPlanLifecycleV1,
    pub signed_plan: CanonicalSignedHistoryRecoveryPlanV1,
    pub cancellation: Option<CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPlanOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPlanRestartStateV1 {
    NotRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPlanExecutionScopeV1 {
    ForegroundRuntimeOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryPlanCustodyV1 {
    pub state: CanonicalRuntimeIpcRecoveryPlanStateV1,
    pub restart_state: CanonicalRuntimeIpcRecoveryPlanRestartStateV1,
    pub execution_scope: CanonicalRuntimeIpcRecoveryPlanExecutionScopeV1,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryPlanTransitionV1 {
    pub outcome: CanonicalRuntimeIpcRecoveryPlanOutcomeV1,
    pub custody: CanonicalRuntimeIpcRecoveryPlanCustodyV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPlanResultV1 {
    HistoryRecoveryPlanApproved(Box<CanonicalRuntimeIpcRecoveryPlanTransitionV1>),
    HistoryRecoveryPlanStatus(Box<CanonicalRuntimeIpcRecoveryPlanCustodyV1>),
    HistoryRecoveryPlanCancelled(Box<CanonicalRuntimeIpcRecoveryPlanTransitionV1>),
    Error { code: u16, message_utf8: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryPlanResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcRecoveryPlanResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcRecoveryPlanResponseV1 {
    pub content: CanonicalRuntimeIpcRecoveryPlanResponseContentV1,
    pub signature: [u8; 64],
}

pub fn encode_runtime_ipc_recovery_plan_request_v1(
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
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
        "recovery plan request",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_recovery_plan_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcRecoveryPlanRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "recovery plan request",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-recovery-plan-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-recovery-plan-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcRecoveryPlanRequestV1 {
        descriptor_id: decoder.fixed_bytes("runtime-ipc-recovery-plan-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-recovery-plan-request-v1.request-id")?,
        bearer_token: decoder.fixed_bytes("runtime-ipc-recovery-plan-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-recovery-plan-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-recovery-plan-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_recovery_plan_request_digest_v1(
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_recovery_plan_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_recovery_plan_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
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
    if let CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
        signed_link,
        ..
    } = &request.command
    {
        let link = &signed_link.content;
        if link.recipient_device_id != descriptor.content.device_id
            || plan_account_id_from_link(signed_link) != descriptor.content.account_id
        {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
    }
    Ok(())
}

pub fn create_runtime_ipc_recovery_plan_state_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPlanStateV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_plan_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
        signed_link,
        confirmed_sas,
        execution_policy,
        valid_for_seconds,
    } = &request.command
    else {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    };
    let expires_at_unix_seconds = request
        .issued_at_unix_seconds
        .checked_add(*valid_for_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    let content = CanonicalHistoryRecoveryPlanContentV1 {
        signed_link: (**signed_link).clone(),
        sas: *confirmed_sas,
        execution_policy: *execution_policy,
        approved_at_unix_seconds: request.issued_at_unix_seconds,
        expires_at_unix_seconds,
    };
    let signature = device_signing_key
        .sign(
            &history_recovery_plan_signing_bytes_v1(&content)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
        )
        .to_bytes();
    let plan = CanonicalSignedHistoryRecoveryPlanV1 { content, signature };
    verify_signed_history_recovery_plan_v1(&plan)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    let mut state = CanonicalRuntimeIpcRecoveryPlanStateV1 {
        record_id: [0; 32],
        lifecycle: CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved,
        signed_plan: plan,
        cancellation: None,
    };
    state.record_id = runtime_ipc_recovery_plan_record_id_v1(&state)?;
    Ok(state)
}

pub fn runtime_ipc_recovery_plan_cancellation_signing_bytes_v1(
    content: &CanonicalRuntimeIpcRecoveryPlanCancellationContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_CANCELLATION_SIGNATURE_DOMAIN,
        &encode_cancellation_content(content)?,
    ))
}

pub fn sign_runtime_ipc_recovery_plan_cancellation_v1(
    device_signing_key: &SigningKey,
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
    cancelled_at_unix_seconds: u64,
) -> Result<CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1, CanonicalRuntimeIpcError> {
    verify_signed_history_recovery_plan_v1(plan)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    let recipient_device_id = plan.content.signed_link.content.recipient_device_id;
    if device_signing_key.verifying_key().to_bytes() != recipient_device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcRecoveryPlanCancellationContentV1 {
        plan_id: history_recovery_plan_id_v1(plan)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
        account_id: plan_account_id(plan),
        recipient_device_id,
        cancelled_at_unix_seconds,
        reason: CanonicalRuntimeIpcRecoveryPlanCancellationReasonV1::UserRequested,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_recovery_plan_cancellation_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    Ok(CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1 { content, signature })
}

pub fn encode_signed_runtime_ipc_recovery_plan_cancellation_v1(
    cancellation: &CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_plan_cancellation_v1(cancellation)?;
    let output = signed_envelope(
        &encode_cancellation_content(&cancellation.content)?,
        &cancellation.signature,
    );
    ensure_bound(
        &output,
        "recovery plan cancellation",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_CANCELLATION_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_recovery_plan_cancellation_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "recovery plan cancellation",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_CANCELLATION_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-recovery-plan-cancellation-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-recovery-plan-cancellation-v1.schema-version")?;
    let content = decode_cancellation_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-recovery-plan-cancellation-v1.signature")?;
    decoder.finish()?;
    let cancellation = CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1 { content, signature };
    verify_signed_runtime_ipc_recovery_plan_cancellation_v1(&cancellation)?;
    Ok(cancellation)
}

pub fn verify_signed_runtime_ipc_recovery_plan_cancellation_v1(
    cancellation: &CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_cancellation_content(&cancellation.content)?;
    verify_ed25519(
        &cancellation.content.recipient_device_id,
        &runtime_ipc_recovery_plan_cancellation_signing_bytes_v1(&cancellation.content)?,
        &cancellation.signature,
    )
}

pub fn cancel_runtime_ipc_recovery_plan_state_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    current: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    now_unix_seconds: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPlanStateV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_plan_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    validate_state(current)?;
    let CanonicalRuntimeIpcRecoveryPlanCommandV1::CancelHistoryRecoveryPlan {
        plan_id,
        expected_record_id,
    } = &request.command
    else {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    };
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id
        || plan_account_id(&current.signed_plan) != descriptor.content.account_id
        || current
            .signed_plan
            .content
            .signed_link
            .content
            .recipient_device_id
            != descriptor.content.device_id
        || history_recovery_plan_id_v1(&current.signed_plan)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?
            != *plan_id
        || current.record_id != *expected_record_id
        || current.lifecycle != CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved
        || current.cancellation.is_some()
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let cancellation = sign_runtime_ipc_recovery_plan_cancellation_v1(
        device_signing_key,
        &current.signed_plan,
        request.issued_at_unix_seconds,
    )?;
    let mut state = CanonicalRuntimeIpcRecoveryPlanStateV1 {
        record_id: [0; 32],
        lifecycle: CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Cancelled,
        signed_plan: current.signed_plan.clone(),
        cancellation: Some(cancellation),
    };
    state.record_id = runtime_ipc_recovery_plan_record_id_v1(&state)?;
    Ok(state)
}

pub fn runtime_ipc_recovery_plan_record_id_v1(
    state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    verify_signed_history_recovery_plan_v1(&state.signed_plan)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if let Some(cancellation) = &state.cancellation {
        verify_cancellation_for_plan(cancellation, &state.signed_plan)?;
    }
    let mut preimage = Vec::new();
    push_array_header(&mut preimage, 3);
    push_unsigned(&mut preimage, CANONICAL_PROFILE_VERSION);
    push_bytes(
        &mut preimage,
        &encode_signed_history_recovery_plan_v1(&state.signed_plan)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    encode_cancellation_option(&mut preimage, state.cancellation.as_ref())?;
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RECORD_ID_DOMAIN,
        &preimage,
    ))
}

pub fn encode_runtime_ipc_recovery_plan_state_v1(
    state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_state(state)?;
    let mut output = Vec::new();
    encode_state(&mut output, state)?;
    ensure_bound(
        &output,
        "recovery plan state",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_recovery_plan_state_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcRecoveryPlanStateV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "recovery plan state",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    let state = decode_state(&mut decoder)?;
    decoder.finish()?;
    validate_state(&state)?;
    Ok(state)
}

pub fn verify_runtime_ipc_recovery_plan_state_v1(
    state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_state(state)
}

pub fn runtime_ipc_recovery_plan_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcRecoveryPlanResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_response_content(content)?;
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content_unchecked(content)?,
    ))
}

pub fn sign_runtime_ipc_recovery_plan_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    now_unix_seconds: u64,
    result: CanonicalRuntimeIpcRecoveryPlanResultV1,
) -> Result<CanonicalSignedRuntimeIpcRecoveryPlanResponseV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_plan_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_result_for_request(&result, request, descriptor)?;
    let content = CanonicalRuntimeIpcRecoveryPlanResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_recovery_plan_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    let signature = device_signing_key
        .sign(&runtime_ipc_recovery_plan_response_signing_bytes_v1(
            &content,
        )?)
        .to_bytes();
    Ok(CanonicalSignedRuntimeIpcRecoveryPlanResponseV1 { content, signature })
}

pub fn encode_signed_runtime_ipc_recovery_plan_response_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryPlanResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_plan_response_v1(response)?;
    let output = signed_envelope(
        &encode_response_content_unchecked(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "recovery plan response",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_recovery_plan_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcRecoveryPlanResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "recovery plan response",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-recovery-plan-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-recovery-plan-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-recovery-plan-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcRecoveryPlanResponseV1 { content, signature };
    verify_signed_runtime_ipc_recovery_plan_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_recovery_plan_response_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryPlanResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &prefixed(
            CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_SIGNATURE_DOMAIN,
            &encode_response_content_unchecked(&response.content)?,
        ),
        &response.signature,
    )
}

pub fn verify_runtime_ipc_recovery_plan_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryPlanResponseV1,
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_request(request)?;
    if response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest != runtime_ipc_recovery_plan_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)? {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_signed_runtime_ipc_recovery_plan_response_v1(response)?;
    verify_result_for_request(&response.content.result, request, descriptor)
}

pub fn runtime_ipc_recovery_plan_response_id_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryPlanResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_recovery_plan_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
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
        CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
            signed_link,
            confirmed_sas,
            execution_policy,
            valid_for_seconds,
        } => {
            verify_signed_link_for_request(signed_link, request.issued_at_unix_seconds)?;
            let expected_sas = history_recovery_sas_v1(
                &signed_link.content.account_device_list,
                signed_link.content.source_certificate.content.device_id,
                signed_link.content.recipient_device_id,
            )
            .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
            if *confirmed_sas != expected_sas
                || !(MIN_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_VALIDITY_SECONDS
                    ..=MAX_CANONICAL_RECOVERY_PLAN_VALIDITY_SECONDS)
                    .contains(valid_for_seconds)
                || request
                    .issued_at_unix_seconds
                    .checked_add(*valid_for_seconds)
                    .is_none()
                || encode_recovery_execution_policy_v1(execution_policy).is_err()
            {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
        CanonicalRuntimeIpcRecoveryPlanCommandV1::HistoryRecoveryPlanStatus { plan_id } => {
            if *plan_id == [0; 32] {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
        CanonicalRuntimeIpcRecoveryPlanCommandV1::CancelHistoryRecoveryPlan {
            plan_id,
            expected_record_id,
        } => {
            if *plan_id == [0; 32] || *expected_record_id == [0; 32] {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
        }
    }
    Ok(())
}

fn verify_signed_link_for_request(
    link: &CanonicalSignedHistoryRecoveryLinkV1,
    approved_at_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_history_recovery_link_v1(link)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?;
    if approved_at_unix_seconds < link.content.issued_at_unix_seconds
        || approved_at_unix_seconds > link.content.expires_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn validate_cancellation_content(
    content: &CanonicalRuntimeIpcRecoveryPlanCancellationContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.plan_id == [0; 32]
        || content.account_id == [0; 32]
        || content.recipient_device_id == [0; 32]
        || content.cancelled_at_unix_seconds == 0
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_cancellation_for_plan(
    cancellation: &CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1,
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_plan_cancellation_v1(cancellation)?;
    verify_signed_history_recovery_plan_v1(plan)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    if cancellation.content.plan_id
        != history_recovery_plan_id_v1(plan)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?
        || cancellation.content.account_id != plan_account_id(plan)
        || cancellation.content.recipient_device_id
            != plan.content.signed_link.content.recipient_device_id
        || cancellation.content.cancelled_at_unix_seconds < plan.content.approved_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn validate_state(
    state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_history_recovery_plan_v1(&state.signed_plan)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    let lifecycle = if let Some(cancellation) = &state.cancellation {
        verify_cancellation_for_plan(cancellation, &state.signed_plan)?;
        CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Cancelled
    } else {
        CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved
    };
    if state.lifecycle != lifecycle
        || state.record_id == [0; 32]
        || state.record_id != runtime_ipc_recovery_plan_record_id_v1(state)?
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_custody(
    custody: &CanonicalRuntimeIpcRecoveryPlanCustodyV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_state(&custody.state)?;
    if custody.restart_state != CanonicalRuntimeIpcRecoveryPlanRestartStateV1::NotRequired
        || custody.execution_scope
            != CanonicalRuntimeIpcRecoveryPlanExecutionScopeV1::ForegroundRuntimeOnly
        || custody.os_background_service_enabled
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcRecoveryPlanResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanApproved(transition)
        | CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanCancelled(transition) => {
            validate_custody(&transition.custody)
        }
        CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanStatus(custody) => {
            validate_custody(custody)
        }
        CanonicalRuntimeIpcRecoveryPlanResultV1::Error { code, message_utf8 } => {
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

fn verify_result_for_request(
    result: &CanonicalRuntimeIpcRecoveryPlanResultV1,
    request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match (result, &request.command) {
        (CanonicalRuntimeIpcRecoveryPlanResultV1::Error { .. }, _) => Ok(()),
        (
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanApproved(transition),
            CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
                signed_link,
                confirmed_sas,
                execution_policy,
                valid_for_seconds,
            },
        ) => {
            validate_custody(&transition.custody)?;
            let state = &transition.custody.state;
            let plan = &state.signed_plan;
            if state.lifecycle != CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved
                || state.cancellation.is_some()
                || plan.content.signed_link != **signed_link
                || plan.content.sas != *confirmed_sas
                || plan.content.execution_policy != *execution_policy
                || plan.content.approved_at_unix_seconds != request.issued_at_unix_seconds
                || plan.content.expires_at_unix_seconds
                    != request
                        .issued_at_unix_seconds
                        .checked_add(*valid_for_seconds)
                        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?
                || plan.content.signed_link.content.recipient_device_id
                    != descriptor.content.device_id
                || plan_account_id(plan) != descriptor.content.account_id
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanStatus(custody),
            CanonicalRuntimeIpcRecoveryPlanCommandV1::HistoryRecoveryPlanStatus { plan_id },
        ) => verify_custody_for_plan_id(custody, *plan_id, descriptor),
        (
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanCancelled(transition),
            CanonicalRuntimeIpcRecoveryPlanCommandV1::CancelHistoryRecoveryPlan {
                plan_id,
                expected_record_id,
            },
        ) => {
            verify_custody_for_plan_id(&transition.custody, *plan_id, descriptor)?;
            let state = &transition.custody.state;
            let cancellation = state
                .cancellation
                .as_ref()
                .ok_or(CanonicalRuntimeIpcError::ContextMismatch)?;
            let approved = CanonicalRuntimeIpcRecoveryPlanStateV1 {
                record_id: *expected_record_id,
                lifecycle: CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved,
                signed_plan: state.signed_plan.clone(),
                cancellation: None,
            };
            if state.lifecycle != CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Cancelled
                || runtime_ipc_recovery_plan_record_id_v1(&approved)? != *expected_record_id
                || cancellation.content.cancelled_at_unix_seconds != request.issued_at_unix_seconds
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

fn verify_custody_for_plan_id(
    custody: &CanonicalRuntimeIpcRecoveryPlanCustodyV1,
    plan_id: [u8; 32],
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_custody(custody)?;
    let plan = &custody.state.signed_plan;
    if history_recovery_plan_id_v1(plan).map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?
        != plan_id
        || plan_account_id(plan) != descriptor.content.account_id
        || plan.content.signed_link.content.recipient_device_id != descriptor.content.device_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn plan_account_id(link_plan: &CanonicalSignedHistoryRecoveryPlanV1) -> [u8; 32] {
    plan_account_id_from_link(&link_plan.content.signed_link)
}

fn plan_account_id_from_link(link: &CanonicalSignedHistoryRecoveryLinkV1) -> [u8; 32] {
    link.content
        .account_device_list
        .content
        .authority_snapshot
        .content
        .account_id
}

fn encode_command(
    output: &mut Vec<u8>,
    command: &CanonicalRuntimeIpcRecoveryPlanCommandV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match command {
        CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
            signed_link,
            confirmed_sas,
            execution_policy,
            valid_for_seconds,
        } => {
            push_array_header(output, 5);
            push_unsigned(output, COMMAND_APPROVE);
            push_bytes(
                output,
                &encode_signed_history_recovery_link_v1(signed_link)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            );
            push_bytes(output, confirmed_sas);
            output.extend_from_slice(
                &encode_recovery_execution_policy_v1(execution_policy)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
            );
            push_unsigned(output, *valid_for_seconds);
        }
        CanonicalRuntimeIpcRecoveryPlanCommandV1::HistoryRecoveryPlanStatus { plan_id } => {
            push_array_header(output, 2);
            push_unsigned(output, COMMAND_STATUS);
            push_bytes(output, plan_id);
        }
        CanonicalRuntimeIpcRecoveryPlanCommandV1::CancelHistoryRecoveryPlan {
            plan_id,
            expected_record_id,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, COMMAND_CANCEL);
            push_bytes(output, plan_id);
            push_bytes(output, expected_record_id);
        }
    }
    Ok(())
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryPlanCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-recovery-plan-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-recovery-plan-command-v1.variant")?,
        length,
    ) {
        (COMMAND_APPROVE, 5) => Ok(
            CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
                signed_link: Box::new(
                    decode_and_verify_signed_history_recovery_link_v1(
                        decoder.bytes("runtime-ipc-recovery-plan-command-v1.signed-link")?,
                    )
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidRequest)?,
                ),
                confirmed_sas: decoder
                    .fixed_bytes("runtime-ipc-recovery-plan-command-v1.confirmed-sas")?,
                execution_policy: decode_execution_policy(decoder, true)?,
                valid_for_seconds: decoder
                    .unsigned("runtime-ipc-recovery-plan-command-v1.valid-for-seconds")?,
            },
        ),
        (COMMAND_STATUS, 2) => Ok(
            CanonicalRuntimeIpcRecoveryPlanCommandV1::HistoryRecoveryPlanStatus {
                plan_id: decoder.fixed_bytes("runtime-ipc-recovery-plan-command-v1.plan-id")?,
            },
        ),
        (COMMAND_CANCEL, 3) => Ok(
            CanonicalRuntimeIpcRecoveryPlanCommandV1::CancelHistoryRecoveryPlan {
                plan_id: decoder.fixed_bytes("runtime-ipc-recovery-plan-command-v1.plan-id")?,
                expected_record_id: decoder
                    .fixed_bytes("runtime-ipc-recovery-plan-command-v1.expected-record-id")?,
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn decode_execution_policy(
    decoder: &mut AccountDecoder<'_>,
    request: bool,
) -> Result<CanonicalRecoveryExecutionPolicyV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-execution-policy-v1", 6)?;
    decoder.expect_version("runtime-ipc-recovery-execution-policy-v1.schema-version")?;
    let policy = CanonicalRecoveryExecutionPolicyV1 {
        allow_ethernet: decode_bool(
            decoder,
            "runtime-ipc-recovery-execution-policy-v1.allow-ethernet",
            request,
        )?,
        allow_wifi: decode_bool(
            decoder,
            "runtime-ipc-recovery-execution-policy-v1.allow-wifi",
            request,
        )?,
        allow_mobile: decode_bool(
            decoder,
            "runtime-ipc-recovery-execution-policy-v1.allow-mobile",
            request,
        )?,
        allow_unknown_network: decode_bool(
            decoder,
            "runtime-ipc-recovery-execution-policy-v1.allow-unknown-network",
            request,
        )?,
        require_external_power: decode_bool(
            decoder,
            "runtime-ipc-recovery-execution-policy-v1.require-external-power",
            request,
        )?,
    };
    encode_recovery_execution_policy_v1(&policy).map_err(|_| {
        if request {
            CanonicalRuntimeIpcError::InvalidRequest
        } else {
            CanonicalRuntimeIpcError::InvalidResponse
        }
    })?;
    Ok(policy)
}

fn encode_cancellation_content(
    content: &CanonicalRuntimeIpcRecoveryPlanCancellationContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_cancellation_content(content)?;
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.plan_id);
    push_bytes(&mut output, &content.account_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_unsigned(&mut output, content.cancelled_at_unix_seconds);
    push_unsigned(&mut output, encode_cancellation_reason(content.reason));
    Ok(output)
}

fn decode_cancellation_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryPlanCancellationContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-plan-cancellation-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-recovery-plan-cancellation-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcRecoveryPlanCancellationContentV1 {
        plan_id: decoder
            .fixed_bytes("runtime-ipc-recovery-plan-cancellation-content-v1.plan-id")?,
        account_id: decoder
            .fixed_bytes("runtime-ipc-recovery-plan-cancellation-content-v1.account-id")?,
        recipient_device_id: decoder
            .fixed_bytes("runtime-ipc-recovery-plan-cancellation-content-v1.recipient-device-id")?,
        cancelled_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-recovery-plan-cancellation-content-v1.cancelled-at-unix-seconds",
        )?,
        reason: decode_cancellation_reason(
            decoder.unsigned("runtime-ipc-recovery-plan-cancellation-content-v1.reason")?,
        )?,
    };
    validate_cancellation_content(&content)?;
    Ok(content)
}

fn encode_cancellation_option(
    output: &mut Vec<u8>,
    cancellation: Option<&CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1>,
) -> Result<(), CanonicalRuntimeIpcError> {
    match cancellation {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(cancellation) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            let cancellation_content = encode_cancellation_content(&cancellation.content)?;
            push_bytes(
                output,
                &signed_envelope(&cancellation_content, &cancellation.signature),
            );
        }
    }
    Ok(())
}

fn decode_cancellation_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<CanonicalSignedRuntimeIpcRecoveryPlanCancellationV1>, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-recovery-plan-cancellation-option-v1")?;
    match (
        decoder.unsigned("runtime-ipc-recovery-plan-cancellation-option-v1.variant")?,
        length,
    ) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(
            decode_and_verify_signed_runtime_ipc_recovery_plan_cancellation_v1(
                decoder.bytes("runtime-ipc-recovery-plan-cancellation-option-v1.value")?,
            )?,
        )),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_state(
    output: &mut Vec<u8>,
    state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 5);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(output, &state.record_id);
    push_unsigned(output, encode_lifecycle(state.lifecycle));
    push_bytes(
        output,
        &encode_signed_history_recovery_plan_v1(&state.signed_plan)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    encode_cancellation_option(output, state.cancellation.as_ref())?;
    Ok(())
}

fn decode_state(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryPlanStateV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-plan-state-v1", 5)?;
    decoder.expect_version("runtime-ipc-recovery-plan-state-v1.schema-version")?;
    let state = CanonicalRuntimeIpcRecoveryPlanStateV1 {
        record_id: decoder.fixed_bytes("runtime-ipc-recovery-plan-state-v1.record-id")?,
        lifecycle: decode_lifecycle(
            decoder.unsigned("runtime-ipc-recovery-plan-state-v1.lifecycle")?,
        )?,
        signed_plan: decode_and_verify_signed_history_recovery_plan_v1(
            decoder.bytes("runtime-ipc-recovery-plan-state-v1.signed-plan")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        cancellation: decode_cancellation_option(decoder)?,
    };
    Ok(state)
}

fn encode_custody(
    output: &mut Vec<u8>,
    custody: &CanonicalRuntimeIpcRecoveryPlanCustodyV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 5);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    encode_state(output, &custody.state)?;
    push_unsigned(output, RESTART_NOT_REQUIRED);
    push_unsigned(output, EXECUTION_FOREGROUND_RUNTIME);
    push_unsigned(output, u64::from(custody.os_background_service_enabled));
    Ok(())
}

fn decode_custody(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryPlanCustodyV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-plan-custody-v1", 5)?;
    decoder.expect_version("runtime-ipc-recovery-plan-custody-v1.schema-version")?;
    let custody = CanonicalRuntimeIpcRecoveryPlanCustodyV1 {
        state: decode_state(decoder)?,
        restart_state: decode_restart(
            decoder.unsigned("runtime-ipc-recovery-plan-custody-v1.restart-state")?,
        )?,
        execution_scope: decode_execution_scope(
            decoder.unsigned("runtime-ipc-recovery-plan-custody-v1.execution-scope")?,
        )?,
        os_background_service_enabled: decode_bool(
            decoder,
            "runtime-ipc-recovery-plan-custody-v1.os-background-service-enabled",
            false,
        )?,
    };
    Ok(custody)
}

fn encode_response_content_unchecked(
    content: &CanonicalRuntimeIpcRecoveryPlanResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
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
) -> Result<CanonicalRuntimeIpcRecoveryPlanResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-plan-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-recovery-plan-response-content-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcRecoveryPlanResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-recovery-plan-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-recovery-plan-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-recovery-plan-response-content-v1.request-digest")?,
        responder_device_id: decoder
            .fixed_bytes("runtime-ipc-recovery-plan-response-content-v1.responder-device-id")?,
        result: decode_result(decoder)?,
    })
}

fn encode_result(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcRecoveryPlanResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanApproved(transition) => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_APPROVED);
            push_unsigned(output, encode_outcome(transition.outcome));
            encode_custody(output, &transition.custody)?;
        }
        CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanStatus(custody) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_STATUS);
            encode_custody(output, custody)?;
        }
        CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanCancelled(transition) => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_CANCELLED);
            push_unsigned(output, encode_outcome(transition.outcome));
            encode_custody(output, &transition.custody)?;
        }
        CanonicalRuntimeIpcRecoveryPlanResultV1::Error { code, message_utf8 } => {
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
) -> Result<CanonicalRuntimeIpcRecoveryPlanResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-recovery-plan-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-recovery-plan-result-v1.variant")?,
        length,
    ) {
        (RESULT_APPROVED, 3) => Ok(
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanApproved(Box::new(
                CanonicalRuntimeIpcRecoveryPlanTransitionV1 {
                    outcome: decode_outcome(
                        decoder.unsigned("runtime-ipc-recovery-plan-result-v1.outcome")?,
                    )?,
                    custody: decode_custody(decoder)?,
                },
            )),
        ),
        (RESULT_STATUS, 2) => Ok(
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanStatus(Box::new(
                decode_custody(decoder)?,
            )),
        ),
        (RESULT_CANCELLED, 3) => Ok(
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanCancelled(Box::new(
                CanonicalRuntimeIpcRecoveryPlanTransitionV1 {
                    outcome: decode_outcome(
                        decoder.unsigned("runtime-ipc-recovery-plan-result-v1.outcome")?,
                    )?,
                    custody: decode_custody(decoder)?,
                },
            )),
        ),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcRecoveryPlanResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-recovery-plan-result-v1.code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-recovery-plan-result-v1.message")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_outcome(outcome: CanonicalRuntimeIpcRecoveryPlanOutcomeV1) -> u64 {
    match outcome {
        CanonicalRuntimeIpcRecoveryPlanOutcomeV1::Committed => OUTCOME_COMMITTED,
        CanonicalRuntimeIpcRecoveryPlanOutcomeV1::AlreadyCommitted => OUTCOME_ALREADY_COMMITTED,
    }
}

fn decode_outcome(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPlanOutcomeV1, CanonicalRuntimeIpcError> {
    match value {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcRecoveryPlanOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => Ok(CanonicalRuntimeIpcRecoveryPlanOutcomeV1::AlreadyCommitted),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_lifecycle(lifecycle: CanonicalRuntimeIpcRecoveryPlanLifecycleV1) -> u64 {
    match lifecycle {
        CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved => LIFECYCLE_APPROVED,
        CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Cancelled => LIFECYCLE_CANCELLED,
    }
}

fn decode_lifecycle(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPlanLifecycleV1, CanonicalRuntimeIpcError> {
    match value {
        LIFECYCLE_APPROVED => Ok(CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved),
        LIFECYCLE_CANCELLED => Ok(CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Cancelled),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_cancellation_reason(reason: CanonicalRuntimeIpcRecoveryPlanCancellationReasonV1) -> u64 {
    match reason {
        CanonicalRuntimeIpcRecoveryPlanCancellationReasonV1::UserRequested => {
            CANCELLATION_USER_REQUESTED
        }
    }
}

fn decode_cancellation_reason(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPlanCancellationReasonV1, CanonicalRuntimeIpcError> {
    match value {
        CANCELLATION_USER_REQUESTED => {
            Ok(CanonicalRuntimeIpcRecoveryPlanCancellationReasonV1::UserRequested)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_restart(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPlanRestartStateV1, CanonicalRuntimeIpcError> {
    match value {
        RESTART_NOT_REQUIRED => Ok(CanonicalRuntimeIpcRecoveryPlanRestartStateV1::NotRequired),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_execution_scope(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPlanExecutionScopeV1, CanonicalRuntimeIpcError> {
    match value {
        EXECUTION_FOREGROUND_RUNTIME => {
            Ok(CanonicalRuntimeIpcRecoveryPlanExecutionScopeV1::ForegroundRuntimeOnly)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
    request: bool,
) -> Result<bool, CanonicalRuntimeIpcError> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
        _ if request => Err(CanonicalRuntimeIpcError::InvalidRequest),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

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
    use crate::canonical_runtime_ipc_v1::{
        CanonicalRuntimeIpcDescriptorContentV1, CanonicalRuntimeIpcLoopbackAddressV1,
        sign_runtime_ipc_descriptor_v1,
    };

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        device_key: SigningKey,
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        approve_request: CanonicalRuntimeIpcRecoveryPlanRequestV1,
        status_request: CanonicalRuntimeIpcRecoveryPlanRequestV1,
        cancel_request: CanonicalRuntimeIpcRecoveryPlanRequestV1,
        approved_state: CanonicalRuntimeIpcRecoveryPlanStateV1,
        cancelled_state: CanonicalRuntimeIpcRecoveryPlanStateV1,
    }

    fn recovery_vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RECOVERY-EXCHANGE-VECTORS.json"
        )))?)
    }

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json"
        )))?)
    }

    fn text<'a>(value: &'a Value, pointer: &str) -> TestResult<&'a str> {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing text vector {pointer}").into())
    }

    fn unsigned(value: &Value, pointer: &str) -> TestResult<u64> {
        value
            .pointer(pointer)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("missing unsigned vector {pointer}").into())
    }

    fn bytes(value: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        decode_hex(text(value, pointer)?)
    }

    fn fixed<const N: usize>(value: &Value, pointer: &str) -> TestResult<[u8; N]> {
        let bytes = bytes(value, pointer)?;
        bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| format!("{pointer} has {} bytes", bytes.len()).into())
    }

    fn decode_hex(value: &str) -> TestResult<Vec<u8>> {
        if !value.len().is_multiple_of(2)
            || value
                .bytes()
                .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
        {
            return Err("invalid lowercase hexadecimal".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|offset| Ok(u8::from_str_radix(&value[offset..offset + 2], 16)?))
            .collect()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn domain(value: &[u8]) -> TestResult<String> {
        Ok(String::from_utf8(value[..value.len() - 1].to_vec())?)
    }

    fn custody(
        state: CanonicalRuntimeIpcRecoveryPlanStateV1,
    ) -> CanonicalRuntimeIpcRecoveryPlanCustodyV1 {
        CanonicalRuntimeIpcRecoveryPlanCustodyV1 {
            state,
            restart_state: CanonicalRuntimeIpcRecoveryPlanRestartStateV1::NotRequired,
            execution_scope: CanonicalRuntimeIpcRecoveryPlanExecutionScopeV1::ForegroundRuntimeOnly,
            os_background_service_enabled: false,
        }
    }

    fn fixture() -> TestResult<Fixture> {
        let recovery = recovery_vectors()?;
        let device_key =
            SigningKey::from_bytes(&fixed(&recovery, "/input/recipient_device_secret_key_hex")?);
        let account_id = fixed(&recovery, "/expected/account_id_hex")?;
        let device_id = fixed(&recovery, "/expected/recipient_device_id_hex")?;
        let descriptor = sign_runtime_ipc_descriptor_v1(
            &device_key,
            CanonicalRuntimeIpcDescriptorContentV1 {
                address: CanonicalRuntimeIpcLoopbackAddressV1::Ipv4([127, 0, 0, 1]),
                port: 43_125,
                bearer_token: [0xa0; 32],
                account_id,
                device_id,
                server_instance_id: [0xc0; 32],
                issued_at_unix_seconds: 1_800_000_000,
                expires_at_unix_seconds: 1_800_086_400,
            },
        )?;
        let descriptor_id = runtime_ipc_descriptor_id_v1(&descriptor)?;
        let link = decode_and_verify_signed_history_recovery_link_v1(&bytes(
            &recovery,
            "/expected/link/signed_cbor_hex",
        )?)?;
        let sas = fixed(&recovery, "/expected/sas_hex")?;
        let approved_at = unsigned(&recovery, "/input/plan_approved_at")?;
        let approve_request = CanonicalRuntimeIpcRecoveryPlanRequestV1 {
            descriptor_id,
            request_id: [0xa1; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: approved_at,
            expires_at_unix_seconds: approved_at + 30,
            command: CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
                signed_link: Box::new(link),
                confirmed_sas: sas,
                execution_policy: CanonicalRecoveryExecutionPolicyV1 {
                    allow_ethernet: true,
                    allow_wifi: true,
                    allow_mobile: false,
                    allow_unknown_network: false,
                    require_external_power: false,
                },
                valid_for_seconds: 86_400,
            },
        };
        let approved_state = create_runtime_ipc_recovery_plan_state_v1(
            &device_key,
            &approve_request,
            &descriptor,
            approve_request.issued_at_unix_seconds,
        )?;
        let plan_id = history_recovery_plan_id_v1(&approved_state.signed_plan)?;
        let status_request = CanonicalRuntimeIpcRecoveryPlanRequestV1 {
            descriptor_id,
            request_id: [0xa2; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: approved_at + 1,
            expires_at_unix_seconds: approved_at + 31,
            command: CanonicalRuntimeIpcRecoveryPlanCommandV1::HistoryRecoveryPlanStatus {
                plan_id,
            },
        };
        let cancel_request = CanonicalRuntimeIpcRecoveryPlanRequestV1 {
            descriptor_id,
            request_id: [0xa3; 32],
            bearer_token: descriptor.content.bearer_token,
            issued_at_unix_seconds: approved_at + 2,
            expires_at_unix_seconds: approved_at + 32,
            command: CanonicalRuntimeIpcRecoveryPlanCommandV1::CancelHistoryRecoveryPlan {
                plan_id,
                expected_record_id: approved_state.record_id,
            },
        };
        let cancelled_state = cancel_runtime_ipc_recovery_plan_state_v1(
            &device_key,
            &cancel_request,
            &descriptor,
            &approved_state,
            cancel_request.issued_at_unix_seconds,
        )?;
        Ok(Fixture {
            device_key,
            descriptor,
            approve_request,
            status_request,
            cancel_request,
            approved_state,
            cancelled_state,
        })
    }

    fn approve_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcRecoveryPlanResponseV1, CanonicalRuntimeIpcError> {
        sign_runtime_ipc_recovery_plan_response_v1(
            &fixture.device_key,
            &fixture.approve_request,
            &fixture.descriptor,
            fixture.approve_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanApproved(Box::new(
                CanonicalRuntimeIpcRecoveryPlanTransitionV1 {
                    outcome: CanonicalRuntimeIpcRecoveryPlanOutcomeV1::Committed,
                    custody: custody(fixture.approved_state.clone()),
                },
            )),
        )
    }

    fn status_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcRecoveryPlanResponseV1, CanonicalRuntimeIpcError> {
        sign_runtime_ipc_recovery_plan_response_v1(
            &fixture.device_key,
            &fixture.status_request,
            &fixture.descriptor,
            fixture.status_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanStatus(Box::new(custody(
                fixture.approved_state.clone(),
            ))),
        )
    }

    fn cancel_response(
        fixture: &Fixture,
    ) -> Result<CanonicalSignedRuntimeIpcRecoveryPlanResponseV1, CanonicalRuntimeIpcError> {
        sign_runtime_ipc_recovery_plan_response_v1(
            &fixture.device_key,
            &fixture.cancel_request,
            &fixture.descriptor,
            fixture.cancel_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcRecoveryPlanResultV1::HistoryRecoveryPlanCancelled(Box::new(
                CanonicalRuntimeIpcRecoveryPlanTransitionV1 {
                    outcome: CanonicalRuntimeIpcRecoveryPlanOutcomeV1::Committed,
                    custody: custody(fixture.cancelled_state.clone()),
                },
            )),
        )
    }

    #[test]
    fn recovery_plan_custody_round_trips_and_binds_context() -> TestResult {
        let fixture = fixture()?;
        for request in [
            &fixture.approve_request,
            &fixture.status_request,
            &fixture.cancel_request,
        ] {
            let encoded = encode_runtime_ipc_recovery_plan_request_v1(request)?;
            assert_eq!(
                decode_and_verify_runtime_ipc_recovery_plan_request_v1(&encoded)?,
                *request
            );
            verify_runtime_ipc_recovery_plan_request_for_descriptor_v1(
                request,
                &fixture.descriptor,
                request.issued_at_unix_seconds,
            )?;
        }
        for (request, response) in [
            (&fixture.approve_request, approve_response(&fixture)?),
            (&fixture.status_request, status_response(&fixture)?),
            (&fixture.cancel_request, cancel_response(&fixture)?),
        ] {
            let encoded = encode_signed_runtime_ipc_recovery_plan_response_v1(&response)?;
            assert_eq!(
                decode_and_verify_signed_runtime_ipc_recovery_plan_response_v1(&encoded)?,
                response
            );
            verify_runtime_ipc_recovery_plan_response_for_request_v1(
                &response,
                request,
                &fixture.descriptor,
            )?;
        }
        Ok(())
    }

    #[test]
    fn recovery_plan_custody_substitutions_fail_closed() -> TestResult {
        let fixture = fixture()?;
        let response = approve_response(&fixture)?;
        let mut changed = fixture.approve_request.clone();
        let CanonicalRuntimeIpcRecoveryPlanCommandV1::ApproveHistoryRecoveryPlan {
            confirmed_sas,
            ..
        } = &mut changed.command
        else {
            unreachable!()
        };
        confirmed_sas[0] ^= 1;
        assert!(encode_runtime_ipc_recovery_plan_request_v1(&changed).is_err());
        assert!(
            verify_runtime_ipc_recovery_plan_response_for_request_v1(
                &response,
                &changed,
                &fixture.descriptor,
            )
            .is_err()
        );

        let mut cancel = fixture.cancel_request.clone();
        let CanonicalRuntimeIpcRecoveryPlanCommandV1::CancelHistoryRecoveryPlan {
            expected_record_id,
            ..
        } = &mut cancel.command
        else {
            unreachable!()
        };
        expected_record_id[0] ^= 1;
        assert!(
            cancel_runtime_ipc_recovery_plan_state_v1(
                &fixture.device_key,
                &cancel,
                &fixture.descriptor,
                &fixture.approved_state,
                cancel.issued_at_unix_seconds,
            )
            .is_err()
        );
        assert!(
            create_runtime_ipc_recovery_plan_state_v1(
                &fixture.device_key,
                &fixture.approve_request,
                &fixture.descriptor,
                fixture.approve_request.expires_at_unix_seconds + 1,
            )
            .is_err()
        );
        assert!(
            cancel_runtime_ipc_recovery_plan_state_v1(
                &fixture.device_key,
                &fixture.cancel_request,
                &fixture.descriptor,
                &fixture.approved_state,
                fixture.cancel_request.expires_at_unix_seconds + 1,
            )
            .is_err()
        );

        let mut cancellation = fixture
            .cancelled_state
            .cancellation
            .clone()
            .ok_or("missing fixture cancellation")?;
        cancellation.signature[0] ^= 1;
        assert!(encode_signed_runtime_ipc_recovery_plan_cancellation_v1(&cancellation).is_err());
        Ok(())
    }

    #[test]
    fn canonical_runtime_ipc_recovery_plan_vectors_match() -> TestResult {
        let fixture = fixture()?;
        let vectors = vectors()?;
        let cases = [
            (
                "approve",
                &fixture.approve_request,
                approve_response(&fixture)?,
            ),
            (
                "status",
                &fixture.status_request,
                status_response(&fixture)?,
            ),
            (
                "cancel",
                &fixture.cancel_request,
                cancel_response(&fixture)?,
            ),
        ];
        for (name, request, response) in cases {
            assert_eq!(
                encode_runtime_ipc_recovery_plan_request_v1(request)?,
                bytes(&vectors, &format!("/expected/{name}/request/cbor_hex"))?
            );
            assert_eq!(
                encode_signed_runtime_ipc_recovery_plan_response_v1(&response)?,
                bytes(
                    &vectors,
                    &format!("/expected/{name}/response/signed_cbor_hex")
                )?
            );
        }
        Ok(())
    }

    fn case_json(
        request: &CanonicalRuntimeIpcRecoveryPlanRequestV1,
        response: &CanonicalSignedRuntimeIpcRecoveryPlanResponseV1,
    ) -> TestResult<Value> {
        let response_content = encode_response_content_unchecked(&response.content)?;
        let response_signing_preimage = prefixed(
            CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_SIGNATURE_DOMAIN,
            &response_content,
        );
        let signed_response = signed_envelope(&response_content, &response.signature);
        let response_id = domain_hash(
            CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_ID_DOMAIN,
            &signed_response,
        );
        Ok(json!({
            "request": {
                "cbor_hex": hex(&encode_runtime_ipc_recovery_plan_request_v1(request)?),
                "digest_hex": hex(&runtime_ipc_recovery_plan_request_digest_v1(request)?)
            },
            "response": {
                "content_cbor_hex": hex(&response_content),
                "signing_preimage_hex": hex(&response_signing_preimage),
                "signature_hex": hex(&response.signature),
                "signed_cbor_hex": hex(&signed_response),
                "id_hex": hex(&response_id)
            }
        }))
    }

    #[test]
    #[ignore = "explicit golden-vector regeneration"]
    fn regenerate_canonical_runtime_ipc_recovery_plan_vectors() -> TestResult {
        let fixture = fixture()?;
        let recovery = recovery_vectors()?;
        let approve = approve_response(&fixture)?;
        let status = status_response(&fixture)?;
        let cancel = cancel_response(&fixture)?;
        let cancellation = fixture
            .cancelled_state
            .cancellation
            .as_ref()
            .ok_or("missing fixture cancellation")?;
        let plan_id = history_recovery_plan_id_v1(&fixture.approved_state.signed_plan)?;
        let document = json!({
            "schema_version": 1,
            "milestone": "M1.1.43",
            "status": "candidate-not-active",
            "changes_active_runtime_ipc": false,
            "encoding": "kilogram-deterministic-cbor-v1",
            "domains": {
                "cancellation_signature": domain(CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_CANCELLATION_SIGNATURE_DOMAIN)?,
                "record_id": domain(CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RECORD_ID_DOMAIN)?,
                "request_digest": domain(CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_REQUEST_DIGEST_DOMAIN)?,
                "response_signature": domain(CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_SIGNATURE_DOMAIN)?,
                "response_id": domain(CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_ID_DOMAIN)?
            },
            "input": {
                "device_secret_key_hex": hex(&fixture.device_key.to_bytes()),
                "descriptor_id_hex": hex(&fixture.approve_request.descriptor_id),
                "bearer_token_hex": hex(&fixture.approve_request.bearer_token),
                "account_id_hex": hex(&fixture.descriptor.content.account_id),
                "recipient_device_id_hex": hex(&fixture.descriptor.content.device_id),
                "signed_link_cbor_hex": text(&recovery, "/expected/link/signed_cbor_hex")?,
                "confirmed_sas_hex": text(&recovery, "/expected/sas_hex")?,
                "allow_ethernet": true,
                "allow_wifi": true,
                "allow_mobile": false,
                "allow_unknown_network": false,
                "require_external_power": false,
                "valid_for_seconds": 86400,
                "approve_request_id_hex": hex(&fixture.approve_request.request_id),
                "approve_issued_at_unix_seconds": fixture.approve_request.issued_at_unix_seconds,
                "approve_expires_at_unix_seconds": fixture.approve_request.expires_at_unix_seconds,
                "status_request_id_hex": hex(&fixture.status_request.request_id),
                "status_issued_at_unix_seconds": fixture.status_request.issued_at_unix_seconds,
                "status_expires_at_unix_seconds": fixture.status_request.expires_at_unix_seconds,
                "cancel_request_id_hex": hex(&fixture.cancel_request.request_id),
                "cancel_issued_at_unix_seconds": fixture.cancel_request.issued_at_unix_seconds,
                "cancel_expires_at_unix_seconds": fixture.cancel_request.expires_at_unix_seconds
            },
            "expected": {
                "plan": {
                    "signed_cbor_hex": hex(&encode_signed_history_recovery_plan_v1(&fixture.approved_state.signed_plan)?),
                    "plan_id_hex": hex(&plan_id),
                    "approved_record_id_hex": hex(&fixture.approved_state.record_id)
                },
                "cancellation": {
                    "content_cbor_hex": hex(&encode_cancellation_content(&cancellation.content)?),
                    "signing_preimage_hex": hex(&runtime_ipc_recovery_plan_cancellation_signing_bytes_v1(&cancellation.content)?),
                    "signature_hex": hex(&cancellation.signature),
                    "signed_cbor_hex": hex(&encode_signed_runtime_ipc_recovery_plan_cancellation_v1(cancellation)?),
                    "cancelled_record_id_hex": hex(&fixture.cancelled_state.record_id)
                },
                "approve": case_json(&fixture.approve_request, &approve)?,
                "status": case_json(&fixture.status_request, &status)?,
                "cancel": case_json(&fixture.cancel_request, &cancel)?
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
