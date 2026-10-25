//! Bounded local operator relay for reproducible S4 process verification.
use anyhow::{Context, Result, ensure};
use iroh_relay::server::{RelayConfig, Server, ServerConfig};
use std::{fs::OpenOptions, io::Write, net::Ipv4Addr, path::PathBuf, time::Duration};
#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(args.len() == 2, "ready-file stop-file");
    let ready = PathBuf::from(&args[0]);
    let stop = PathBuf::from(&args[1]);
    let pending = ready.with_extension("pending");
    ensure!(
        !ready.exists() && !stop.exists() && !pending.exists(),
        "fresh relay control files required"
    );
    let mut config = ServerConfig::default();
    config.relay = Some(RelayConfig::new((Ipv4Addr::LOCALHOST, 0)));
    let relay = Server::spawn(config)
        .await
        .context("spawn own loopback relay")?;
    let address = relay.http_addr().context("relay HTTP listener")?;
    ensure!(address.ip().is_loopback(), "loopback relay");
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .with_context(|| format!("create relay ready temporary {}", pending.display()))?;
    marker
        .write_all(format!("http://{address}/").as_bytes())
        .with_context(|| format!("write relay ready temporary {}", pending.display()))?;
    marker
        .sync_all()
        .with_context(|| format!("sync writable relay ready temporary {}", pending.display()))?;
    drop(marker);
    std::fs::rename(&pending, &ready).with_context(|| {
        format!(
            "atomically publish relay ready {} -> {}",
            pending.display(),
            ready.display()
        )
    })?;
    let deadline = std::time::Instant::now() + Duration::from_secs(3600);
    while !stop.exists() && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    relay.shutdown().await?;
    Ok(())
}
