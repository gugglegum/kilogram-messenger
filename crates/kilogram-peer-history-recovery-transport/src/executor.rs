use std::{fmt, future::Future, time::Duration};

use iroh::{Endpoint, EndpointAddr, EndpointId};
use thiserror::Error;
use tokio::time::timeout;

use super::{
    AcknowledgeHistoryRecoveryTransferOutcome, HISTORY_RECOVERY_TRANSFER_ALPN,
    HistoryRecoveryTransferOutbox, HistoryRecoveryTransferOutboxError,
    HistoryRecoveryTransferOutboxStatus, HistoryRecoveryTransferOutboxStatusView,
    HistoryRecoveryTransferSessionReceipt, PreparedHistoryRecoveryTransferSession,
    SignedHistoryRecoveryTransferSessionReceipt, endpoint_bytes,
    send_prepared_history_recovery_transfer,
};

pub const MAX_HISTORY_RECOVERY_TRANSFER_EXECUTION_ATTEMPTS: u8 = 8;
const DEFAULT_EXECUTION_ATTEMPTS: u8 = 3;
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// A deliberately small, caller-owned retry policy for one already durable
/// transfer intent. It contains no schedule, background authority or backoff
/// state; invoking the executor is always an explicit caller decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryTransferExecutionPolicy {
    max_attempts: u8,
    connect_timeout: Duration,
}

impl HistoryRecoveryTransferExecutionPolicy {
    pub fn new(
        max_attempts: u8,
        connect_timeout: Duration,
    ) -> Result<Self, HistoryRecoveryTransferExecutionPolicyError> {
        if max_attempts == 0 || max_attempts > MAX_HISTORY_RECOVERY_TRANSFER_EXECUTION_ATTEMPTS {
            return Err(HistoryRecoveryTransferExecutionPolicyError::Attempts(
                max_attempts,
            ));
        }
        if connect_timeout.is_zero() {
            return Err(HistoryRecoveryTransferExecutionPolicyError::ConnectTimeout);
        }
        Ok(Self {
            max_attempts,
            connect_timeout,
        })
    }

    pub fn max_attempts(self) -> u8 {
        self.max_attempts
    }

    pub fn connect_timeout(self) -> Duration {
        self.connect_timeout
    }
}

impl Default for HistoryRecoveryTransferExecutionPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_EXECUTION_ATTEMPTS,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryTransferAttemptFailureClass {
    /// No authenticated Iroh connection was established, so no application
    /// request bytes could have reached the recipient through this attempt.
    PreConnect,
    /// An authenticated connection existed. The executor conservatively treats
    /// every later failure as ambiguous, even when a lower layer reports that
    /// it happened while opening the request stream.
    PostConnectAmbiguous,
}

impl fmt::Display for HistoryRecoveryTransferAttemptFailureClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreConnect => formatter.write_str("pre-connect"),
            Self::PostConnectAmbiguous => formatter.write_str("post-connect-ambiguous"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryTransferExecutionOutcome {
    /// A durable acknowledgement already existed before this invocation. No
    /// connect call or application stream was opened.
    AlreadyAcknowledgedWithoutNetwork(HistoryRecoveryTransferOutboxStatusView),
    /// A verified recipient receipt was returned and is now durable. A racing
    /// executor may have persisted the exact same receipt first.
    AcknowledgedAfterNetwork {
        attempts: u8,
        receipt: HistoryRecoveryTransferSessionReceipt,
        receipt_was_already_durable: bool,
    },
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryTransferExecutionPolicyError {
    #[error(
        "history recovery transfer execution attempts must be in 1..={MAX_HISTORY_RECOVERY_TRANSFER_EXECUTION_ATTEMPTS}, got {0}"
    )]
    Attempts(u8),
    #[error("history recovery transfer connect timeout must be nonzero")]
    ConnectTimeout,
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryTransferExecutionError {
    #[error("history recovery transfer intent is not prepared")]
    IntentNotPrepared,
    #[error("local Iroh Endpoint does not match the durable source Endpoint binding")]
    SourceEndpointBinding,
    #[error("target Iroh Endpoint does not match the durable recipient Endpoint binding")]
    RecipientEndpointBinding,
    #[error(
        "history recovery transfer exhausted {attempts} exact attempt(s); last failure was {last_class} at {last_stage}: {last_detail}"
    )]
    AttemptsExhausted {
        attempts: u8,
        last_class: HistoryRecoveryTransferAttemptFailureClass,
        last_stage: &'static str,
        last_detail: String,
    },
    #[error(transparent)]
    Outbox(#[from] HistoryRecoveryTransferOutboxError),
}

#[derive(Debug)]
struct TransferAttemptFailure {
    class: HistoryRecoveryTransferAttemptFailureClass,
    stage: &'static str,
    detail: String,
}

impl TransferAttemptFailure {
    fn pre_connect(stage: &'static str, error: impl fmt::Display) -> Self {
        Self {
            class: HistoryRecoveryTransferAttemptFailureClass::PreConnect,
            stage,
            detail: error.to_string(),
        }
    }

    fn post_connect_ambiguous(stage: &'static str, error: impl fmt::Display) -> Self {
        Self {
            class: HistoryRecoveryTransferAttemptFailureClass::PostConnectAmbiguous,
            stage,
            detail: error.to_string(),
        }
    }
}

/// Explicitly executes one pending durable transfer intent.
///
/// The executor never creates or modifies a request. Every attempt sends the
/// exact source-Device-signed bytes loaded from the outbox, and only the
/// dedicated M1.2.23 ALPN is offered. An exhausted or locally uncommitted
/// attempt leaves the intent pending so a later explicit invocation can safely
/// retry the same nonce, request ID and transfer ID.
pub async fn execute_pending_history_recovery_transfer(
    endpoint: &Endpoint,
    target: EndpointAddr,
    outbox: &HistoryRecoveryTransferOutbox,
    transfer_id: [u8; 32],
    policy: HistoryRecoveryTransferExecutionPolicy,
) -> Result<HistoryRecoveryTransferExecutionOutcome, HistoryRecoveryTransferExecutionError> {
    let source_endpoint_id = endpoint.id();
    let recipient_endpoint_id = target.id;
    execute_pending_with_attempt(
        outbox,
        transfer_id,
        source_endpoint_id,
        recipient_endpoint_id,
        policy,
        |prepared| {
            let target = target.clone();
            async move {
                let connection = match timeout(
                    policy.connect_timeout(),
                    endpoint.connect(target, HISTORY_RECOVERY_TRANSFER_ALPN),
                )
                .await
                {
                    Ok(Ok(connection)) => connection,
                    Ok(Err(error)) => {
                        return Err(TransferAttemptFailure::pre_connect(
                            "connect authenticated recipient",
                            error,
                        ));
                    }
                    Err(error) => {
                        return Err(TransferAttemptFailure::pre_connect(
                            "connect authenticated recipient",
                            error,
                        ));
                    }
                };
                send_prepared_history_recovery_transfer(&connection, &prepared)
                    .await
                    .map_err(|error| {
                        TransferAttemptFailure::post_connect_ambiguous(
                            "exchange exact signed request and receipt",
                            error,
                        )
                    })
            }
        },
    )
    .await
}

async fn execute_pending_with_attempt<F, Fut>(
    outbox: &HistoryRecoveryTransferOutbox,
    transfer_id: [u8; 32],
    source_endpoint_id: EndpointId,
    recipient_endpoint_id: EndpointId,
    policy: HistoryRecoveryTransferExecutionPolicy,
    mut attempt: F,
) -> Result<HistoryRecoveryTransferExecutionOutcome, HistoryRecoveryTransferExecutionError>
where
    F: FnMut(PreparedHistoryRecoveryTransferSession) -> Fut,
    Fut: Future<
        Output = Result<SignedHistoryRecoveryTransferSessionReceipt, TransferAttemptFailure>,
    >,
{
    let prepared = outbox
        .load(transfer_id)?
        .ok_or(HistoryRecoveryTransferExecutionError::IntentNotPrepared)?;
    if prepared.source_endpoint_id() != endpoint_bytes(source_endpoint_id) {
        return Err(HistoryRecoveryTransferExecutionError::SourceEndpointBinding);
    }
    if prepared.recipient_endpoint_id() != endpoint_bytes(recipient_endpoint_id) {
        return Err(HistoryRecoveryTransferExecutionError::RecipientEndpointBinding);
    }
    match outbox.status(transfer_id)? {
        HistoryRecoveryTransferOutboxStatus::Absent => {
            return Err(HistoryRecoveryTransferExecutionError::IntentNotPrepared);
        }
        HistoryRecoveryTransferOutboxStatus::Acknowledged(status) => {
            return Ok(
                HistoryRecoveryTransferExecutionOutcome::AlreadyAcknowledgedWithoutNetwork(status),
            );
        }
        HistoryRecoveryTransferOutboxStatus::Pending(_) => {}
    }

    for attempt_number in 1..=policy.max_attempts() {
        match attempt(prepared.clone()).await {
            Ok(receipt) => {
                let summary = receipt.summary();
                let receipt_was_already_durable = matches!(
                    outbox.acknowledge(transfer_id, receipt.encoded_receipt())?,
                    AcknowledgeHistoryRecoveryTransferOutcome::AlreadyAcknowledged(_)
                );
                return Ok(
                    HistoryRecoveryTransferExecutionOutcome::AcknowledgedAfterNetwork {
                        attempts: attempt_number,
                        receipt: summary,
                        receipt_was_already_durable,
                    },
                );
            }
            Err(failure) if attempt_number == policy.max_attempts() => {
                return Err(HistoryRecoveryTransferExecutionError::AttemptsExhausted {
                    attempts: attempt_number,
                    last_class: failure.class,
                    last_stage: failure.stage,
                    last_detail: failure.detail,
                });
            }
            Err(_) => {}
        }
    }
    unreachable!("validated history recovery execution policy always has at least one attempt")
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroh::{SecretKey, endpoint::presets};
    use kilogram_identity::DeviceIdentity;
    use kilogram_peer_history_recovery_execution::{
        HistoryRecoveryTransferEnvelope, RecipientHistoryTransferInbox,
    };

    use super::*;
    use crate::{
        TransferStageDisposition, VerifiedTransferSessionResponse,
        receive_history_recovery_transfer,
    };

    const SOURCE_SECRET_HEX: &str =
        "3fc00be615014da89b58a3a10ceb9b98e369449f6de6c702a521d638935a4e80";
    const RECIPIENT_SECRET_HEX: &str =
        "57d154005415eb97298e29fea08e518d6a35238dc9aa32aa688e401f526efcd8";

    struct PreparedFixture {
        outbox: HistoryRecoveryTransferOutbox,
        _state: tempfile::TempDir,
        prepared: PreparedHistoryRecoveryTransferSession,
        source_endpoint_id: EndpointId,
        recipient_endpoint_id: EndpointId,
    }

    #[test]
    fn execution_policy_is_strictly_bounded() -> Result<(), Box<dyn std::error::Error>> {
        assert!(HistoryRecoveryTransferExecutionPolicy::new(0, Duration::from_secs(1)).is_err());
        assert!(
            HistoryRecoveryTransferExecutionPolicy::new(
                MAX_HISTORY_RECOVERY_TRANSFER_EXECUTION_ATTEMPTS + 1,
                Duration::from_secs(1),
            )
            .is_err()
        );
        assert!(HistoryRecoveryTransferExecutionPolicy::new(1, Duration::ZERO).is_err());
        assert_eq!(
            HistoryRecoveryTransferExecutionPolicy::new(2, Duration::from_secs(7))?.max_attempts(),
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn retries_exact_bytes_then_commits_verified_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = prepared_fixture(
            SecretKey::generate().public(),
            SecretKey::generate().public(),
        )?;
        let transfer_id = fixture.prepared.transfer_id();
        let expected = fixture.prepared.encoded_request().to_vec();
        let receipt = staged_receipt(&fixture.prepared)?;
        let observed = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_for_attempt = Arc::clone(&observed);
        let calls_for_attempt = Arc::clone(&calls);
        let outcome = execute_pending_with_attempt(
            &fixture.outbox,
            transfer_id,
            fixture.source_endpoint_id,
            fixture.recipient_endpoint_id,
            HistoryRecoveryTransferExecutionPolicy::new(3, Duration::from_secs(1))?,
            move |prepared| {
                let receipt = receipt.clone();
                let observed = Arc::clone(&observed_for_attempt);
                let attempt = calls_for_attempt.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    observed
                        .lock()
                        .map_err(|_| {
                            TransferAttemptFailure::post_connect_ambiguous(
                                "record exact request",
                                "observed requests mutex poisoned",
                            )
                        })?
                        .push(prepared.encoded_request().to_vec());
                    match attempt {
                        1 => Err(TransferAttemptFailure::pre_connect(
                            "connect authenticated recipient",
                            "offline",
                        )),
                        2 => Err(TransferAttemptFailure::post_connect_ambiguous(
                            "read signed receipt",
                            "connection lost",
                        )),
                        _ => Ok(receipt),
                    }
                }
            },
        )
        .await?;
        assert!(matches!(
            outcome,
            HistoryRecoveryTransferExecutionOutcome::AcknowledgedAfterNetwork {
                attempts: 3,
                receipt_was_already_durable: false,
                ..
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let observed = observed
            .lock()
            .map_err(|_| io::Error::other("observed requests mutex poisoned"))?;
        assert_eq!(observed.len(), 3);
        assert!(observed.iter().all(|request| request == &expected));
        assert!(matches!(
            fixture.outbox.status(transfer_id)?,
            HistoryRecoveryTransferOutboxStatus::Acknowledged(_)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn exhausted_attempt_classes_leave_exact_intent_pending_across_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = prepared_fixture(
            SecretKey::generate().public(),
            SecretKey::generate().public(),
        )?;
        let transfer_id = fixture.prepared.transfer_id();
        let exact_request = fixture.prepared.encoded_request().to_vec();
        let pre_connect = execute_pending_with_attempt(
            &fixture.outbox,
            transfer_id,
            fixture.source_endpoint_id,
            fixture.recipient_endpoint_id,
            HistoryRecoveryTransferExecutionPolicy::new(1, Duration::from_secs(1))?,
            |_| async {
                Err(TransferAttemptFailure::pre_connect(
                    "connect authenticated recipient",
                    "offline",
                ))
            },
        )
        .await;
        assert!(matches!(
            pre_connect,
            Err(HistoryRecoveryTransferExecutionError::AttemptsExhausted {
                attempts: 1,
                last_class: HistoryRecoveryTransferAttemptFailureClass::PreConnect,
                ..
            })
        ));
        assert_eq!(
            fixture
                .outbox
                .load(transfer_id)?
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing pending request"))?
                .encoded_request(),
            exact_request
        );

        let result = execute_pending_with_attempt(
            &fixture.outbox,
            transfer_id,
            fixture.source_endpoint_id,
            fixture.recipient_endpoint_id,
            HistoryRecoveryTransferExecutionPolicy::new(2, Duration::from_secs(1))?,
            |_| async {
                Err(TransferAttemptFailure::post_connect_ambiguous(
                    "read signed receipt",
                    "connection lost",
                ))
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(HistoryRecoveryTransferExecutionError::AttemptsExhausted {
                attempts: 2,
                last_class: HistoryRecoveryTransferAttemptFailureClass::PostConnectAmbiguous,
                ..
            })
        ));
        let reloaded = fixture
            .outbox
            .load(transfer_id)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing pending request"))?;
        assert_eq!(reloaded.encoded_request(), exact_request);
        assert!(matches!(
            fixture.outbox.status(transfer_id)?,
            HistoryRecoveryTransferOutboxStatus::Pending(_)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn acknowledged_intent_never_invokes_network_attempt()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = prepared_fixture(
            SecretKey::generate().public(),
            SecretKey::generate().public(),
        )?;
        let transfer_id = fixture.prepared.transfer_id();
        let receipt = staged_receipt(&fixture.prepared)?;
        fixture
            .outbox
            .acknowledge(transfer_id, receipt.encoded_receipt())?;
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_attempt = Arc::clone(&calls);
        let outcome = execute_pending_with_attempt(
            &fixture.outbox,
            transfer_id,
            fixture.source_endpoint_id,
            fixture.recipient_endpoint_id,
            HistoryRecoveryTransferExecutionPolicy::default(),
            move |_| {
                calls_for_attempt.fetch_add(1, Ordering::SeqCst);
                async {
                    Err(TransferAttemptFailure::pre_connect(
                        "unexpected attempt",
                        "must not run",
                    ))
                }
            },
        )
        .await?;
        assert!(matches!(
            outcome,
            HistoryRecoveryTransferExecutionOutcome::AlreadyAcknowledgedWithoutNetwork(_)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn endpoint_substitution_fails_before_network_attempt()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = prepared_fixture(
            SecretKey::generate().public(),
            SecretKey::generate().public(),
        )?;
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_attempt = Arc::clone(&calls);
        let result = execute_pending_with_attempt(
            &fixture.outbox,
            fixture.prepared.transfer_id(),
            fixture.source_endpoint_id,
            SecretKey::generate().public(),
            HistoryRecoveryTransferExecutionPolicy::default(),
            move |_| {
                calls_for_attempt.fetch_add(1, Ordering::SeqCst);
                async {
                    Err(TransferAttemptFailure::pre_connect(
                        "unexpected attempt",
                        "must not run",
                    ))
                }
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(HistoryRecoveryTransferExecutionError::RecipientEndpointBinding)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn explicit_iroh_executor_uses_dedicated_alpn_and_commits_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = Endpoint::builder(presets::Minimal)
            .clear_ip_transports()
            .clear_relay_transports()
            .bind_addr("127.0.0.1:0")?
            .alpns(vec![HISTORY_RECOVERY_TRANSFER_ALPN.to_vec()])
            .bind()
            .await?;
        let client = Endpoint::builder(presets::Minimal)
            .clear_ip_transports()
            .clear_relay_transports()
            .bind_addr("127.0.0.1:0")?
            .bind()
            .await?;
        let fixture = prepared_fixture(client.id(), server.id())?;
        let transfer_id = fixture.prepared.transfer_id();
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&fixture_envelope()?)?;
        let recipient = recipient_identity()?;
        let recipient_endpoint_id = server.id();
        let inbox_state = tempfile::tempdir()?;
        let inbox = RecipientHistoryTransferInbox::new(
            inbox_state.path(),
            envelope.owner_account_id(),
            recipient.device_id(),
        );
        let server_socket = server.bound_sockets().into_iter().next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "test endpoint did not bind a loopback socket",
            )
        })?;
        let server_task = tokio::spawn(async move {
            let incoming = server.accept().await.ok_or_else(|| {
                io::Error::new(io::ErrorKind::ConnectionAborted, "test endpoint closed")
            })?;
            let connection = incoming
                .await
                .map_err(|error| io::Error::other(error.to_string()))?;
            receive_history_recovery_transfer(
                &connection,
                recipient_endpoint_id,
                &recipient,
                &inbox,
            )
            .await
            .map_err(|error| io::Error::other(error.to_string()))
        });
        let target = EndpointAddr::new(recipient_endpoint_id).with_ip_addr(server_socket);
        let outcome = execute_pending_history_recovery_transfer(
            &client,
            target,
            &fixture.outbox,
            transfer_id,
            HistoryRecoveryTransferExecutionPolicy::new(1, Duration::from_secs(10))?,
        )
        .await?;
        assert!(matches!(
            outcome,
            HistoryRecoveryTransferExecutionOutcome::AcknowledgedAfterNetwork {
                attempts: 1,
                receipt_was_already_durable: false,
                ..
            }
        ));
        server_task
            .await
            .map_err(|error| io::Error::other(error.to_string()))??;
        assert!(matches!(
            fixture.outbox.status(transfer_id)?,
            HistoryRecoveryTransferOutboxStatus::Acknowledged(_)
        ));
        client.close().await;
        Ok(())
    }

    fn prepared_fixture(
        source_endpoint_id: EndpointId,
        recipient_endpoint_id: EndpointId,
    ) -> Result<PreparedFixture, Box<dyn std::error::Error>> {
        let envelope_bytes = fixture_envelope()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?;
        let source = source_identity()?;
        let state = tempfile::tempdir()?;
        let outbox = HistoryRecoveryTransferOutbox::open(
            state.path(),
            envelope.owner_account_id(),
            source.device_id(),
        )?;
        let prepared = match outbox.prepare(
            &envelope_bytes,
            &source,
            source_endpoint_id,
            recipient_endpoint_id,
            [0x75; 32],
        )? {
            crate::PrepareHistoryRecoveryTransferOutcome::Prepared(prepared)
            | crate::PrepareHistoryRecoveryTransferOutcome::AlreadyPrepared(prepared) => prepared,
        };
        Ok(PreparedFixture {
            outbox,
            _state: state,
            prepared,
            source_endpoint_id,
            recipient_endpoint_id,
        })
    }

    fn staged_receipt(
        prepared: &PreparedHistoryRecoveryTransferSession,
    ) -> Result<SignedHistoryRecoveryTransferSessionReceipt, Box<dyn std::error::Error>> {
        let response = VerifiedTransferSessionResponse::create(
            &prepared.request,
            &recipient_identity()?,
            TransferStageDisposition::Staged,
        )?;
        Ok(
            SignedHistoryRecoveryTransferSessionReceipt::decode_and_verify(
                &response.encode()?,
                prepared,
            )?,
        )
    }

    fn fixture_envelope() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let compact = include_str!("../tests/fixtures/m1222-transfer-envelope.b64")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        Ok(STANDARD.decode(compact)?)
    }

    fn source_identity() -> Result<DeviceIdentity, Box<dyn std::error::Error>> {
        Ok(DeviceIdentity::from_secret_bytes(decode_secret(
            SOURCE_SECRET_HEX,
        )?))
    }

    fn recipient_identity() -> Result<DeviceIdentity, Box<dyn std::error::Error>> {
        Ok(DeviceIdentity::from_secret_bytes(decode_secret(
            RECIPIENT_SECRET_HEX,
        )?))
    }

    fn decode_secret(value: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        if value.len() != 64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid test secret").into());
        }
        let mut secret = [0_u8; 32];
        for (index, byte) in secret.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
        }
        Ok(secret)
    }
}
