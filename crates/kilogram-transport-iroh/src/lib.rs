use anyhow::{Context, Result};
use iroh::endpoint::{RecvStream, SendStream};
use kilogram_protocol::{ClientRequest, ServerResponse};

pub const ALPN: &[u8] = b"kilogram/m0/sync/1";
pub const MAX_WIRE_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

pub async fn read_client_request(receive: &mut RecvStream) -> Result<ClientRequest> {
    let bytes = receive
        .read_to_end(MAX_WIRE_MESSAGE_BYTES)
        .await
        .context("read client protocol request")?;
    ClientRequest::decode(&bytes).context("decode and verify client protocol request")
}

pub async fn write_client_request(send: &mut SendStream, request: &ClientRequest) -> Result<()> {
    send.write_all(&request.encode()?)
        .await
        .context("send client protocol request")?;
    send.finish().context("finish client protocol request")?;
    Ok(())
}

pub async fn read_server_response(receive: &mut RecvStream) -> Result<ServerResponse> {
    let bytes = receive
        .read_to_end(MAX_WIRE_MESSAGE_BYTES)
        .await
        .context("read server protocol response")?;
    ServerResponse::decode(&bytes).context("decode and verify server protocol response")
}

pub async fn write_server_response(send: &mut SendStream, response: &ServerResponse) -> Result<()> {
    send.write_all(&response.encode()?)
        .await
        .context("send server protocol response")?;
    send.finish().context("finish server protocol response")?;
    Ok(())
}
