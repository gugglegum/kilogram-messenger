//! Inactive deterministic-CBOR history-recovery execution boundary.
//!
//! Routine IPC can start an already approved recovery plan or inspect its
//! current execution snapshot. Attempt results are not caller-controlled:
//! the foreground runtime appends recipient-Device-signed scheduler states
//! whose progress is bound to canonical recovery checkpoints. The active
//! Postcard runtime IPC remains unchanged.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::{
    canonical_account_v1::{AccountDecoder, signed_envelope},
    canonical_recovery_exchange_v1::{
        CanonicalSignedHistoryRecoveryPlanV1, history_recovery_plan_id_v1,
        verify_signed_history_recovery_plan_v1,
    },
    canonical_recovery_v1::{
        CanonicalHistoryRecoveryCheckpointContentV1, CanonicalSignedHistoryRecoveryCheckpointV1,
        decode_and_verify_signed_history_recovery_checkpoint_v1,
        encode_signed_history_recovery_checkpoint_v1, history_recovery_checkpoint_signing_bytes_v1,
        validate_history_recovery_successor_v1, verify_signed_history_recovery_checkpoint_v1,
    },
    canonical_runtime_ipc_recovery_plan_v1::{
        CanonicalRuntimeIpcRecoveryPlanLifecycleV1, CanonicalRuntimeIpcRecoveryPlanStateV1,
        decode_and_verify_runtime_ipc_recovery_plan_state_v1,
        encode_runtime_ipc_recovery_plan_state_v1, verify_runtime_ipc_recovery_plan_state_v1,
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

pub const MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_STATE_BYTES: usize = 512 * 1024;
pub const MIN_CANONICAL_RECOVERY_RETRY_BASE_SECONDS: u64 = 1;
pub const MAX_CANONICAL_RECOVERY_RETRY_BASE_SECONDS: u64 = 300;
pub const MAX_CANONICAL_RECOVERY_RETRY_SECONDS: u64 = 3_600;
pub const MIN_CANONICAL_RECOVERY_ATTEMPT_LEASE_SECONDS: u64 = 30;
pub const MAX_CANONICAL_RECOVERY_ATTEMPT_LEASE_SECONDS: u64 = 2 * 60 * 60;

pub const CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-scheduler-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_STATE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-scheduler-state-id:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_JITTER_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-scheduler-jitter:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-execution-request-digest:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-execution-response-signature:cbor-v1\0";
pub const CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_ID_DOMAIN: &[u8] =
    b"kilogram:runtime-ipc-recovery-execution-response-id:cbor-v1\0";

const COMMAND_START: u64 = 0;
const COMMAND_STATUS: u64 = 1;
const RESULT_STARTED: u64 = 0;
const RESULT_STATUS: u64 = 1;
const RESULT_ERROR: u64 = 2;
const OUTCOME_COMMITTED: u64 = 0;
const OUTCOME_ALREADY_COMMITTED: u64 = 1;
const TRANSITION_INITIALIZED: u64 = 0;
const TRANSITION_ATTEMPT_STARTED: u64 = 1;
const TRANSITION_ATTEMPT_FAILED: u64 = 2;
const TRANSITION_ATTEMPT_PROGRESSED: u64 = 3;
const TRANSITION_CANCELLED: u64 = 4;
const TRANSITION_COMPLETED: u64 = 5;
const LIFECYCLE_ACTIVE: u64 = 0;
const LIFECYCLE_ATTEMPTING: u64 = 1;
const LIFECYCLE_CANCELLED: u64 = 2;
const LIFECYCLE_COMPLETED: u64 = 3;
const NETWORK_ETHERNET: u64 = 0;
const NETWORK_WIFI: u64 = 1;
const NETWORK_MOBILE: u64 = 2;
const NETWORK_UNKNOWN: u64 = 3;
const POWER_EXTERNAL: u64 = 0;
const POWER_BATTERY: u64 = 1;
const POWER_UNKNOWN: u64 = 2;
const READINESS_READY: u64 = 0;
const READINESS_DEFERRED: u64 = 1;
const READINESS_BLOCKED_NETWORK: u64 = 2;
const READINESS_BLOCKED_POWER: u64 = 3;
const READINESS_ATTEMPTING: u64 = 4;
const READINESS_ATTEMPT_LEASE_EXPIRED: u64 = 5;
const READINESS_CLOCK_ROLLBACK: u64 = 6;
const READINESS_PLAN_EXPIRED: u64 = 7;
const READINESS_CANCELLED: u64 = 8;
const READINESS_COMPLETED: u64 = 9;
const OPTION_NONE: u64 = 0;
const OPTION_SOME: u64 = 1;
const RESTART_NOT_REQUIRED: u64 = 0;
const EXECUTION_FOREGROUND_RUNTIME: u64 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoverySchedulerTransitionV1 {
    Initialized,
    AttemptStarted,
    AttemptFailed,
    AttemptProgressed,
    Cancelled,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoverySchedulerLifecycleV1 {
    Active,
    Attempting,
    Cancelled,
    Completed,
}

impl CanonicalRuntimeIpcRecoverySchedulerLifecycleV1 {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoverySchedulerStateContentV1 {
    pub plan_id: [u8; 32],
    pub recipient_device_id: [u8; 32],
    pub generation: u64,
    pub previous_state_id: Option<[u8; 32]>,
    pub transition: CanonicalRuntimeIpcRecoverySchedulerTransitionV1,
    pub lifecycle: CanonicalRuntimeIpcRecoverySchedulerLifecycleV1,
    pub total_attempts: u64,
    pub consecutive_failures: u32,
    pub last_observed_unix_seconds: u64,
    pub next_attempt_at_unix_seconds: u64,
    pub scheduled_delay_seconds: u64,
    pub retry_base_seconds: u64,
    pub retry_max_seconds: u64,
    pub attempt_lease_seconds: u64,
    pub latest_checkpoint: CanonicalSignedHistoryRecoveryCheckpointV1,
    pub last_failure_code: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcRecoverySchedulerStateV1 {
    pub content: CanonicalRuntimeIpcRecoverySchedulerStateContentV1,
    pub signature: [u8; 64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryNetworkV1 {
    Ethernet,
    Wifi,
    Mobile,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryPowerV1 {
    External,
    Battery,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryReadinessV1 {
    Ready,
    Deferred { not_before_unix_seconds: u64 },
    BlockedNetwork,
    BlockedPower,
    Attempting { lease_expires_at_unix_seconds: u64 },
    AttemptLeaseExpired,
    ClockRollback { last_observed_unix_seconds: u64 },
    PlanExpired,
    Cancelled,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryExecutionSnapshotV1 {
    pub plan_state: CanonicalRuntimeIpcRecoveryPlanStateV1,
    pub scheduler_state: CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    pub observed_at_unix_seconds: u64,
    pub observed_network: CanonicalRuntimeIpcRecoveryNetworkV1,
    pub observed_power: CanonicalRuntimeIpcRecoveryPowerV1,
    pub readiness: CanonicalRuntimeIpcRecoveryReadinessV1,
    pub restart_not_required: bool,
    pub foreground_runtime_only: bool,
    pub os_background_service_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryExecutionCommandV1 {
    StartHistoryRecoveryExecution {
        plan_id: [u8; 32],
        expected_plan_record_id: [u8; 32],
        retry_base_seconds: u64,
        retry_max_seconds: u64,
        attempt_lease_seconds: u64,
    },
    HistoryRecoveryExecutionStatus {
        plan_id: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryExecutionRequestV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub bearer_token: [u8; 32],
    pub issued_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub command: CanonicalRuntimeIpcRecoveryExecutionCommandV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryExecutionOutcomeV1 {
    Committed,
    AlreadyCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalRuntimeIpcRecoveryExecutionResultV1 {
    HistoryRecoveryExecutionStarted {
        outcome: CanonicalRuntimeIpcRecoveryExecutionOutcomeV1,
        snapshot: Box<CanonicalRuntimeIpcRecoveryExecutionSnapshotV1>,
    },
    HistoryRecoveryExecutionStatus(Box<CanonicalRuntimeIpcRecoveryExecutionSnapshotV1>),
    Error {
        code: u16,
        message_utf8: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRuntimeIpcRecoveryExecutionResponseContentV1 {
    pub descriptor_id: [u8; 32],
    pub request_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub responder_device_id: [u8; 32],
    pub result: CanonicalRuntimeIpcRecoveryExecutionResultV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1 {
    pub content: CanonicalRuntimeIpcRecoveryExecutionResponseContentV1,
    pub signature: [u8; 64],
}

pub fn initialize_runtime_ipc_recovery_scheduler_state_v1(
    device_signing_key: &SigningKey,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    attempt_lease_seconds: u64,
    now_unix_seconds: u64,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    validate_approved_plan_state(plan_state, now_unix_seconds)?;
    validate_scheduler_config(retry_base_seconds, retry_max_seconds, attempt_lease_seconds)?;
    let plan = &plan_state.signed_plan;
    let recipient_device_id = plan.content.signed_link.content.recipient_device_id;
    ensure_signing_key(device_signing_key, recipient_device_id)?;
    let link = &plan.content.signed_link.content;
    let checkpoint_content = CanonicalHistoryRecoveryCheckpointContentV1 {
        account_id: plan_account_id(plan),
        conversation_id: link.conversation_id,
        source_device_id: link.source_certificate.content.device_id,
        recipient_device_id,
        sas: plan.content.sas,
        approved_range_start: link.approved_range_start,
        approved_range_end: link.approved_range_end,
        page_size: link.page_size,
        inventory_claim: None,
        next_range_start: link.approved_range_start,
        previous_checkpoint_id: None,
    };
    let checkpoint = CanonicalSignedHistoryRecoveryCheckpointV1 {
        signature: device_signing_key
            .sign(
                &history_recovery_checkpoint_signing_bytes_v1(&checkpoint_content)
                    .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            )
            .to_bytes(),
        content: checkpoint_content,
    };
    verify_checkpoint_for_plan(&checkpoint, plan)?;
    sign_scheduler_state(
        device_signing_key,
        CanonicalRuntimeIpcRecoverySchedulerStateContentV1 {
            plan_id: history_recovery_plan_id_v1(plan)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            recipient_device_id,
            generation: 0,
            previous_state_id: None,
            transition: CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized,
            lifecycle: CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active,
            total_attempts: 0,
            consecutive_failures: 0,
            last_observed_unix_seconds: now_unix_seconds,
            next_attempt_at_unix_seconds: now_unix_seconds,
            scheduled_delay_seconds: 0,
            retry_base_seconds,
            retry_max_seconds,
            attempt_lease_seconds,
            latest_checkpoint: checkpoint,
            last_failure_code: None,
        },
    )
}

pub fn start_runtime_ipc_recovery_scheduler_attempt_v1(
    device_signing_key: &SigningKey,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    current: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    now_unix_seconds: u64,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    validate_approved_plan_state(plan_state, now_unix_seconds)?;
    validate_scheduler_for_plan(current, plan_state)?;
    if current.content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
        || now_unix_seconds < current.content.next_attempt_at_unix_seconds
        || now_unix_seconds < current.content.last_observed_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    let next_attempt_at_unix_seconds = now_unix_seconds
        .checked_add(current.content.attempt_lease_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    sign_scheduler_successor(
        device_signing_key,
        current,
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptStarted,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting,
        current
            .content
            .total_attempts
            .checked_add(1)
            .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?,
        current.content.consecutive_failures,
        now_unix_seconds,
        next_attempt_at_unix_seconds,
        current.content.attempt_lease_seconds,
        current.content.latest_checkpoint.clone(),
        None,
    )
}

pub fn fail_runtime_ipc_recovery_scheduler_attempt_v1(
    device_signing_key: &SigningKey,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    current: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    now_unix_seconds: u64,
    failure_code: u16,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    validate_approved_plan_state(plan_state, now_unix_seconds)?;
    validate_scheduler_for_plan(current, plan_state)?;
    if current.content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
        || now_unix_seconds < current.content.last_observed_unix_seconds
        || failure_code == 0
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    let consecutive_failures = current
        .content
        .consecutive_failures
        .checked_add(1)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    let delay = recovery_backoff_delay_seconds(
        current.content.plan_id,
        consecutive_failures,
        current.content.total_attempts,
        current.content.retry_base_seconds,
        current.content.retry_max_seconds,
    );
    let next_attempt_at_unix_seconds = now_unix_seconds
        .checked_add(delay)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    sign_scheduler_successor(
        device_signing_key,
        current,
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptFailed,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active,
        current.content.total_attempts,
        consecutive_failures,
        now_unix_seconds,
        next_attempt_at_unix_seconds,
        delay,
        current.content.latest_checkpoint.clone(),
        Some(failure_code),
    )
}

pub fn progress_runtime_ipc_recovery_scheduler_attempt_v1(
    device_signing_key: &SigningKey,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    current: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    checkpoint: CanonicalSignedHistoryRecoveryCheckpointV1,
    now_unix_seconds: u64,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    validate_approved_plan_state(plan_state, now_unix_seconds)?;
    validate_scheduler_for_plan(current, plan_state)?;
    if current.content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
        || now_unix_seconds < current.content.last_observed_unix_seconds
        || checkpoint.content.next_range_start >= checkpoint.content.approved_range_end
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    validate_checkpoint_successor(&current.content.latest_checkpoint, &checkpoint, plan_state)?;
    let delay = recovery_backoff_delay_seconds(
        current.content.plan_id,
        1,
        current.content.total_attempts,
        current.content.retry_base_seconds,
        current.content.retry_max_seconds,
    );
    let next_attempt_at_unix_seconds = now_unix_seconds
        .checked_add(delay)
        .ok_or(CanonicalRuntimeIpcError::InvalidRequest)?;
    sign_scheduler_successor(
        device_signing_key,
        current,
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptProgressed,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active,
        current.content.total_attempts,
        0,
        now_unix_seconds,
        next_attempt_at_unix_seconds,
        delay,
        checkpoint,
        None,
    )
}

pub fn complete_runtime_ipc_recovery_scheduler_attempt_v1(
    device_signing_key: &SigningKey,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    current: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    checkpoint: CanonicalSignedHistoryRecoveryCheckpointV1,
    now_unix_seconds: u64,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    validate_approved_plan_state(plan_state, now_unix_seconds)?;
    validate_scheduler_for_plan(current, plan_state)?;
    if current.content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
        || now_unix_seconds < current.content.last_observed_unix_seconds
        || checkpoint.content.next_range_start != checkpoint.content.approved_range_end
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    validate_checkpoint_successor(&current.content.latest_checkpoint, &checkpoint, plan_state)?;
    sign_scheduler_successor(
        device_signing_key,
        current,
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Completed,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Completed,
        current.content.total_attempts,
        0,
        now_unix_seconds,
        now_unix_seconds,
        0,
        checkpoint,
        None,
    )
}

pub fn cancel_runtime_ipc_recovery_scheduler_state_v1(
    device_signing_key: &SigningKey,
    cancelled_plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    current: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    now_unix_seconds: u64,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_plan_state_v1(cancelled_plan_state)?;
    validate_scheduler_for_plan(current, cancelled_plan_state)?;
    if cancelled_plan_state.lifecycle != CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Cancelled
        || cancelled_plan_state.cancellation.is_none()
        || current.content.lifecycle.is_terminal()
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    let observed = now_unix_seconds.max(current.content.last_observed_unix_seconds);
    sign_scheduler_successor(
        device_signing_key,
        current,
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Cancelled,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Cancelled,
        current.content.total_attempts,
        current.content.consecutive_failures,
        observed,
        observed,
        0,
        current.content.latest_checkpoint.clone(),
        None,
    )
}

pub fn runtime_ipc_recovery_scheduler_state_signing_bytes_v1(
    content: &CanonicalRuntimeIpcRecoverySchedulerStateContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_scheduler_state_content(content)?;
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_SIGNATURE_DOMAIN,
        &encode_scheduler_state_content_unchecked(content)?,
    ))
}

pub fn encode_signed_runtime_ipc_recovery_scheduler_state_v1(
    state: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(state)?;
    let output = signed_envelope(
        &encode_scheduler_state_content_unchecked(&state.content)?,
        &state.signature,
    );
    ensure_bound(
        &output,
        "recovery scheduler state",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_STATE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_recovery_scheduler_state_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "recovery scheduler state",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_STATE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-recovery-scheduler-state-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-recovery-scheduler-state-v1.schema-version")?;
    let content = decode_scheduler_state_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-recovery-scheduler-state-v1.signature")?;
    decoder.finish()?;
    let state = CanonicalSignedRuntimeIpcRecoverySchedulerStateV1 { content, signature };
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(&state)?;
    Ok(state)
}

pub fn verify_signed_runtime_ipc_recovery_scheduler_state_v1(
    state: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_scheduler_state_content(&state.content)?;
    verify_ed25519(
        &state.content.recipient_device_id,
        &prefixed(
            CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_SIGNATURE_DOMAIN,
            &encode_scheduler_state_content_unchecked(&state.content)?,
        ),
        &state.signature,
    )
}

pub fn runtime_ipc_recovery_scheduler_state_id_v1(
    state: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_STATE_ID_DOMAIN,
        &encode_signed_runtime_ipc_recovery_scheduler_state_v1(state)?,
    ))
}

pub fn verify_runtime_ipc_recovery_scheduler_successor_v1(
    previous: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    successor: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(previous)?;
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(successor)?;
    let p = &previous.content;
    let n = &successor.content;
    if p.plan_id != n.plan_id
        || p.recipient_device_id != n.recipient_device_id
        || n.generation
            != p.generation
                .checked_add(1)
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?
        || n.previous_state_id != Some(runtime_ipc_recovery_scheduler_state_id_v1(previous)?)
        || n.last_observed_unix_seconds < p.last_observed_unix_seconds
        || p.lifecycle.is_terminal()
        || p.retry_base_seconds != n.retry_base_seconds
        || p.retry_max_seconds != n.retry_max_seconds
        || p.attempt_lease_seconds != n.attempt_lease_seconds
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    validate_scheduler_transition(previous, successor)
}

#[allow(clippy::too_many_arguments)]
fn sign_scheduler_successor(
    device_signing_key: &SigningKey,
    previous: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    transition: CanonicalRuntimeIpcRecoverySchedulerTransitionV1,
    lifecycle: CanonicalRuntimeIpcRecoverySchedulerLifecycleV1,
    total_attempts: u64,
    consecutive_failures: u32,
    last_observed_unix_seconds: u64,
    next_attempt_at_unix_seconds: u64,
    scheduled_delay_seconds: u64,
    latest_checkpoint: CanonicalSignedHistoryRecoveryCheckpointV1,
    last_failure_code: Option<u16>,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(previous)?;
    ensure_signing_key(device_signing_key, previous.content.recipient_device_id)?;
    let state = sign_scheduler_state(
        device_signing_key,
        CanonicalRuntimeIpcRecoverySchedulerStateContentV1 {
            plan_id: previous.content.plan_id,
            recipient_device_id: previous.content.recipient_device_id,
            generation: previous
                .content
                .generation
                .checked_add(1)
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?,
            previous_state_id: Some(runtime_ipc_recovery_scheduler_state_id_v1(previous)?),
            transition,
            lifecycle,
            total_attempts,
            consecutive_failures,
            last_observed_unix_seconds,
            next_attempt_at_unix_seconds,
            scheduled_delay_seconds,
            retry_base_seconds: previous.content.retry_base_seconds,
            retry_max_seconds: previous.content.retry_max_seconds,
            attempt_lease_seconds: previous.content.attempt_lease_seconds,
            latest_checkpoint,
            last_failure_code,
        },
    )?;
    verify_runtime_ipc_recovery_scheduler_successor_v1(previous, &state)?;
    Ok(state)
}

fn sign_scheduler_state(
    device_signing_key: &SigningKey,
    content: CanonicalRuntimeIpcRecoverySchedulerStateContentV1,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    validate_scheduler_state_content(&content)?;
    ensure_signing_key(device_signing_key, content.recipient_device_id)?;
    let signature = device_signing_key
        .sign(&prefixed(
            CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_SIGNATURE_DOMAIN,
            &encode_scheduler_state_content_unchecked(&content)?,
        ))
        .to_bytes();
    let state = CanonicalSignedRuntimeIpcRecoverySchedulerStateV1 { content, signature };
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(&state)?;
    Ok(state)
}

fn validate_scheduler_state_content(
    content: &CanonicalRuntimeIpcRecoverySchedulerStateContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_scheduler_config(
        content.retry_base_seconds,
        content.retry_max_seconds,
        content.attempt_lease_seconds,
    )?;
    verify_signed_history_recovery_checkpoint_v1(&content.latest_checkpoint)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    let delay = content
        .next_attempt_at_unix_seconds
        .checked_sub(content.last_observed_unix_seconds)
        .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
    if content.plan_id == [0; 32]
        || content.recipient_device_id == [0; 32]
        || content.latest_checkpoint.content.recipient_device_id != content.recipient_device_id
        || delay != content.scheduled_delay_seconds
        || content.scheduled_delay_seconds > MAX_CANONICAL_RECOVERY_ATTEMPT_LEASE_SECONDS
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    if content.generation == 0 {
        if content.previous_state_id.is_some()
            || content.transition != CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized
            || content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
            || content.total_attempts != 0
            || content.consecutive_failures != 0
            || content.scheduled_delay_seconds != 0
            || content.last_failure_code.is_some()
            || content.latest_checkpoint.content.next_range_start
                != content.latest_checkpoint.content.approved_range_start
            || content
                .latest_checkpoint
                .content
                .previous_checkpoint_id
                .is_some()
        {
            return Err(CanonicalRuntimeIpcError::InvalidResponse);
        }
    } else if content.previous_state_id.is_none()
        || content.transition == CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match content.transition {
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptFailed => {
            if matches!(content.last_failure_code, None | Some(0))
                || content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
                || content.scheduled_delay_seconds > MAX_CANONICAL_RECOVERY_RETRY_SECONDS
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptStarted => {
            if content.last_failure_code.is_some()
                || content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
                || content.scheduled_delay_seconds != content.attempt_lease_seconds
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptProgressed => {
            if content.last_failure_code.is_some()
                || content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
                || content.consecutive_failures != 0
                || content.latest_checkpoint.content.next_range_start
                    >= content.latest_checkpoint.content.approved_range_end
                || content.scheduled_delay_seconds > MAX_CANONICAL_RECOVERY_RETRY_SECONDS
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Completed => {
            if content.last_failure_code.is_some()
                || content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Completed
                || content.consecutive_failures != 0
                || content.scheduled_delay_seconds != 0
                || content.latest_checkpoint.content.next_range_start
                    != content.latest_checkpoint.content.approved_range_end
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Cancelled => {
            if content.last_failure_code.is_some()
                || content.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Cancelled
                || content.scheduled_delay_seconds != 0
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized => {}
    }
    Ok(())
}

fn validate_scheduler_transition(
    previous: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    successor: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let p = &previous.content;
    let n = &successor.content;
    match n.transition {
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized => {
            Err(CanonicalRuntimeIpcError::InvalidResponse)
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptStarted => {
            let total_attempts = p
                .total_attempts
                .checked_add(1)
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
            if p.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
                || n.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
                || n.total_attempts != total_attempts
                || n.consecutive_failures != p.consecutive_failures
                || n.latest_checkpoint != p.latest_checkpoint
                || n.scheduled_delay_seconds != p.attempt_lease_seconds
                || n.last_failure_code.is_some()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptFailed => {
            let failures = p
                .consecutive_failures
                .checked_add(1)
                .ok_or(CanonicalRuntimeIpcError::InvalidResponse)?;
            let expected_delay = recovery_backoff_delay_seconds(
                p.plan_id,
                failures,
                p.total_attempts,
                p.retry_base_seconds,
                p.retry_max_seconds,
            );
            if p.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
                || n.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
                || n.total_attempts != p.total_attempts
                || n.consecutive_failures != failures
                || n.latest_checkpoint != p.latest_checkpoint
                || n.scheduled_delay_seconds != expected_delay
                || n.last_failure_code.is_none()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptProgressed => {
            let expected_delay = recovery_backoff_delay_seconds(
                p.plan_id,
                1,
                p.total_attempts,
                p.retry_base_seconds,
                p.retry_max_seconds,
            );
            if p.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
                || n.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
                || n.total_attempts != p.total_attempts
                || n.consecutive_failures != 0
                || n.scheduled_delay_seconds != expected_delay
                || n.last_failure_code.is_some()
                || validate_history_recovery_successor_v1(
                    &p.latest_checkpoint,
                    &n.latest_checkpoint,
                )
                .is_err()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Completed => {
            if p.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting
                || n.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Completed
                || n.total_attempts != p.total_attempts
                || n.consecutive_failures != 0
                || n.scheduled_delay_seconds != 0
                || n.last_failure_code.is_some()
                || validate_history_recovery_successor_v1(
                    &p.latest_checkpoint,
                    &n.latest_checkpoint,
                )
                .is_err()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Cancelled => {
            if n.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Cancelled
                || n.total_attempts != p.total_attempts
                || n.consecutive_failures != p.consecutive_failures
                || n.latest_checkpoint != p.latest_checkpoint
                || n.scheduled_delay_seconds != 0
                || n.last_failure_code.is_some()
            {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(())
        }
    }
}

fn encode_scheduler_state_content_unchecked(
    content: &CanonicalRuntimeIpcRecoverySchedulerStateContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    let mut output = Vec::new();
    push_array_header(&mut output, 17);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.plan_id);
    push_bytes(&mut output, &content.recipient_device_id);
    push_unsigned(&mut output, content.generation);
    encode_digest_option(&mut output, content.previous_state_id.as_ref());
    push_unsigned(&mut output, encode_scheduler_transition(content.transition));
    push_unsigned(&mut output, encode_scheduler_lifecycle(content.lifecycle));
    push_unsigned(&mut output, content.total_attempts);
    push_unsigned(&mut output, u64::from(content.consecutive_failures));
    push_unsigned(&mut output, content.last_observed_unix_seconds);
    push_unsigned(&mut output, content.next_attempt_at_unix_seconds);
    push_unsigned(&mut output, content.scheduled_delay_seconds);
    push_unsigned(&mut output, content.retry_base_seconds);
    push_unsigned(&mut output, content.retry_max_seconds);
    push_unsigned(&mut output, content.attempt_lease_seconds);
    push_bytes(
        &mut output,
        &encode_signed_history_recovery_checkpoint_v1(&content.latest_checkpoint)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
    );
    encode_failure_option(&mut output, content.last_failure_code);
    Ok(output)
}

fn decode_scheduler_state_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoverySchedulerStateContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-scheduler-state-content-v1", 17)?;
    decoder.expect_version("runtime-ipc-recovery-scheduler-state-content-v1.schema-version")?;
    let content = CanonicalRuntimeIpcRecoverySchedulerStateContentV1 {
        plan_id: decoder.fixed_bytes("runtime-ipc-recovery-scheduler-state-content-v1.plan-id")?,
        recipient_device_id: decoder
            .fixed_bytes("runtime-ipc-recovery-scheduler-state-content-v1.recipient-device-id")?,
        generation: decoder
            .unsigned("runtime-ipc-recovery-scheduler-state-content-v1.generation")?,
        previous_state_id: decode_digest_option(
            decoder,
            "runtime-ipc-recovery-scheduler-state-content-v1.previous-state-id",
        )?,
        transition: decode_scheduler_transition(
            decoder.unsigned("runtime-ipc-recovery-scheduler-state-content-v1.transition")?,
        )?,
        lifecycle: decode_scheduler_lifecycle(
            decoder.unsigned("runtime-ipc-recovery-scheduler-state-content-v1.lifecycle")?,
        )?,
        total_attempts: decoder
            .unsigned("runtime-ipc-recovery-scheduler-state-content-v1.total-attempts")?,
        consecutive_failures: u32::try_from(
            decoder
                .unsigned("runtime-ipc-recovery-scheduler-state-content-v1.consecutive-failures")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        last_observed_unix_seconds: decoder.unsigned(
            "runtime-ipc-recovery-scheduler-state-content-v1.last-observed-unix-seconds",
        )?,
        next_attempt_at_unix_seconds: decoder.unsigned(
            "runtime-ipc-recovery-scheduler-state-content-v1.next-attempt-at-unix-seconds",
        )?,
        scheduled_delay_seconds: decoder
            .unsigned("runtime-ipc-recovery-scheduler-state-content-v1.scheduled-delay-seconds")?,
        retry_base_seconds: decoder
            .unsigned("runtime-ipc-recovery-scheduler-state-content-v1.retry-base-seconds")?,
        retry_max_seconds: decoder
            .unsigned("runtime-ipc-recovery-scheduler-state-content-v1.retry-max-seconds")?,
        attempt_lease_seconds: decoder
            .unsigned("runtime-ipc-recovery-scheduler-state-content-v1.attempt-lease-seconds")?,
        latest_checkpoint: decode_and_verify_signed_history_recovery_checkpoint_v1(
            decoder.bytes("runtime-ipc-recovery-scheduler-state-content-v1.latest-checkpoint")?,
        )
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
        last_failure_code: decode_failure_option(decoder)?,
    };
    validate_scheduler_state_content(&content)?;
    Ok(content)
}

fn validate_scheduler_config(
    retry_base_seconds: u64,
    retry_max_seconds: u64,
    attempt_lease_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    if !(MIN_CANONICAL_RECOVERY_RETRY_BASE_SECONDS..=MAX_CANONICAL_RECOVERY_RETRY_BASE_SECONDS)
        .contains(&retry_base_seconds)
        || !(retry_base_seconds..=MAX_CANONICAL_RECOVERY_RETRY_SECONDS).contains(&retry_max_seconds)
        || !(MIN_CANONICAL_RECOVERY_ATTEMPT_LEASE_SECONDS
            ..=MAX_CANONICAL_RECOVERY_ATTEMPT_LEASE_SECONDS)
            .contains(&attempt_lease_seconds)
    {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    }
    Ok(())
}

fn recovery_backoff_delay_seconds(
    plan_id: [u8; 32],
    failure_exponent: u32,
    attempt_ordinal: u64,
    base_seconds: u64,
    max_seconds: u64,
) -> u64 {
    let shift = failure_exponent.saturating_sub(1).min(63);
    let exponential = base_seconds
        .checked_shl(shift)
        .unwrap_or(u64::MAX)
        .min(max_seconds);
    let floor = exponential.div_ceil(2);
    let span = exponential.saturating_sub(floor).saturating_add(1);
    let mut hasher = blake3::Hasher::new();
    hasher.update(CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_JITTER_DOMAIN);
    hasher.update(&plan_id);
    hasher.update(&failure_exponent.to_le_bytes());
    hasher.update(&attempt_ordinal.to_le_bytes());
    let digest = hasher.finalize();
    let mut sample = [0_u8; 8];
    sample.copy_from_slice(&digest.as_bytes()[..8]);
    floor.saturating_add(u64::from_le_bytes(sample) % span)
}

pub fn derive_runtime_ipc_recovery_readiness_v1(
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    scheduler_state: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    observed_at_unix_seconds: u64,
    observed_network: CanonicalRuntimeIpcRecoveryNetworkV1,
    observed_power: CanonicalRuntimeIpcRecoveryPowerV1,
) -> Result<CanonicalRuntimeIpcRecoveryReadinessV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_plan_state_v1(plan_state)?;
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(scheduler_state)?;
    validate_scheduler_context_unchecked(scheduler_state, plan_state)?;
    derive_runtime_ipc_recovery_readiness_unchecked(
        plan_state,
        scheduler_state,
        observed_at_unix_seconds,
        observed_network,
        observed_power,
    )
}

fn derive_runtime_ipc_recovery_readiness_unchecked(
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    scheduler_state: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    observed_at_unix_seconds: u64,
    observed_network: CanonicalRuntimeIpcRecoveryNetworkV1,
    observed_power: CanonicalRuntimeIpcRecoveryPowerV1,
) -> Result<CanonicalRuntimeIpcRecoveryReadinessV1, CanonicalRuntimeIpcError> {
    let scheduler = &scheduler_state.content;
    if plan_state.lifecycle == CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Cancelled {
        if scheduler.lifecycle != CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Cancelled {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
        return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Cancelled);
    }
    match scheduler.lifecycle {
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Cancelled => {
            return Err(CanonicalRuntimeIpcError::ContextMismatch);
        }
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Completed => {
            return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Completed);
        }
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active
        | CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting => {}
    }
    if observed_at_unix_seconds > plan_state.signed_plan.content.expires_at_unix_seconds {
        return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::PlanExpired);
    }
    if observed_at_unix_seconds < scheduler.last_observed_unix_seconds {
        return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::ClockRollback {
            last_observed_unix_seconds: scheduler.last_observed_unix_seconds,
        });
    }
    if scheduler.lifecycle == CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting {
        if observed_at_unix_seconds >= scheduler.next_attempt_at_unix_seconds {
            return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::AttemptLeaseExpired);
        }
        return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Attempting {
            lease_expires_at_unix_seconds: scheduler.next_attempt_at_unix_seconds,
        });
    }
    let policy = plan_state.signed_plan.content.execution_policy;
    let network_allowed = match observed_network {
        CanonicalRuntimeIpcRecoveryNetworkV1::Ethernet => policy.allow_ethernet,
        CanonicalRuntimeIpcRecoveryNetworkV1::Wifi => policy.allow_wifi,
        CanonicalRuntimeIpcRecoveryNetworkV1::Mobile => policy.allow_mobile,
        CanonicalRuntimeIpcRecoveryNetworkV1::Unknown => policy.allow_unknown_network,
    };
    if !network_allowed {
        return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::BlockedNetwork);
    }
    if policy.require_external_power
        && observed_power != CanonicalRuntimeIpcRecoveryPowerV1::External
    {
        return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::BlockedPower);
    }
    if observed_at_unix_seconds < scheduler.next_attempt_at_unix_seconds {
        return Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Deferred {
            not_before_unix_seconds: scheduler.next_attempt_at_unix_seconds,
        });
    }
    Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Ready)
}

pub fn create_runtime_ipc_recovery_execution_snapshot_v1(
    plan_state: CanonicalRuntimeIpcRecoveryPlanStateV1,
    scheduler_state: CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    observed_at_unix_seconds: u64,
    observed_network: CanonicalRuntimeIpcRecoveryNetworkV1,
    observed_power: CanonicalRuntimeIpcRecoveryPowerV1,
) -> Result<CanonicalRuntimeIpcRecoveryExecutionSnapshotV1, CanonicalRuntimeIpcError> {
    let readiness = derive_runtime_ipc_recovery_readiness_v1(
        &plan_state,
        &scheduler_state,
        observed_at_unix_seconds,
        observed_network,
        observed_power,
    )?;
    Ok(CanonicalRuntimeIpcRecoveryExecutionSnapshotV1 {
        plan_state,
        scheduler_state,
        observed_at_unix_seconds,
        observed_network,
        observed_power,
        readiness,
        restart_not_required: true,
        foreground_runtime_only: true,
        os_background_service_enabled: false,
    })
}

pub fn encode_runtime_ipc_recovery_execution_request_v1(
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
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
        "recovery execution request",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_REQUEST_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_runtime_ipc_recovery_execution_request_v1(
    bytes: &[u8],
) -> Result<CanonicalRuntimeIpcRecoveryExecutionRequestV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "recovery execution request",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_REQUEST_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("runtime-ipc-recovery-execution-request-v1", 7)?;
    decoder.expect_version("runtime-ipc-recovery-execution-request-v1.schema-version")?;
    let request = CanonicalRuntimeIpcRecoveryExecutionRequestV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-recovery-execution-request-v1.descriptor-id")?,
        request_id: decoder.fixed_bytes("runtime-ipc-recovery-execution-request-v1.request-id")?,
        bearer_token: decoder
            .fixed_bytes("runtime-ipc-recovery-execution-request-v1.bearer-token")?,
        issued_at_unix_seconds: decoder
            .unsigned("runtime-ipc-recovery-execution-request-v1.issued-at-unix-seconds")?,
        expires_at_unix_seconds: decoder
            .unsigned("runtime-ipc-recovery-execution-request-v1.expires-at-unix-seconds")?,
        command: decode_command(&mut decoder)?,
    };
    decoder.finish()?;
    validate_request(&request)?;
    Ok(request)
}

pub fn runtime_ipc_recovery_execution_request_digest_v1(
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_REQUEST_DIGEST_DOMAIN,
        &encode_runtime_ipc_recovery_execution_request_v1(request)?,
    ))
}

pub fn verify_runtime_ipc_recovery_execution_request_for_descriptor_v1(
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
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
    Ok(())
}

pub fn create_runtime_ipc_recovery_execution_state_for_request_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    now_unix_seconds: u64,
) -> Result<CanonicalSignedRuntimeIpcRecoverySchedulerStateV1, CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_execution_request_for_descriptor_v1(
        request,
        descriptor,
        now_unix_seconds,
    )?;
    let CanonicalRuntimeIpcRecoveryExecutionCommandV1::StartHistoryRecoveryExecution {
        plan_id,
        expected_plan_record_id,
        retry_base_seconds,
        retry_max_seconds,
        attempt_lease_seconds,
    } = request.command
    else {
        return Err(CanonicalRuntimeIpcError::InvalidRequest);
    };
    validate_approved_plan_state(plan_state, now_unix_seconds)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id
        || plan_account_id(&plan_state.signed_plan) != descriptor.content.account_id
        || plan_recipient_device_id(&plan_state.signed_plan) != descriptor.content.device_id
        || history_recovery_plan_id_v1(&plan_state.signed_plan)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?
            != plan_id
        || plan_state.record_id != expected_plan_record_id
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    initialize_runtime_ipc_recovery_scheduler_state_v1(
        device_signing_key,
        plan_state,
        retry_base_seconds,
        retry_max_seconds,
        attempt_lease_seconds,
        request.issued_at_unix_seconds,
    )
}

pub fn runtime_ipc_recovery_execution_response_signing_bytes_v1(
    content: &CanonicalRuntimeIpcRecoveryExecutionResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    validate_response_content(content)?;
    Ok(prefixed(
        CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_SIGNATURE_DOMAIN,
        &encode_response_content_unchecked(content)?,
    ))
}

pub fn sign_runtime_ipc_recovery_execution_response_v1(
    device_signing_key: &SigningKey,
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
    result: CanonicalRuntimeIpcRecoveryExecutionResultV1,
) -> Result<CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    validate_request(request)?;
    if device_signing_key.verifying_key().to_bytes() != descriptor.content.device_id
        || request.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)?
        || request.bearer_token != descriptor.content.bearer_token
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    let content = CanonicalRuntimeIpcRecoveryExecutionResponseContentV1 {
        descriptor_id: request.descriptor_id,
        request_id: request.request_id,
        request_digest: runtime_ipc_recovery_execution_request_digest_v1(request)?,
        responder_device_id: descriptor.content.device_id,
        result,
    };
    validate_response_content(&content)?;
    verify_result_for_request(&content.result, request, descriptor)?;
    let signature = device_signing_key
        .sign(&prefixed(
            CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_SIGNATURE_DOMAIN,
            &encode_response_content_unchecked(&content)?,
        ))
        .to_bytes();
    Ok(CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1 { content, signature })
}

pub fn encode_signed_runtime_ipc_recovery_execution_response_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_execution_response_v1(response)?;
    let output = signed_envelope(
        &encode_response_content_unchecked(&response.content)?,
        &response.signature,
    );
    ensure_bound(
        &output,
        "recovery execution response",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_BYTES,
    )?;
    Ok(output)
}

pub fn decode_and_verify_signed_runtime_ipc_recovery_execution_response_v1(
    bytes: &[u8],
) -> Result<CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1, CanonicalRuntimeIpcError> {
    ensure_bound(
        bytes,
        "recovery execution response",
        MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_BYTES,
    )?;
    validate_canonical_profile_v1(bytes)?;
    let mut decoder = AccountDecoder::new(bytes);
    decoder.expect_array("signed-runtime-ipc-recovery-execution-response-v1", 3)?;
    decoder.expect_version("signed-runtime-ipc-recovery-execution-response-v1.schema-version")?;
    let content = decode_response_content(&mut decoder)?;
    let signature =
        decoder.fixed_bytes("signed-runtime-ipc-recovery-execution-response-v1.signature")?;
    decoder.finish()?;
    let response = CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1 { content, signature };
    verify_signed_runtime_ipc_recovery_execution_response_v1(&response)?;
    Ok(response)
}

pub fn verify_signed_runtime_ipc_recovery_execution_response_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_response_content(&response.content)?;
    verify_ed25519(
        &response.content.responder_device_id,
        &prefixed(
            CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_SIGNATURE_DOMAIN,
            &encode_response_content_unchecked(&response.content)?,
        ),
        &response.signature,
    )
}

pub fn verify_runtime_ipc_recovery_execution_response_for_request_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1,
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    validate_request(request)?;
    if response.content.descriptor_id != request.descriptor_id
        || response.content.request_id != request.request_id
        || response.content.request_digest
            != runtime_ipc_recovery_execution_request_digest_v1(request)?
        || response.content.responder_device_id != descriptor.content.device_id
        || request.bearer_token != descriptor.content.bearer_token
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_signed_runtime_ipc_descriptor_v1(descriptor)?;
    if response.content.descriptor_id != runtime_ipc_descriptor_id_v1(descriptor)? {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    verify_signed_runtime_ipc_recovery_execution_response_v1(response)?;
    verify_result_for_request(&response.content.result, request, descriptor)
}

pub fn runtime_ipc_recovery_execution_response_id_v1(
    response: &CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1,
) -> Result<[u8; 32], CanonicalRuntimeIpcError> {
    Ok(domain_hash(
        CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_ID_DOMAIN,
        &encode_signed_runtime_ipc_recovery_execution_response_v1(response)?,
    ))
}

fn validate_request(
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
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
    match request.command {
        CanonicalRuntimeIpcRecoveryExecutionCommandV1::StartHistoryRecoveryExecution {
            plan_id,
            expected_plan_record_id,
            retry_base_seconds,
            retry_max_seconds,
            attempt_lease_seconds,
        } => {
            if plan_id == [0; 32] || expected_plan_record_id == [0; 32] {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
            validate_scheduler_config(retry_base_seconds, retry_max_seconds, attempt_lease_seconds)
        }
        CanonicalRuntimeIpcRecoveryExecutionCommandV1::HistoryRecoveryExecutionStatus {
            plan_id,
        } => {
            if plan_id == [0; 32] {
                return Err(CanonicalRuntimeIpcError::InvalidRequest);
            }
            Ok(())
        }
    }
}

fn validate_response_content(
    content: &CanonicalRuntimeIpcRecoveryExecutionResponseContentV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    if content.descriptor_id == [0; 32]
        || content.request_id == [0; 32]
        || content.request_digest == [0; 32]
        || content.responder_device_id == [0; 32]
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    match &content.result {
        CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStarted {
            snapshot,
            ..
        }
        | CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStatus(snapshot) => {
            validate_snapshot(snapshot)
        }
        CanonicalRuntimeIpcRecoveryExecutionResultV1::Error { code, message_utf8 } => {
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

fn validate_snapshot(
    snapshot: &CanonicalRuntimeIpcRecoveryExecutionSnapshotV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_plan_state_v1(&snapshot.plan_state)?;
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(&snapshot.scheduler_state)?;
    validate_scheduler_context_unchecked(&snapshot.scheduler_state, &snapshot.plan_state)?;
    let expected = derive_runtime_ipc_recovery_readiness_unchecked(
        &snapshot.plan_state,
        &snapshot.scheduler_state,
        snapshot.observed_at_unix_seconds,
        snapshot.observed_network,
        snapshot.observed_power,
    )?;
    if snapshot.readiness != expected
        || !snapshot.restart_not_required
        || !snapshot.foreground_runtime_only
        || snapshot.os_background_service_enabled
    {
        return Err(CanonicalRuntimeIpcError::InvalidResponse);
    }
    Ok(())
}

fn verify_result_for_request(
    result: &CanonicalRuntimeIpcRecoveryExecutionResultV1,
    request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
    descriptor: &CanonicalSignedRuntimeIpcDescriptorV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match (result, &request.command) {
        (CanonicalRuntimeIpcRecoveryExecutionResultV1::Error { .. }, _) => Ok(()),
        (
            CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStarted {
                snapshot,
                ..
            },
            CanonicalRuntimeIpcRecoveryExecutionCommandV1::StartHistoryRecoveryExecution {
                plan_id,
                expected_plan_record_id,
                retry_base_seconds,
                retry_max_seconds,
                attempt_lease_seconds,
            },
        ) => {
            let state = &snapshot.scheduler_state.content;
            if snapshot.plan_state.record_id != *expected_plan_record_id
                || state.plan_id != *plan_id
                || state.generation != 0
                || state.transition != CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized
                || state.retry_base_seconds != *retry_base_seconds
                || state.retry_max_seconds != *retry_max_seconds
                || state.attempt_lease_seconds != *attempt_lease_seconds
                || plan_account_id(&snapshot.plan_state.signed_plan)
                    != descriptor.content.account_id
                || state.recipient_device_id != descriptor.content.device_id
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        (
            CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStatus(snapshot),
            CanonicalRuntimeIpcRecoveryExecutionCommandV1::HistoryRecoveryExecutionStatus {
                plan_id,
            },
        ) => {
            if snapshot.scheduler_state.content.plan_id != *plan_id
                || plan_account_id(&snapshot.plan_state.signed_plan)
                    != descriptor.content.account_id
                || snapshot.scheduler_state.content.recipient_device_id
                    != descriptor.content.device_id
            {
                return Err(CanonicalRuntimeIpcError::ContextMismatch);
            }
            Ok(())
        }
        _ => Err(CanonicalRuntimeIpcError::ContextMismatch),
    }
}

fn validate_approved_plan_state(
    state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
    now_unix_seconds: u64,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_runtime_ipc_recovery_plan_state_v1(state)?;
    if state.lifecycle != CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved
        || state.cancellation.is_some()
        || now_unix_seconds < state.signed_plan.content.approved_at_unix_seconds
        || now_unix_seconds > state.signed_plan.content.expires_at_unix_seconds
    {
        return Err(CanonicalRuntimeIpcError::RequestNotCurrent);
    }
    Ok(())
}

fn validate_scheduler_for_plan(
    scheduler: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_runtime_ipc_recovery_scheduler_state_v1(scheduler)?;
    verify_runtime_ipc_recovery_plan_state_v1(plan_state)?;
    validate_scheduler_context_unchecked(scheduler, plan_state)
}

fn validate_scheduler_context_unchecked(
    scheduler: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let plan = &plan_state.signed_plan;
    if scheduler.content.plan_id
        != history_recovery_plan_id_v1(plan)
            .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?
        || scheduler.content.recipient_device_id != plan_recipient_device_id(plan)
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    validate_checkpoint_context_unchecked(&scheduler.content.latest_checkpoint, plan)
}

fn verify_checkpoint_for_plan(
    checkpoint: &CanonicalSignedHistoryRecoveryCheckpointV1,
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_signed_history_recovery_plan_v1(plan)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    verify_signed_history_recovery_checkpoint_v1(checkpoint)
        .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
    validate_checkpoint_context_unchecked(checkpoint, plan)
}

fn validate_checkpoint_context_unchecked(
    checkpoint: &CanonicalSignedHistoryRecoveryCheckpointV1,
    plan: &CanonicalSignedHistoryRecoveryPlanV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    let link = &plan.content.signed_link.content;
    if checkpoint.content.account_id != plan_account_id(plan)
        || checkpoint.content.conversation_id != link.conversation_id
        || checkpoint.content.source_device_id != link.source_certificate.content.device_id
        || checkpoint.content.recipient_device_id != link.recipient_device_id
        || checkpoint.content.sas != plan.content.sas
        || checkpoint.content.approved_range_start != link.approved_range_start
        || checkpoint.content.approved_range_end != link.approved_range_end
        || checkpoint.content.page_size != link.page_size
    {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn validate_checkpoint_successor(
    previous: &CanonicalSignedHistoryRecoveryCheckpointV1,
    successor: &CanonicalSignedHistoryRecoveryCheckpointV1,
    plan_state: &CanonicalRuntimeIpcRecoveryPlanStateV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    verify_checkpoint_for_plan(previous, &plan_state.signed_plan)?;
    verify_checkpoint_for_plan(successor, &plan_state.signed_plan)?;
    validate_history_recovery_successor_v1(previous, successor)
        .map_err(|_| CanonicalRuntimeIpcError::ContextMismatch)
}

fn plan_account_id(plan: &CanonicalSignedHistoryRecoveryPlanV1) -> [u8; 32] {
    plan.content
        .signed_link
        .content
        .account_device_list
        .content
        .authority_snapshot
        .content
        .account_id
}

fn plan_recipient_device_id(plan: &CanonicalSignedHistoryRecoveryPlanV1) -> [u8; 32] {
    plan.content.signed_link.content.recipient_device_id
}

fn ensure_signing_key(
    key: &SigningKey,
    expected_device_id: [u8; 32],
) -> Result<(), CanonicalRuntimeIpcError> {
    if key.verifying_key().to_bytes() != expected_device_id {
        return Err(CanonicalRuntimeIpcError::ContextMismatch);
    }
    Ok(())
}

fn encode_command(output: &mut Vec<u8>, command: &CanonicalRuntimeIpcRecoveryExecutionCommandV1) {
    match command {
        CanonicalRuntimeIpcRecoveryExecutionCommandV1::StartHistoryRecoveryExecution {
            plan_id,
            expected_plan_record_id,
            retry_base_seconds,
            retry_max_seconds,
            attempt_lease_seconds,
        } => {
            push_array_header(output, 6);
            push_unsigned(output, COMMAND_START);
            push_bytes(output, plan_id);
            push_bytes(output, expected_plan_record_id);
            push_unsigned(output, *retry_base_seconds);
            push_unsigned(output, *retry_max_seconds);
            push_unsigned(output, *attempt_lease_seconds);
        }
        CanonicalRuntimeIpcRecoveryExecutionCommandV1::HistoryRecoveryExecutionStatus {
            plan_id,
        } => {
            push_array_header(output, 2);
            push_unsigned(output, COMMAND_STATUS);
            push_bytes(output, plan_id);
        }
    }
}

fn decode_command(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryExecutionCommandV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-recovery-execution-command-v1")?;
    match (
        decoder.unsigned("runtime-ipc-recovery-execution-command-v1.variant")?,
        length,
    ) {
        (COMMAND_START, 6) => Ok(
            CanonicalRuntimeIpcRecoveryExecutionCommandV1::StartHistoryRecoveryExecution {
                plan_id: decoder
                    .fixed_bytes("runtime-ipc-recovery-execution-command-v1.plan-id")?,
                expected_plan_record_id: decoder.fixed_bytes(
                    "runtime-ipc-recovery-execution-command-v1.expected-plan-record-id",
                )?,
                retry_base_seconds: decoder
                    .unsigned("runtime-ipc-recovery-execution-command-v1.retry-base-seconds")?,
                retry_max_seconds: decoder
                    .unsigned("runtime-ipc-recovery-execution-command-v1.retry-max-seconds")?,
                attempt_lease_seconds: decoder
                    .unsigned("runtime-ipc-recovery-execution-command-v1.attempt-lease-seconds")?,
            },
        ),
        (COMMAND_STATUS, 2) => Ok(
            CanonicalRuntimeIpcRecoveryExecutionCommandV1::HistoryRecoveryExecutionStatus {
                plan_id: decoder
                    .fixed_bytes("runtime-ipc-recovery-execution-command-v1.plan-id")?,
            },
        ),
        _ => Err(CanonicalRuntimeIpcError::InvalidRequest),
    }
}

fn encode_snapshot_unchecked(
    output: &mut Vec<u8>,
    snapshot: &CanonicalRuntimeIpcRecoveryExecutionSnapshotV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    push_array_header(output, 10);
    push_unsigned(output, CANONICAL_PROFILE_VERSION);
    push_bytes(
        output,
        &encode_runtime_ipc_recovery_plan_state_v1(&snapshot.plan_state)?,
    );
    push_bytes(
        output,
        &encode_signed_runtime_ipc_recovery_scheduler_state_v1(&snapshot.scheduler_state)?,
    );
    push_unsigned(output, snapshot.observed_at_unix_seconds);
    push_unsigned(output, encode_network(snapshot.observed_network));
    push_unsigned(output, encode_power(snapshot.observed_power));
    encode_readiness(output, snapshot.readiness);
    push_unsigned(output, RESTART_NOT_REQUIRED);
    push_unsigned(output, EXECUTION_FOREGROUND_RUNTIME);
    push_unsigned(output, u64::from(snapshot.os_background_service_enabled));
    Ok(())
}

fn decode_snapshot(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryExecutionSnapshotV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-execution-snapshot-v1", 10)?;
    decoder.expect_version("runtime-ipc-recovery-execution-snapshot-v1.schema-version")?;
    let plan_state = decode_and_verify_runtime_ipc_recovery_plan_state_v1(
        decoder.bytes("runtime-ipc-recovery-execution-snapshot-v1.plan-state")?,
    )?;
    let scheduler_state = decode_and_verify_signed_runtime_ipc_recovery_scheduler_state_v1(
        decoder.bytes("runtime-ipc-recovery-execution-snapshot-v1.scheduler-state")?,
    )?;
    let observed_at_unix_seconds =
        decoder.unsigned("runtime-ipc-recovery-execution-snapshot-v1.observed-at-unix-seconds")?;
    let observed_network = decode_network(
        decoder.unsigned("runtime-ipc-recovery-execution-snapshot-v1.observed-network")?,
    )?;
    let observed_power = decode_power(
        decoder.unsigned("runtime-ipc-recovery-execution-snapshot-v1.observed-power")?,
    )?;
    let readiness = decode_readiness(decoder)?;
    let restart_not_required =
        match decoder.unsigned("runtime-ipc-recovery-execution-snapshot-v1.restart-state")? {
            RESTART_NOT_REQUIRED => true,
            _ => return Err(CanonicalRuntimeIpcError::InvalidResponse),
        };
    let foreground_runtime_only =
        match decoder.unsigned("runtime-ipc-recovery-execution-snapshot-v1.execution-scope")? {
            EXECUTION_FOREGROUND_RUNTIME => true,
            _ => return Err(CanonicalRuntimeIpcError::InvalidResponse),
        };
    let os_background_service_enabled = decode_bool(
        decoder,
        "runtime-ipc-recovery-execution-snapshot-v1.os-background-service-enabled",
    )?;
    Ok(CanonicalRuntimeIpcRecoveryExecutionSnapshotV1 {
        plan_state,
        scheduler_state,
        observed_at_unix_seconds,
        observed_network,
        observed_power,
        readiness,
        restart_not_required,
        foreground_runtime_only,
        os_background_service_enabled,
    })
}

fn encode_response_content_unchecked(
    content: &CanonicalRuntimeIpcRecoveryExecutionResponseContentV1,
) -> Result<Vec<u8>, CanonicalRuntimeIpcError> {
    let mut output = Vec::new();
    push_array_header(&mut output, 6);
    push_unsigned(&mut output, CANONICAL_PROFILE_VERSION);
    push_bytes(&mut output, &content.descriptor_id);
    push_bytes(&mut output, &content.request_id);
    push_bytes(&mut output, &content.request_digest);
    push_bytes(&mut output, &content.responder_device_id);
    encode_result_unchecked(&mut output, &content.result)?;
    Ok(output)
}

fn decode_response_content(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryExecutionResponseContentV1, CanonicalRuntimeIpcError> {
    decoder.expect_array("runtime-ipc-recovery-execution-response-content-v1", 6)?;
    decoder.expect_version("runtime-ipc-recovery-execution-response-content-v1.schema-version")?;
    Ok(CanonicalRuntimeIpcRecoveryExecutionResponseContentV1 {
        descriptor_id: decoder
            .fixed_bytes("runtime-ipc-recovery-execution-response-content-v1.descriptor-id")?,
        request_id: decoder
            .fixed_bytes("runtime-ipc-recovery-execution-response-content-v1.request-id")?,
        request_digest: decoder
            .fixed_bytes("runtime-ipc-recovery-execution-response-content-v1.request-digest")?,
        responder_device_id: decoder.fixed_bytes(
            "runtime-ipc-recovery-execution-response-content-v1.responder-device-id",
        )?,
        result: decode_result(decoder)?,
    })
}

fn encode_result_unchecked(
    output: &mut Vec<u8>,
    result: &CanonicalRuntimeIpcRecoveryExecutionResultV1,
) -> Result<(), CanonicalRuntimeIpcError> {
    match result {
        CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStarted {
            outcome,
            snapshot,
        } => {
            push_array_header(output, 3);
            push_unsigned(output, RESULT_STARTED);
            push_unsigned(output, encode_outcome(*outcome));
            encode_snapshot_unchecked(output, snapshot)?;
        }
        CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStatus(snapshot) => {
            push_array_header(output, 2);
            push_unsigned(output, RESULT_STATUS);
            encode_snapshot_unchecked(output, snapshot)?;
        }
        CanonicalRuntimeIpcRecoveryExecutionResultV1::Error { code, message_utf8 } => {
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
) -> Result<CanonicalRuntimeIpcRecoveryExecutionResultV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-recovery-execution-result-v1")?;
    match (
        decoder.unsigned("runtime-ipc-recovery-execution-result-v1.variant")?,
        length,
    ) {
        (RESULT_STARTED, 3) => Ok(
            CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStarted {
                outcome: decode_outcome(
                    decoder.unsigned("runtime-ipc-recovery-execution-result-v1.outcome")?,
                )?,
                snapshot: Box::new(decode_snapshot(decoder)?),
            },
        ),
        (RESULT_STATUS, 2) => Ok(
            CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStatus(Box::new(
                decode_snapshot(decoder)?,
            )),
        ),
        (RESULT_ERROR, 3) => Ok(CanonicalRuntimeIpcRecoveryExecutionResultV1::Error {
            code: u16::try_from(decoder.unsigned("runtime-ipc-recovery-execution-result-v1.code")?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?,
            message_utf8: decoder
                .bytes("runtime-ipc-recovery-execution-result-v1.message")?
                .to_vec(),
        }),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_readiness(output: &mut Vec<u8>, readiness: CanonicalRuntimeIpcRecoveryReadinessV1) {
    match readiness {
        CanonicalRuntimeIpcRecoveryReadinessV1::Ready => {
            push_array_header(output, 1);
            push_unsigned(output, READINESS_READY);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::Deferred {
            not_before_unix_seconds,
        } => {
            push_array_header(output, 2);
            push_unsigned(output, READINESS_DEFERRED);
            push_unsigned(output, not_before_unix_seconds);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::BlockedNetwork => {
            push_array_header(output, 1);
            push_unsigned(output, READINESS_BLOCKED_NETWORK);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::BlockedPower => {
            push_array_header(output, 1);
            push_unsigned(output, READINESS_BLOCKED_POWER);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::Attempting {
            lease_expires_at_unix_seconds,
        } => {
            push_array_header(output, 2);
            push_unsigned(output, READINESS_ATTEMPTING);
            push_unsigned(output, lease_expires_at_unix_seconds);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::AttemptLeaseExpired => {
            push_array_header(output, 1);
            push_unsigned(output, READINESS_ATTEMPT_LEASE_EXPIRED);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::ClockRollback {
            last_observed_unix_seconds,
        } => {
            push_array_header(output, 2);
            push_unsigned(output, READINESS_CLOCK_ROLLBACK);
            push_unsigned(output, last_observed_unix_seconds);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::PlanExpired => {
            push_array_header(output, 1);
            push_unsigned(output, READINESS_PLAN_EXPIRED);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::Cancelled => {
            push_array_header(output, 1);
            push_unsigned(output, READINESS_CANCELLED);
        }
        CanonicalRuntimeIpcRecoveryReadinessV1::Completed => {
            push_array_header(output, 1);
            push_unsigned(output, READINESS_COMPLETED);
        }
    }
}

fn decode_readiness(
    decoder: &mut AccountDecoder<'_>,
) -> Result<CanonicalRuntimeIpcRecoveryReadinessV1, CanonicalRuntimeIpcError> {
    let length = decoder.array("runtime-ipc-recovery-readiness-v1")?;
    match (
        decoder.unsigned("runtime-ipc-recovery-readiness-v1.variant")?,
        length,
    ) {
        (READINESS_READY, 1) => Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Ready),
        (READINESS_DEFERRED, 2) => Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Deferred {
            not_before_unix_seconds: decoder
                .unsigned("runtime-ipc-recovery-readiness-v1.not-before-unix-seconds")?,
        }),
        (READINESS_BLOCKED_NETWORK, 1) => {
            Ok(CanonicalRuntimeIpcRecoveryReadinessV1::BlockedNetwork)
        }
        (READINESS_BLOCKED_POWER, 1) => Ok(CanonicalRuntimeIpcRecoveryReadinessV1::BlockedPower),
        (READINESS_ATTEMPTING, 2) => Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Attempting {
            lease_expires_at_unix_seconds: decoder
                .unsigned("runtime-ipc-recovery-readiness-v1.lease-expires-at-unix-seconds")?,
        }),
        (READINESS_ATTEMPT_LEASE_EXPIRED, 1) => {
            Ok(CanonicalRuntimeIpcRecoveryReadinessV1::AttemptLeaseExpired)
        }
        (READINESS_CLOCK_ROLLBACK, 2) => {
            Ok(CanonicalRuntimeIpcRecoveryReadinessV1::ClockRollback {
                last_observed_unix_seconds: decoder
                    .unsigned("runtime-ipc-recovery-readiness-v1.last-observed-unix-seconds")?,
            })
        }
        (READINESS_PLAN_EXPIRED, 1) => Ok(CanonicalRuntimeIpcRecoveryReadinessV1::PlanExpired),
        (READINESS_CANCELLED, 1) => Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Cancelled),
        (READINESS_COMPLETED, 1) => Ok(CanonicalRuntimeIpcRecoveryReadinessV1::Completed),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_digest_option(output: &mut Vec<u8>, value: Option<&[u8; 32]>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_bytes(output, value);
        }
    }
}

fn decode_digest_option(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<Option<[u8; 32]>, CanonicalRuntimeIpcError> {
    let length = decoder.array(field)?;
    match (decoder.unsigned(field)?, length) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => Ok(Some(decoder.fixed_bytes(field)?)),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_failure_option(output: &mut Vec<u8>, value: Option<u16>) {
    match value {
        None => {
            push_array_header(output, 1);
            push_unsigned(output, OPTION_NONE);
        }
        Some(value) => {
            push_array_header(output, 2);
            push_unsigned(output, OPTION_SOME);
            push_unsigned(output, u64::from(value));
        }
    }
}

fn decode_failure_option(
    decoder: &mut AccountDecoder<'_>,
) -> Result<Option<u16>, CanonicalRuntimeIpcError> {
    let field = "runtime-ipc-recovery-scheduler-state-content-v1.last-failure-code";
    let length = decoder.array(field)?;
    match (decoder.unsigned(field)?, length) {
        (OPTION_NONE, 1) => Ok(None),
        (OPTION_SOME, 2) => {
            let value = u16::try_from(decoder.unsigned(field)?)
                .map_err(|_| CanonicalRuntimeIpcError::InvalidResponse)?;
            if value == 0 {
                return Err(CanonicalRuntimeIpcError::InvalidResponse);
            }
            Ok(Some(value))
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_scheduler_transition(value: CanonicalRuntimeIpcRecoverySchedulerTransitionV1) -> u64 {
    match value {
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized => TRANSITION_INITIALIZED,
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptStarted => {
            TRANSITION_ATTEMPT_STARTED
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptFailed => {
            TRANSITION_ATTEMPT_FAILED
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptProgressed => {
            TRANSITION_ATTEMPT_PROGRESSED
        }
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Cancelled => TRANSITION_CANCELLED,
        CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Completed => TRANSITION_COMPLETED,
    }
}

fn decode_scheduler_transition(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoverySchedulerTransitionV1, CanonicalRuntimeIpcError> {
    match value {
        TRANSITION_INITIALIZED => Ok(CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Initialized),
        TRANSITION_ATTEMPT_STARTED => {
            Ok(CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptStarted)
        }
        TRANSITION_ATTEMPT_FAILED => {
            Ok(CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptFailed)
        }
        TRANSITION_ATTEMPT_PROGRESSED => {
            Ok(CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptProgressed)
        }
        TRANSITION_CANCELLED => Ok(CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Cancelled),
        TRANSITION_COMPLETED => Ok(CanonicalRuntimeIpcRecoverySchedulerTransitionV1::Completed),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_scheduler_lifecycle(value: CanonicalRuntimeIpcRecoverySchedulerLifecycleV1) -> u64 {
    match value {
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active => LIFECYCLE_ACTIVE,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting => LIFECYCLE_ATTEMPTING,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Cancelled => LIFECYCLE_CANCELLED,
        CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Completed => LIFECYCLE_COMPLETED,
    }
}

fn decode_scheduler_lifecycle(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoverySchedulerLifecycleV1, CanonicalRuntimeIpcError> {
    match value {
        LIFECYCLE_ACTIVE => Ok(CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active),
        LIFECYCLE_ATTEMPTING => Ok(CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting),
        LIFECYCLE_CANCELLED => Ok(CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Cancelled),
        LIFECYCLE_COMPLETED => Ok(CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Completed),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_network(value: CanonicalRuntimeIpcRecoveryNetworkV1) -> u64 {
    match value {
        CanonicalRuntimeIpcRecoveryNetworkV1::Ethernet => NETWORK_ETHERNET,
        CanonicalRuntimeIpcRecoveryNetworkV1::Wifi => NETWORK_WIFI,
        CanonicalRuntimeIpcRecoveryNetworkV1::Mobile => NETWORK_MOBILE,
        CanonicalRuntimeIpcRecoveryNetworkV1::Unknown => NETWORK_UNKNOWN,
    }
}

fn decode_network(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryNetworkV1, CanonicalRuntimeIpcError> {
    match value {
        NETWORK_ETHERNET => Ok(CanonicalRuntimeIpcRecoveryNetworkV1::Ethernet),
        NETWORK_WIFI => Ok(CanonicalRuntimeIpcRecoveryNetworkV1::Wifi),
        NETWORK_MOBILE => Ok(CanonicalRuntimeIpcRecoveryNetworkV1::Mobile),
        NETWORK_UNKNOWN => Ok(CanonicalRuntimeIpcRecoveryNetworkV1::Unknown),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_power(value: CanonicalRuntimeIpcRecoveryPowerV1) -> u64 {
    match value {
        CanonicalRuntimeIpcRecoveryPowerV1::External => POWER_EXTERNAL,
        CanonicalRuntimeIpcRecoveryPowerV1::Battery => POWER_BATTERY,
        CanonicalRuntimeIpcRecoveryPowerV1::Unknown => POWER_UNKNOWN,
    }
}

fn decode_power(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryPowerV1, CanonicalRuntimeIpcError> {
    match value {
        POWER_EXTERNAL => Ok(CanonicalRuntimeIpcRecoveryPowerV1::External),
        POWER_BATTERY => Ok(CanonicalRuntimeIpcRecoveryPowerV1::Battery),
        POWER_UNKNOWN => Ok(CanonicalRuntimeIpcRecoveryPowerV1::Unknown),
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn encode_outcome(value: CanonicalRuntimeIpcRecoveryExecutionOutcomeV1) -> u64 {
    match value {
        CanonicalRuntimeIpcRecoveryExecutionOutcomeV1::Committed => OUTCOME_COMMITTED,
        CanonicalRuntimeIpcRecoveryExecutionOutcomeV1::AlreadyCommitted => {
            OUTCOME_ALREADY_COMMITTED
        }
    }
}

fn decode_outcome(
    value: u64,
) -> Result<CanonicalRuntimeIpcRecoveryExecutionOutcomeV1, CanonicalRuntimeIpcError> {
    match value {
        OUTCOME_COMMITTED => Ok(CanonicalRuntimeIpcRecoveryExecutionOutcomeV1::Committed),
        OUTCOME_ALREADY_COMMITTED => {
            Ok(CanonicalRuntimeIpcRecoveryExecutionOutcomeV1::AlreadyCommitted)
        }
        _ => Err(CanonicalRuntimeIpcError::InvalidResponse),
    }
}

fn decode_bool(
    decoder: &mut AccountDecoder<'_>,
    field: &'static str,
) -> Result<bool, CanonicalRuntimeIpcError> {
    match decoder.unsigned(field)? {
        0 => Ok(false),
        1 => Ok(true),
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
    use super::*;
    use crate::{
        canonical_recovery_exchange_v1::{
            decode_and_verify_signed_history_recovery_plan_v1,
            encode_signed_history_recovery_plan_v1,
        },
        canonical_recovery_v1::{
            CanonicalRecoveryInventoryClaimV1, encode_history_recovery_checkpoint_content_v1,
            history_recovery_checkpoint_id_v1,
        },
        canonical_runtime_ipc_recovery_plan_v1::{
            CanonicalRuntimeIpcRecoveryPlanLifecycleV1, CanonicalRuntimeIpcRecoveryPlanStateV1,
            runtime_ipc_recovery_plan_record_id_v1,
        },
        canonical_runtime_ipc_v1::{
            CanonicalRuntimeIpcDescriptorContentV1, CanonicalRuntimeIpcLoopbackAddressV1,
            encode_signed_runtime_ipc_descriptor_v1, sign_runtime_ipc_descriptor_v1,
        },
    };
    use serde_json::{Value, json};
    use std::{fs, path::PathBuf};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    struct Fixture {
        recipient: SigningKey,
        descriptor: CanonicalSignedRuntimeIpcDescriptorV1,
        plan_state: CanonicalRuntimeIpcRecoveryPlanStateV1,
        start_request: CanonicalRuntimeIpcRecoveryExecutionRequestV1,
        status_request: CanonicalRuntimeIpcRecoveryExecutionRequestV1,
    }

    fn vectors() -> TestResult<Value> {
        Ok(serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json"
        )))?)
    }

    fn decode_hex(value: &str) -> TestResult<Vec<u8>> {
        if !value.len().is_multiple_of(2) {
            return Err("hex input has odd length".into());
        }
        (0..value.len())
            .step_by(2)
            .map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
            .collect()
    }

    fn fixed<const N: usize>(value: &Value, pointer: &str) -> TestResult<[u8; N]> {
        let text = value
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or("vector field is missing")?;
        Ok(decode_hex(text)?
            .try_into()
            .map_err(|_| "vector field has wrong length")?)
    }

    fn bytes(value: &Value, pointer: &str) -> TestResult<Vec<u8>> {
        decode_hex(
            value
                .pointer(pointer)
                .and_then(Value::as_str)
                .ok_or("vector field is missing")?,
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn domain(bytes: &[u8]) -> TestResult<String> {
        Ok(String::from_utf8(
            bytes
                .strip_suffix(&[0])
                .ok_or("domain separator is missing")?
                .to_vec(),
        )?)
    }

    fn fixture() -> TestResult<Fixture> {
        let vectors = vectors()?;
        let recipient = SigningKey::from_bytes(&fixed(&vectors, "/input/device_secret_key_hex")?);
        let plan = decode_and_verify_signed_history_recovery_plan_v1(&bytes(
            &vectors,
            "/expected/plan/signed_cbor_hex",
        )?)?;
        let mut plan_state = CanonicalRuntimeIpcRecoveryPlanStateV1 {
            record_id: [0; 32],
            lifecycle: CanonicalRuntimeIpcRecoveryPlanLifecycleV1::Approved,
            signed_plan: plan,
            cancellation: None,
        };
        plan_state.record_id = runtime_ipc_recovery_plan_record_id_v1(&plan_state)?;
        let descriptor = sign_runtime_ipc_descriptor_v1(
            &recipient,
            CanonicalRuntimeIpcDescriptorContentV1 {
                address: CanonicalRuntimeIpcLoopbackAddressV1::Ipv4([127, 0, 0, 1]),
                port: 32123,
                bearer_token: fixed(&vectors, "/input/bearer_token_hex")?,
                account_id: fixed(&vectors, "/input/account_id_hex")?,
                device_id: recipient.verifying_key().to_bytes(),
                server_instance_id: [0x99; 32],
                issued_at_unix_seconds: 1_800_000_000,
                expires_at_unix_seconds: 1_800_086_400,
            },
        )?;
        let descriptor_id = runtime_ipc_descriptor_id_v1(&descriptor)?;
        let plan_id = history_recovery_plan_id_v1(&plan_state.signed_plan)?;
        Ok(Fixture {
            recipient,
            start_request: CanonicalRuntimeIpcRecoveryExecutionRequestV1 {
                descriptor_id,
                request_id: [0xaa; 32],
                bearer_token: fixed(&vectors, "/input/bearer_token_hex")?,
                issued_at_unix_seconds: 1_800_000_065,
                expires_at_unix_seconds: 1_800_000_085,
                command:
                    CanonicalRuntimeIpcRecoveryExecutionCommandV1::StartHistoryRecoveryExecution {
                        plan_id,
                        expected_plan_record_id: plan_state.record_id,
                        retry_base_seconds: 5,
                        retry_max_seconds: 300,
                        attempt_lease_seconds: 120,
                    },
            },
            status_request: CanonicalRuntimeIpcRecoveryExecutionRequestV1 {
                descriptor_id,
                request_id: [0xbb; 32],
                bearer_token: fixed(&vectors, "/input/bearer_token_hex")?,
                issued_at_unix_seconds: 1_800_000_066,
                expires_at_unix_seconds: 1_800_000_086,
                command:
                    CanonicalRuntimeIpcRecoveryExecutionCommandV1::HistoryRecoveryExecutionStatus {
                        plan_id,
                    },
            },
            descriptor,
            plan_state,
        })
    }

    #[test]
    fn execution_request_response_and_readiness_round_trip() -> TestResult {
        let fixture = fixture()?;
        let scheduler = create_runtime_ipc_recovery_execution_state_for_request_v1(
            &fixture.recipient,
            &fixture.start_request,
            &fixture.descriptor,
            &fixture.plan_state,
            1_800_000_065,
        )?;
        let snapshot = create_runtime_ipc_recovery_execution_snapshot_v1(
            fixture.plan_state.clone(),
            scheduler,
            1_800_000_065,
            CanonicalRuntimeIpcRecoveryNetworkV1::Ethernet,
            CanonicalRuntimeIpcRecoveryPowerV1::External,
        )?;
        assert_eq!(
            snapshot.readiness,
            CanonicalRuntimeIpcRecoveryReadinessV1::Ready
        );
        let response = sign_runtime_ipc_recovery_execution_response_v1(
            &fixture.recipient,
            &fixture.start_request,
            &fixture.descriptor,
            CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStarted {
                outcome: CanonicalRuntimeIpcRecoveryExecutionOutcomeV1::Committed,
                snapshot: Box::new(snapshot),
            },
        )?;
        let request_bytes =
            encode_runtime_ipc_recovery_execution_request_v1(&fixture.start_request)?;
        assert_eq!(
            decode_and_verify_runtime_ipc_recovery_execution_request_v1(&request_bytes)?,
            fixture.start_request
        );
        let response_bytes = encode_signed_runtime_ipc_recovery_execution_response_v1(&response)?;
        let decoded =
            decode_and_verify_signed_runtime_ipc_recovery_execution_response_v1(&response_bytes)?;
        verify_runtime_ipc_recovery_execution_response_for_request_v1(
            &decoded,
            &fixture.start_request,
            &fixture.descriptor,
        )?;
        Ok(())
    }

    #[test]
    fn scheduler_chain_binds_attempts_backoff_and_checkpoints() -> TestResult {
        let fixture = fixture()?;
        let initialized = initialize_runtime_ipc_recovery_scheduler_state_v1(
            &fixture.recipient,
            &fixture.plan_state,
            8,
            40,
            120,
            1_800_000_065,
        )?;
        let started = start_runtime_ipc_recovery_scheduler_attempt_v1(
            &fixture.recipient,
            &fixture.plan_state,
            &initialized,
            1_800_000_065,
        )?;
        let failed = fail_runtime_ipc_recovery_scheduler_attempt_v1(
            &fixture.recipient,
            &fixture.plan_state,
            &started,
            1_800_000_066,
            7,
        )?;
        assert_eq!(failed.content.consecutive_failures, 1);
        assert!((4..=8).contains(&failed.content.scheduled_delay_seconds));
        verify_runtime_ipc_recovery_scheduler_successor_v1(&started, &failed)?;
        let mut fork = failed.clone();
        fork.content.previous_state_id = Some([0x55; 32]);
        assert!(verify_runtime_ipc_recovery_scheduler_successor_v1(&started, &fork).is_err());
        Ok(())
    }

    #[test]
    fn scheduler_successor_rejects_overflow_zero_failure_and_nondeterministic_delay() -> TestResult
    {
        let fixture = fixture()?;
        let initialized = initialize_runtime_ipc_recovery_scheduler_state_v1(
            &fixture.recipient,
            &fixture.plan_state,
            8,
            40,
            120,
            1_800_000_065,
        )?;
        let started = start_runtime_ipc_recovery_scheduler_attempt_v1(
            &fixture.recipient,
            &fixture.plan_state,
            &initialized,
            1_800_000_065,
        )?;
        let failed = fail_runtime_ipc_recovery_scheduler_attempt_v1(
            &fixture.recipient,
            &fixture.plan_state,
            &started,
            1_800_000_066,
            7,
        )?;

        let mut zero_failure = failed.content.clone();
        zero_failure.last_failure_code = Some(0);
        assert!(sign_scheduler_state(&fixture.recipient, zero_failure).is_err());

        let mut max_attempts = failed.content.clone();
        max_attempts.total_attempts = u64::MAX;
        let max_attempts = sign_scheduler_state(&fixture.recipient, max_attempts)?;
        let mut wrapped_attempt = max_attempts.content.clone();
        wrapped_attempt.generation += 1;
        wrapped_attempt.previous_state_id =
            Some(runtime_ipc_recovery_scheduler_state_id_v1(&max_attempts)?);
        wrapped_attempt.transition =
            CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptStarted;
        wrapped_attempt.lifecycle = CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Attempting;
        wrapped_attempt.next_attempt_at_unix_seconds = wrapped_attempt
            .last_observed_unix_seconds
            .checked_add(wrapped_attempt.attempt_lease_seconds)
            .ok_or("attempt lease overflow")?;
        wrapped_attempt.scheduled_delay_seconds = wrapped_attempt.attempt_lease_seconds;
        wrapped_attempt.last_failure_code = None;
        let wrapped_attempt = sign_scheduler_state(&fixture.recipient, wrapped_attempt)?;
        assert!(
            verify_runtime_ipc_recovery_scheduler_successor_v1(&max_attempts, &wrapped_attempt)
                .is_err()
        );

        let mut max_failures = started.content.clone();
        max_failures.consecutive_failures = u32::MAX;
        let max_failures = sign_scheduler_state(&fixture.recipient, max_failures)?;
        let mut wrapped_failure = max_failures.content.clone();
        wrapped_failure.generation += 1;
        wrapped_failure.previous_state_id =
            Some(runtime_ipc_recovery_scheduler_state_id_v1(&max_failures)?);
        wrapped_failure.transition =
            CanonicalRuntimeIpcRecoverySchedulerTransitionV1::AttemptFailed;
        wrapped_failure.lifecycle = CanonicalRuntimeIpcRecoverySchedulerLifecycleV1::Active;
        wrapped_failure.last_observed_unix_seconds += 1;
        wrapped_failure.scheduled_delay_seconds = recovery_backoff_delay_seconds(
            wrapped_failure.plan_id,
            u32::MAX,
            wrapped_failure.total_attempts,
            wrapped_failure.retry_base_seconds,
            wrapped_failure.retry_max_seconds,
        );
        wrapped_failure.next_attempt_at_unix_seconds = wrapped_failure
            .last_observed_unix_seconds
            .checked_add(wrapped_failure.scheduled_delay_seconds)
            .ok_or("retry delay overflow")?;
        wrapped_failure.last_failure_code = Some(7);
        let wrapped_failure = sign_scheduler_state(&fixture.recipient, wrapped_failure)?;
        assert!(
            verify_runtime_ipc_recovery_scheduler_successor_v1(&max_failures, &wrapped_failure)
                .is_err()
        );

        let mut checkpoint_content = started.content.latest_checkpoint.content.clone();
        checkpoint_content.inventory_claim = Some(CanonicalRecoveryInventoryClaimV1 {
            event_count: checkpoint_content.approved_range_end,
            digest: [0x63; 32],
        });
        checkpoint_content.next_range_start = checkpoint_content
            .approved_range_start
            .checked_add(1)
            .ok_or("checkpoint range overflow")?;
        checkpoint_content.previous_checkpoint_id = Some(history_recovery_checkpoint_id_v1(
            &started.content.latest_checkpoint,
        )?);
        let checkpoint = CanonicalSignedHistoryRecoveryCheckpointV1 {
            signature: fixture
                .recipient
                .sign(&history_recovery_checkpoint_signing_bytes_v1(
                    &checkpoint_content,
                )?)
                .to_bytes(),
            content: checkpoint_content,
        };
        let progressed = progress_runtime_ipc_recovery_scheduler_attempt_v1(
            &fixture.recipient,
            &fixture.plan_state,
            &started,
            checkpoint,
            1_800_000_066,
        )?;
        let mut nondeterministic = progressed.content.clone();
        nondeterministic.scheduled_delay_seconds =
            nondeterministic.scheduled_delay_seconds.saturating_add(1);
        nondeterministic.next_attempt_at_unix_seconds = nondeterministic
            .last_observed_unix_seconds
            .checked_add(nondeterministic.scheduled_delay_seconds)
            .ok_or("nondeterministic delay overflow")?;
        let nondeterministic = sign_scheduler_state(&fixture.recipient, nondeterministic)?;
        assert!(
            verify_runtime_ipc_recovery_scheduler_successor_v1(&started, &nondeterministic)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn readiness_blocks_disallowed_context_and_rejects_substitution() -> TestResult {
        let fixture = fixture()?;
        let scheduler = initialize_runtime_ipc_recovery_scheduler_state_v1(
            &fixture.recipient,
            &fixture.plan_state,
            5,
            300,
            120,
            1_800_000_065,
        )?;
        assert_eq!(
            derive_runtime_ipc_recovery_readiness_v1(
                &fixture.plan_state,
                &scheduler,
                1_800_000_065,
                CanonicalRuntimeIpcRecoveryNetworkV1::Mobile,
                CanonicalRuntimeIpcRecoveryPowerV1::External,
            )?,
            CanonicalRuntimeIpcRecoveryReadinessV1::BlockedNetwork
        );
        let mut substituted = scheduler.clone();
        substituted.content.plan_id = [0x42; 32];
        assert!(validate_scheduler_for_plan(&substituted, &fixture.plan_state).is_err());
        let _ = &fixture.status_request;
        Ok(())
    }

    #[test]
    #[ignore = "explicit golden-vector regeneration"]
    fn regenerate_canonical_runtime_ipc_recovery_execution_vectors() -> TestResult {
        let fixture = fixture()?;
        let initialized = create_runtime_ipc_recovery_execution_state_for_request_v1(
            &fixture.recipient,
            &fixture.start_request,
            &fixture.descriptor,
            &fixture.plan_state,
            fixture.start_request.issued_at_unix_seconds,
        )?;
        let start_snapshot = create_runtime_ipc_recovery_execution_snapshot_v1(
            fixture.plan_state.clone(),
            initialized.clone(),
            fixture.start_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcRecoveryNetworkV1::Ethernet,
            CanonicalRuntimeIpcRecoveryPowerV1::External,
        )?;
        let start_response = sign_runtime_ipc_recovery_execution_response_v1(
            &fixture.recipient,
            &fixture.start_request,
            &fixture.descriptor,
            CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStarted {
                outcome: CanonicalRuntimeIpcRecoveryExecutionOutcomeV1::Committed,
                snapshot: Box::new(start_snapshot),
            },
        )?;
        let started = start_runtime_ipc_recovery_scheduler_attempt_v1(
            &fixture.recipient,
            &fixture.plan_state,
            &initialized,
            fixture.start_request.issued_at_unix_seconds,
        )?;
        let failed = fail_runtime_ipc_recovery_scheduler_attempt_v1(
            &fixture.recipient,
            &fixture.plan_state,
            &started,
            fixture.status_request.issued_at_unix_seconds,
            7,
        )?;
        let status_snapshot = create_runtime_ipc_recovery_execution_snapshot_v1(
            fixture.plan_state.clone(),
            failed.clone(),
            fixture.status_request.issued_at_unix_seconds,
            CanonicalRuntimeIpcRecoveryNetworkV1::Wifi,
            CanonicalRuntimeIpcRecoveryPowerV1::Battery,
        )?;
        let status_response = sign_runtime_ipc_recovery_execution_response_v1(
            &fixture.recipient,
            &fixture.status_request,
            &fixture.descriptor,
            CanonicalRuntimeIpcRecoveryExecutionResultV1::HistoryRecoveryExecutionStatus(Box::new(
                status_snapshot,
            )),
        )?;
        let plan = &fixture.plan_state.signed_plan;
        let link = &plan.content.signed_link.content;
        let initial_checkpoint = &initialized.content.latest_checkpoint;

        let scheduler_json =
            |state: &CanonicalSignedRuntimeIpcRecoverySchedulerStateV1| -> TestResult<Value> {
                let content = encode_scheduler_state_content_unchecked(&state.content)?;
                Ok(json!({
                    "content_cbor_hex": hex(&content),
                    "signing_preimage_hex": hex(&prefixed(
                        CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_SIGNATURE_DOMAIN,
                        &content,
                    )),
                    "signature_hex": hex(&state.signature),
                    "signed_cbor_hex": hex(&encode_signed_runtime_ipc_recovery_scheduler_state_v1(state)?),
                    "state_id_hex": hex(&runtime_ipc_recovery_scheduler_state_id_v1(state)?)
                }))
            };
        let case_json = |request: &CanonicalRuntimeIpcRecoveryExecutionRequestV1,
                         response: &CanonicalSignedRuntimeIpcRecoveryExecutionResponseV1|
         -> TestResult<Value> {
            let response_content = encode_response_content_unchecked(&response.content)?;
            Ok(json!({
                "request": {
                    "cbor_hex": hex(&encode_runtime_ipc_recovery_execution_request_v1(request)?),
                    "digest_hex": hex(&runtime_ipc_recovery_execution_request_digest_v1(request)?)
                },
                "response": {
                    "content_cbor_hex": hex(&response_content),
                    "signing_preimage_hex": hex(&prefixed(
                        CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_SIGNATURE_DOMAIN,
                        &response_content,
                    )),
                    "signature_hex": hex(&response.signature),
                    "signed_cbor_hex": hex(&encode_signed_runtime_ipc_recovery_execution_response_v1(response)?),
                    "id_hex": hex(&runtime_ipc_recovery_execution_response_id_v1(response)?)
                }
            }))
        };
        let document = json!({
            "schema_version": 1,
            "milestone": "M1.1.44",
            "status": "candidate-not-active",
            "changes_active_runtime_ipc": false,
            "encoding": "kilogram-deterministic-cbor-v1",
            "domains": {
                "scheduler_signature": domain(CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_SIGNATURE_DOMAIN)?,
                "scheduler_state_id": domain(CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_STATE_ID_DOMAIN)?,
                "scheduler_jitter": domain(CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_JITTER_DOMAIN)?,
                "request_digest": domain(CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_REQUEST_DIGEST_DOMAIN)?,
                "response_signature": domain(CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_SIGNATURE_DOMAIN)?,
                "response_id": domain(CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_ID_DOMAIN)?
            },
            "input": {
                "device_secret_key_hex": hex(&fixture.recipient.to_bytes()),
                "descriptor_cbor_hex": hex(&encode_signed_runtime_ipc_descriptor_v1(&fixture.descriptor)?),
                "descriptor_id_hex": hex(&fixture.start_request.descriptor_id),
                "bearer_token_hex": hex(&fixture.start_request.bearer_token),
                "account_id_hex": hex(&fixture.descriptor.content.account_id),
                "recipient_device_id_hex": hex(&fixture.descriptor.content.device_id),
                "signed_plan_cbor_hex": hex(&encode_signed_history_recovery_plan_v1(plan)?),
                "plan_state_cbor_hex": hex(&encode_runtime_ipc_recovery_plan_state_v1(&fixture.plan_state)?),
                "plan_id_hex": hex(&history_recovery_plan_id_v1(plan)?),
                "plan_record_id_hex": hex(&fixture.plan_state.record_id),
                "conversation_id_hex": hex(&link.conversation_id),
                "source_device_id_hex": hex(&link.source_certificate.content.device_id),
                "confirmed_sas_hex": hex(&plan.content.sas),
                "approved_range_start": link.approved_range_start,
                "approved_range_end": link.approved_range_end,
                "page_size": link.page_size,
                "retry_base_seconds": initialized.content.retry_base_seconds,
                "retry_max_seconds": initialized.content.retry_max_seconds,
                "attempt_lease_seconds": initialized.content.attempt_lease_seconds,
                "start_request_id_hex": hex(&fixture.start_request.request_id),
                "start_issued_at_unix_seconds": fixture.start_request.issued_at_unix_seconds,
                "start_expires_at_unix_seconds": fixture.start_request.expires_at_unix_seconds,
                "status_request_id_hex": hex(&fixture.status_request.request_id),
                "status_issued_at_unix_seconds": fixture.status_request.issued_at_unix_seconds,
                "status_expires_at_unix_seconds": fixture.status_request.expires_at_unix_seconds,
                "failure_code": 7
            },
            "expected": {
                "initial_checkpoint": {
                    "content_cbor_hex": hex(&encode_history_recovery_checkpoint_content_v1(&initial_checkpoint.content)?),
                    "signing_preimage_hex": hex(&history_recovery_checkpoint_signing_bytes_v1(&initial_checkpoint.content)?),
                    "signature_hex": hex(&initial_checkpoint.signature),
                    "signed_cbor_hex": hex(&encode_signed_history_recovery_checkpoint_v1(initial_checkpoint)?)
                },
                "initialized_scheduler": scheduler_json(&initialized)?,
                "started_scheduler": scheduler_json(&started)?,
                "failed_scheduler": {
                    "scheduled_delay_seconds": failed.content.scheduled_delay_seconds,
                    "next_attempt_at_unix_seconds": failed.content.next_attempt_at_unix_seconds,
                    "state": scheduler_json(&failed)?
                },
                "start": case_json(&fixture.start_request, &start_response)?,
                "status": case_json(&fixture.status_request, &status_response)?
            }
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../CANONICAL-RUNTIME-IPC-RECOVERY-EXECUTION-VECTORS.json");
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
        Ok(())
    }
}
