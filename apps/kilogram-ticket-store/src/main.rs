use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use kilogram_ticket_store::{
    DEFAULT_GLOBAL_REQUESTS_PER_MINUTE, DEFAULT_MAX_CHANNELS, DEFAULT_MAX_CONCURRENT_CONNECTIONS,
    DEFAULT_MAX_RECORD_BYTES, DEFAULT_MAX_TOTAL_BYTES, DEFAULT_PER_IP_REQUESTS_PER_MINUTE,
    DEFAULT_RETENTION_SECONDS, StoreConfig, TicketStoreServer,
};

#[derive(Debug, Parser)]
#[command(name = "kilogram-ticket-store")]
#[command(about = "Loopback-only opaque ticket and blind mailbox store for an HTTPS reverse proxy")]
struct Arguments {
    /// Loopback HTTP address used by the local HTTPS reverse proxy.
    #[arg(long, default_value = "127.0.0.1:8787")]
    listen: SocketAddr,

    /// Directory containing the durable Redb database.
    #[arg(long)]
    data_dir: PathBuf,

    /// Fixed service-side retention applied to every accepted value.
    #[arg(long, default_value_t = DEFAULT_RETENTION_SECONDS)]
    retention_seconds: u64,

    /// Maximum opaque request body accepted by PUT.
    #[arg(long, default_value_t = DEFAULT_MAX_RECORD_BYTES)]
    max_record_bytes: usize,

    /// Maximum number of non-expired channels retained at once.
    #[arg(long, default_value_t = DEFAULT_MAX_CHANNELS)]
    max_channels: u64,

    /// Maximum combined bytes of all non-expired opaque bodies.
    #[arg(long, default_value_t = DEFAULT_MAX_TOTAL_BYTES)]
    max_total_bytes: u64,

    /// Requests accepted from one source IP in a fixed one-minute window.
    #[arg(long, default_value_t = DEFAULT_PER_IP_REQUESTS_PER_MINUTE)]
    per_ip_requests_per_minute: u64,

    /// Requests accepted globally in a fixed one-minute window.
    #[arg(long, default_value_t = DEFAULT_GLOBAL_REQUESTS_PER_MINUTE)]
    global_requests_per_minute: u64,

    /// Maximum simultaneous HTTP connections.
    #[arg(long, default_value_t = DEFAULT_MAX_CONCURRENT_CONNECTIONS)]
    max_concurrent_connections: usize,

    /// Trust an HTTPS reverse proxy that overwrites X-Real-IP with the client IP.
    #[arg(long)]
    trust_x_real_ip: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = Arguments::parse();
    let server = TicketStoreServer::bind(StoreConfig {
        listen: arguments.listen,
        data_dir: arguments.data_dir,
        retention_seconds: arguments.retention_seconds,
        max_record_bytes: arguments.max_record_bytes,
        max_channels: arguments.max_channels,
        max_total_bytes: arguments.max_total_bytes,
        per_ip_requests_per_minute: arguments.per_ip_requests_per_minute,
        global_requests_per_minute: arguments.global_requests_per_minute,
        max_concurrent_connections: arguments.max_concurrent_connections,
        trust_x_real_ip: arguments.trust_x_real_ip,
    })
    .await?;
    println!("status=listening");
    println!("listen_address={}", server.local_addr());
    println!("transport_security=reverse-proxy-https-required");
    println!("storage_format=opaque-redb-v1");
    println!("blind_mailbox_store_key={}", server.mailbox_store_key());
    println!("blind_mailbox_transport=reverse-proxy-https-required");
    println!("retention_seconds={}", server.config().retention_seconds);
    println!("max_record_bytes={}", server.config().max_record_bytes);
    println!("max_channels={}", server.config().max_channels);
    println!("max_total_bytes={}", server.config().max_total_bytes);
    println!(
        "per_ip_requests_per_minute={}",
        server.config().per_ip_requests_per_minute
    );
    println!(
        "global_requests_per_minute={}",
        server.config().global_requests_per_minute
    );
    println!(
        "rate_limit_identity={}",
        if server.config().trust_x_real_ip {
            "trusted-proxy-x-real-ip"
        } else {
            "tcp-peer-ip"
        }
    );
    server
        .run_until(async { tokio::signal::ctrl_c().await.context("wait for Ctrl+C") })
        .await?;
    println!("status=stopped");
    Ok(())
}
