use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use iroh::endpoint::{Connection, RecvStream, SendStream};
use kilogram_protocol::canonical_sync_v2::{
    CanonicalSyncClientFrameV2, CanonicalSyncServerFrameV2, decode_and_verify_sync_client_frame_v2,
    decode_and_verify_sync_server_frame_v2, encode_sync_client_frame_v2,
    encode_sync_server_frame_v2,
};
use kilogram_session::canonical_sync_v2_adapter::{
    CanonicalSyncV2Client, CanonicalSyncV2DurableClientAction,
    CanonicalSyncV2DurableResponderAction, CanonicalSyncV2MembershipPolicy, CanonicalSyncV2Server,
    CanonicalSyncV2ServerInventoryOutcome,
};
use kilogram_store::CanonicalSyncV2CarrierStore;
use tokio::time::timeout;

pub const CANONICAL_SYNC_V2_STREAM_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_CANONICAL_SYNC_V2_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalSyncV2ServeOutcome {
    Complete,
    StoppedAfterDiff,
    ConnectionInterrupted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalSyncV2ClientOutcome {
    Complete,
    StoppedAfterDiff,
}

pub async fn serve_canonical_sync_v2_connection(
    connection: &Connection,
    server: &CanonicalSyncV2Server<
        '_,
        CanonicalSyncV2CarrierStore,
        CanonicalSyncV2MembershipPolicy,
    >,
    stop_after_diff: bool,
) -> Result<CanonicalSyncV2ServeOutcome> {
    loop {
        let runtime = server.restore_durable_responder_runtime()?;
        let action = server.durable_responder_action(&runtime)?;
        if matches!(action, CanonicalSyncV2DurableResponderAction::Complete) {
            return Ok(CanonicalSyncV2ServeOutcome::Complete);
        }
        let streams = timeout(CANONICAL_SYNC_V2_STREAM_TIMEOUT, connection.accept_bi()).await;
        let (mut send, mut receive) = match streams {
            Ok(Ok(streams)) => streams,
            Ok(Err(_)) | Err(_) => {
                return Ok(CanonicalSyncV2ServeOutcome::ConnectionInterrupted);
            }
        };
        let frame = read_canonical_sync_v2_client_frame(&mut receive).await?;
        match (action, frame) {
            (
                CanonicalSyncV2DurableResponderAction::AwaitInventory,
                CanonicalSyncClientFrameV2::Inventory(inventory),
            ) => {
                let pending = server.accept_durable_inventory(&runtime, &inventory)?;
                let pending_action = server.durable_responder_action(&pending)?;
                let response = pending_action
                    .frame()
                    .context("durable responder did not journal a response")?;
                write_canonical_sync_v2_server_frame(&mut send, &response).await?;
                if matches!(
                    pending_action,
                    CanonicalSyncV2DurableResponderAction::TransmitDiff(_)
                ) {
                    server.mark_durable_diff_sent(&pending)?;
                    if stop_after_diff {
                        return Ok(CanonicalSyncV2ServeOutcome::StoppedAfterDiff);
                    }
                }
            }
            (
                pending @ (CanonicalSyncV2DurableResponderAction::TransmitDiff(_)
                | CanonicalSyncV2DurableResponderAction::RetransmitDiff(_)),
                CanonicalSyncClientFrameV2::Inventory(inventory),
            ) => {
                let rebuilt = match server.accept_inventory(&inventory)? {
                    CanonicalSyncV2ServerInventoryOutcome::Accepted(round) => round.frame(),
                    CanonicalSyncV2ServerInventoryOutcome::Rejected(rejected) => {
                        CanonicalSyncServerFrameV2::Rejected(rejected)
                    }
                };
                let expected = pending.frame().context("pending diff missing")?;
                ensure!(
                    rebuilt == expected,
                    "inventory retry changed pending response"
                );
                write_canonical_sync_v2_server_frame(&mut send, &expected).await?;
                if matches!(
                    pending,
                    CanonicalSyncV2DurableResponderAction::TransmitDiff(_)
                ) {
                    server.mark_durable_diff_sent(&runtime)?;
                }
                if stop_after_diff {
                    return Ok(CanonicalSyncV2ServeOutcome::StoppedAfterDiff);
                }
            }
            (
                pending @ (CanonicalSyncV2DurableResponderAction::TransmitDiff(_)
                | CanonicalSyncV2DurableResponderAction::RetransmitDiff(_)),
                CanonicalSyncClientFrameV2::Events(batch),
            ) => {
                let ready = if matches!(
                    pending,
                    CanonicalSyncV2DurableResponderAction::TransmitDiff(_)
                ) {
                    server.mark_durable_diff_sent(&runtime)?
                } else {
                    runtime
                };
                let (_, complete) = server.accept_durable_event_batch(&ready, batch)?;
                let response = server
                    .durable_responder_action(&complete)?
                    .frame()
                    .context("durable complete frame missing")?;
                write_canonical_sync_v2_server_frame(&mut send, &response).await?;
                server.mark_durable_complete_sent(&complete)?;
            }
            (
                CanonicalSyncV2DurableResponderAction::TransmitComplete(complete),
                CanonicalSyncClientFrameV2::Events(_),
            ) => {
                write_canonical_sync_v2_server_frame(
                    &mut send,
                    &CanonicalSyncServerFrameV2::Complete(complete),
                )
                .await?;
                server.mark_durable_complete_sent(&runtime)?;
            }
            (actual, _) => {
                bail!("unexpected canonical frame for durable responder action {actual:?}")
            }
        }
    }
}

pub async fn drive_canonical_sync_v2_client(
    connection: &Connection,
    client: &CanonicalSyncV2Client<
        '_,
        CanonicalSyncV2CarrierStore,
        CanonicalSyncV2MembershipPolicy,
    >,
    stop_after_diff: bool,
) -> Result<CanonicalSyncV2ClientOutcome> {
    loop {
        let runtime = client.restore_durable_runtime()?;
        match client.durable_action(&runtime)? {
            CanonicalSyncV2DurableClientAction::PrepareInventory => {
                client.begin_durable_round(&runtime)?;
            }
            CanonicalSyncV2DurableClientAction::TransmitInventory(inventory) => {
                let response = exchange_canonical_sync_v2_client_frame(
                    connection,
                    &CanonicalSyncClientFrameV2::Inventory(inventory),
                )
                .await?;
                let CanonicalSyncServerFrameV2::Diff(diff) = response else {
                    bail!("canonical client expected Diff")
                };
                client.accept_durable_diff(&runtime, diff)?;
                if stop_after_diff {
                    return Ok(CanonicalSyncV2ClientOutcome::StoppedAfterDiff);
                }
            }
            CanonicalSyncV2DurableClientAction::TransmitEventBatch(batch) => {
                let (mut send, mut receive) = open_canonical_sync_v2_stream(connection).await?;
                write_canonical_sync_v2_client_frame(
                    &mut send,
                    &CanonicalSyncClientFrameV2::Events(batch),
                )
                .await?;
                let waiting = client.mark_durable_batch_sent(&runtime)?;
                let response = read_canonical_sync_v2_server_frame(&mut receive).await?;
                let CanonicalSyncServerFrameV2::Complete(complete) = response else {
                    bail!("canonical client expected Complete")
                };
                client.accept_durable_complete(&waiting, complete)?;
            }
            CanonicalSyncV2DurableClientAction::RetransmitEventBatch(batch) => {
                let response = exchange_canonical_sync_v2_client_frame(
                    connection,
                    &CanonicalSyncClientFrameV2::Events(batch),
                )
                .await?;
                let CanonicalSyncServerFrameV2::Complete(complete) = response else {
                    bail!("canonical client expected retransmitted Complete")
                };
                client.accept_durable_complete(&runtime, complete)?;
            }
            CanonicalSyncV2DurableClientAction::Complete => {
                return Ok(CanonicalSyncV2ClientOutcome::Complete);
            }
            other => bail!("unsupported durable client action in canonical runtime: {other:?}"),
        }
    }
}

pub async fn open_canonical_sync_v2_stream(
    connection: &Connection,
) -> Result<(SendStream, RecvStream)> {
    timeout(CANONICAL_SYNC_V2_STREAM_TIMEOUT, connection.open_bi())
        .await
        .context("open canonical sync v2 stream deadline")?
        .context("open canonical sync v2 stream")
}

pub async fn exchange_canonical_sync_v2_client_frame(
    connection: &Connection,
    frame: &CanonicalSyncClientFrameV2,
) -> Result<CanonicalSyncServerFrameV2> {
    let (mut send, mut receive) = open_canonical_sync_v2_stream(connection).await?;
    write_canonical_sync_v2_client_frame(&mut send, frame).await?;
    read_canonical_sync_v2_server_frame(&mut receive).await
}

pub async fn write_canonical_sync_v2_client_frame(
    send: &mut SendStream,
    frame: &CanonicalSyncClientFrameV2,
) -> Result<()> {
    let bytes = encode_sync_client_frame_v2(frame)?;
    timeout(CANONICAL_SYNC_V2_STREAM_TIMEOUT, send.write_all(&bytes))
        .await
        .context("write canonical client frame deadline")??;
    send.finish().context("finish canonical client frame")?;
    Ok(())
}

pub async fn read_canonical_sync_v2_client_frame(
    receive: &mut RecvStream,
) -> Result<CanonicalSyncClientFrameV2> {
    let bytes = timeout(
        CANONICAL_SYNC_V2_STREAM_TIMEOUT,
        receive.read_to_end(MAX_CANONICAL_SYNC_V2_FRAME_BYTES),
    )
    .await
    .context("read canonical client frame deadline")??;
    decode_and_verify_sync_client_frame_v2(&bytes).map_err(Into::into)
}

pub async fn write_canonical_sync_v2_server_frame(
    send: &mut SendStream,
    frame: &CanonicalSyncServerFrameV2,
) -> Result<()> {
    let bytes = encode_sync_server_frame_v2(frame)?;
    timeout(CANONICAL_SYNC_V2_STREAM_TIMEOUT, send.write_all(&bytes))
        .await
        .context("write canonical server frame deadline")??;
    send.finish().context("finish canonical server frame")?;
    let stopped = timeout(CANONICAL_SYNC_V2_STREAM_TIMEOUT, send.stopped())
        .await
        .context("confirm canonical server frame delivery deadline")?
        .context("confirm canonical server frame delivery")?;
    ensure!(
        stopped.is_none(),
        "canonical client stopped the response stream with code {stopped:?}"
    );
    Ok(())
}

pub async fn read_canonical_sync_v2_server_frame(
    receive: &mut RecvStream,
) -> Result<CanonicalSyncServerFrameV2> {
    let bytes = timeout(
        CANONICAL_SYNC_V2_STREAM_TIMEOUT,
        receive.read_to_end(MAX_CANONICAL_SYNC_V2_FRAME_BYTES),
    )
    .await
    .context("read canonical server frame deadline")??;
    decode_and_verify_sync_server_frame_v2(&bytes).map_err(Into::into)
}
