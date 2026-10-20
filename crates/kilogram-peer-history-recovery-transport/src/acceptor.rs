use std::{future::Future, time::Duration};

use iroh::{Endpoint, endpoint::Incoming};
use kilogram_identity::DeviceIdentity;
use kilogram_peer_history_recovery_execution::RecipientHistoryTransferInbox;
use thiserror::Error;
use tokio::time::timeout;

use super::{
    HISTORY_RECOVERY_TRANSFER_ALPN, TransferStageDisposition, complete_history_recovery_transfer,
    read_history_recovery_transfer_request,
};

pub const MAX_HISTORY_RECOVERY_RECIPIENT_ACCEPT_SESSIONS: u8 = 8;
const DEFAULT_ACCEPT_SESSIONS: u8 = 1;
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_COMPLETION_TIMEOUT: Duration = Duration::from_secs(210);
const MAX_ACCEPT_TIMEOUT: Duration = Duration::from_secs(300);

/// A finite caller-owned budget for a dedicated recipient endpoint. The
/// acceptor never survives this function invocation and processes exactly one
/// connection at a time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryRecipientAcceptPolicy {
    max_sessions: u8,
    idle_timeout: Duration,
    request_timeout: Duration,
    completion_timeout: Duration,
}

impl HistoryRecoveryRecipientAcceptPolicy {
    pub fn new(
        max_sessions: u8,
        idle_timeout: Duration,
        request_timeout: Duration,
        completion_timeout: Duration,
    ) -> Result<Self, HistoryRecoveryRecipientAcceptPolicyError> {
        if max_sessions == 0 || max_sessions > MAX_HISTORY_RECOVERY_RECIPIENT_ACCEPT_SESSIONS {
            return Err(HistoryRecoveryRecipientAcceptPolicyError::Sessions(
                max_sessions,
            ));
        }
        if idle_timeout.is_zero() || idle_timeout > MAX_ACCEPT_TIMEOUT {
            return Err(HistoryRecoveryRecipientAcceptPolicyError::IdleTimeout);
        }
        if request_timeout.is_zero() || request_timeout > MAX_ACCEPT_TIMEOUT {
            return Err(HistoryRecoveryRecipientAcceptPolicyError::RequestTimeout);
        }
        if completion_timeout.is_zero() || completion_timeout > MAX_ACCEPT_TIMEOUT {
            return Err(HistoryRecoveryRecipientAcceptPolicyError::CompletionTimeout);
        }
        Ok(Self {
            max_sessions,
            idle_timeout,
            request_timeout,
            completion_timeout,
        })
    }

    pub fn max_sessions(self) -> u8 {
        self.max_sessions
    }

    pub fn idle_timeout(self) -> Duration {
        self.idle_timeout
    }

    pub fn request_timeout(self) -> Duration {
        self.request_timeout
    }

    pub fn completion_timeout(self) -> Duration {
        self.completion_timeout
    }
}

impl Default for HistoryRecoveryRecipientAcceptPolicy {
    fn default() -> Self {
        Self {
            max_sessions: DEFAULT_ACCEPT_SESSIONS,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            completion_timeout: DEFAULT_COMPLETION_TIMEOUT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryRecipientAcceptFailureClass {
    Accept,
    Handshake,
    WrongAlpn,
    RequestTimeout,
    CompletionTimeout,
    TransferRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryRecoveryRecipientAcceptTermination {
    SessionBudgetConsumed,
    IdleDeadline,
    EndpointClosed,
}

/// Privacy-safe aggregate evidence for one finite invocation. No Endpoint ID,
/// Device ID, transfer ID, network coordinate, path or error detail is exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryRecipientAcceptReport {
    pub slots_consumed: u8,
    pub staged: u8,
    pub already_staged: u8,
    pub rejected: u8,
    pub last_failure: Option<HistoryRecoveryRecipientAcceptFailureClass>,
    pub termination: HistoryRecoveryRecipientAcceptTermination,
    pub maximum_in_flight: u8,
    pub contains_identity: bool,
    pub contains_network_coordinates: bool,
    pub contains_message_bodies: bool,
    pub grants_import_authority: bool,
}

impl HistoryRecoveryRecipientAcceptReport {
    fn new() -> Self {
        Self {
            slots_consumed: 0,
            staged: 0,
            already_staged: 0,
            rejected: 0,
            last_failure: None,
            termination: HistoryRecoveryRecipientAcceptTermination::SessionBudgetConsumed,
            maximum_in_flight: 1,
            contains_identity: false,
            contains_network_coordinates: false,
            contains_message_bodies: false,
            grants_import_authority: false,
        }
    }

    fn reject(&mut self, failure: HistoryRecoveryRecipientAcceptFailureClass) {
        self.rejected = self.rejected.saturating_add(1);
        self.last_failure = Some(failure);
    }
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryRecipientAcceptPolicyError {
    #[error(
        "history recovery recipient session budget must be in 1..={MAX_HISTORY_RECOVERY_RECIPIENT_ACCEPT_SESSIONS}, got {0}"
    )]
    Sessions(u8),
    #[error("history recovery recipient idle timeout must be in 1ns..=300s")]
    IdleTimeout,
    #[error("history recovery recipient request timeout must be in 1ns..=300s")]
    RequestTimeout,
    #[error("history recovery recipient completion timeout must be in 1ns..=300s")]
    CompletionTimeout,
}

/// Accepts a finite number of connections on a caller-owned dedicated history
/// recovery endpoint. Each connection consumes one slot even when malformed.
/// There is no spawned task, background lifetime or retry schedule.
pub async fn accept_history_recovery_transfers(
    endpoint: &Endpoint,
    recipient: &DeviceIdentity,
    inbox: &RecipientHistoryTransferInbox,
    policy: HistoryRecoveryRecipientAcceptPolicy,
) -> HistoryRecoveryRecipientAcceptReport {
    run_bounded_accept_loop(policy, || async {
        let incoming = match timeout(policy.idle_timeout(), endpoint.accept()).await {
            Err(_) => return HistoryRecoveryRecipientAcceptStep::IdleDeadline,
            Ok(None) => return HistoryRecoveryRecipientAcceptStep::EndpointClosed,
            Ok(Some(incoming)) => incoming,
        };
        HistoryRecoveryRecipientAcceptStep::Session(
            receive_one(incoming, endpoint.id(), recipient, inbox, policy).await,
        )
    })
    .await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryRecoveryRecipientAcceptStep {
    Session(Result<TransferStageDisposition, HistoryRecoveryRecipientAcceptFailureClass>),
    IdleDeadline,
    EndpointClosed,
}

async fn run_bounded_accept_loop<Next, NextFuture>(
    policy: HistoryRecoveryRecipientAcceptPolicy,
    mut next: Next,
) -> HistoryRecoveryRecipientAcceptReport
where
    Next: FnMut() -> NextFuture,
    NextFuture: Future<Output = HistoryRecoveryRecipientAcceptStep>,
{
    let mut report = HistoryRecoveryRecipientAcceptReport::new();
    for _ in 0..policy.max_sessions() {
        let outcome = match next().await {
            HistoryRecoveryRecipientAcceptStep::IdleDeadline => {
                report.termination = HistoryRecoveryRecipientAcceptTermination::IdleDeadline;
                return report;
            }
            HistoryRecoveryRecipientAcceptStep::EndpointClosed => {
                report.termination = HistoryRecoveryRecipientAcceptTermination::EndpointClosed;
                return report;
            }
            HistoryRecoveryRecipientAcceptStep::Session(outcome) => outcome,
        };
        report.slots_consumed = report.slots_consumed.saturating_add(1);
        match outcome {
            Err(failure) => report.reject(failure),
            Ok(TransferStageDisposition::Staged) => {
                report.staged = report.staged.saturating_add(1);
            }
            Ok(TransferStageDisposition::AlreadyStaged) => {
                report.already_staged = report.already_staged.saturating_add(1);
            }
        }
    }
    report
}

async fn receive_one(
    incoming: Incoming,
    local_recipient_endpoint_id: iroh::EndpointId,
    recipient: &DeviceIdentity,
    inbox: &RecipientHistoryTransferInbox,
    policy: HistoryRecoveryRecipientAcceptPolicy,
) -> Result<TransferStageDisposition, HistoryRecoveryRecipientAcceptFailureClass> {
    let accepting = incoming
        .accept()
        .map_err(|_| HistoryRecoveryRecipientAcceptFailureClass::Accept)?;
    let connection = match timeout(policy.request_timeout(), accepting).await {
        Err(_) => return Err(HistoryRecoveryRecipientAcceptFailureClass::RequestTimeout),
        Ok(Err(_)) => return Err(HistoryRecoveryRecipientAcceptFailureClass::Handshake),
        Ok(Ok(connection)) => connection,
    };
    if connection.alpn() != HISTORY_RECOVERY_TRANSFER_ALPN {
        connection.close(0_u8.into(), b"unexpected history recovery ALPN");
        return Err(HistoryRecoveryRecipientAcceptFailureClass::WrongAlpn);
    }
    let pending = match timeout(
        policy.request_timeout(),
        read_history_recovery_transfer_request(&connection, local_recipient_endpoint_id, recipient),
    )
    .await
    {
        Err(_) => {
            connection.close(0_u8.into(), b"history recovery request timeout");
            return Err(HistoryRecoveryRecipientAcceptFailureClass::RequestTimeout);
        }
        Ok(Err(_)) => {
            connection.close(0_u8.into(), b"history recovery request rejected");
            return Err(HistoryRecoveryRecipientAcceptFailureClass::TransferRejected);
        }
        Ok(Ok(pending)) => pending,
    };
    match timeout(
        policy.completion_timeout(),
        complete_history_recovery_transfer(pending, recipient, inbox),
    )
    .await
    {
        Err(_) => {
            connection.close(0_u8.into(), b"history recovery completion timeout");
            Err(HistoryRecoveryRecipientAcceptFailureClass::CompletionTimeout)
        }
        Ok(Err(_)) => {
            connection.close(0_u8.into(), b"history recovery transfer rejected");
            Err(HistoryRecoveryRecipientAcceptFailureClass::TransferRejected)
        }
        Ok(Ok(receipt)) => Ok(receipt.disposition),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, io};

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroh::{EndpointAddr, endpoint::presets};
    use kilogram_peer_history_recovery_execution::HistoryRecoveryTransferEnvelope;

    use super::*;
    use crate::{PreparedHistoryRecoveryTransferSession, send_prepared_history_recovery_transfer};

    const SOURCE_SECRET_HEX: &str =
        "3fc00be615014da89b58a3a10ceb9b98e369449f6de6c702a521d638935a4e80";
    const RECIPIENT_SECRET_HEX: &str =
        "57d154005415eb97298e29fea08e518d6a35238dc9aa32aa688e401f526efcd8";

    #[test]
    fn accept_policy_is_strictly_bounded() -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            HistoryRecoveryRecipientAcceptPolicy::new(
                0,
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .is_err()
        );
        assert!(
            HistoryRecoveryRecipientAcceptPolicy::new(
                MAX_HISTORY_RECOVERY_RECIPIENT_ACCEPT_SESSIONS + 1,
                Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .is_err()
        );
        assert!(
            HistoryRecoveryRecipientAcceptPolicy::new(
                1,
                Duration::ZERO,
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .is_err()
        );
        assert!(
            HistoryRecoveryRecipientAcceptPolicy::new(
                1,
                Duration::from_secs(1),
                Duration::ZERO,
                Duration::from_secs(1),
            )
            .is_err()
        );
        assert!(
            HistoryRecoveryRecipientAcceptPolicy::new(
                1,
                Duration::from_secs(1),
                Duration::from_secs(1),
                MAX_ACCEPT_TIMEOUT + Duration::from_nanos(1),
            )
            .is_err()
        );
        let policy = HistoryRecoveryRecipientAcceptPolicy::new(
            2,
            Duration::from_secs(3),
            Duration::from_secs(4),
            Duration::from_secs(5),
        )?;
        assert_eq!(policy.max_sessions(), 2);
        assert_eq!(policy.idle_timeout(), Duration::from_secs(3));
        assert_eq!(policy.request_timeout(), Duration::from_secs(4));
        assert_eq!(policy.completion_timeout(), Duration::from_secs(5));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rejected_sessions_do_not_block_later_valid_transfer()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = HistoryRecoveryRecipientAcceptPolicy::new(
            3,
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )?;
        let mut steps = VecDeque::from([
            HistoryRecoveryRecipientAcceptStep::Session(Err(
                HistoryRecoveryRecipientAcceptFailureClass::WrongAlpn,
            )),
            HistoryRecoveryRecipientAcceptStep::Session(Err(
                HistoryRecoveryRecipientAcceptFailureClass::TransferRejected,
            )),
            HistoryRecoveryRecipientAcceptStep::Session(Ok(TransferStageDisposition::Staged)),
        ]);
        let report = run_bounded_accept_loop(policy, || {
            let step = steps
                .pop_front()
                .unwrap_or(HistoryRecoveryRecipientAcceptStep::EndpointClosed);
            async move { step }
        })
        .await;
        assert_eq!(report.slots_consumed, 3);
        assert_eq!(report.staged, 1);
        assert_eq!(report.already_staged, 0);
        assert_eq!(report.rejected, 2);
        assert_eq!(
            report.termination,
            HistoryRecoveryRecipientAcceptTermination::SessionBudgetConsumed
        );
        assert_eq!(report.maximum_in_flight, 1);
        assert!(!report.contains_identity);
        assert!(!report.contains_network_coordinates);
        assert!(!report.contains_message_bodies);
        assert!(!report.grants_import_authority);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn request_timeout_does_not_block_later_valid_transfer()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = HistoryRecoveryRecipientAcceptPolicy::new(
            2,
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )?;
        let mut steps = VecDeque::from([
            HistoryRecoveryRecipientAcceptStep::Session(Err(
                HistoryRecoveryRecipientAcceptFailureClass::RequestTimeout,
            )),
            HistoryRecoveryRecipientAcceptStep::Session(Ok(TransferStageDisposition::Staged)),
        ]);
        let report = run_bounded_accept_loop(policy, || {
            let step = steps
                .pop_front()
                .unwrap_or(HistoryRecoveryRecipientAcceptStep::EndpointClosed);
            async move { step }
        })
        .await;
        assert_eq!(report.slots_consumed, 2);
        assert_eq!(report.staged, 1);
        assert_eq!(report.rejected, 1);
        assert_eq!(
            report.last_failure,
            Some(HistoryRecoveryRecipientAcceptFailureClass::RequestTimeout)
        );
        assert_eq!(
            report.termination,
            HistoryRecoveryRecipientAcceptTermination::SessionBudgetConsumed
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn real_loopback_stages_one_transfer() -> Result<(), Box<dyn std::error::Error>> {
        let server = test_server().await?;
        let server_socket = loopback_socket(&server)?;
        let client = test_client().await?;
        let recipient_endpoint_id = server.id();
        let recipient = recipient_identity()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&fixture_envelope()?)?;
        let state = tempfile::tempdir()?;
        let inbox = RecipientHistoryTransferInbox::new(
            state.path(),
            envelope.owner_account_id(),
            recipient.device_id(),
        );
        let policy = HistoryRecoveryRecipientAcceptPolicy::new(
            1,
            Duration::from_secs(10),
            Duration::from_secs(10),
            Duration::from_secs(180),
        )?;
        let prepared = PreparedHistoryRecoveryTransferSession::create(
            &fixture_envelope()?,
            &source_identity()?,
            client.id(),
            recipient_endpoint_id,
            [0x76; 32],
        )?;
        let server_task = tokio::spawn(async move {
            accept_history_recovery_transfers(&server, &recipient, &inbox, policy).await
        });
        let client_result = async {
            let connection = client
                .connect(
                    EndpointAddr::new(recipient_endpoint_id).with_ip_addr(server_socket),
                    HISTORY_RECOVERY_TRANSFER_ALPN,
                )
                .await
                .map_err(|error| io::Error::other(format!("valid connect failed: {error}")))?;
            let receipt = send_prepared_history_recovery_transfer(&connection, &prepared)
                .await
                .map_err(|error| io::Error::other(format!("valid transfer failed: {error}")))?;
            Ok::<_, Box<dyn std::error::Error>>(receipt)
        }
        .await;
        let report = server_task
            .await
            .map_err(|error| io::Error::other(error.to_string()))?;
        let receipt = client_result.map_err(|error| {
            io::Error::other(format!(
                "client failed with recipient accept report {report:?}: {error}"
            ))
        })?;
        assert_eq!(
            receipt.summary().disposition,
            TransferStageDisposition::Staged
        );
        assert_eq!(report.slots_consumed, 1);
        assert_eq!(report.staged, 1);
        assert_eq!(report.rejected, 0);
        assert_eq!(
            report.termination,
            HistoryRecoveryRecipientAcceptTermination::SessionBudgetConsumed
        );
        client.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn idle_deadline_returns_body_free_report() -> Result<(), Box<dyn std::error::Error>> {
        let server = test_server().await?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&fixture_envelope()?)?;
        let recipient = recipient_identity()?;
        let state = tempfile::tempdir()?;
        let inbox = RecipientHistoryTransferInbox::new(
            state.path(),
            envelope.owner_account_id(),
            recipient.device_id(),
        );
        let report = accept_history_recovery_transfers(
            &server,
            &recipient,
            &inbox,
            HistoryRecoveryRecipientAcceptPolicy::new(
                1,
                Duration::from_millis(25),
                Duration::from_secs(1),
                Duration::from_secs(1),
            )?,
        )
        .await;
        assert_eq!(report.slots_consumed, 0);
        assert_eq!(report.rejected, 0);
        assert_eq!(
            report.termination,
            HistoryRecoveryRecipientAcceptTermination::IdleDeadline
        );
        assert!(!report.contains_identity);
        assert!(!report.contains_network_coordinates);
        assert!(!report.contains_message_bodies);
        server.close().await;
        Ok(())
    }

    async fn test_server() -> Result<Endpoint, Box<dyn std::error::Error>> {
        Ok(Endpoint::builder(presets::Minimal)
            .clear_ip_transports()
            .clear_relay_transports()
            .bind_addr("127.0.0.1:0")?
            .alpns(vec![HISTORY_RECOVERY_TRANSFER_ALPN.to_vec()])
            .bind()
            .await?)
    }

    async fn test_client() -> Result<Endpoint, Box<dyn std::error::Error>> {
        Ok(Endpoint::builder(presets::Minimal)
            .clear_ip_transports()
            .clear_relay_transports()
            .bind_addr("127.0.0.1:0")?
            .bind()
            .await?)
    }

    fn loopback_socket(
        endpoint: &Endpoint,
    ) -> Result<std::net::SocketAddr, Box<dyn std::error::Error>> {
        endpoint.bound_sockets().into_iter().next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "test endpoint did not bind a loopback socket",
            )
            .into()
        })
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
