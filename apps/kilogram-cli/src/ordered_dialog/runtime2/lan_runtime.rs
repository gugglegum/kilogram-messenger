use super::{
    discovery::{
        DiscoveryObservation2, DiscoveryQuota2, NetworkFreshness2, NetworkGeneration2, Publication2,
    },
    lan_metadata::{self, Inspection},
    runtime::{Handle2, start_transport},
    *,
};
use kilogram_transport_iroh::ordered2::bind_fresh;
use kilogram_windows_lan::{Binding, Cancellation, DiscoverySocket, Receive, is_peer_datagram};
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinSet,
    time,
};
use wire::lan_discovery::DATAGRAM_BYTES;

const INSPECT_INTERVAL: Duration = Duration::from_secs(2);
const CLEANUP: Duration = Duration::from_secs(3);
const RECEIVER_WAKE: Duration = Duration::from_millis(100);
const QUEUE: usize = 4;
type Datagram = [u8; DATAGRAM_BYTES];

struct Observation {
    binding: Binding,
    freshness: Arc<NetworkFreshness2>,
}

fn forget(active: &mut Option<Arc<Observation>>, watch: &watch::Sender<Option<Arc<Observation>>>) {
    if let Some(old) = active.take() {
        old.freshness.revoke();
        watch.send_replace(None);
    }
}

async fn cancelled(stop: &mut watch::Receiver<bool>) {
    let _ = stop.wait_for(|stop| *stop).await;
}

async fn monitor(
    mut inspect: impl FnMut() -> Result<Inspection>,
    interval: Duration,
    watch: watch::Sender<Option<Arc<Observation>>>,
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    let mut active: Option<Arc<Observation>> = None;
    let mut pending: Option<Inspection> = None;
    let mut tick = time::interval(interval);
    tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    loop {
        let inspection = async {
            match &mut pending {
                Some(worker) => (worker.started, worker.result().await),
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            biased;
            _ = cancelled(&mut stop) => break,
            (started, result) = inspection => {
                pending = None;
                if let Ok(binding) = result && started.elapsed() < interval {
                    let valid_until = started + interval * 2;
                    if !active.as_ref().is_some_and(|old| old.binding == binding && old.freshness.renew(valid_until)) {
                        forget(&mut active, &watch);
                        let observation = Arc::new(Observation {
                            binding,
                            freshness: Arc::new(NetworkFreshness2::new(valid_until)),
                        });
                        watch.send_replace(Some(observation.clone()));
                        active = Some(observation);
                    }
                } else {
                    forget(&mut active, &watch);
                }
            }
            _ = tick.tick() => {
                if pending.is_some() {
                    // Keep ownership of the pending inspection. Its late result cannot
                    // re-enable the old generation or create further workers.
                    forget(&mut active, &watch);
                } else {
                    pending = inspect().ok();
                    if pending.is_none() {
                        forget(&mut active, &watch);
                    }
                }
            }
        }
    }
    forget(&mut active, &watch);
    if let Some(mut pending) = pending {
        let _ = time::timeout(CLEANUP, pending.result())
            .await
            .context("LAN metadata worker cleanup unconfirmed")?;
    }
    Ok(())
}

struct SocketWorker {
    cancel: Cancellation,
    worker: thread::JoinHandle<()>,
    done: oneshot::Receiver<Result<()>>,
    incoming: mpsc::Receiver<Datagram>,
    outgoing: mpsc::Sender<Publication2>,
}

impl SocketWorker {
    fn open(observation: Arc<Observation>) -> Result<Self> {
        ensure!(
            observation.freshness.is_current(),
            "LAN observation expired before bind"
        );
        let mut socket = DiscoverySocket::open_observed(observation.binding)?;
        let cancel = socket.cancellation();
        let (inbox, incoming) = mpsc::channel(QUEUE);
        let (outgoing, mut outbox) = mpsc::channel::<Publication2>(1);
        let (complete, done) = oneshot::channel();
        let worker = thread::Builder::new()
            .name("kilogram-lan-socket".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    let binding = observation.binding;
                    let mut quota = DiscoveryQuota2::default();
                    while observation.freshness.is_current() {
                        if let Ok(publication) = outbox.try_recv() {
                            publication.send_if_current(now, |raw| socket.send(raw))?;
                        }
                        let mut bytes = [0; DATAGRAM_BYTES];
                        match socket.receive(&mut bytes, Instant::now() + RECEIVER_WAKE)? {
                            Receive::Cancelled => break,
                            Receive::Packet(info)
                                if observation.freshness.is_current()
                                    && is_peer_datagram(
                                        info,
                                        binding.peer(),
                                        binding.if_index(),
                                    )
                                    && quota.take(Instant::now()) =>
                            {
                                let _ = inbox.try_send(bytes);
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                })();
                drop(socket);
                let _ = complete.send(result);
            })?;
        Ok(Self {
            cancel,
            worker,
            done,
            incoming,
            outgoing,
        })
    }

    async fn close(self) -> Result<()> {
        let cancel = self.cancel.cancel();
        let result = time::timeout(CLEANUP, self.done)
            .await
            .context("LAN socket cleanup unconfirmed")?;
        self.worker
            .join()
            .map_err(|_| anyhow::anyhow!("LAN socket worker panicked"))?;
        cancel?;
        // A socket operation error still has a confirmed owned close.
        if let Err(error) = result.context("LAN socket worker result lost")? {
            eprintln!("od2_discovery_socket {error:#}");
        }
        Ok(())
    }
}

fn advertisement_delay(connected: bool, jitter: u16) -> Duration {
    let (low, width) = if connected {
        (14_000, 2_001)
    } else {
        (2_000, 3_001)
    };
    Duration::from_millis(low + u64::from(jitter) % width)
}

async fn exchange(socket: &mut SocketWorker, handle: &Handle2, generation: NetworkGeneration2) {
    let mut announcement_at = time::Instant::now();
    let mut observed: Option<DiscoveryObservation2> = None;
    loop {
        tokio::select! {
            biased;
            _ = time::sleep_until(announcement_at) => {
                let announcement = handle.call(move |owner| {
                    let (publication, observed) = owner.announce_lan(generation)?;
                    let connected = (0..2).all(|c| owner.require(c, true).is_ok_and(|(s, _)| s.head.active));
                    Ok((publication, observed, connected))
                }).await;
                let mut connected = false;
                match announcement {
                    Ok((publication, current, live)) => {
                        if observed.as_ref() != Some(&current) {
                            // Frames already buffered belong to the prior observation.
                            for _ in 0..QUEUE {
                                if socket.incoming.try_recv().is_err() { break; }
                            }
                        }
                        observed = Some(current);
                        connected = live;
                        let _ = socket.outgoing.try_send(publication);
                    }
                    Err(_) => observed = None,
                }
                let jitter = shared_admin::random::<2>().map(u16::from_ne_bytes).unwrap_or(0);
                announcement_at = time::Instant::now() + advertisement_delay(connected, jitter);
            }
            bytes = socket.incoming.recv() => {
                let Some(bytes) = bytes else { break };
                if let Some(observed) = observed.clone() {
                    let _ = handle.call(move |owner| owner.accept_lan_discovery(&observed, &bytes)).await;
                }
            }
        }
    }
}

async fn epoch(
    profile: &config::Profile2,
    instance: Id,
    generation: NetworkGeneration2,
    observation: Arc<Observation>,
    handle: &Handle2,
    observations: &mut watch::Receiver<Option<Arc<Observation>>>,
    stop: &mut watch::Receiver<bool>,
) -> Result<()> {
    let mut socket = match SocketWorker::open(observation.clone()) {
        Ok(socket) => socket,
        Err(error) => {
            eprintln!("od2_discovery_waiting_socket {error:#}");
            return Ok(());
        }
    };
    let mut transport = JoinSet::new();
    let (stop_transport, transport_stop) = watch::channel(false);
    let mut endpoint = None;
    let network = async {
        let bound = time::timeout(INSPECT_INTERVAL, bind_fresh(&profile.route))
            .await
            .context("LAN Iroh bind deadline")??;
        endpoint = Some(bound.clone());
        let proof = observation.freshness.clone();
        let bound_for_owner = bound.clone();
        let targets = handle
            .call(move |owner| {
                owner.attach_lan_generation(generation, proof, instance, bound_for_owner)
            })
            .await?;
        start_transport(
            &mut transport,
            handle,
            &bound,
            profile.route.lan(),
            &profile.enabled,
            Some(targets),
            transport_stop,
        );
        exchange(&mut socket, handle, generation).await;
        Ok::<_, anyhow::Error>(())
    };
    let result = tokio::select! {
        biased;
        _ = cancelled(stop) => Ok(()),
        _ = observations.wait_for(|current| current.as_ref().is_none_or(|current| !Arc::ptr_eq(current, &observation))) => Ok(()),
        result = network => result,
    };
    observation.freshness.revoke();
    stop_transport.send_replace(true);
    let transport_cleanup = time::timeout(CLEANUP, async {
        let mut failure = None;
        while let Some(task) = transport.join_next().await {
            if let Err(error) = task {
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    })
    .await
    .context("LAN transport cleanup unconfirmed");
    if transport_cleanup.is_err() {
        transport.abort_all();
    }
    let socket = socket.close().await;
    let endpoint = async {
        if let Some(endpoint) = endpoint {
            time::timeout(CLEANUP, endpoint.close())
                .await
                .context("LAN endpoint cleanup unconfirmed")?;
        }
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let owner = time::timeout(
        CLEANUP,
        handle.call(move |owner| {
            owner.detach_lan_generation(generation);
            Ok(())
        }),
    )
    .await
    .context("LAN owner detach unconfirmed");
    socket?;
    endpoint?;
    owner??;
    transport_cleanup??;
    if let Err(error) = result {
        eprintln!("od2_discovery_waiting_endpoint {error:#}");
    }
    Ok(())
}

async fn supervise(
    profile: config::Profile2,
    instance: Id,
    handle: Handle2,
    mut observations: watch::Receiver<Option<Arc<Observation>>>,
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    let mut generation = 0u64;
    loop {
        let observation = tokio::select! {
            biased;
            _ = cancelled(&mut stop) => return Ok(()),
            current = observations.wait_for(|current| current.as_ref().is_some_and(|current| current.freshness.is_current())) =>
                current.context("LAN monitor stopped")?.clone().context("LAN observation absent")?,
        };
        generation = generation
            .checked_add(1)
            .context("LAN network generation exhausted")?;
        epoch(
            &profile,
            instance,
            NetworkGeneration2(generation),
            observation,
            &handle,
            &mut observations,
            &mut stop,
        )
        .await?;
        tokio::select! {
            biased;
            _ = cancelled(&mut stop) => return Ok(()),
            _ = time::sleep(INSPECT_INTERVAL) => {}
        }
    }
}

pub(super) async fn run(
    profile: config::Profile2,
    instance: Id,
    handle: Handle2,
    stop: watch::Receiver<bool>,
) -> Result<()> {
    let route = profile.route.lan().context("LAN Auto route")?;
    let scope = lan_metadata::scope(&profile.lan_discovery)?;
    let (reports, observations) = watch::channel(None);
    let (stop_monitor, monitor_stop) = watch::channel(false);
    let monitor = tokio::spawn(monitor(
        move || Inspection::start(route, Some(scope)),
        INSPECT_INTERVAL,
        reports,
        monitor_stop,
    ));
    let result = supervise(profile, instance, handle, observations, stop).await;
    stop_monitor.send_replace(true);
    let monitoring = monitor.await.context("LAN metadata monitor failed")?;
    result.and(monitoring)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ordered_dialog::runtime2::discovery::DISCOVERY_VALIDATIONS_PER_SECOND;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn t13d_revocation_and_quota_are_bounded() -> Result<()> {
        let start = Instant::now();
        let proof = NetworkFreshness2::new(start + Duration::from_secs(5));
        ensure!(proof.is_current());
        ensure!(proof.renew(start + Duration::from_secs(10)));
        proof.revoke();
        ensure!(!proof.is_current() && !proof.renew(start + Duration::from_secs(10)));
        let expired = NetworkFreshness2::new(start);
        ensure!(!expired.is_current() && !expired.renew(start + Duration::from_secs(10)));
        let mut quota = DiscoveryQuota2::default();
        for _ in 0..DISCOVERY_VALIDATIONS_PER_SECOND {
            ensure!(quota.take(start));
        }
        ensure!(!quota.take(start + Duration::from_millis(999)));
        ensure!(quota.take(start + Duration::from_secs(1)));
        let (queue, _receiver) = mpsc::channel::<Datagram>(QUEUE);
        for _ in 0..QUEUE {
            queue.try_send([0; DATAGRAM_BYTES])?;
        }
        ensure!(queue.try_send([0; DATAGRAM_BYTES]).is_err());
        for jitter in 0..=u16::MAX {
            ensure!(
                (Duration::from_secs(2)..=Duration::from_secs(5))
                    .contains(&advertisement_delay(false, jitter))
            );
            ensure!(
                (Duration::from_secs(14)..=Duration::from_secs(16))
                    .contains(&advertisement_delay(true, jitter))
            );
        }
        println!(
            "T13D_LIMITS queue=4 validations_per_second=8 searching_ms=2000..5000 connected_ms=14000..16000"
        );
        Ok(())
    }

    #[tokio::test]
    async fn t13d_slow_metadata_does_not_spawn_more_workers() -> Result<()> {
        let launches = Arc::new(AtomicUsize::new(0));
        let counter = launches.clone();
        let (release, hold) = std::sync::mpsc::sync_channel(1);
        let mut first = Some(hold);
        let (closing, stop) = watch::channel(false);
        let (reports, observed) = watch::channel(None);
        let task = tokio::spawn(monitor(
            move || {
                counter.fetch_add(1, Ordering::Relaxed);
                let hold = first.take();
                Inspection::spawn(move || {
                    if let Some(hold) = hold {
                        let _ = hold.recv();
                    }
                    anyhow::bail!("synthetic unavailable network")
                })
            },
            Duration::from_millis(30),
            reports,
            stop,
        ));
        time::timeout(Duration::from_secs(2), async {
            while launches.load(Ordering::Relaxed) == 0 {
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await?;
        time::sleep(Duration::from_millis(160)).await;
        ensure!(launches.load(Ordering::Relaxed) == 1 && observed.borrow().is_none());
        closing.send_replace(true);
        release.send(())?;
        time::timeout(Duration::from_secs(4), task).await???;
        ensure!(launches.load(Ordering::Relaxed) == 1);
        println!("T13D_METADATA launches=1 delayed_ms=160 interval_ms=30 worker_joined=true");
        Ok(())
    }
}
