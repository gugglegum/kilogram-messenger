use std::{
    collections::BTreeSet,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use tokio::{net::UdpSocket, task::JoinHandle, time::timeout};

use crate::recovery_link::MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES;

const HISTORY_RECOVERY_LINK_PREFIX: &str = "kilogram://history-recovery/v1/";
const DISCOVERY_MULTICAST_ADDRESS: Ipv4Addr = Ipv4Addr::new(239, 255, 75, 71);
const DISCOVERY_LOOPBACK_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;
const DISCOVERY_PORT: u16 = 45_371;
const DISCOVERY_PUBLICATION_INTERVAL: Duration = Duration::from_millis(750);
const MAX_DISCOVERY_DATAGRAM_BYTES: usize = 4_096;
const MAX_DISCOVERY_DATAGRAMS: usize = 512;

pub(crate) const DEFAULT_DISCOVERY_WAIT_SECONDS: u64 = 3;
pub(crate) const MAX_DISCOVERY_WAIT_SECONDS: u64 = 30;
pub(crate) const DEFAULT_DISCOVERY_CANDIDATES: usize = 8;
pub(crate) const MAX_DISCOVERY_CANDIDATES: usize = 16;

pub(crate) struct RecoveryDiscoveryPublisher {
    task: JoinHandle<()>,
}

impl Drop for RecoveryDiscoveryPublisher {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RecoveryDiscoveryPublication {
    pub multicast_initial_sent: bool,
    pub loopback_initial_sent: bool,
}

#[derive(Debug)]
pub(crate) struct RecoveryDiscoveryScan {
    pub candidates: Vec<String>,
    pub datagrams_received: usize,
    pub datagrams_rejected: usize,
    pub duplicate_candidates: usize,
    pub multicast_joined: bool,
    pub datagram_limit_reached: bool,
    pub candidate_limit_reached: bool,
}

pub(crate) fn multicast_target() -> SocketAddrV4 {
    SocketAddrV4::new(DISCOVERY_MULTICAST_ADDRESS, DISCOVERY_PORT)
}

pub(crate) fn loopback_target() -> SocketAddrV4 {
    SocketAddrV4::new(DISCOVERY_LOOPBACK_ADDRESS, DISCOVERY_PORT)
}

pub(crate) fn publication_interval() -> Duration {
    DISCOVERY_PUBLICATION_INTERVAL
}

pub(crate) async fn start_recovery_discovery_publisher(
    payload: String,
) -> Result<(RecoveryDiscoveryPublisher, RecoveryDiscoveryPublication)> {
    validate_payload_shape(&payload)?;
    let payload: Arc<[u8]> = Arc::from(payload.into_bytes());
    let socket = Arc::new(
        UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
            .await
            .context("bind history recovery discovery publisher")?,
    );
    socket
        .set_multicast_ttl_v4(1)
        .context("limit history recovery discovery multicast to the local network")?;
    socket
        .set_multicast_loop_v4(true)
        .context("enable local history recovery discovery multicast loop")?;

    let multicast_initial_sent = socket.send_to(&payload, multicast_target()).await.is_ok();
    let loopback_initial_sent = socket.send_to(&payload, loopback_target()).await.is_ok();
    ensure!(
        multicast_initial_sent || loopback_initial_sent,
        "publish initial history recovery discovery datagram"
    );

    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(DISCOVERY_PUBLICATION_INTERVAL);
        interval.tick().await;
        loop {
            interval.tick().await;
            let _ = socket.send_to(&payload, multicast_target()).await;
            let _ = socket.send_to(&payload, loopback_target()).await;
        }
    });

    Ok((
        RecoveryDiscoveryPublisher { task },
        RecoveryDiscoveryPublication {
            multicast_initial_sent,
            loopback_initial_sent,
        },
    ))
}

pub(crate) async fn discover_recovery_links(
    wait: Duration,
    max_candidates: usize,
    mut accepts: impl FnMut(&str) -> bool,
) -> Result<RecoveryDiscoveryScan> {
    ensure!(
        !wait.is_zero() && wait <= Duration::from_secs(MAX_DISCOVERY_WAIT_SECONDS),
        "history recovery discovery wait must be between 1ms and {MAX_DISCOVERY_WAIT_SECONDS}s"
    );
    ensure!(
        (1..=MAX_DISCOVERY_CANDIDATES).contains(&max_candidates),
        "history recovery discovery candidate limit must be between 1 and {MAX_DISCOVERY_CANDIDATES}"
    );

    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT))
        .await
        .with_context(|| {
            format!(
                "bind history recovery discovery receiver on UDP {}",
                DISCOVERY_PORT
            )
        })?;
    let multicast_joined = socket
        .join_multicast_v4(DISCOVERY_MULTICAST_ADDRESS, Ipv4Addr::UNSPECIFIED)
        .is_ok();
    let deadline = tokio::time::Instant::now() + wait;
    let mut buffer = [0_u8; MAX_DISCOVERY_DATAGRAM_BYTES];
    let mut accepted = BTreeSet::new();
    let mut datagrams_received = 0;
    let mut datagrams_rejected = 0;
    let mut duplicate_candidates = 0;
    let mut candidate_limit_reached = false;

    while datagrams_received < MAX_DISCOVERY_DATAGRAMS {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let received = match timeout(remaining, socket.recv_from(&mut buffer)).await {
            Ok(result) => result.context("receive history recovery discovery datagram")?,
            Err(_) => break,
        };
        datagrams_received += 1;
        let payload = match std::str::from_utf8(&buffer[..received.0]) {
            Ok(payload) if validate_payload_shape(payload).is_ok() => payload.trim(),
            _ => {
                datagrams_rejected += 1;
                continue;
            }
        };
        if !accepts(payload) {
            datagrams_rejected += 1;
            continue;
        }
        if accepted.contains(payload) {
            duplicate_candidates += 1;
            continue;
        }
        if accepted.len() == max_candidates {
            candidate_limit_reached = true;
            break;
        }
        accepted.insert(payload.to_owned());
    }

    Ok(RecoveryDiscoveryScan {
        candidates: accepted.into_iter().collect(),
        datagrams_received,
        datagrams_rejected,
        duplicate_candidates,
        multicast_joined,
        datagram_limit_reached: datagrams_received == MAX_DISCOVERY_DATAGRAMS,
        candidate_limit_reached,
    })
}

fn validate_payload_shape(payload: &str) -> Result<()> {
    let payload = payload.trim();
    ensure!(
        !payload.is_empty(),
        "history recovery discovery payload is empty"
    );
    ensure!(
        payload.len() <= MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES,
        "history recovery discovery payload exceeds the signed-link limit"
    );
    ensure!(
        payload.is_ascii(),
        "history recovery discovery payload is not ASCII"
    );
    ensure!(
        payload.starts_with(HISTORY_RECOVERY_LINK_PREFIX),
        "history recovery discovery payload has an unsupported prefix"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback_publication_is_bounded_deduplicated_and_opt_in() -> Result<()> {
        let payload = format!("{HISTORY_RECOVERY_LINK_PREFIX}test-payload");
        let scan_payload = payload.clone();
        let scan = tokio::spawn(async move {
            discover_recovery_links(Duration::from_millis(900), 2, |candidate| {
                candidate == scan_payload
            })
            .await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let (_publisher, publication) = start_recovery_discovery_publisher(payload.clone()).await?;
        let report = scan.await.context("join discovery scan")??;

        assert!(publication.multicast_initial_sent || publication.loopback_initial_sent);
        assert_eq!(report.candidates, vec![payload]);
        assert!(report.datagrams_received >= 1);
        assert!(!report.candidate_limit_reached);
        Ok(())
    }
}
