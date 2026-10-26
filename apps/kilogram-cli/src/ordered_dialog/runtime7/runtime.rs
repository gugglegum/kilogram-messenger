//! One owner thread/mailbox16, global connection permits2, exact IPC3.
use super::*;
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse, RuntimeIpcServer};
use kilogram_transport_iroh::ordered as transport;
use owner::Owner7;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
pub(super) type Job7 = Box<dyn FnOnce(&mut Owner7) + Send>;
#[derive(Clone)]
pub(super) struct Handle7 {
    sender: mpsc::Sender<Job7>,
    pub stop: Arc<AtomicBool>,
    resources: Arc<resources::Resources7>,
}
impl Handle7 {
    pub async fn call<T: Send + 'static, F: FnOnce(&mut Owner7) -> Result<T> + Send + 'static>(
        &self,
        f: F,
    ) -> Result<T> {
        let permit = self
            .sender
            .reserve()
            .await
            .context("indexed owner stopped")?;
        let (tx, rx) = oneshot::channel();
        self.resources
            .queued(self.sender.max_capacity() - self.sender.capacity());
        permit.send(Box::new(move |o| {
            let _ = tx.send(f(o));
        }));
        rx.await.context("indexed owner reply cancelled")?
    }
}
fn actor(mut owner: Owner7) -> Result<(Handle7, std::thread::JoinHandle<()>)> {
    let (sender, mut receiver) = mpsc::channel::<Job7>(16);
    let stop = Arc::new(AtomicBool::new(false));
    owner.resources.mailbox(&sender);
    let handle = Handle7 {
        sender,
        stop: stop.clone(),
        resources: owner.resources.clone(),
    };
    let join = std::thread::Builder::new()
        .name("kilogram-indexed-L1-owner".into())
        .spawn(move || {
            let mut next = 0;
            let mut receive = [false; 2];
            let mut background_error: [Option<String>; 2] = [None, None];
            while !stop.load(Ordering::Acquire) {
                let before = owner.stats.packet_attempts;
                match receiver.try_recv() {
                    Ok(job) => job(&mut owner),
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                    Err(_) => (),
                }
                if let Err(error) = owner.recover() {
                    eprintln!("l1_owner_recovery_failed {error:#}");
                    stop.store(true, Ordering::Release);
                    break;
                }
                if !owner.enabled.is_empty() && owner.stats.packet_attempts == before {
                    let c = owner.enabled[next % owner.enabled.len()];
                    next = (next + 1) % owner.enabled.len();
                    let result = if receive[c as usize] {
                        owner.drain(c)
                    } else {
                        owner.materialize(c)
                    };
                    receive[c as usize] = !receive[c as usize];
                    if let Err(error) = result {
                        let message = format!("{error:#}");
                        if background_error[c as usize].as_ref() != Some(&message) {
                            eprintln!("l1_background c={c} error={message}");
                            background_error[c as usize] = Some(message);
                        }
                    } else if matches!(result, Ok(true)) {
                        background_error[c as usize] = None;
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            for c in 0..owner.specs.len() {
                owner.invalidate(c as u8);
            }
        })?;
    Ok((handle, join))
}

fn publish_file(owner: &mut Owner7, c: u8, directory: &Path) -> Result<Vec<u8>> {
    owner.current(c)?;
    std::fs::create_dir_all(directory)?;
    let context = owner.spec(c)?.c;
    let role = owner.role(c)?;
    let path = directory.join(format!("{}.locator", state::hex(&context)));
    if path.exists() {
        let raw = admin::read_public(&path, 8192)?;
        Descriptor::parse(owner.spec(c)?, role, &raw).context("unowned locator artifact")?;
    }
    let raw = owner.publish_locator(c)?;
    if path.exists() && admin::read_public(&path, 8192)? == raw {
        return Ok(raw);
    }
    let mut temp = tempfile::NamedTempFile::new_in(directory)?;
    use std::io::Write;
    temp.write_all(&raw)?;
    temp.as_file().sync_all()?;
    temp.persist(&path).map_err(|e| e.error)?;
    Ok(raw)
}
fn status(owner: &mut Owner7) -> Result<Vec<u8>> {
    let mut contexts = Vec::new();
    for c in 0..owner.specs.len() {
        let c = c as u8;
        let head = owner.head(c)?;
        let meta = owner.repo()?.metadata(c)?;
        let counts = owner.repo()?.counts(c)?;
        let repair = owner.repo()?.repair_state(c)?;
        let policy = owner.current(c).err().map(|e| format!("{e:#}"));
        let active = owner.active(c)?;
        let public_native = if policy.is_none() && head.phase == 3 {
            let native = owner.native(c, &head)?;
            native.session.as_ref().map(|s|serde_json::json!({"sid":s.sid(),"keys":s.keys().iter().map(|k|state::hex(k)).collect::<Vec<_>>(),"own_next":meta.next[head.role as usize],"peer_next":meta.next[1-head.role as usize]}))
        } else {
            None
        };
        let own = owner.descriptor(c, true).ok();
        let published = policy.is_none()
            && own.as_ref().is_some_and(|d| {
                owner
                    .endpoint
                    .as_ref()
                    .is_some_and(|e| d.endpoint == *e.id.as_bytes())
                    && d.valid_now(now().unwrap_or(u64::MAX)).is_ok()
            });
        let consent = owner.control(c, store::L1Control::Consent)?;
        let consent_ref = state::hex(&wire::hash(handshake::CONSENT, &consent));
        let own_revision = own.as_ref().map(|d| d.revision);
        let peer_revision = owner.descriptor(c, false).ok().map(|d| d.revision);
        let lease_generation = owner.leases[usize::from(c)]
            .as_ref()
            .filter(|lease| {
                policy.is_none()
                    && lease.policy_generation == meta.policy_generation
                    && lease.gate().is_ok()
            })
            .map(|lease| lease.generation);
        contexts.push(serde_json::json!({"slot":c,"context":state::hex(&head.c),"enabled":owner.enabled.contains(&c),"role":head.role,"phase":head.phase,"native_generation":head.native_generation,"native_hash":head.blob.map(|b|state::hex(&b)),"d":head.d.map(|b|state::hex(&b)),"e":head.e.map(|b|state::hex(&b)),"sid":head.sid,"mode":"L1","wire_stage":1,"active":active,"lease_current":lease_generation.is_some(),"lease_generation":lease_generation,"publication_ready":published,"policy_error":policy,"policy_generation":meta.policy_generation,"wall_high_water":meta.wall_high_water,"current_consent":consent_ref,"own_locator_revision":own_revision,"peer_locator_revision":peer_revision,"next":meta.next,"counts":counts,"repair_free":repair.repair_free,"head_pending":repair.head,"native":public_native,"handoff":"UnsupportedInCut2","provenance":"LegacySnapshot/FullL1"}));
    }
    let generation = owner.repo()?.generation();
    let source = state::hex(&owner.repo()?.source_id());
    let backend = owner.repo()?.backend_stats()?;
    let (config_revision, config_hash) = owner.repo()?.config_status()?;
    Ok(serde_json::to_vec(
        &serde_json::json!({"version":3,"profile_mode":"ordered-dialog-indexed","storage_stage":"Ready","source_id":source,"root_generation":generation,"config_revision":config_revision,"config_hash":state::hex(&config_hash),"contexts":contexts,"stats":owner.stats,"window":owner.window.snapshot(),"resources":owner.resources.snapshot(),"backend":backend}),
    )?)
}
fn ipc(owner: &mut Owner7, raw: &[u8], locator_directory: &Path) -> Result<Vec<u8>> {
    let value = wire::decode(raw, 32 * 1024)?;
    let f = value.list()?;
    ensure!(
        f.len() >= 2 && f[0].uint()? == 3,
        "UnsupportedVersion: expected IPC3"
    );
    let tag = f[1].uint()?;
    let slot = |v: &Value| -> Result<u8> {
        let n = v.uint()?;
        ensure!(n < 2, "slot range");
        Ok(n as u8)
    };
    let result = match tag {
        1 => {
            value.fields(2)?;
            b(status(owner)?)
        }
        2 => {
            let f = value.fields(5)?;
            let c = slot(&f[2])?;
            ensure!(owner.enabled.contains(&c), "C not enabled");
            let result = owner.queue(c, f[3].fixed()?, f[4].text()?.to_owned())?;
            a([opt(result.queue_sequence.map(u)), opt(result.event.map(b))])
        }
        3 => {
            let f = value.fields(4)?;
            let c = slot(&f[2])?;
            let request = f[3].fixed()?;
            let result = owner.repo()?.request(c, request)?;
            if let Some(result) = result {
                let delivered = if let Some(id) = result.event {
                    let raw = owner.repo()?.event(c, id)?.context("request source")?;
                    let parsed = wire::decode(&raw, wire::MAX_EVENT)?;
                    let core = wire::Core::parse(&parsed.fields(5)?[1])?;
                    owner.repo()?.receipt(c, core.role as u8, core.n)?.is_some()
                } else {
                    false
                };
                opt(Some(a([
                    opt(result.queue_sequence.map(u)),
                    opt(result.event.map(b)),
                    u(u64::from(delivered)),
                ])))
            } else {
                opt(None)
            }
        }
        4 => {
            let f = value.fields(5)?;
            let c = slot(&f[2])?;
            let cursor = f[3]
                .option()?
                .map(|v| -> Result<db::HistoryCursor> {
                    Ok(db::HistoryCursor::from_bytes(v.bytes()?)?)
                })
                .transpose()?;
            let limit = usize::try_from(f[4].uint()?)?;
            let _refs = owner.window.reserve(c, 34)?;
            b(owner.repo()?.history(c, cursor.as_ref(), limit)?)
        }
        5 => {
            let f = value.fields(6)?;
            let c = slot(&f[2])?;
            ensure!(f[3].uint()? == 1, "UnsupportedInCut2: locator family2");
            let raw = f[5].option()?;
            match f[4].uint()? {
                0 => {
                    owner.import_locator(
                        c,
                        raw.context("locator import requires Some")?
                            .bytes()?
                            .to_vec(),
                    )?;
                    a([])
                }
                1 => {
                    ensure!(raw.is_none(), "locator export requires None");
                    b(publish_file(owner, c, locator_directory)?)
                }
                _ => anyhow::bail!("unknown locator action"),
            }
        }
        6 => {
            let f = value.fields(3)?;
            owner.pin_authority(f[2].bytes()?)?;
            a([])
        }
        11 => {
            let f = value.fields(5)?;
            let c = slot(&f[2])?;
            let expected = f[3].option()?.map(Value::fixed::<32>).transpose()?;
            owner.renew_consent(c, expected, f[4].bytes()?.to_vec())?;
            a([])
        }
        35 => {
            let f = value.fields(6)?;
            let raw = f[5].bytes()?;
            ensure!(raw.len() <= 16 * 1024, "candidate config cap");
            let config: db::migration7::MigrationConfig = serde_json::from_slice(raw)?;
            owner.capacity(f[2].uint()?, f[3].uint()?, f[4].fixed()?, config)?;
            a([])
        }
        7..=10 => anyhow::bail!("UnsupportedInCut2: Consent2/handoff"),
        32..=34 => anyhow::bail!("MigrationAdminOnly"),
        _ => anyhow::bail!("UnsupportedInCut2: IPC3 tag"),
    };
    Ok(wire::encode(&a([u(3), u(tag), result]), 48 * 1024)?)
}

pub async fn run(profile: Profile3, profile_path: PathBuf) -> Result<()> {
    ensure!(
        !profile.enabled_contexts.is_empty(),
        "NeedContext: zero-C enrollment has no enabled runtime"
    );
    let repository = profile.open(&profile_path)?;
    let enabled = profile
        .enabled_contexts
        .iter()
        .map(|s| state::parse_id(s))
        .collect::<Result<Vec<_>>>()?;
    let mut owner = Owner7::new(repository, &enabled)?;
    let endpoint = transport::bind_loopback("127.0.0.1:0".parse()?).await?;
    owner.endpoint = Some(endpoint.addr());
    for c in owner.enabled.clone() {
        if let Err(error) = publish_file(&mut owner, c, &profile.locator_directory) {
            eprintln!("l1_publication_blocked slot={c} {error:#}");
        }
    }
    let account = owner.account;
    let device = owner.device.identity().device_id();
    let (server, mut work) =
        RuntimeIpcServer::start(profile.ipc_file.clone(), account, owner.device.identity()).await?;
    let enabled = owner.enabled.clone();
    let contexts = owner.specs.iter().map(|s| s.c).collect::<Vec<_>>();
    let roles = enabled
        .iter()
        .map(|c| Ok((*c, owner.role(*c)?)))
        .collect::<Result<Vec<_>>>()?;
    let resources = owner.resources.clone();
    let (handle, join) = actor(owner)?;
    let permits = Arc::new(Semaphore::new(2));
    let mut tasks = tokio::task::JoinSet::new();
    let inbound = Arc::new(
        roles
            .iter()
            .filter(|(_, role)| *role == 1)
            .map(|(c, _)| (contexts[*c as usize], Arc::new(Mutex::new(()))))
            .collect::<std::collections::BTreeMap<_, _>>(),
    );
    let accept_endpoint = endpoint.clone();
    let accept_handle = handle.clone();
    let accept_permits = permits.clone();
    let accept_resources = resources.clone();
    tasks.spawn(async move {
        let mut children = tokio::task::JoinSet::new();
        while let Some(incoming) = accept_endpoint.accept().await {
            let Ok(permit) = accept_permits.clone().try_acquire_owned() else {
                accept_resources.refused();
                incoming.refuse();
                continue;
            };
            let accounting = accept_resources.acquired();
            let endpoint = accept_endpoint.clone();
            let handle = accept_handle.clone();
            let inbound = inbound.clone();
            children.spawn(async move {
                let result = async {
                    let connection = tokio::time::timeout(Duration::from_secs(15), incoming)
                        .await
                        .context("L1 TLS timeout")??;
                    let (link, _guard) = tokio::time::timeout(
                        Duration::from_secs(30),
                        network::admit_b(&handle, &endpoint, &connection, &inbound),
                    )
                    .await
                    .context("L1 admission timeout")??;
                    network::established(handle, connection, link, None).await
                }
                .await;
                if let Err(error) = result {
                    eprintln!("l1_incoming_closed {error:#}");
                }
                drop(accounting);
                drop(permit);
            });
            while children.try_join_next().is_some() {}
        }
        children.abort_all();
        while children.join_next().await.is_some() {}
    });
    for (c, role) in roles {
        if role != 0 {
            continue;
        }
        let handle = handle.clone();
        let endpoint = endpoint.clone();
        let permits = permits.clone();
        let resources = resources.clone();
        let directory = profile.locator_directory.clone();
        tasks.spawn(async move {
            loop {
                let publish_directory = directory.clone();
                let _ = handle
                    .call(move |o| publish_file(o, c, &publish_directory))
                    .await;
                let permit = match permits.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        resources.refused();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let accounting = resources.acquired();
                let result = tokio::time::timeout(
                    Duration::from_secs(30),
                    network::admit_a(&handle, &endpoint, c),
                )
                .await;
                match result {
                    Ok(Ok((connection, link, pair))) => {
                        if let Err(error) =
                            network::established(handle.clone(), connection, link, Some(pair)).await
                        {
                            eprintln!("l1_outgoing_closed slot={c} {error:#}");
                        }
                    }
                    Ok(Err(error)) => eprintln!("l1_wait slot={c} {error:#}"),
                    Err(_) => eprintln!("l1_admission_timeout slot={c}"),
                }
                drop(accounting);
                drop(permit);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }
    // B contexts also need publication after a current permission repair.
    let publication_handle = handle.clone();
    let directory = profile.locator_directory.clone();
    tasks.spawn(async move {
        loop {
            for c in &enabled {
                let directory = directory.clone();
                let c = *c;
                let _ = publication_handle
                    .call(move |o| publish_file(o, c, &directory))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
    println!("profile_mode=ordered-dialog-indexed");
    println!("endpoint_id={}", endpoint.id());
    println!("ipc_file={}", profile.ipc_file.display());
    println!("status=runtime-listening");
    loop {
        let item = tokio::select! {item=work.recv()=>item,_=tokio::time::sleep(Duration::from_millis(100))=>{if handle.stop.load(Ordering::Acquire){break;}continue;}};
        let Some(item) = item else { break };
        let (command, response) = item.into_parts();
        let shutdown = matches!(command, RuntimeIpcCommand::Shutdown);
        let reply = match command {
            RuntimeIpcCommand::Shutdown => RuntimeIpcResponse::ShutdownAccepted,
            RuntimeIpcCommand::Ping => RuntimeIpcResponse::Pong {
                account_id: account,
                device_id: device,
            },
            RuntimeIpcCommand::OrderedDialog { request } => {
                let directory = profile.locator_directory.clone();
                match handle.call(move |o| ipc(o, &request, &directory)).await {
                    Ok(response) => RuntimeIpcResponse::OrderedDialog { response },
                    Err(error) => RuntimeIpcResponse::Error {
                        message: format!("{error:#}"),
                    },
                }
            }
            _ => RuntimeIpcResponse::Error {
                message: "UnsupportedProfile: explicit IPC3 required".into(),
            },
        };
        let _ = response.send(reply);
        server.publish_change();
        if shutdown {
            break;
        }
    }
    handle.stop.store(true, Ordering::Release);
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    endpoint.close().await;
    server.shutdown().await?;
    join.join()
        .map_err(|_| anyhow::anyhow!("indexed owner panicked"))?;
    Ok(())
}
