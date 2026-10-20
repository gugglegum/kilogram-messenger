use std::{future::Future, time::Duration};

use iroh::{Endpoint, EndpointAddr};
use kilogram_identity::{AccountId, DeviceId};
use thiserror::Error;

use super::{
    HistoryRecoveryTransferAttemptFailureClass, HistoryRecoveryTransferExecutionError,
    HistoryRecoveryTransferExecutionOutcome, HistoryRecoveryTransferExecutionPolicy,
    HistoryRecoveryTransferOutbox, HistoryRecoveryTransferOutboxError,
    HistoryRecoveryTransferOutboxStatus, endpoint_bytes, execute_pending_history_recovery_transfer,
};

pub const MAX_HISTORY_RECOVERY_SCHEDULE_SOURCES: usize = 8;
pub const MAX_HISTORY_RECOVERY_SCHEDULE_ROUNDS: u8 = 8;
pub const MAX_HISTORY_RECOVERY_SCHEDULE_TOTAL_ATTEMPTS: u8 = 64;

const DEFAULT_SCHEDULE_ROUNDS: u8 = 3;
const DEFAULT_SCHEDULE_TOTAL_ATTEMPTS: u8 = 24;
const DEFAULT_SCHEDULE_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// A finite caller-selected schedule. One attempt per pending source is made in
/// input order in each round; there is no timer, backoff or persisted worker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryMultiSourceSchedulePolicy {
    max_rounds: u8,
    max_total_attempts: u8,
    connect_timeout: Duration,
}

impl HistoryRecoveryMultiSourceSchedulePolicy {
    pub fn new(
        max_rounds: u8,
        max_total_attempts: u8,
        connect_timeout: Duration,
    ) -> Result<Self, HistoryRecoveryMultiSourceSchedulePolicyError> {
        if max_rounds == 0 || max_rounds > MAX_HISTORY_RECOVERY_SCHEDULE_ROUNDS {
            return Err(HistoryRecoveryMultiSourceSchedulePolicyError::Rounds(
                max_rounds,
            ));
        }
        if max_total_attempts == 0
            || max_total_attempts > MAX_HISTORY_RECOVERY_SCHEDULE_TOTAL_ATTEMPTS
        {
            return Err(
                HistoryRecoveryMultiSourceSchedulePolicyError::TotalAttempts(max_total_attempts),
            );
        }
        if connect_timeout.is_zero() {
            return Err(HistoryRecoveryMultiSourceSchedulePolicyError::ConnectTimeout);
        }
        Ok(Self {
            max_rounds,
            max_total_attempts,
            connect_timeout,
        })
    }

    pub fn max_rounds(self) -> u8 {
        self.max_rounds
    }

    pub fn max_total_attempts(self) -> u8 {
        self.max_total_attempts
    }

    pub fn connect_timeout(self) -> Duration {
        self.connect_timeout
    }
}

impl Default for HistoryRecoveryMultiSourceSchedulePolicy {
    fn default() -> Self {
        Self {
            max_rounds: DEFAULT_SCHEDULE_ROUNDS,
            max_total_attempts: DEFAULT_SCHEDULE_TOTAL_ATTEMPTS,
            connect_timeout: DEFAULT_SCHEDULE_CONNECT_TIMEOUT,
        }
    }
}

/// One already durable source-side intent. The scheduler neither creates nor
/// modifies this request and does not discover alternate targets.
pub struct HistoryRecoveryScheduledSourceTransfer<'a> {
    endpoint: &'a Endpoint,
    target: EndpointAddr,
    outbox: &'a HistoryRecoveryTransferOutbox,
    transfer_id: [u8; 32],
}

impl<'a> HistoryRecoveryScheduledSourceTransfer<'a> {
    pub fn new(
        endpoint: &'a Endpoint,
        target: EndpointAddr,
        outbox: &'a HistoryRecoveryTransferOutbox,
        transfer_id: [u8; 32],
    ) -> Self {
        Self {
            endpoint,
            target,
            outbox,
            transfer_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryMultiSourceScheduleSourceOutcome {
    Pending,
    AcknowledgedWithoutNetwork,
    AcknowledgedAfterNetwork,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryMultiSourceScheduleSourceView {
    pub source_index: u8,
    pub attempts: u8,
    pub outcome: HistoryRecoveryMultiSourceScheduleSourceOutcome,
    pub last_failure: Option<HistoryRecoveryTransferAttemptFailureClass>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryMultiSourceScheduleTermination {
    AllAcknowledged,
    RoundBudgetConsumed,
    TotalAttemptBudgetConsumed,
}

/// Privacy-safe evidence for one explicit invocation. `source_index` is only
/// the ordinal in the caller-supplied slice; no durable or network identity is
/// copied into this report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryMultiSourceScheduleReport {
    pub sources: Vec<HistoryRecoveryMultiSourceScheduleSourceView>,
    pub rounds_started: u8,
    pub total_attempts: u8,
    pub acknowledged_without_network: u8,
    pub acknowledged_after_network: u8,
    pub pending: u8,
    pub termination: HistoryRecoveryMultiSourceScheduleTermination,
    pub maximum_in_flight: u8,
    pub deterministic_round_robin: bool,
    pub contains_identity: bool,
    pub contains_network_coordinates: bool,
    pub contains_message_bodies: bool,
    pub grants_import_authority: bool,
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryMultiSourceSchedulePolicyError {
    #[error(
        "history recovery schedule rounds must be in 1..={MAX_HISTORY_RECOVERY_SCHEDULE_ROUNDS}, got {0}"
    )]
    Rounds(u8),
    #[error(
        "history recovery schedule total attempts must be in 1..={MAX_HISTORY_RECOVERY_SCHEDULE_TOTAL_ATTEMPTS}, got {0}"
    )]
    TotalAttempts(u8),
    #[error("history recovery schedule connect timeout must be nonzero")]
    ConnectTimeout,
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryMultiSourceScheduleError {
    #[error(
        "history recovery schedule source count must be in 1..={MAX_HISTORY_RECOVERY_SCHEDULE_SOURCES}, got {0}"
    )]
    SourceCount(usize),
    #[error("history recovery schedule source index {source_index} has no durable intent")]
    IntentNotPrepared { source_index: u8 },
    #[error("history recovery schedule contains duplicate source Devices")]
    DuplicateSource,
    #[error("history recovery schedule contains a duplicate transfer intent")]
    DuplicateTransfer,
    #[error("history recovery schedule source index {source_index} belongs to another Account")]
    AccountMismatch { source_index: u8 },
    #[error("history recovery schedule source index {source_index} targets another recipient")]
    RecipientMismatch { source_index: u8 },
    #[error("history recovery schedule source index {source_index} has the wrong local Endpoint")]
    SourceEndpointBinding { source_index: u8 },
    #[error("history recovery schedule source index {source_index} has the wrong target Endpoint")]
    RecipientEndpointBinding { source_index: u8 },
    #[error("history recovery schedule source index {source_index} outbox failed: {source}")]
    Outbox {
        source_index: u8,
        #[source]
        source: HistoryRecoveryTransferOutboxError,
    },
    #[error("history recovery schedule source index {source_index} execution failed: {source}")]
    Execution {
        source_index: u8,
        #[source]
        source: HistoryRecoveryTransferExecutionError,
    },
    #[error("validated history recovery schedule could not construct its one-attempt policy")]
    PolicyInvariant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduledAttemptOutcome {
    AcknowledgedWithoutNetwork,
    AcknowledgedAfterNetwork,
    Retryable(HistoryRecoveryTransferAttemptFailureClass),
}

/// Executes a bounded fair round over distinct source-Device intents.
///
/// All candidates are fully preflighted before the first network attempt. A
/// retryable failure leaves the exact durable request pending and yields to the
/// next source. Unexpected local/outbox failures stop the invocation; earlier
/// receipts, if any, remain durable and make a later invocation idempotent.
pub async fn schedule_pending_history_recovery_transfers(
    candidates: &[HistoryRecoveryScheduledSourceTransfer<'_>],
    policy: HistoryRecoveryMultiSourceSchedulePolicy,
) -> Result<HistoryRecoveryMultiSourceScheduleReport, HistoryRecoveryMultiSourceScheduleError> {
    let initially_acknowledged = preflight_candidates(candidates)?;
    let execution_policy = HistoryRecoveryTransferExecutionPolicy::new(1, policy.connect_timeout())
        .map_err(|_| HistoryRecoveryMultiSourceScheduleError::PolicyInvariant)?;
    run_bounded_schedule(
        candidates.len(),
        &initially_acknowledged,
        policy,
        |source_index| {
            let candidate = &candidates[source_index];
            let endpoint = candidate.endpoint;
            let target = candidate.target.clone();
            let outbox = candidate.outbox;
            let transfer_id = candidate.transfer_id;
            async move {
                match execute_pending_history_recovery_transfer(
                    endpoint,
                    target,
                    outbox,
                    transfer_id,
                    execution_policy,
                )
                .await
                {
                    Ok(
                        HistoryRecoveryTransferExecutionOutcome::AlreadyAcknowledgedWithoutNetwork(
                            _,
                        ),
                    ) => Ok(ScheduledAttemptOutcome::AcknowledgedWithoutNetwork),
                    Ok(HistoryRecoveryTransferExecutionOutcome::AcknowledgedAfterNetwork {
                        ..
                    }) => Ok(ScheduledAttemptOutcome::AcknowledgedAfterNetwork),
                    Err(HistoryRecoveryTransferExecutionError::AttemptsExhausted {
                        last_class,
                        ..
                    }) => Ok(ScheduledAttemptOutcome::Retryable(last_class)),
                    Err(error) => Err(error),
                }
            }
        },
    )
    .await
}

fn preflight_candidates(
    candidates: &[HistoryRecoveryScheduledSourceTransfer<'_>],
) -> Result<Vec<bool>, HistoryRecoveryMultiSourceScheduleError> {
    if candidates.is_empty() || candidates.len() > MAX_HISTORY_RECOVERY_SCHEDULE_SOURCES {
        return Err(HistoryRecoveryMultiSourceScheduleError::SourceCount(
            candidates.len(),
        ));
    }
    let mut owner_account_id: Option<AccountId> = None;
    let mut recipient_device_id: Option<DeviceId> = None;
    let mut source_device_ids = Vec::with_capacity(candidates.len());
    let mut transfer_ids = Vec::with_capacity(candidates.len());
    let mut initially_acknowledged = Vec::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        let source_index = u8::try_from(index)
            .map_err(|_| HistoryRecoveryMultiSourceScheduleError::SourceCount(candidates.len()))?;
        let prepared = candidate
            .outbox
            .load(candidate.transfer_id)
            .map_err(|source| HistoryRecoveryMultiSourceScheduleError::Outbox {
                source_index,
                source,
            })?
            .ok_or(HistoryRecoveryMultiSourceScheduleError::IntentNotPrepared { source_index })?;
        if prepared.source_endpoint_id() != endpoint_bytes(candidate.endpoint.id()) {
            return Err(
                HistoryRecoveryMultiSourceScheduleError::SourceEndpointBinding { source_index },
            );
        }
        if prepared.recipient_endpoint_id() != endpoint_bytes(candidate.target.id) {
            return Err(
                HistoryRecoveryMultiSourceScheduleError::RecipientEndpointBinding { source_index },
            );
        }
        if owner_account_id.is_some_and(|owner| owner != prepared.owner_account_id()) {
            return Err(HistoryRecoveryMultiSourceScheduleError::AccountMismatch { source_index });
        }
        if recipient_device_id.is_some_and(|recipient| recipient != prepared.recipient_device_id())
        {
            return Err(HistoryRecoveryMultiSourceScheduleError::RecipientMismatch {
                source_index,
            });
        }
        if source_device_ids.contains(&prepared.source_device_id()) {
            return Err(HistoryRecoveryMultiSourceScheduleError::DuplicateSource);
        }
        if transfer_ids.contains(&prepared.transfer_id()) {
            return Err(HistoryRecoveryMultiSourceScheduleError::DuplicateTransfer);
        }
        owner_account_id = Some(prepared.owner_account_id());
        recipient_device_id = Some(prepared.recipient_device_id());
        source_device_ids.push(prepared.source_device_id());
        transfer_ids.push(prepared.transfer_id());
        let acknowledged =
            match candidate
                .outbox
                .status(candidate.transfer_id)
                .map_err(|source| HistoryRecoveryMultiSourceScheduleError::Outbox {
                    source_index,
                    source,
                })? {
                HistoryRecoveryTransferOutboxStatus::Absent => {
                    return Err(HistoryRecoveryMultiSourceScheduleError::IntentNotPrepared {
                        source_index,
                    });
                }
                HistoryRecoveryTransferOutboxStatus::Pending(_) => false,
                HistoryRecoveryTransferOutboxStatus::Acknowledged(_) => true,
            };
        initially_acknowledged.push(acknowledged);
    }
    Ok(initially_acknowledged)
}

async fn run_bounded_schedule<Attempt, AttemptFuture>(
    source_count: usize,
    initially_acknowledged: &[bool],
    policy: HistoryRecoveryMultiSourceSchedulePolicy,
    mut attempt: Attempt,
) -> Result<HistoryRecoveryMultiSourceScheduleReport, HistoryRecoveryMultiSourceScheduleError>
where
    Attempt: FnMut(usize) -> AttemptFuture,
    AttemptFuture:
        Future<Output = Result<ScheduledAttemptOutcome, HistoryRecoveryTransferExecutionError>>,
{
    if initially_acknowledged.len() != source_count {
        return Err(HistoryRecoveryMultiSourceScheduleError::SourceCount(
            source_count,
        ));
    }
    let mut sources = Vec::with_capacity(source_count);
    for (index, acknowledged) in initially_acknowledged.iter().copied().enumerate() {
        let source_index = u8::try_from(index)
            .map_err(|_| HistoryRecoveryMultiSourceScheduleError::SourceCount(source_count))?;
        sources.push(HistoryRecoveryMultiSourceScheduleSourceView {
            source_index,
            attempts: 0,
            outcome: if acknowledged {
                HistoryRecoveryMultiSourceScheduleSourceOutcome::AcknowledgedWithoutNetwork
            } else {
                HistoryRecoveryMultiSourceScheduleSourceOutcome::Pending
            },
            last_failure: None,
        });
    }
    let mut rounds_started = 0_u8;
    let mut total_attempts = 0_u8;
    let mut termination = HistoryRecoveryMultiSourceScheduleTermination::AllAcknowledged;

    'rounds: for _ in 0..policy.max_rounds() {
        if all_acknowledged(&sources) {
            break;
        }
        let pending_in_round = sources
            .iter()
            .filter(|source| {
                source.outcome == HistoryRecoveryMultiSourceScheduleSourceOutcome::Pending
            })
            .count() as u8;
        let remaining_attempts = policy.max_total_attempts().saturating_sub(total_attempts);
        if remaining_attempts < pending_in_round {
            termination = HistoryRecoveryMultiSourceScheduleTermination::TotalAttemptBudgetConsumed;
            break;
        }
        rounds_started = rounds_started.saturating_add(1);
        for (source_index, source) in sources.iter_mut().enumerate() {
            if source.outcome != HistoryRecoveryMultiSourceScheduleSourceOutcome::Pending {
                continue;
            }
            if total_attempts == policy.max_total_attempts() {
                termination =
                    HistoryRecoveryMultiSourceScheduleTermination::TotalAttemptBudgetConsumed;
                break 'rounds;
            }
            source.attempts = source.attempts.saturating_add(1);
            total_attempts = total_attempts.saturating_add(1);
            match attempt(source_index).await {
                Ok(ScheduledAttemptOutcome::AcknowledgedWithoutNetwork) => {
                    source.outcome =
                        HistoryRecoveryMultiSourceScheduleSourceOutcome::AcknowledgedWithoutNetwork;
                    source.last_failure = None;
                }
                Ok(ScheduledAttemptOutcome::AcknowledgedAfterNetwork) => {
                    source.outcome =
                        HistoryRecoveryMultiSourceScheduleSourceOutcome::AcknowledgedAfterNetwork;
                    source.last_failure = None;
                }
                Ok(ScheduledAttemptOutcome::Retryable(failure)) => {
                    source.last_failure = Some(failure);
                }
                Err(error) => {
                    return Err(HistoryRecoveryMultiSourceScheduleError::Execution {
                        source_index: source.source_index,
                        source: error,
                    });
                }
            }
        }
    }
    if all_acknowledged(&sources) {
        termination = HistoryRecoveryMultiSourceScheduleTermination::AllAcknowledged;
    } else if termination
        != HistoryRecoveryMultiSourceScheduleTermination::TotalAttemptBudgetConsumed
    {
        termination = HistoryRecoveryMultiSourceScheduleTermination::RoundBudgetConsumed;
    }
    Ok(build_report(
        sources,
        rounds_started,
        total_attempts,
        termination,
    ))
}

fn all_acknowledged(sources: &[HistoryRecoveryMultiSourceScheduleSourceView]) -> bool {
    sources
        .iter()
        .all(|source| source.outcome != HistoryRecoveryMultiSourceScheduleSourceOutcome::Pending)
}

fn build_report(
    sources: Vec<HistoryRecoveryMultiSourceScheduleSourceView>,
    rounds_started: u8,
    total_attempts: u8,
    termination: HistoryRecoveryMultiSourceScheduleTermination,
) -> HistoryRecoveryMultiSourceScheduleReport {
    let acknowledged_without_network = sources
        .iter()
        .filter(|source| {
            source.outcome
                == HistoryRecoveryMultiSourceScheduleSourceOutcome::AcknowledgedWithoutNetwork
        })
        .count() as u8;
    let acknowledged_after_network = sources
        .iter()
        .filter(|source| {
            source.outcome
                == HistoryRecoveryMultiSourceScheduleSourceOutcome::AcknowledgedAfterNetwork
        })
        .count() as u8;
    let pending = sources
        .iter()
        .filter(|source| source.outcome == HistoryRecoveryMultiSourceScheduleSourceOutcome::Pending)
        .count() as u8;
    HistoryRecoveryMultiSourceScheduleReport {
        sources,
        rounds_started,
        total_attempts,
        acknowledged_without_network,
        acknowledged_after_network,
        pending,
        termination,
        maximum_in_flight: 1,
        deterministic_round_robin: true,
        contains_identity: false,
        contains_network_coordinates: false,
        contains_message_bodies: false,
        grants_import_authority: false,
    }
}

#[cfg(test)]
mod tests {
    use std::{future::ready, io};

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroh::endpoint::presets;
    use kilogram_identity::DeviceIdentity;
    use kilogram_peer_history_recovery_execution::HistoryRecoveryTransferEnvelope;

    use super::*;

    #[test]
    fn schedule_policy_is_strictly_bounded() -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            HistoryRecoveryMultiSourceSchedulePolicy::new(0, 1, Duration::from_secs(1)).is_err()
        );
        assert!(
            HistoryRecoveryMultiSourceSchedulePolicy::new(
                MAX_HISTORY_RECOVERY_SCHEDULE_ROUNDS + 1,
                1,
                Duration::from_secs(1)
            )
            .is_err()
        );
        assert!(
            HistoryRecoveryMultiSourceSchedulePolicy::new(1, 0, Duration::from_secs(1)).is_err()
        );
        assert!(
            HistoryRecoveryMultiSourceSchedulePolicy::new(
                1,
                MAX_HISTORY_RECOVERY_SCHEDULE_TOTAL_ATTEMPTS + 1,
                Duration::from_secs(1)
            )
            .is_err()
        );
        assert!(HistoryRecoveryMultiSourceSchedulePolicy::new(1, 1, Duration::ZERO).is_err());
        let policy = HistoryRecoveryMultiSourceSchedulePolicy::new(2, 7, Duration::from_secs(11))?;
        assert_eq!(policy.max_rounds(), 2);
        assert_eq!(policy.max_total_attempts(), 7);
        assert_eq!(policy.connect_timeout(), Duration::from_secs(11));
        Ok(())
    }

    #[tokio::test]
    async fn round_robin_yields_after_failure_and_reaches_all_sources()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = HistoryRecoveryMultiSourceSchedulePolicy::new(3, 8, Duration::from_secs(1))?;
        let mut calls = [0_u8; 3];
        let mut order = Vec::new();
        let report = run_bounded_schedule(3, &[false, false, false], policy, |source_index| {
            calls[source_index] = calls[source_index].saturating_add(1);
            order.push(source_index);
            let outcome = match (source_index, calls[source_index]) {
                (0, 1) => ScheduledAttemptOutcome::Retryable(
                    HistoryRecoveryTransferAttemptFailureClass::PreConnect,
                ),
                (2, 1 | 2) => ScheduledAttemptOutcome::Retryable(
                    HistoryRecoveryTransferAttemptFailureClass::PostConnectAmbiguous,
                ),
                _ => ScheduledAttemptOutcome::AcknowledgedAfterNetwork,
            };
            ready(Ok(outcome))
        })
        .await?;
        assert_eq!(order, vec![0, 1, 2, 0, 2, 2]);
        assert_eq!(report.rounds_started, 3);
        assert_eq!(report.total_attempts, 6);
        assert_eq!(report.acknowledged_after_network, 3);
        assert_eq!(report.pending, 0);
        assert_eq!(
            report.termination,
            HistoryRecoveryMultiSourceScheduleTermination::AllAcknowledged
        );
        assert_eq!(
            report
                .sources
                .iter()
                .map(|source| source.attempts)
                .collect::<Vec<_>>(),
            vec![2, 1, 3]
        );
        assert_eq!(report.maximum_in_flight, 1);
        assert!(report.deterministic_round_robin);
        assert!(!report.contains_identity);
        assert!(!report.contains_network_coordinates);
        assert!(!report.contains_message_bodies);
        assert!(!report.grants_import_authority);
        Ok(())
    }

    #[tokio::test]
    async fn total_attempt_budget_never_starts_a_partial_round()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = HistoryRecoveryMultiSourceSchedulePolicy::new(
            MAX_HISTORY_RECOVERY_SCHEDULE_ROUNDS,
            4,
            Duration::from_secs(1),
        )?;
        let mut order = Vec::new();
        let report = run_bounded_schedule(3, &[false, false, false], policy, |source_index| {
            order.push(source_index);
            ready(Ok(if source_index == 0 {
                ScheduledAttemptOutcome::AcknowledgedAfterNetwork
            } else {
                ScheduledAttemptOutcome::Retryable(
                    HistoryRecoveryTransferAttemptFailureClass::PreConnect,
                )
            }))
        })
        .await?;
        assert_eq!(order, vec![0, 1, 2]);
        assert_eq!(report.rounds_started, 1);
        assert_eq!(report.total_attempts, 3);
        assert_eq!(report.pending, 2);
        assert_eq!(
            report.termination,
            HistoryRecoveryMultiSourceScheduleTermination::TotalAttemptBudgetConsumed
        );
        assert_eq!(report.sources[1].attempts, 1);
        assert_eq!(report.sources[2].attempts, 1);
        Ok(())
    }

    #[tokio::test]
    async fn acknowledged_sources_are_skipped_without_attempts()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = HistoryRecoveryMultiSourceSchedulePolicy::new(1, 3, Duration::from_secs(1))?;
        let mut order = Vec::new();
        let report = run_bounded_schedule(3, &[true, false, true], policy, |source_index| {
            order.push(source_index);
            ready(Ok(ScheduledAttemptOutcome::AcknowledgedAfterNetwork))
        })
        .await?;
        assert_eq!(order, vec![1]);
        assert_eq!(report.acknowledged_without_network, 2);
        assert_eq!(report.acknowledged_after_network, 1);
        assert_eq!(report.total_attempts, 1);
        assert_eq!(report.sources[0].attempts, 0);
        assert_eq!(report.sources[2].attempts, 0);
        Ok(())
    }

    #[tokio::test]
    async fn round_budget_leaves_exact_sources_pending_for_next_invocation()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = HistoryRecoveryMultiSourceSchedulePolicy::new(2, 8, Duration::from_secs(1))?;
        let mut order = Vec::new();
        let report = run_bounded_schedule(2, &[false, false], policy, |source_index| {
            order.push(source_index);
            ready(Ok(ScheduledAttemptOutcome::Retryable(
                HistoryRecoveryTransferAttemptFailureClass::PostConnectAmbiguous,
            )))
        })
        .await?;
        assert_eq!(order, vec![0, 1, 0, 1]);
        assert_eq!(report.rounds_started, 2);
        assert_eq!(report.total_attempts, 4);
        assert_eq!(report.pending, 2);
        assert_eq!(
            report.termination,
            HistoryRecoveryMultiSourceScheduleTermination::RoundBudgetConsumed
        );
        assert!(report.sources.iter().all(|source| {
            source.attempts == 2
                && source.last_failure
                    == Some(HistoryRecoveryTransferAttemptFailureClass::PostConnectAmbiguous)
        }));
        Ok(())
    }

    #[tokio::test]
    async fn empty_and_fatal_schedules_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let empty: Vec<HistoryRecoveryScheduledSourceTransfer<'_>> = Vec::new();
        assert!(matches!(
            schedule_pending_history_recovery_transfers(
                &empty,
                HistoryRecoveryMultiSourceSchedulePolicy::default()
            )
            .await,
            Err(HistoryRecoveryMultiSourceScheduleError::SourceCount(0))
        ));

        let policy = HistoryRecoveryMultiSourceSchedulePolicy::new(1, 2, Duration::from_secs(1))?;
        let result = run_bounded_schedule(2, &[false, false], policy, |source_index| {
            ready(if source_index == 0 {
                Ok(ScheduledAttemptOutcome::Retryable(
                    HistoryRecoveryTransferAttemptFailureClass::PreConnect,
                ))
            } else {
                Err(HistoryRecoveryTransferExecutionError::IntentNotPrepared)
            })
        })
        .await;
        assert!(matches!(
            result,
            Err(HistoryRecoveryMultiSourceScheduleError::Execution {
                source_index: 1,
                source: HistoryRecoveryTransferExecutionError::IntentNotPrepared,
            })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn candidate_preflight_rejects_endpoint_and_duplicate_source_before_network()
    -> Result<(), Box<dyn std::error::Error>> {
        let source_endpoint = test_endpoint().await?;
        let recipient_endpoint = test_endpoint().await?;
        let foreign_endpoint = test_endpoint().await?;
        let source = source_identity()?;
        let envelope_bytes = fixture_envelope()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?;
        let state = tempfile::tempdir()?;
        let outbox = HistoryRecoveryTransferOutbox::open(
            state.path(),
            envelope.owner_account_id(),
            source.device_id(),
        )?;
        let prepared = match outbox.prepare(
            &envelope_bytes,
            &source,
            source_endpoint.id(),
            recipient_endpoint.id(),
            [0x79; 32],
        )? {
            super::super::PrepareHistoryRecoveryTransferOutcome::Prepared(prepared)
            | super::super::PrepareHistoryRecoveryTransferOutcome::AlreadyPrepared(prepared) => {
                prepared
            }
        };
        let target = EndpointAddr::new(recipient_endpoint.id());
        let wrong_endpoint = [HistoryRecoveryScheduledSourceTransfer::new(
            &foreign_endpoint,
            target.clone(),
            &outbox,
            prepared.transfer_id(),
        )];
        assert!(matches!(
            schedule_pending_history_recovery_transfers(
                &wrong_endpoint,
                HistoryRecoveryMultiSourceSchedulePolicy::default()
            )
            .await,
            Err(HistoryRecoveryMultiSourceScheduleError::SourceEndpointBinding { source_index: 0 })
        ));

        let duplicates = [
            HistoryRecoveryScheduledSourceTransfer::new(
                &source_endpoint,
                target.clone(),
                &outbox,
                prepared.transfer_id(),
            ),
            HistoryRecoveryScheduledSourceTransfer::new(
                &source_endpoint,
                target,
                &outbox,
                prepared.transfer_id(),
            ),
        ];
        assert!(matches!(
            schedule_pending_history_recovery_transfers(
                &duplicates,
                HistoryRecoveryMultiSourceSchedulePolicy::default()
            )
            .await,
            Err(HistoryRecoveryMultiSourceScheduleError::DuplicateSource)
        ));
        source_endpoint.close().await;
        recipient_endpoint.close().await;
        foreign_endpoint.close().await;
        Ok(())
    }

    async fn test_endpoint() -> Result<Endpoint, Box<dyn std::error::Error>> {
        Ok(Endpoint::builder(presets::Minimal)
            .clear_ip_transports()
            .clear_relay_transports()
            .bind_addr("127.0.0.1:0")?
            .bind()
            .await?)
    }

    fn fixture_envelope() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let compact = include_str!("../tests/fixtures/m1222-transfer-envelope.b64")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        Ok(STANDARD.decode(compact)?)
    }

    fn source_identity() -> Result<DeviceIdentity, Box<dyn std::error::Error>> {
        const SOURCE_SECRET_HEX: &str =
            "3fc00be615014da89b58a3a10ceb9b98e369449f6de6c702a521d638935a4e80";
        if SOURCE_SECRET_HEX.len() != 64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid test secret").into());
        }
        let mut secret = [0_u8; 32];
        for (index, byte) in secret.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&SOURCE_SECRET_HEX[index * 2..index * 2 + 2], 16)?;
        }
        Ok(DeviceIdentity::from_secret_bytes(secret))
    }
}
