#![forbid(unsafe_code)]

//! An authenticated, bounded Iroh session for one portable old-history
//! recovery transfer.
//!
//! This crate is deliberately not registered by the active Kilogram runtime.
//! It binds the already source-Device-signed M1.2.22 transfer envelope to both
//! authenticated Iroh Endpoint IDs and to a fresh caller-supplied nonce.  The
//! recipient stages the exact envelope durably before signing its response.

use std::time::Duration;

use iroh::{
    EndpointId,
    endpoint::{Connection, RecvStream, SendStream},
};
use kilogram_identity::{DeviceId, DeviceIdentity, IdentityError};
use kilogram_peer_history_recovery_execution::{
    HistoryRecoveryTransferEnvelope, HistoryRecoveryTransferError,
    MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES, RecipientHistoryTransferInbox,
    StageHistoryRecoveryTransferOutcome,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::timeout;

pub const HISTORY_RECOVERY_TRANSFER_ALPN: &[u8] = b"kilogram/m1/history-recovery/postcard/1";
pub const HISTORY_RECOVERY_TRANSFER_IO_TIMEOUT: Duration = Duration::from_secs(180);
pub const MAX_HISTORY_RECOVERY_TRANSFER_REQUEST_BYTES: usize =
    MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES + 4 * 1024;
pub const MAX_HISTORY_RECOVERY_TRANSFER_RESPONSE_BYTES: usize = 4 * 1024;

const REQUEST_VERSION: u8 = 1;
const RESPONSE_VERSION: u8 = 1;
const REQUEST_SIGNATURE_DOMAIN: &[u8] = b"kilogram:history-recovery-transfer-session-request:v1\0";
const REQUEST_ID_DOMAIN: &[u8] = b"kilogram:history-recovery-transfer-session-request-id:v1\0";
const RESPONSE_SIGNATURE_DOMAIN: &[u8] =
    b"kilogram:history-recovery-transfer-session-response:v1\0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TransferSessionRequestContent {
    version: u8,
    source_endpoint_id: [u8; 32],
    recipient_endpoint_id: [u8; 32],
    session_nonce: [u8; 32],
    transfer_id: [u8; 32],
    envelope: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedTransferSessionRequestWire {
    content: TransferSessionRequestContent,
    signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum TransferStageDispositionWire {
    Staged,
    AlreadyStaged,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TransferSessionResponseContent {
    version: u8,
    request_id: [u8; 32],
    transfer_id: [u8; 32],
    source_endpoint_id: [u8; 32],
    recipient_endpoint_id: [u8; 32],
    recipient_device_id: DeviceId,
    disposition: TransferStageDispositionWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SignedTransferSessionResponseWire {
    content: TransferSessionResponseContent,
    signature: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferStageDisposition {
    Staged,
    AlreadyStaged,
}

impl From<TransferStageDispositionWire> for TransferStageDisposition {
    fn from(value: TransferStageDispositionWire) -> Self {
        match value {
            TransferStageDispositionWire::Staged => Self::Staged,
            TransferStageDispositionWire::AlreadyStaged => Self::AlreadyStaged,
        }
    }
}

impl From<TransferStageDisposition> for TransferStageDispositionWire {
    fn from(value: TransferStageDisposition) -> Self {
        match value {
            TransferStageDisposition::Staged => Self::Staged,
            TransferStageDisposition::AlreadyStaged => Self::AlreadyStaged,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryRecoveryTransferSessionReceipt {
    pub request_id: [u8; 32],
    pub transfer_id: [u8; 32],
    pub recipient_device_id: DeviceId,
    pub disposition: TransferStageDisposition,
}

struct VerifiedTransferSessionRequest {
    wire: SignedTransferSessionRequestWire,
    envelope: HistoryRecoveryTransferEnvelope,
    request_id: [u8; 32],
}

impl VerifiedTransferSessionRequest {
    fn create(
        envelope_bytes: &[u8],
        source: &DeviceIdentity,
        source_endpoint_id: EndpointId,
        recipient_endpoint_id: EndpointId,
        session_nonce: [u8; 32],
    ) -> Result<Self, HistoryRecoveryTransferTransportError> {
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(envelope_bytes)?;
        if envelope.source_device_id() != source.device_id() {
            return Err(HistoryRecoveryTransferTransportError::WrongSourceDevice);
        }
        let content = TransferSessionRequestContent {
            version: REQUEST_VERSION,
            source_endpoint_id: endpoint_bytes(source_endpoint_id),
            recipient_endpoint_id: endpoint_bytes(recipient_endpoint_id),
            session_nonce,
            transfer_id: envelope.transfer_id()?,
            envelope: envelope_bytes.to_vec(),
        };
        validate_request_content(&content, &envelope)?;
        let signature = source
            .sign(&signing_bytes(REQUEST_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        let wire = SignedTransferSessionRequestWire { content, signature };
        let encoded = encode_request_wire(&wire)?;
        let request_id = request_id(&encoded);
        Ok(Self {
            wire,
            envelope,
            request_id,
        })
    }

    fn decode_for_recipient(
        bytes: &[u8],
        authenticated_source_endpoint_id: EndpointId,
        local_recipient_endpoint_id: EndpointId,
    ) -> Result<Self, HistoryRecoveryTransferTransportError> {
        validate_length(
            bytes,
            MAX_HISTORY_RECOVERY_TRANSFER_REQUEST_BYTES,
            HistoryRecoveryTransferTransportError::RequestLength,
        )?;
        let wire: SignedTransferSessionRequestWire = postcard::from_bytes(bytes)?;
        if encode_request_wire(&wire)? != bytes {
            return Err(HistoryRecoveryTransferTransportError::NonCanonicalRequest);
        }
        if wire.content.source_endpoint_id != endpoint_bytes(authenticated_source_endpoint_id)
            || wire.content.recipient_endpoint_id != endpoint_bytes(local_recipient_endpoint_id)
        {
            return Err(HistoryRecoveryTransferTransportError::EndpointBinding);
        }
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&wire.content.envelope)?;
        validate_request_content(&wire.content, &envelope)?;
        envelope.source_device_id().verify(
            &signing_bytes(REQUEST_SIGNATURE_DOMAIN, &wire.content)?,
            &wire.signature,
        )?;
        Ok(Self {
            request_id: request_id(bytes),
            wire,
            envelope,
        })
    }

    fn encode(&self) -> Result<Vec<u8>, HistoryRecoveryTransferTransportError> {
        encode_request_wire(&self.wire)
    }
}

struct VerifiedTransferSessionResponse {
    wire: SignedTransferSessionResponseWire,
}

impl VerifiedTransferSessionResponse {
    fn create(
        request: &VerifiedTransferSessionRequest,
        recipient: &DeviceIdentity,
        disposition: TransferStageDisposition,
    ) -> Result<Self, HistoryRecoveryTransferTransportError> {
        if request.envelope.recipient_device_id() != recipient.device_id() {
            return Err(HistoryRecoveryTransferTransportError::WrongRecipientDevice);
        }
        let content = TransferSessionResponseContent {
            version: RESPONSE_VERSION,
            request_id: request.request_id,
            transfer_id: request.wire.content.transfer_id,
            source_endpoint_id: request.wire.content.source_endpoint_id,
            recipient_endpoint_id: request.wire.content.recipient_endpoint_id,
            recipient_device_id: recipient.device_id(),
            disposition: disposition.into(),
        };
        validate_response_content(&content, request)?;
        let signature = recipient
            .sign(&signing_bytes(RESPONSE_SIGNATURE_DOMAIN, &content)?)
            .to_vec();
        Ok(Self {
            wire: SignedTransferSessionResponseWire { content, signature },
        })
    }

    fn decode_for_source(
        bytes: &[u8],
        request: &VerifiedTransferSessionRequest,
        expected_recipient_device_id: DeviceId,
        authenticated_recipient_endpoint_id: EndpointId,
        local_source_endpoint_id: EndpointId,
    ) -> Result<Self, HistoryRecoveryTransferTransportError> {
        validate_length(
            bytes,
            MAX_HISTORY_RECOVERY_TRANSFER_RESPONSE_BYTES,
            HistoryRecoveryTransferTransportError::ResponseLength,
        )?;
        let wire: SignedTransferSessionResponseWire = postcard::from_bytes(bytes)?;
        if encode_response_wire(&wire)? != bytes {
            return Err(HistoryRecoveryTransferTransportError::NonCanonicalResponse);
        }
        validate_response_content(&wire.content, request)?;
        if wire.content.recipient_device_id != expected_recipient_device_id {
            return Err(HistoryRecoveryTransferTransportError::WrongRecipientDevice);
        }
        if wire.content.recipient_endpoint_id != endpoint_bytes(authenticated_recipient_endpoint_id)
            || wire.content.source_endpoint_id != endpoint_bytes(local_source_endpoint_id)
        {
            return Err(HistoryRecoveryTransferTransportError::EndpointBinding);
        }
        wire.content.recipient_device_id.verify(
            &signing_bytes(RESPONSE_SIGNATURE_DOMAIN, &wire.content)?,
            &wire.signature,
        )?;
        Ok(Self { wire })
    }

    fn encode(&self) -> Result<Vec<u8>, HistoryRecoveryTransferTransportError> {
        encode_response_wire(&self.wire)
    }

    fn receipt(&self) -> HistoryRecoveryTransferSessionReceipt {
        HistoryRecoveryTransferSessionReceipt {
            request_id: self.wire.content.request_id,
            transfer_id: self.wire.content.transfer_id,
            recipient_device_id: self.wire.content.recipient_device_id,
            disposition: self.wire.content.disposition.into(),
        }
    }
}

/// Sends and verifies one exact recovery transfer over an already authenticated
/// Iroh connection.  No fallback protocol is attempted.
pub async fn send_history_recovery_transfer(
    connection: &Connection,
    local_source_endpoint_id: EndpointId,
    remote_recipient_endpoint_id: EndpointId,
    source: &DeviceIdentity,
    expected_recipient_device_id: DeviceId,
    session_nonce: [u8; 32],
    envelope_bytes: &[u8],
) -> Result<HistoryRecoveryTransferSessionReceipt, HistoryRecoveryTransferTransportError> {
    if connection.remote_id() != remote_recipient_endpoint_id {
        return Err(HistoryRecoveryTransferTransportError::EndpointBinding);
    }
    let request = VerifiedTransferSessionRequest::create(
        envelope_bytes,
        source,
        local_source_endpoint_id,
        remote_recipient_endpoint_id,
        session_nonce,
    )?;
    if request.envelope.recipient_device_id() != expected_recipient_device_id {
        return Err(HistoryRecoveryTransferTransportError::WrongRecipientDevice);
    }
    let request_bytes = request.encode()?;
    let (mut send, mut receive) = io_timeout("open request stream", connection.open_bi()).await?;
    write_finished(&mut send, &request_bytes, "write request").await?;
    let response_bytes = read_bounded(
        &mut receive,
        MAX_HISTORY_RECOVERY_TRANSFER_RESPONSE_BYTES,
        "read response",
    )
    .await?;
    let response = VerifiedTransferSessionResponse::decode_for_source(
        &response_bytes,
        &request,
        expected_recipient_device_id,
        remote_recipient_endpoint_id,
        local_source_endpoint_id,
    )?;
    wait_stopped(&mut send, "confirm request delivery").await?;
    Ok(response.receipt())
}

/// Receives, validates and durably stages one exact recovery transfer before
/// signing a response.  A lost response can be retried with the same envelope;
/// the append-only inbox reports `AlreadyStaged` rather than duplicating it.
pub async fn receive_history_recovery_transfer(
    connection: &Connection,
    local_recipient_endpoint_id: EndpointId,
    recipient: &DeviceIdentity,
    inbox: &RecipientHistoryTransferInbox,
) -> Result<HistoryRecoveryTransferSessionReceipt, HistoryRecoveryTransferTransportError> {
    let authenticated_source_endpoint_id = connection.remote_id();
    let (mut send, mut receive) =
        io_timeout("accept request stream", connection.accept_bi()).await?;
    let request_bytes = read_bounded(
        &mut receive,
        MAX_HISTORY_RECOVERY_TRANSFER_REQUEST_BYTES,
        "read request",
    )
    .await?;
    let request = VerifiedTransferSessionRequest::decode_for_recipient(
        &request_bytes,
        authenticated_source_endpoint_id,
        local_recipient_endpoint_id,
    )?;
    if request.envelope.recipient_device_id() != recipient.device_id() {
        return Err(HistoryRecoveryTransferTransportError::WrongRecipientDevice);
    }
    let disposition = match inbox.stage_envelope_bytes(&request.wire.content.envelope)? {
        StageHistoryRecoveryTransferOutcome::Staged(_) => TransferStageDisposition::Staged,
        StageHistoryRecoveryTransferOutcome::AlreadyStaged(_) => {
            TransferStageDisposition::AlreadyStaged
        }
    };
    let response = VerifiedTransferSessionResponse::create(&request, recipient, disposition)?;
    write_finished(&mut send, &response.encode()?, "write response").await?;
    wait_stopped(&mut send, "confirm response delivery").await?;
    Ok(response.receipt())
}

fn validate_request_content(
    content: &TransferSessionRequestContent,
    envelope: &HistoryRecoveryTransferEnvelope,
) -> Result<(), HistoryRecoveryTransferTransportError> {
    if content.version != REQUEST_VERSION
        || content.source_endpoint_id == content.recipient_endpoint_id
        || content.session_nonce == [0_u8; 32]
    {
        return Err(HistoryRecoveryTransferTransportError::InvalidRequest);
    }
    if content.envelope.is_empty()
        || content.envelope.len() > MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES
    {
        return Err(HistoryRecoveryTransferTransportError::RequestLength(
            content.envelope.len(),
        ));
    }
    if content.transfer_id != envelope.transfer_id()? {
        return Err(HistoryRecoveryTransferTransportError::TransferIdMismatch);
    }
    Ok(())
}

fn validate_response_content(
    content: &TransferSessionResponseContent,
    request: &VerifiedTransferSessionRequest,
) -> Result<(), HistoryRecoveryTransferTransportError> {
    if content.version != RESPONSE_VERSION
        || content.request_id != request.request_id
        || content.transfer_id != request.wire.content.transfer_id
        || content.source_endpoint_id != request.wire.content.source_endpoint_id
        || content.recipient_endpoint_id != request.wire.content.recipient_endpoint_id
    {
        return Err(HistoryRecoveryTransferTransportError::ResponseBinding);
    }
    Ok(())
}

fn encode_request_wire(
    wire: &SignedTransferSessionRequestWire,
) -> Result<Vec<u8>, HistoryRecoveryTransferTransportError> {
    let bytes = postcard::to_allocvec(wire)?;
    validate_length(
        &bytes,
        MAX_HISTORY_RECOVERY_TRANSFER_REQUEST_BYTES,
        HistoryRecoveryTransferTransportError::RequestLength,
    )?;
    Ok(bytes)
}

fn encode_response_wire(
    wire: &SignedTransferSessionResponseWire,
) -> Result<Vec<u8>, HistoryRecoveryTransferTransportError> {
    let bytes = postcard::to_allocvec(wire)?;
    validate_length(
        &bytes,
        MAX_HISTORY_RECOVERY_TRANSFER_RESPONSE_BYTES,
        HistoryRecoveryTransferTransportError::ResponseLength,
    )?;
    Ok(bytes)
}

fn signing_bytes<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<Vec<u8>, HistoryRecoveryTransferTransportError> {
    let encoded = postcard::to_allocvec(value)?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

fn request_id(encoded_request: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REQUEST_ID_DOMAIN);
    hasher.update(encoded_request);
    *hasher.finalize().as_bytes()
}

fn endpoint_bytes(endpoint_id: EndpointId) -> [u8; 32] {
    *endpoint_id.as_bytes()
}

fn validate_length<E>(
    bytes: &[u8],
    maximum: usize,
    error: impl FnOnce(usize) -> E,
) -> Result<(), E> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(error(bytes.len()));
    }
    Ok(())
}

async fn read_bounded(
    receive: &mut RecvStream,
    maximum: usize,
    stage: &'static str,
) -> Result<Vec<u8>, HistoryRecoveryTransferTransportError> {
    io_timeout(stage, receive.read_to_end(maximum)).await
}

async fn write_finished(
    send: &mut SendStream,
    bytes: &[u8],
    stage: &'static str,
) -> Result<(), HistoryRecoveryTransferTransportError> {
    io_timeout(stage, send.write_all(bytes)).await?;
    send.finish()
        .map_err(|error| transport_error(stage, error))?;
    Ok(())
}

async fn wait_stopped(
    send: &mut SendStream,
    stage: &'static str,
) -> Result<(), HistoryRecoveryTransferTransportError> {
    let stopped = io_timeout(stage, send.stopped()).await?;
    if let Some(code) = stopped {
        return Err(HistoryRecoveryTransferTransportError::PeerStopped {
            stage,
            code: code.into_inner(),
        });
    }
    Ok(())
}

async fn io_timeout<T, E>(
    stage: &'static str,
    operation: impl Future<Output = Result<T, E>>,
) -> Result<T, HistoryRecoveryTransferTransportError>
where
    E: std::fmt::Display,
{
    timeout(HISTORY_RECOVERY_TRANSFER_IO_TIMEOUT, operation)
        .await
        .map_err(|_| HistoryRecoveryTransferTransportError::Timeout(stage))?
        .map_err(|error| transport_error(stage, error))
}

fn transport_error(
    stage: &'static str,
    error: impl std::fmt::Display,
) -> HistoryRecoveryTransferTransportError {
    HistoryRecoveryTransferTransportError::Transport {
        stage,
        detail: error.to_string(),
    }
}

#[derive(Debug, Error)]
pub enum HistoryRecoveryTransferTransportError {
    #[error("history recovery transfer request has invalid length {0}")]
    RequestLength(usize),
    #[error("history recovery transfer response has invalid length {0}")]
    ResponseLength(usize),
    #[error("history recovery transfer request is invalid")]
    InvalidRequest,
    #[error("history recovery transfer request is not canonical")]
    NonCanonicalRequest,
    #[error("history recovery transfer response is not canonical")]
    NonCanonicalResponse,
    #[error("history recovery transfer Iroh Endpoint binding does not match")]
    EndpointBinding,
    #[error("history recovery transfer ID does not match the signed envelope")]
    TransferIdMismatch,
    #[error("history recovery transfer source Device does not match")]
    WrongSourceDevice,
    #[error("history recovery transfer recipient Device does not match")]
    WrongRecipientDevice,
    #[error("history recovery transfer response does not bind its request")]
    ResponseBinding,
    #[error("history recovery transfer transport timed out while attempting to {0}")]
    Timeout(&'static str),
    #[error("history recovery transfer transport failed while attempting to {stage}: {detail}")]
    Transport { stage: &'static str, detail: String },
    #[error("history recovery transfer peer stopped {stage} with code {code}")]
    PeerStopped { stage: &'static str, code: u64 },
    #[error(transparent)]
    Transfer(#[from] HistoryRecoveryTransferError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Encoding(#[from] postcard::Error),
}

#[cfg(test)]
mod tests {
    use std::io;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroh::{Endpoint, EndpointAddr, SecretKey, endpoint::presets};
    use kilogram_peer_history_recovery_execution::RecipientHistoryTransferInboxStatus;

    use super::*;

    const SOURCE_SECRET_HEX: &str =
        "3fc00be615014da89b58a3a10ceb9b98e369449f6de6c702a521d638935a4e80";
    const RECIPIENT_SECRET_HEX: &str =
        "57d154005415eb97298e29fea08e518d6a35238dc9aa32aa688e401f526efcd8";

    #[test]
    fn signed_frames_bind_both_endpoints_devices_and_exact_transfer()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope_bytes = fixture_envelope()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?;
        let source = source_identity()?;
        let recipient = recipient_identity()?;
        assert_eq!(envelope.source_device_id(), source.device_id());
        assert_eq!(envelope.recipient_device_id(), recipient.device_id());

        let source_endpoint_id = SecretKey::generate().public();
        let recipient_endpoint_id = SecretKey::generate().public();
        let foreign_endpoint_id = SecretKey::generate().public();
        let request = VerifiedTransferSessionRequest::create(
            &envelope_bytes,
            &source,
            source_endpoint_id,
            recipient_endpoint_id,
            [0x41; 32],
        )?;
        let request_bytes = request.encode()?;
        let decoded = VerifiedTransferSessionRequest::decode_for_recipient(
            &request_bytes,
            source_endpoint_id,
            recipient_endpoint_id,
        )?;
        assert_eq!(decoded.request_id, request.request_id);
        assert_eq!(decoded.wire.content.transfer_id, envelope.transfer_id()?);
        assert!(matches!(
            VerifiedTransferSessionRequest::decode_for_recipient(
                &request_bytes,
                foreign_endpoint_id,
                recipient_endpoint_id,
            ),
            Err(HistoryRecoveryTransferTransportError::EndpointBinding)
        ));

        let response = VerifiedTransferSessionResponse::create(
            &decoded,
            &recipient,
            TransferStageDisposition::Staged,
        )?;
        let response_bytes = response.encode()?;
        let verified = VerifiedTransferSessionResponse::decode_for_source(
            &response_bytes,
            &request,
            recipient.device_id(),
            recipient_endpoint_id,
            source_endpoint_id,
        )?;
        assert_eq!(
            verified.receipt().disposition,
            TransferStageDisposition::Staged
        );
        assert!(matches!(
            VerifiedTransferSessionResponse::decode_for_source(
                &response_bytes,
                &request,
                recipient.device_id(),
                foreign_endpoint_id,
                source_endpoint_id,
            ),
            Err(HistoryRecoveryTransferTransportError::EndpointBinding)
        ));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn loopback_stages_then_acknowledges_exact_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope_bytes = fixture_envelope()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?;
        let source = source_identity()?;
        let recipient = recipient_identity()?;
        let state = tempfile::tempdir()?;
        let state_path = state.path().to_path_buf();
        let server = test_server().await?;
        let client = test_client().await?;
        let source_endpoint_id = client.id();
        let recipient_endpoint_id = server.id();
        let server_addr = loopback_addr(&server)?;
        let owner_account_id = envelope.owner_account_id();
        let recipient_device_id = envelope.recipient_device_id();
        let server_state = state_path.clone();
        let server_task = tokio::spawn(async move {
            let incoming = server.accept().await.ok_or_else(|| {
                io::Error::new(io::ErrorKind::ConnectionAborted, "test endpoint closed")
            })?;
            let connection = incoming.await?;
            let inbox = RecipientHistoryTransferInbox::new(
                &server_state,
                owner_account_id,
                recipient.device_id(),
            );
            let first = receive_history_recovery_transfer(
                &connection,
                recipient_endpoint_id,
                &recipient,
                &inbox,
            )
            .await?;
            let second = receive_history_recovery_transfer(
                &connection,
                recipient_endpoint_id,
                &recipient,
                &inbox,
            )
            .await?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((first, second))
        });

        let connection = client
            .connect(server_addr, HISTORY_RECOVERY_TRANSFER_ALPN)
            .await?;
        let first = send_history_recovery_transfer(
            &connection,
            source_endpoint_id,
            recipient_endpoint_id,
            &source,
            recipient_device_id,
            [0x52; 32],
            &envelope_bytes,
        )
        .await?;
        let second = send_history_recovery_transfer(
            &connection,
            source_endpoint_id,
            recipient_endpoint_id,
            &source,
            recipient_device_id,
            [0x52; 32],
            &envelope_bytes,
        )
        .await?;
        let (server_first, server_second) = server_task
            .await
            .map_err(|error| io::Error::other(error.to_string()))?
            .map_err(|error| io::Error::other(error.to_string()))?;
        assert_eq!(first, server_first);
        assert_eq!(second, server_second);
        assert_eq!(first.disposition, TransferStageDisposition::Staged);
        assert_eq!(second.disposition, TransferStageDisposition::AlreadyStaged);
        assert_eq!(first.request_id, second.request_id);
        assert_eq!(first.transfer_id, envelope.transfer_id()?);

        let inbox = RecipientHistoryTransferInbox::new(
            &state_path,
            envelope.owner_account_id(),
            envelope.recipient_device_id(),
        );
        assert_eq!(
            inbox.load_staged(envelope.transfer_id()?)?.transfer_id(),
            envelope.transfer_id()?
        );
        client.close().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn authenticated_endpoint_substitution_fails_before_staging()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope_bytes = fixture_envelope()?;
        let envelope = HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?;
        let source = source_identity()?;
        let recipient = recipient_identity()?;
        let state = tempfile::tempdir()?;
        let server = test_server().await?;
        let client = test_client().await?;
        let actual_source_endpoint_id = client.id();
        let declared_source_endpoint_id = SecretKey::generate().public();
        let recipient_endpoint_id = server.id();
        let request = VerifiedTransferSessionRequest::create(
            &envelope_bytes,
            &source,
            declared_source_endpoint_id,
            recipient_endpoint_id,
            [0x63; 32],
        )?;
        let server_addr = loopback_addr(&server)?;
        let state_path = state.path().to_path_buf();
        let server_state = state_path.clone();
        let server_task = tokio::spawn(async move {
            let incoming = server.accept().await.ok_or_else(|| {
                io::Error::new(io::ErrorKind::ConnectionAborted, "test endpoint closed")
            })?;
            let connection = incoming.await?;
            assert_eq!(connection.remote_id(), actual_source_endpoint_id);
            let inbox = RecipientHistoryTransferInbox::new(
                &server_state,
                envelope.owner_account_id(),
                recipient.device_id(),
            );
            let result = receive_history_recovery_transfer(
                &connection,
                recipient_endpoint_id,
                &recipient,
                &inbox,
            )
            .await;
            assert!(matches!(
                result,
                Err(HistoryRecoveryTransferTransportError::EndpointBinding)
            ));
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        });

        let connection = client
            .connect(server_addr, HISTORY_RECOVERY_TRANSFER_ALPN)
            .await?;
        let (mut send, _receive) = connection.open_bi().await?;
        send.write_all(&request.encode()?).await?;
        send.finish()?;
        server_task
            .await
            .map_err(|error| io::Error::other(error.to_string()))?
            .map_err(|error| io::Error::other(error.to_string()))?;

        let inbox = RecipientHistoryTransferInbox::new(
            &state_path,
            HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?.owner_account_id(),
            recipient_identity()?.device_id(),
        );
        assert!(matches!(
            inbox.status(
                HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?
                    .consent_record_id(),
                HistoryRecoveryTransferEnvelope::decode_and_verify(&envelope_bytes)?
                    .selection_index(),
            )?,
            RecipientHistoryTransferInboxStatus::Absent
        ));
        client.close().await;
        Ok(())
    }

    #[test]
    fn zero_nonce_and_trailing_bytes_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let envelope_bytes = fixture_envelope()?;
        let source = source_identity()?;
        let source_endpoint_id = SecretKey::generate().public();
        let recipient_endpoint_id = SecretKey::generate().public();
        assert!(matches!(
            VerifiedTransferSessionRequest::create(
                &envelope_bytes,
                &source,
                source_endpoint_id,
                recipient_endpoint_id,
                [0_u8; 32],
            ),
            Err(HistoryRecoveryTransferTransportError::InvalidRequest)
        ));
        let request = VerifiedTransferSessionRequest::create(
            &envelope_bytes,
            &source,
            source_endpoint_id,
            recipient_endpoint_id,
            [0x74; 32],
        )?;
        let mut bytes = request.encode()?;
        bytes.push(0);
        assert!(
            VerifiedTransferSessionRequest::decode_for_recipient(
                &bytes,
                source_endpoint_id,
                recipient_endpoint_id,
            )
            .is_err()
        );
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

    fn loopback_addr(endpoint: &Endpoint) -> Result<EndpointAddr, Box<dyn std::error::Error>> {
        let socket = endpoint.bound_sockets().into_iter().next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "test endpoint did not bind a loopback socket",
            )
        })?;
        Ok(EndpointAddr::new(endpoint.id()).with_ip_addr(socket))
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
