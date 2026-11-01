//! One owner thread/mailbox16, global connection permits2, exact IPC3.
use super::*;
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse, RuntimeIpcServer};
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
    pub(super) verification_enabled: bool,
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
pub(super) fn actor(mut owner: Owner7) -> Result<(Handle7, std::thread::JoinHandle<()>)> {
    let (sender, mut receiver) = mpsc::channel::<Job7>(16);
    let stop = Arc::new(AtomicBool::new(false));
    owner.resources.mailbox(&sender);
    let handle = Handle7 {
        sender,
        stop: stop.clone(),
        resources: owner.resources.clone(),
        verification_enabled: owner.verification.enabled(),
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
            let resolved = owner
                .repository
                .as_ref()
                .is_some_and(|r| !r.requires_reopen());
            owner.verification.finish(&owner.stats, resolved);
        })?;
    Ok((handle, join))
}

fn publish_file(owner: &mut Owner7, c: u8, directory: &Path) -> Result<Vec<u8>> {
    publish_origin_status(owner, c, directory);
    if owner.repo()?.existing_state(c)?.mode != store::ExistingMode::L1 {
        return publish_file2(owner, c, directory);
    }
    let result = publish_file1(owner, c, directory);
    owner.publication_errors[c as usize][0] = result.as_ref().err().map(public_error);
    result
}
fn publish_file1(owner: &mut Owner7, c: u8, directory: &Path) -> Result<Vec<u8>> {
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
fn publish_file2(owner: &mut Owner7, c: u8, directory: &Path) -> Result<Vec<u8>> {
    publish_origin_status(owner, c, directory);
    let result = publish_file2_inner(owner, c, directory);
    owner.publication_errors[c as usize][1] = result.as_ref().err().map(public_error);
    result
}
fn publish_file2_inner(owner: &mut Owner7, c: u8, directory: &Path) -> Result<Vec<u8>> {
    use kilogram_protocol::ordered_dialog_v2 as v2;
    owner.current_existing(c)?;
    std::fs::create_dir_all(directory)?;
    let path = directory.join(format!("{}.locator2", state::hex(&owner.spec(c)?.c)));
    if path.exists() {
        let raw = admin::read_public(&path, 8192)?;
        let old = v2::ParsedRuntimeLocator2::parse(&raw)?;
        let role = owner.role(c)?;
        old.verify(
            owner.spec(c)?.c,
            &owner.peer2(c, role)?,
            &owner.peer2(c, 1 - role)?,
        )?;
    }
    let raw = owner.publish_locator2(c)?;
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
fn public_error(error: &anyhow::Error) -> String {
    let text = format!("{error:#}");
    let mut end = text.len().min(256);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
fn publish_origin_status(owner: &mut Owner7, c: u8, directory: &Path) {
    match publish_origin_file(owner, c, directory) {
        Ok(ready) => {
            owner.origin_ready[c as usize] = ready;
            owner.origin_error[c as usize] = None;
        }
        Err(error) => {
            owner.origin_ready[c as usize] = false;
            owner.origin_error[c as usize] = Some(public_error(&error));
        }
    }
}
fn publish_origin_file(owner: &mut Owner7, c: u8, directory: &Path) -> Result<bool> {
    if owner.head(c)?.phase != 3
        || owner
            .repo()?
            .control(c, store::L1Control::Ready(0))?
            .is_none()
        || owner
            .repo()?
            .control(c, store::L1Control::Ready(1))?
            .is_none()
    {
        return Ok(false);
    }
    let scope = owner.existing(c, |controls| controls.scope(c))?;
    let directory = directory.join(state::hex(&scope.c()));
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("existing-origin.cbor");
    if path.exists() {
        let raw = admin::read_public(
            &path,
            kilogram_protocol::ordered_dialog_v2::MAX_EXISTING_ORIGIN,
        )?;
        let prior =
            kilogram_protocol::ordered_dialog_v2::VerifiedExistingScope1::verify_export(&raw)?;
        ensure!(
            prior.c() == scope.c() && raw == scope.raw(),
            "existing-origin publication conflict"
        );
        return Ok(true);
    }
    let mut temp = tempfile::NamedTempFile::new_in(&directory)?;
    use std::io::Write;
    temp.write_all(scope.raw())?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(&path).map_err(|e| e.error)?;
    Ok(true)
}

fn upgrade_status(owner: &mut Owner7, c: u8) -> Result<Vec<u8>> {
    let state = owner.repo()?.existing_state(c)?;
    let status = owner.existing(c, |controls| controls.status(c))?;
    let policy = owner
        .existing(c, |controls| controls.current_permission(c))
        .err()
        .map(|e| format!("{e:#}"));
    let role = owner.role(c)?;
    let admitted = owner.leases[usize::from(c)]
        .as_ref()
        .is_some_and(|lease| lease.family == 2 && lease.gate().is_ok());
    let current = policy.is_none() && admitted;
    Ok(serde_json::to_vec(&serde_json::json!({
        "version":3,"slot":c,"context":state::hex(&owner.spec(c)?.c),
        "mode":format!("{:?}",state.mode),"wire_stage":state.mode.wire(),
        "status":status,"policy_error":policy,"cuts":state.cuts,
        "q":state.q.map(|q|state::hex(&q)),"freeze":state.mode.frozen(),
        "control_debt":state.control_debt,
        "controls":{"p":state.propose.map(|id|state::hex(&id)),"a":state.accept.map(|id|state::hex(&id)),"k":state.select.map(|id|state::hex(&id)),"committed":state.committed.map(|id|state::hex(&id))},
        "native_encoding":if state.mode.selected(){"Mixed7"}else{"Legacy7"},
        "receive_event2":state.mode.selected()&&current,
        "send_event2":state.mode==store::ExistingMode::Active&&current,
        "policy_paused":policy.is_some(),"lease_current":admitted,
        "coordinator":role==0,
    }))?)
}
fn status(owner: &mut Owner7) -> Result<Vec<u8>> {
    let mut contexts = Vec::new();
    let mut routes = Vec::new();
    for c in 0..owner.specs.len() {
        let c = c as u8;
        let head = owner.head(c)?;
        let meta = owner.repo()?.metadata(c)?;
        let counts = owner.repo()?.counts(c)?;
        let repair = owner.repo()?.repair_state(c)?;
        let existing = owner.repo()?.existing_state(c)?;
        let family = owner.leases[usize::from(c)]
            .as_ref()
            .map(|lease| lease.family)
            .unwrap_or(if existing.mode == store::ExistingMode::L1 {
                1
            } else {
                2
            });
        let permission = owner.data_permission(c);
        let policy = permission.err().map(|e| format!("{e:#}"));
        let active = owner.data_ready(c)?
            && matches!(
                existing.mode,
                store::ExistingMode::L1 | store::ExistingMode::Active
            );
        let public_native = if policy.is_none() && head.phase == 3 {
            if existing.mode.selected() {
                let native = owner.mixed_native(c, &head)?;
                let s = &native.session;
                let mut value = serde_json::json!({"sid":s.sid(),"keys":s.keys().iter().map(|k|state::hex(k)).collect::<Vec<_>>(),"own_next":meta.next[head.role as usize],"peer_next":meta.next[1-head.role as usize]});
                if owner.verification.enabled() {
                    value["verification_guard"] = serde_json::json!({"own_g":native.guard.own().g,"own_index":native.guard.own().index,"own_first":native.guard.own().first.map(|id|state::hex(&id)),"peer_g":native.guard.peer().g,"peer_index":native.guard.peer().index,"peer_first":native.guard.peer().first.map(|id|state::hex(&id)),"rotate_due":native.guard.rotate_due()});
                }
                Some(value)
            } else {
                let native = owner.native(c, &head)?;
                native.session.as_ref().map(|s|serde_json::json!({"sid":s.sid(),"keys":s.keys().iter().map(|k|state::hex(k)).collect::<Vec<_>>(),"own_next":meta.next[head.role as usize],"peer_next":meta.next[1-head.role as usize]}))
            }
        } else {
            None
        };
        let own = owner.descriptor(c, true).ok();
        let mut published = policy.is_none()
            && own.as_ref().is_some_and(|d| {
                owner
                    .endpoint
                    .as_ref()
                    .is_some_and(|e| d.endpoint == *e.id.as_bytes())
                    && d.valid_now(now().unwrap_or(u64::MAX)).is_ok()
            });
        let consent = owner.control(c, store::L1Control::Consent)?;
        let consent_ref = existing
            .current_consent
            .map(|id| state::hex(&id))
            .unwrap_or_else(|| state::hex(&wire::hash(handshake::CONSENT, &consent)));
        let mut own_revision = own.as_ref().map(|d| d.revision);
        let mut peer_revision = owner.descriptor(c, false).ok().map(|d| d.revision);
        if family == 2 {
            let own = owner.locator2(c, true).ok();
            published = policy.is_none()
                && own.as_ref().is_some_and(|locator| {
                    owner.endpoint.as_ref().is_some_and(|endpoint| {
                        endpoint.id.as_bytes() == &locator.fields().endpoint
                    })
                });
            own_revision = own.map(|locator| locator.fields().revision);
            peer_revision = owner
                .locator2(c, false)
                .ok()
                .map(|locator| locator.fields().revision);
        }
        published &= owner.publication_errors[c as usize][usize::from(family - 1)].is_none()
            && (family == 1 || owner.origin_ready[c as usize]);
        let upgrade = owner.existing(c, |controls| controls.status(c))?;
        let handoff_policy = owner
            .current_existing(c)
            .err()
            .map(|error| format!("{error:#}"));
        let lease_generation = owner.leases[usize::from(c)]
            .as_ref()
            .filter(|lease| {
                policy.is_none()
                    && lease.policy_generation == meta.policy_generation
                    && lease.gate().is_ok()
            })
            .map(|lease| lease.generation);
        let selected_path = owner.leases[usize::from(c)]
            .as_ref()
            .filter(|lease| lease_generation.is_some() && lease.gate().is_ok())
            .and_then(|lease| {
                lease
                    .connection
                    .paths()
                    .iter()
                    .find(|path| path.is_selected())
                    .map(|path| {
                        if path.is_relay() {
                            "relay"
                        } else if path.is_ip() {
                            "direct"
                        } else {
                            "unknown"
                        }
                    })
            });
        routes.push(serde_json::json!({"slot":c,"policy":owner.route.effective()?.as_str(),"selected_path":selected_path}));
        contexts.push(serde_json::json!({"slot":c,"context":state::hex(&head.c),"enabled":owner.enabled.contains(&c),"role":head.role,"phase":head.phase,"native_generation":head.native_generation,"native_hash":head.blob.map(|b|state::hex(&b)),"d":head.d.map(|b|state::hex(&b)),"e":head.e.map(|b|state::hex(&b)),"sid":head.sid,"mode":format!("{:?}",existing.mode),"wire_stage":existing.mode.wire(),"q":existing.q.map(|q|state::hex(&q)),"cuts":existing.cuts,"freeze":existing.mode.frozen(),"active":active,"lease_current":lease_generation.is_some(),"lease_generation":lease_generation,"lease_family":family,"publication_ready":published,"publication_error":owner.publication_errors[c as usize][usize::from(family-1)],"existing_origin_ready":owner.origin_ready[c as usize],"origin_publication_error":owner.origin_error[c as usize],"policy_error":policy,"policy_generation":meta.policy_generation,"wall_high_water":meta.wall_high_water,"current_consent":consent_ref,"current_consent2":existing.current_consent.map(|id|state::hex(&id)),"own_locator_revision":own_revision,"peer_locator_revision":peer_revision,"next":meta.next,"counts":counts,"repair_free":repair.repair_free,"head_pending":repair.head,"native":public_native,"native_encoding":if existing.mode.selected(){"Mixed7"}else{"Legacy7"},"upgrade":upgrade,"handoff_policy_error":handoff_policy,"control_debt":existing.control_debt,"alpn1_allowed":existing.mode==store::ExistingMode::L1,"provenance":"LegacySnapshot/FullL1/FullMixed"}));
    }
    for (slot, row) in contexts.iter_mut().enumerate() {
        let state = owner.repo()?.existing_state(slot as u8)?;
        row["controls"] = serde_json::json!({"p":state.propose.map(|id|state::hex(&id)),"a":state.accept.map(|id|state::hex(&id)),"k":state.select.map(|id|state::hex(&id)),"committed":state.committed.map(|id|state::hex(&id))});
    }
    let generation = owner.repo()?.generation();
    let source = state::hex(&owner.repo()?.source_id());
    let backend = owner.repo()?.backend_stats()?;
    let (config_revision, config_hash) = owner.repo()?.config_status()?;
    Ok(serde_json::to_vec(
        &serde_json::json!({"version":3,"profile_mode":"ordered-dialog-indexed","storage_stage":"Ready","source_id":source,"root_generation":generation,"config_revision":config_revision,"config_hash":state::hex(&config_hash),"contexts":contexts,"routes":routes,"stats":owner.stats,"window":owner.window.snapshot(),"resources":owner.resources.snapshot(),"backend":backend}),
    )?)
}
pub(super) fn ipc(owner: &mut Owner7, raw: &[u8], locator_directory: &Path) -> Result<Vec<u8>> {
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
                    let (_, role, _) = data2::declared(&raw)?;
                    let core = parsed.fields(5)?[1].list()?;
                    owner.repo()?.receipt(c, role, core[6].uint()?)?.is_some()
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
            let family = f[3].uint()?;
            ensure!(matches!(family, 1 | 2), "locator family must be1|2");
            let raw = f[5].option()?;
            match f[4].uint()? {
                0 => {
                    let raw = raw
                        .context("locator import requires Some")?
                        .bytes()?
                        .to_vec();
                    ensure!(raw.len() <= 8192, "locator cap");
                    if family == 1 {
                        owner.import_locator(c, raw)?;
                    } else {
                        owner.import_locator2(c, raw)?;
                    }
                    a([])
                }
                1 => {
                    ensure!(raw.is_none(), "locator export requires None");
                    if family == 1 {
                        ensure!(
                            owner.repo()?.existing_state(c)?.mode == store::ExistingMode::L1,
                            "ALPN1 forbidden after durable Propose/Prepared"
                        );
                        b(publish_file(owner, c, locator_directory)?)
                    } else {
                        b(publish_file2(owner, c, locator_directory)?)
                    }
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
        7 => {
            let f = value.fields(6)?;
            let c = slot(&f[2])?;
            let expected = f[3].option()?.map(Value::fixed::<32>).transpose()?;
            let historical = f[5].uint()?;
            ensure!(historical <= 1, "historical must be0|1");
            let raw = f[4].bytes()?.to_vec();
            ensure!(raw.len() <= 4096, "Consent2 cap");
            owner.existing(c, |controls| {
                controls.import_consent(c, expected, raw, historical == 1)
            })?;
            owner.wake.notify_waiters();
            a([])
        }
        8 => {
            let f = value.fields(4)?;
            let c = slot(&f[2])?;
            ensure!(owner.enabled.contains(&c), "C not enabled");
            let request = f[3].fixed()?;
            owner.existing(c, |controls| controls.request_upgrade(c, request))?;
            owner.wake.notify_waiters();
            b(upgrade_status(owner, c)?)
        }
        9 => {
            let f = value.fields(3)?;
            b(upgrade_status(owner, slot(&f[2])?)?)
        }
        10 => {
            let f = value.fields(4)?;
            let c = slot(&f[2])?;
            let request = f[3].fixed()?;
            owner.existing(c, |controls| controls.cancel(c, request))?;
            owner.wake.notify_waiters();
            b(upgrade_status(owner, c)?)
        }
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
    owner.configure_route(profile.route_config()?)?;
    let endpoint = kilogram_transport_iroh::ordered2::bind_existing(&owner.route).await?;
    owner.endpoint = Some(endpoint.addr());
    for c in owner.enabled.clone() {
        if let Err(error) = publish_file(&mut owner, c, &profile.locator_directory) {
            eprintln!("l1_publication_blocked slot={c} {error:#}");
        }
        if owner.repo()?.existing_state(c)?.current_consent.is_some()
            && let Err(error) = publish_file2(&mut owner, c, &profile.locator_directory)
        {
            eprintln!("existing_publication_blocked slot={c} {error:#}");
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
                    let whole = std::time::Instant::now() + control::PROBE_WHOLE7;
                    let admission = std::time::Instant::now() + Duration::from_secs(30);
                    let connection = tokio::time::timeout(Duration::from_secs(15), incoming)
                        .await
                        .context("L1 TLS timeout")??;
                    if connection.alpn() == kilogram_protocol::ordered_dialog_v2::ALPN {
                        let (c, lease, _guard) = network2::admit_b(
                            &handle,
                            &endpoint,
                            &connection,
                            &inbound,
                            admission,
                            whole,
                        )
                        .await?;
                        return network2::established(handle, connection, c, lease).await;
                    }
                    ensure!(connection.alpn() == wire::ALPN, "unsupported indexed ALPN");
                    let (link, _guard) = tokio::time::timeout_at(
                        admission.into(),
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
        tasks.spawn(async move {
            if let Err(error) = network2::coordinate(handle, endpoint, c, permits, resources).await
            {
                eprintln!("indexed_coordinator_stopped slot={c} {error:#}");
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
                    .call(move |o| {
                        let result = publish_file(o, c, &directory);
                        if o.repo()?.existing_state(c)?.current_consent.is_some() {
                            publish_file2(o, c, &directory)?;
                        }
                        result
                    })
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
