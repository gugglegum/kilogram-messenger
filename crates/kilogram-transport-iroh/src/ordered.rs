//! Opt-in OD1 framing and actual TLS-exporter binding. No legacy fallback.
#[cfg(test)]
use anyhow::Context;
use anyhow::{Result, ensure};
use iroh::{
    Endpoint,
    endpoint::{Connection, RecvStream, SendStream, presets},
};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Frame, Id, a, b};
use std::net::SocketAddr;

/// Loopback acceptance still uses real UDP/QUIC/TLS sockets. Minimal has no
/// external lookup services; default IP sockets are replaced with a bound IP.
pub async fn bind_loopback(address: SocketAddr) -> Result<Endpoint> {
    ensure!(
        address.ip().is_loopback(),
        "OD1 local runtime requires a loopback bind"
    );
    Ok(Endpoint::builder(presets::Minimal)
        .transport_config(
            iroh::endpoint::QuicTransportConfig::builder()
                .max_concurrent_bidi_streams(4_u32.into())
                .max_concurrent_uni_streams(0_u32.into())
                .receive_window((4_u32 * 1024 * 1024).into())
                .build(),
        )
        .clear_ip_transports()
        .clear_relay_transports()
        .alpns(vec![wire::ALPN.to_vec()])
        .bind_addr(address)?
        .bind()
        .await?)
}

pub fn binding(connection: &Connection, local: Id, dialer: bool) -> Result<Id> {
    ensure!(connection.alpn() == wire::ALPN, "unsupported OD1 ALPN");
    let remote = *connection.remote_id().as_bytes();
    let (a_id, b_id) = if dialer {
        (local, remote)
    } else {
        (remote, local)
    };
    let context = wire::encode(&a([b(wire::ALPN), b(a_id), b(b_id)]), 256)?;
    let mut material = [0_u8; 32];
    connection
        .export_keying_material(&mut material, b"kilogram ordered-dialog v1", &context)
        .map_err(|_| anyhow::anyhow!("OD1 TLS exporter failed"))?;
    let id = wire::hash(b"kg:od1:channel-binding\0", &material);
    material.fill(0);
    Ok(id)
}

pub async fn write(send: &mut SendStream, frame: &Frame) -> Result<Vec<u8>> {
    let raw = frame.raw()?;
    ensure!(raw.len() + 4 <= wire::MAX_WIRE, "OD1 full frame byte cap");
    let length = u32::try_from(raw.len())?;
    send.write_all(&length.to_be_bytes()).await?;
    send.write_all(&raw).await?;
    send.finish()?;
    Ok(raw)
}

pub async fn read(receive: &mut RecvStream) -> Result<(Frame, Vec<u8>)> {
    let mut length = [0; 4];
    receive.read_exact(&mut length).await?;
    let length = usize::try_from(u32::from_be_bytes(length))?;
    ensure!(
        length > 0 && length <= wire::MAX_WIRE - 4,
        "OD1 wire length cap"
    );
    let mut raw = vec![0; length];
    receive.read_exact(&mut raw).await?;
    let mut trailing = [0; 1];
    ensure!(
        receive.read(&mut trailing).await?.is_none(),
        "OD1 trailing stream bytes"
    );
    Ok((Frame::parse(&raw)?, raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn ordered_alpn_does_not_downgrade_to_a_real_legacy_peer() -> Result<()> {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let ordered = bind_loopback("127.0.0.1:0".parse()?).await?;
            let legacy = Endpoint::builder(presets::Minimal)
                .clear_ip_transports()
                .clear_relay_transports()
                .alpns(vec![crate::ALPN.to_vec()])
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>()?)?
                .bind()
                .await?;
            let (dial, accept) = tokio::join!(ordered.connect(legacy.addr(), wire::ALPN), async {
                legacy
                    .accept()
                    .await
                    .context("legacy incoming")?
                    .await
                    .map_err(anyhow::Error::from)
            });
            assert!(dial.is_err() && accept.is_err());
            ordered.close().await;
            legacy.close().await;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("mixed ALPN rejection timeout")??;
        Ok(())
    }
    async fn pair(a: &Endpoint, b: &Endpoint) -> Result<(Connection, Connection)> {
        let (left, right) = tokio::try_join!(
            async { Ok::<_, anyhow::Error>(a.connect(b.addr(), wire::ALPN).await?) },
            async { Ok::<_, anyhow::Error>(b.accept().await.context("missing incoming")?.await?) },
        )?;
        Ok((left, right))
    }
    #[tokio::test]
    async fn ordered_exporter_is_symmetric_and_changes_on_real_reconnect() -> Result<()> {
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            let a = bind_loopback("127.0.0.1:0".parse()?).await?;
            let b = bind_loopback("127.0.0.1:0".parse()?).await?;
            let (left, right) = pair(&a, &b).await?;
            let first = binding(&left, *a.id().as_bytes(), true)?;
            assert_eq!(first, binding(&right, *b.id().as_bytes(), false)?);
            left.close(0_u32.into(), b"reconnect");
            right.close(0_u32.into(), b"reconnect");
            let (left, right) = pair(&a, &b).await?;
            let second = binding(&left, *a.id().as_bytes(), true)?;
            assert_eq!(second, binding(&right, *b.id().as_bytes(), false)?);
            assert_ne!(first, second);
            left.close(0_u32.into(), b"done");
            right.close(0_u32.into(), b"done");
            a.close().await;
            b.close().await;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("OD1 actual TLS reconnect timeout")??;
        Ok(())
    }
}
