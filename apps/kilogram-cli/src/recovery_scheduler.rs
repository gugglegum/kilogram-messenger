use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use kilogram_identity::{DeviceId, DeviceIdentity};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

const SCHEDULER_STATE_VERSION: u8 = 1;
const SCHEDULER_STATE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-scheduler-state-signature:v1\0";
const SCHEDULER_STATE_ID_DOMAIN: &[u8] = b"kilogram:history-recovery-scheduler-state-id:v1\0";
const SCHEDULER_JITTER_DOMAIN: &[u8] = b"kilogram:history-recovery-scheduler-jitter:v1\0";
const SCHEDULER_DIRECTORY: &str = "scheduler";
const SCHEDULER_STATE_EXTENSION: &str = "scheduler-state";
const MAX_SCHEDULER_STATE_BYTES: usize = 4 * 1024;
const MAX_SCHEDULER_STATE_RECORDS: usize = 4_096;

pub(crate) const DEFAULT_RECOVERY_RETRY_BASE_SECONDS: u64 = 5;
pub(crate) const DEFAULT_RECOVERY_RETRY_MAX_SECONDS: u64 = 300;
pub(crate) const MAX_RECOVERY_RETRY_BASE_SECONDS: u64 = 300;
pub(crate) const MAX_RECOVERY_RETRY_MAX_SECONDS: u64 = 3_600;
pub(crate) const MAX_RECOVERY_ATTEMPT_LEASE_SECONDS: u64 = 2 * 60 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum RecoverySchedulerLifecycle {
    Active,
    Attempting,
    Cancelled,
    Completed,
}

impl RecoverySchedulerLifecycle {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Attempting => "attempting",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
        }
    }

    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum RecoverySchedulerTransition {
    Initialized,
    AttemptStarted,
    AttemptFailed,
    AttemptProgressed,
    Cancelled,
    Completed,
}

impl RecoverySchedulerTransition {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Initialized => "initialized",
            Self::AttemptStarted => "attempt-started",
            Self::AttemptFailed => "attempt-failed",
            Self::AttemptProgressed => "attempt-progressed",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryBackoffConfig {
    base_seconds: u64,
    max_seconds: u64,
}

impl RecoveryBackoffConfig {
    pub(crate) fn new(base_seconds: u64, max_seconds: u64) -> Result<Self> {
        ensure!(
            base_seconds <= MAX_RECOVERY_RETRY_BASE_SECONDS,
            "recovery retry base must not exceed {MAX_RECOVERY_RETRY_BASE_SECONDS} seconds"
        );
        ensure!(
            max_seconds <= MAX_RECOVERY_RETRY_MAX_SECONDS,
            "recovery retry maximum must not exceed {MAX_RECOVERY_RETRY_MAX_SECONDS} seconds"
        );
        ensure!(
            base_seconds <= max_seconds,
            "recovery retry base must not exceed its maximum"
        );
        Ok(Self {
            base_seconds,
            max_seconds,
        })
    }

    pub(crate) fn base_seconds(self) -> u64 {
        self.base_seconds
    }

    pub(crate) fn max_seconds(self) -> u64 {
        self.max_seconds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoverySchedulerReadiness {
    Ready,
    Deferred { not_before_unix_seconds: u64 },
    ClockRollback { last_observed_unix_seconds: u64 },
    AttemptLeaseExpired,
    Cancelled,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RecoverySchedulerStateContent {
    version: u8,
    plan_id: [u8; 32],
    recipient_device_id: DeviceId,
    generation: u64,
    previous_state_id: Option<[u8; 32]>,
    transition: RecoverySchedulerTransition,
    lifecycle: RecoverySchedulerLifecycle,
    total_attempts: u64,
    consecutive_failures: u32,
    last_observed_unix_seconds: u64,
    next_attempt_at_unix_seconds: u64,
    scheduled_delay_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct SignedRecoverySchedulerState {
    content: RecoverySchedulerStateContent,
    signature: Vec<u8>,
}

impl SignedRecoverySchedulerState {
    pub(crate) fn initialize(
        identity: &DeviceIdentity,
        plan_id: [u8; 32],
        now_unix_seconds: u64,
    ) -> Result<Self> {
        let content = RecoverySchedulerStateContent {
            version: SCHEDULER_STATE_VERSION,
            plan_id,
            recipient_device_id: identity.device_id(),
            generation: 0,
            previous_state_id: None,
            transition: RecoverySchedulerTransition::Initialized,
            lifecycle: RecoverySchedulerLifecycle::Active,
            total_attempts: 0,
            consecutive_failures: 0,
            last_observed_unix_seconds: now_unix_seconds,
            next_attempt_at_unix_seconds: now_unix_seconds,
            scheduled_delay_seconds: 0,
        };
        Self::sign(identity, content)
    }

    pub(crate) fn start_attempt(
        &self,
        identity: &DeviceIdentity,
        now_unix_seconds: u64,
        attempt_lease_seconds: u64,
    ) -> Result<Self> {
        ensure!(
            self.readiness(now_unix_seconds) == RecoverySchedulerReadiness::Ready,
            "history recovery scheduler is not ready to start an attempt"
        );
        ensure!(
            attempt_lease_seconds > 0,
            "history recovery attempt lease must be positive"
        );
        let next_attempt_at_unix_seconds = now_unix_seconds
            .checked_add(attempt_lease_seconds)
            .context("history recovery attempt lease deadline overflows")?;
        self.sign_successor(
            identity,
            RecoverySchedulerTransition::AttemptStarted,
            RecoverySchedulerLifecycle::Attempting,
            self.total_attempts()
                .checked_add(1)
                .context("history recovery total attempt counter overflows")?,
            self.consecutive_failures(),
            now_unix_seconds,
            next_attempt_at_unix_seconds,
            attempt_lease_seconds,
        )
    }

    pub(crate) fn record_failure(
        &self,
        identity: &DeviceIdentity,
        now_unix_seconds: u64,
        backoff: RecoveryBackoffConfig,
    ) -> Result<Self> {
        ensure!(
            self.lifecycle() == RecoverySchedulerLifecycle::Attempting,
            "only an in-progress history recovery attempt can fail"
        );
        ensure!(
            now_unix_seconds >= self.last_observed_unix_seconds(),
            "wall clock moved backwards during a history recovery attempt"
        );
        let failures = self
            .consecutive_failures()
            .checked_add(1)
            .context("history recovery consecutive failure counter overflows")?;
        let delay = recovery_backoff_delay_seconds(
            self.plan_id(),
            failures,
            self.total_attempts(),
            backoff,
        );
        let next_attempt_at_unix_seconds = now_unix_seconds
            .checked_add(delay)
            .context("history recovery retry deadline overflows")?;
        self.sign_successor(
            identity,
            RecoverySchedulerTransition::AttemptFailed,
            RecoverySchedulerLifecycle::Active,
            self.total_attempts(),
            failures,
            now_unix_seconds,
            next_attempt_at_unix_seconds,
            delay,
        )
    }

    pub(crate) fn record_progress(
        &self,
        identity: &DeviceIdentity,
        now_unix_seconds: u64,
        backoff: RecoveryBackoffConfig,
    ) -> Result<Self> {
        ensure!(
            self.lifecycle() == RecoverySchedulerLifecycle::Attempting,
            "only an in-progress history recovery attempt can record progress"
        );
        ensure!(
            now_unix_seconds >= self.last_observed_unix_seconds(),
            "wall clock moved backwards during a history recovery attempt"
        );
        let delay =
            recovery_backoff_delay_seconds(self.plan_id(), 1, self.total_attempts(), backoff);
        let next_attempt_at_unix_seconds = now_unix_seconds
            .checked_add(delay)
            .context("history recovery progress deadline overflows")?;
        self.sign_successor(
            identity,
            RecoverySchedulerTransition::AttemptProgressed,
            RecoverySchedulerLifecycle::Active,
            self.total_attempts(),
            0,
            now_unix_seconds,
            next_attempt_at_unix_seconds,
            delay,
        )
    }

    pub(crate) fn cancel(&self, identity: &DeviceIdentity, now_unix_seconds: u64) -> Result<Self> {
        ensure!(
            !self.lifecycle().is_terminal(),
            "history recovery scheduler is already terminal"
        );
        let observed = now_unix_seconds.max(self.last_observed_unix_seconds());
        self.sign_successor(
            identity,
            RecoverySchedulerTransition::Cancelled,
            RecoverySchedulerLifecycle::Cancelled,
            self.total_attempts(),
            self.consecutive_failures(),
            observed,
            observed,
            0,
        )
    }

    pub(crate) fn complete(
        &self,
        identity: &DeviceIdentity,
        now_unix_seconds: u64,
    ) -> Result<Self> {
        ensure!(
            !self.lifecycle().is_terminal(),
            "history recovery scheduler is already terminal"
        );
        let observed = now_unix_seconds.max(self.last_observed_unix_seconds());
        self.sign_successor(
            identity,
            RecoverySchedulerTransition::Completed,
            RecoverySchedulerLifecycle::Completed,
            self.total_attempts(),
            self.consecutive_failures(),
            observed,
            observed,
            0,
        )
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self).context("encode recovery scheduler state")?;
        ensure!(
            encoded.len() <= MAX_SCHEDULER_STATE_BYTES,
            "history recovery scheduler state exceeds {MAX_SCHEDULER_STATE_BYTES} bytes"
        );
        Ok(encoded)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_SCHEDULER_STATE_BYTES,
            "history recovery scheduler state must contain 1..={MAX_SCHEDULER_STATE_BYTES} bytes"
        );
        let state: Self =
            postcard::from_bytes(bytes).context("decode history recovery scheduler state")?;
        state.verify()?;
        Ok(state)
    }

    pub(crate) fn verify(&self) -> Result<()> {
        validate_content(&self.content)?;
        self.recipient_device_id()
            .verify(&signing_bytes(&self.content)?, &self.signature)
            .context("verify recipient signature on history recovery scheduler state")
    }

    pub(crate) fn verify_successor_of(&self, previous: &Self) -> Result<()> {
        self.verify()?;
        previous.verify()?;
        ensure!(
            self.plan_id() == previous.plan_id(),
            "history recovery scheduler chain changes plan ID"
        );
        ensure!(
            self.recipient_device_id() == previous.recipient_device_id(),
            "history recovery scheduler chain changes recipient device"
        );
        ensure!(
            self.generation() == previous.generation().saturating_add(1),
            "history recovery scheduler chain skips or repeats a generation"
        );
        ensure!(
            self.previous_state_id() == Some(previous.state_id()?),
            "history recovery scheduler chain is forked or missing its previous state"
        );
        ensure!(
            self.last_observed_unix_seconds() >= previous.last_observed_unix_seconds(),
            "history recovery scheduler chain regresses observed wall-clock time"
        );
        ensure!(
            !previous.lifecycle().is_terminal(),
            "history recovery scheduler terminal state has a successor"
        );
        validate_transition(previous, self)
    }

    pub(crate) fn state_id(&self) -> Result<[u8; 32]> {
        self.verify()?;
        let encoded = postcard::to_allocvec(self).context("encode scheduler state ID")?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(SCHEDULER_STATE_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }

    pub(crate) fn readiness(&self, now_unix_seconds: u64) -> RecoverySchedulerReadiness {
        match self.lifecycle() {
            RecoverySchedulerLifecycle::Cancelled => RecoverySchedulerReadiness::Cancelled,
            RecoverySchedulerLifecycle::Completed => RecoverySchedulerReadiness::Completed,
            RecoverySchedulerLifecycle::Active => {
                if now_unix_seconds < self.last_observed_unix_seconds() {
                    RecoverySchedulerReadiness::ClockRollback {
                        last_observed_unix_seconds: self.last_observed_unix_seconds(),
                    }
                } else if now_unix_seconds >= self.next_attempt_at_unix_seconds() {
                    RecoverySchedulerReadiness::Ready
                } else {
                    RecoverySchedulerReadiness::Deferred {
                        not_before_unix_seconds: self.next_attempt_at_unix_seconds(),
                    }
                }
            }
            RecoverySchedulerLifecycle::Attempting => {
                if now_unix_seconds < self.last_observed_unix_seconds() {
                    RecoverySchedulerReadiness::ClockRollback {
                        last_observed_unix_seconds: self.last_observed_unix_seconds(),
                    }
                } else if now_unix_seconds >= self.next_attempt_at_unix_seconds() {
                    RecoverySchedulerReadiness::AttemptLeaseExpired
                } else {
                    RecoverySchedulerReadiness::Deferred {
                        not_before_unix_seconds: self.next_attempt_at_unix_seconds(),
                    }
                }
            }
        }
    }

    pub(crate) fn plan_id(&self) -> [u8; 32] {
        self.content.plan_id
    }

    pub(crate) fn recipient_device_id(&self) -> DeviceId {
        self.content.recipient_device_id
    }

    pub(crate) fn generation(&self) -> u64 {
        self.content.generation
    }

    pub(crate) fn previous_state_id(&self) -> Option<[u8; 32]> {
        self.content.previous_state_id
    }

    pub(crate) fn transition(&self) -> RecoverySchedulerTransition {
        self.content.transition
    }

    pub(crate) fn lifecycle(&self) -> RecoverySchedulerLifecycle {
        self.content.lifecycle
    }

    pub(crate) fn total_attempts(&self) -> u64 {
        self.content.total_attempts
    }

    pub(crate) fn consecutive_failures(&self) -> u32 {
        self.content.consecutive_failures
    }

    pub(crate) fn last_observed_unix_seconds(&self) -> u64 {
        self.content.last_observed_unix_seconds
    }

    pub(crate) fn next_attempt_at_unix_seconds(&self) -> u64 {
        self.content.next_attempt_at_unix_seconds
    }

    pub(crate) fn scheduled_delay_seconds(&self) -> u64 {
        self.content.scheduled_delay_seconds
    }

    #[allow(clippy::too_many_arguments)]
    fn sign_successor(
        &self,
        identity: &DeviceIdentity,
        transition: RecoverySchedulerTransition,
        lifecycle: RecoverySchedulerLifecycle,
        total_attempts: u64,
        consecutive_failures: u32,
        last_observed_unix_seconds: u64,
        next_attempt_at_unix_seconds: u64,
        scheduled_delay_seconds: u64,
    ) -> Result<Self> {
        self.verify()?;
        ensure!(
            identity.device_id() == self.recipient_device_id(),
            "history recovery scheduler successor signer is not the plan recipient"
        );
        let content = RecoverySchedulerStateContent {
            version: SCHEDULER_STATE_VERSION,
            plan_id: self.plan_id(),
            recipient_device_id: self.recipient_device_id(),
            generation: self
                .generation()
                .checked_add(1)
                .context("history recovery scheduler generation overflows")?,
            previous_state_id: Some(self.state_id()?),
            transition,
            lifecycle,
            total_attempts,
            consecutive_failures,
            last_observed_unix_seconds,
            next_attempt_at_unix_seconds,
            scheduled_delay_seconds,
        };
        let next = Self::sign(identity, content)?;
        next.verify_successor_of(self)?;
        Ok(next)
    }

    fn sign(identity: &DeviceIdentity, content: RecoverySchedulerStateContent) -> Result<Self> {
        validate_content(&content)?;
        ensure!(
            identity.device_id() == content.recipient_device_id,
            "history recovery scheduler state signer is not its recipient"
        );
        let signature = identity.sign(&signing_bytes(&content)?).to_vec();
        let state = Self { content, signature };
        state.verify()?;
        Ok(state)
    }
}

pub(crate) fn load_recovery_scheduler_state(
    state_dir: &Path,
    plan_id: [u8; 32],
    recipient_device_id: DeviceId,
) -> Result<Option<SignedRecoverySchedulerState>> {
    let directory = scheduler_plan_directory(state_dir, plan_id);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some(SCHEDULER_STATE_EXTENSION) {
            continue;
        }
        ensure!(
            entry.file_type()?.is_file(),
            "history recovery scheduler record is not a regular file: {}",
            path.display()
        );
        ensure!(
            records.len() < MAX_SCHEDULER_STATE_RECORDS,
            "history recovery scheduler state exceeds {MAX_SCHEDULER_STATE_RECORDS} records"
        );
        let metadata = entry.metadata()?;
        ensure!(
            metadata.len() <= MAX_SCHEDULER_STATE_BYTES as u64,
            "history recovery scheduler record exceeds {MAX_SCHEDULER_STATE_BYTES} bytes"
        );
        let state = SignedRecoverySchedulerState::decode(
            &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        )
        .with_context(|| {
            format!(
                "verify history recovery scheduler record {}",
                path.display()
            )
        })?;
        ensure!(
            state.plan_id() == plan_id,
            "history recovery scheduler record belongs to a different plan"
        );
        ensure!(
            state.recipient_device_id() == recipient_device_id,
            "history recovery scheduler record belongs to a different recipient"
        );
        ensure!(
            entry.file_name().to_string_lossy() == scheduler_state_file_name(&state)?,
            "history recovery scheduler record filename does not match its signed content"
        );
        records.push(state);
    }
    records.sort_by_key(SignedRecoverySchedulerState::generation);
    if records.is_empty() {
        return Ok(None);
    }
    ensure!(
        records[0].generation() == 0,
        "history recovery scheduler chain is missing its initial state"
    );
    for pair in records.windows(2) {
        pair[1].verify_successor_of(&pair[0])?;
    }
    Ok(records.pop())
}

pub(crate) fn persist_recovery_scheduler_state(
    state_dir: &Path,
    state: &SignedRecoverySchedulerState,
) -> Result<PathBuf> {
    state.verify()?;
    let existing =
        load_recovery_scheduler_state(state_dir, state.plan_id(), state.recipient_device_id())?;
    match existing.as_ref() {
        Some(previous) => state.verify_successor_of(previous)?,
        None => ensure!(
            state.generation() == 0
                && state.transition() == RecoverySchedulerTransition::Initialized,
            "the first persisted recovery scheduler state must initialize generation zero"
        ),
    }
    let state_root = fs::canonicalize(state_dir)
        .with_context(|| format!("canonicalize scheduler state root {}", state_dir.display()))?;
    let directory = scheduler_plan_directory(&state_root, state.plan_id());
    fs::create_dir_all(&directory)?;
    let directory = fs::canonicalize(&directory)?;
    ensure!(
        directory.starts_with(&state_root),
        "history recovery scheduler directory escapes the device state root"
    );
    let path = directory.join(scheduler_state_file_name(state)?);
    let encoded = state.encode()?;
    let mut temporary = NamedTempFile::new_in(&directory)?;
    temporary.write_all(&encoded)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&path) {
        Ok(file) => file.sync_all()?,
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            ensure!(
                fs::read(&path)? == encoded,
                "existing history recovery scheduler record differs from the signed transition"
            );
        }
        Err(error) => return Err(error.error.into()),
    }
    Ok(path)
}

fn validate_content(content: &RecoverySchedulerStateContent) -> Result<()> {
    ensure!(
        content.version == SCHEDULER_STATE_VERSION,
        "unsupported history recovery scheduler state version: {}",
        content.version
    );
    ensure!(
        content.next_attempt_at_unix_seconds >= content.last_observed_unix_seconds,
        "history recovery scheduler deadline precedes its observed time"
    );
    ensure!(
        content
            .next_attempt_at_unix_seconds
            .saturating_sub(content.last_observed_unix_seconds)
            == content.scheduled_delay_seconds,
        "history recovery scheduler delay does not match its signed deadline"
    );
    ensure!(
        content.scheduled_delay_seconds <= MAX_RECOVERY_ATTEMPT_LEASE_SECONDS,
        "history recovery scheduler delay exceeds the supported bound"
    );
    if content.generation == 0 {
        ensure!(
            content.previous_state_id.is_none()
                && content.transition == RecoverySchedulerTransition::Initialized
                && content.lifecycle == RecoverySchedulerLifecycle::Active
                && content.total_attempts == 0
                && content.consecutive_failures == 0
                && content.scheduled_delay_seconds == 0,
            "history recovery scheduler generation zero is not a valid initialization"
        );
    } else {
        ensure!(
            content.previous_state_id.is_some()
                && content.transition != RecoverySchedulerTransition::Initialized,
            "history recovery scheduler successor has invalid chain metadata"
        );
    }
    Ok(())
}

fn validate_transition(
    previous: &SignedRecoverySchedulerState,
    next: &SignedRecoverySchedulerState,
) -> Result<()> {
    match next.transition() {
        RecoverySchedulerTransition::Initialized => {
            bail!("history recovery scheduler cannot initialize twice")
        }
        RecoverySchedulerTransition::AttemptStarted => {
            ensure!(
                previous.lifecycle() == RecoverySchedulerLifecycle::Active
                    && next.lifecycle() == RecoverySchedulerLifecycle::Attempting
                    && next.total_attempts() == previous.total_attempts().saturating_add(1)
                    && next.consecutive_failures() == previous.consecutive_failures()
                    && next.scheduled_delay_seconds() > 0,
                "invalid history recovery scheduler attempt-start transition"
            );
        }
        RecoverySchedulerTransition::AttemptFailed => {
            ensure!(
                previous.lifecycle() == RecoverySchedulerLifecycle::Attempting
                    && next.lifecycle() == RecoverySchedulerLifecycle::Active
                    && next.total_attempts() == previous.total_attempts()
                    && next.consecutive_failures()
                        == previous.consecutive_failures().saturating_add(1),
                "invalid history recovery scheduler failure transition"
            );
            ensure!(
                next.scheduled_delay_seconds() <= MAX_RECOVERY_RETRY_MAX_SECONDS,
                "history recovery scheduler failure delay exceeds retry maximum"
            );
        }
        RecoverySchedulerTransition::AttemptProgressed => {
            ensure!(
                previous.lifecycle() == RecoverySchedulerLifecycle::Attempting
                    && next.lifecycle() == RecoverySchedulerLifecycle::Active
                    && next.total_attempts() == previous.total_attempts()
                    && next.consecutive_failures() == 0,
                "invalid history recovery scheduler progress transition"
            );
            ensure!(
                next.scheduled_delay_seconds() <= MAX_RECOVERY_RETRY_MAX_SECONDS,
                "history recovery scheduler progress delay exceeds retry maximum"
            );
        }
        RecoverySchedulerTransition::Cancelled => {
            ensure!(
                next.lifecycle() == RecoverySchedulerLifecycle::Cancelled
                    && next.total_attempts() == previous.total_attempts()
                    && next.consecutive_failures() == previous.consecutive_failures()
                    && next.scheduled_delay_seconds() == 0,
                "invalid history recovery scheduler cancellation transition"
            );
        }
        RecoverySchedulerTransition::Completed => {
            ensure!(
                next.lifecycle() == RecoverySchedulerLifecycle::Completed
                    && next.total_attempts() == previous.total_attempts()
                    && next.consecutive_failures() == previous.consecutive_failures()
                    && next.scheduled_delay_seconds() == 0,
                "invalid history recovery scheduler completion transition"
            );
        }
    }
    Ok(())
}

fn recovery_backoff_delay_seconds(
    plan_id: [u8; 32],
    failure_exponent: u32,
    attempt_ordinal: u64,
    backoff: RecoveryBackoffConfig,
) -> u64 {
    if backoff.base_seconds() == 0 {
        return 0;
    }
    let shift = failure_exponent.saturating_sub(1).min(63);
    let exponential = backoff
        .base_seconds()
        .checked_shl(shift)
        .unwrap_or(u64::MAX)
        .min(backoff.max_seconds());
    let floor = exponential.div_ceil(2);
    let span = exponential.saturating_sub(floor).saturating_add(1);
    let mut hasher = blake3::Hasher::new();
    hasher.update(SCHEDULER_JITTER_DOMAIN);
    hasher.update(&plan_id);
    hasher.update(&failure_exponent.to_le_bytes());
    hasher.update(&attempt_ordinal.to_le_bytes());
    let digest = hasher.finalize();
    let mut sample_bytes = [0_u8; 8];
    sample_bytes.copy_from_slice(&digest.as_bytes()[..8]);
    floor.saturating_add(u64::from_le_bytes(sample_bytes) % span)
}

fn signing_bytes(content: &RecoverySchedulerStateContent) -> Result<Vec<u8>> {
    let encoded = postcard::to_allocvec(content).context("encode scheduler state content")?;
    let mut bytes = Vec::with_capacity(SCHEDULER_STATE_SIGNATURE_DOMAIN.len() + encoded.len());
    bytes.extend_from_slice(SCHEDULER_STATE_SIGNATURE_DOMAIN);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn scheduler_plan_directory(state_dir: &Path, plan_id: [u8; 32]) -> PathBuf {
    state_dir
        .join("history-recovery")
        .join(SCHEDULER_DIRECTORY)
        .join(encode_hex(&plan_id))
}

fn scheduler_state_file_name(state: &SignedRecoverySchedulerState) -> Result<String> {
    Ok(format!(
        "{:020}-{}.{}",
        state.generation(),
        encode_hex(&state.state_id()?),
        SCHEDULER_STATE_EXTENSION
    ))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_scheduler_chain_survives_restart_and_rejects_forks() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let identity = DeviceIdentity::generate()?;
        let plan_id = [7_u8; 32];
        let backoff = RecoveryBackoffConfig::new(10, 300)?;
        let initialized = SignedRecoverySchedulerState::initialize(&identity, plan_id, 1_000)?;
        persist_recovery_scheduler_state(directory.path(), &initialized)?;
        let started = initialized.start_attempt(&identity, 1_000, 120)?;
        assert!(matches!(
            started.readiness(1_119),
            RecoverySchedulerReadiness::Deferred { .. }
        ));
        assert_eq!(
            started.readiness(1_120),
            RecoverySchedulerReadiness::AttemptLeaseExpired
        );
        persist_recovery_scheduler_state(directory.path(), &started)?;
        let failed = started.record_failure(&identity, 1_010, backoff)?;
        persist_recovery_scheduler_state(directory.path(), &failed)?;

        let resumed =
            load_recovery_scheduler_state(directory.path(), plan_id, identity.device_id())?
                .context("scheduler state did not survive restart")?;
        assert_eq!(resumed, failed);
        assert!(matches!(
            resumed.readiness(999),
            RecoverySchedulerReadiness::ClockRollback { .. }
        ));
        assert!(matches!(
            resumed.readiness(1_010),
            RecoverySchedulerReadiness::Deferred { .. }
        ));
        assert_eq!(
            resumed.readiness(resumed.next_attempt_at_unix_seconds()),
            RecoverySchedulerReadiness::Ready
        );

        let fork = started.record_failure(&identity, 1_011, backoff)?;
        assert!(persist_recovery_scheduler_state(directory.path(), &fork).is_err());
        Ok(())
    }

    #[test]
    fn backoff_is_bounded_jittered_and_cancellation_is_terminal() -> Result<()> {
        let identity = DeviceIdentity::generate()?;
        let plan_id = [9_u8; 32];
        let backoff = RecoveryBackoffConfig::new(8, 40)?;
        let mut state = SignedRecoverySchedulerState::initialize(&identity, plan_id, 2_000)?;
        let mut delays = Vec::new();
        for attempt in 0_u64..5 {
            let now = state
                .next_attempt_at_unix_seconds()
                .max(2_000 + attempt * 100);
            state = state.start_attempt(&identity, now, 120)?;
            state = state.record_failure(&identity, now + 1, backoff)?;
            delays.push(state.scheduled_delay_seconds());
        }
        assert!((4..=8).contains(&delays[0]));
        assert!((8..=16).contains(&delays[1]));
        assert!((16..=32).contains(&delays[2]));
        assert!((20..=40).contains(&delays[3]));
        assert!((20..=40).contains(&delays[4]));
        assert!(delays.windows(2).any(|pair| pair[0] != pair[1]));

        let cancelled = state.cancel(&identity, 1_000)?;
        assert_eq!(cancelled.lifecycle(), RecoverySchedulerLifecycle::Cancelled);
        assert_eq!(
            cancelled.readiness(10_000),
            RecoverySchedulerReadiness::Cancelled
        );
        assert!(cancelled.start_attempt(&identity, 10_000, 120).is_err());
        Ok(())
    }
}
