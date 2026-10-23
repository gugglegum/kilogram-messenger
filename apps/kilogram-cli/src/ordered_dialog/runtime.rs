//! Real OD1 QUIC actors. One authenticated connection per enabled context.
use super::{
    admin,
    engine::{self, Changes, Engine, Lease, prefix},
    handshake::{self, Credentials, Descriptor, Spec},
    state,
};
use anyhow::{Context, Result, ensure};
use iroh::{Endpoint, endpoint::Connection};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Frame, Id, Value, a, b, opt, u};
use kilogram_state::{OrderedVault, StateDirectoryLock};
use kilogram_transport_iroh::ordered as transport;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Notify};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u64,
    pub profile_mode: String,
    pub state_dir: PathBuf,
    pub ipc_file: PathBuf,
    pub locator_directory: PathBuf,
    pub enabled_contexts: Vec<String>,
    pub bind: String,
}
impl Profile {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = admin::read_public(path, 16 * 1024)?;
        let profile: Self = serde_json::from_slice(&raw)?;
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1 && self.profile_mode == "ordered-dialog-v1",
            "unsupported OD1 runtime profile"
        );
        ensure!(
            !self.enabled_contexts.is_empty() && self.enabled_contexts.len() <= 2,
            "OD1 enabled contexts max2"
        );
        let ids = self
            .enabled_contexts
            .iter()
            .map(|c| state::parse_id(c))
            .collect::<Result<std::collections::BTreeSet<_>>>()?;
        ensure!(
            ids.len() == self.enabled_contexts.len(),
            "duplicate enabled context"
        );
        let root = std::fs::canonicalize(&self.state_dir)?;
        for path in [&self.ipc_file, &self.locator_directory] {
            let mut absolute = if path.is_absolute() {
                path.clone()
            } else {
                std::env::current_dir()?.join(path)
            };
            while !absolute.exists() {
                ensure!(absolute.pop(), "artifact has no existing ancestor");
            }
            ensure!(
                !std::fs::canonicalize(&absolute)?.starts_with(&root),
                "runtime public artifacts must be outside state"
            );
        }
        Ok(())
    }
}
pub type Shared = Arc<Mutex<Engine>>;
#[derive(Clone, Default)]
struct Hint {
    send: u64,
    receive: u64,
    ordinary: u64,
    blocked: u64,
}
struct Link {
    c: Id,
    binding: Id,
    generation: u64,
    deadline: u64,
    admitted_at: u64,
    peer_role: u64,
    hint: Mutex<Hint>,
    _context_owner: Option<tokio::sync::OwnedMutexGuard<()>>,
}
static CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(1);
#[derive(Default)]
pub struct RuntimeControl {
    pub wake: Notify,
    pub statuses: Mutex<BTreeMap<Id, String>>,
}

fn descriptor_path(c: Id, local: bool) -> String {
    format!(
        "{}{}-locator",
        prefix(c),
        if local { "local" } else { "peer" }
    )
}
pub fn load_descriptor(engine: &Engine, c: Id, local: bool) -> Result<Descriptor> {
    let role = engine.role(c)?;
    Descriptor::parse(
        engine.spec(c)?,
        if local { role } else { 1 - role },
        &admin::required(&engine.store, &descriptor_path(c, local))?,
    )
}
pub fn import_locator(engine: &mut Engine, c: Id, raw: &[u8]) -> Result<()> {
    let role = engine.role(c)?;
    let descriptor = Descriptor::parse(engine.spec(c)?, 1 - role, raw)?;
    descriptor.valid_now(engine::now()?)?;
    if let Some(current) = engine.store.get(&descriptor_path(c, false))? {
        let old = Descriptor::parse(engine.spec(c)?, 1 - role, &current)?;
        ensure!(
            descriptor.revision >= old.revision
                && (descriptor.revision != old.revision || current == raw),
            "locator rollback/conflict"
        );
        if current == raw {
            return Ok(());
        }
    }
    let mut head = engine.head(c)?;
    let mut changes = Changes::default();
    changes.put(descriptor_path(c, false), raw.to_vec());
    head.bump(false)?;
    engine.save(&mut head, None, changes)?;
    engine.leases.remove(&c);
    Ok(())
}

async fn rpc(connection: &Connection, frame: &Frame, spec: &Spec, peer_role: u64) -> Result<Frame> {
    let started = Instant::now();
    let trace = super::test_support::load()?.trace_rpc;
    if trace {
        eprintln!(
            "od1_rpc_begin kind={} request={}",
            frame.kind,
            state::hex(&frame.request)
        );
        if frame.kind == 11 {
            let f = frame.body.fields(12)?;
            eprintln!(
                "od1_prefix_request context={} start={} end={} ordinary_slots={} ordinary_bytes={} head_bytes={}",
                state::hex(&f[0].fixed::<32>()?),
                f[4].uint()?,
                f[5].uint()?,
                f[9].uint()?,
                f[10].uint()?,
                f[11].uint()?
            );
        }
    }
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (mut send, mut receive) = connection.open_bi().await?;
        transport::write(&mut send, frame).await?;
        let (reply, _) = transport::read(&mut receive).await?;
        reply.verify(
            frame.binding,
            spec.peers[peer_role as usize].cert.device_id(),
        )?;
        ensure!(reply.request == frame.request, "response correlation");
        if reply.kind == 14 {
            let f = reply.body.fields(8)?;
            ensure!(
                f[0].fixed::<32>()? == spec.c && f[3].fixed::<16>()? == frame.request,
                "round error correlation"
            );
            anyhow::bail!(
                "peer round error status={} code={}",
                f[4].uint()?,
                f[5].uint()?
            );
        }
        Ok(reply)
    })
    .await
    .context("OD1 RPC timeout")?;
    if trace {
        eprintln!(
            "od1_rpc_end kind={} elapsed_ms={} ok={}",
            frame.kind,
            started.elapsed().as_millis(),
            result.is_ok()
        );
    }
    result
}
fn signed(engine: &Engine, kind: u64, binding: Id, body: Value) -> Result<Frame> {
    Ok(Frame::signed(
        kind,
        binding,
        admin::random::<16>()?,
        body,
        engine.device.identity(),
    )?)
}

fn hello(
    engine: &Engine,
    c: Id,
    role: u64,
    local: &Descriptor,
    remote: &Descriptor,
) -> Result<Value> {
    let credential = admin::credentials(&engine.store)?;
    let consent = admin::required(&engine.store, &admin::consent_path(c))?;
    let schedule = super::test_support::load()?;
    Ok(a([
        b(c),
        u(role),
        b(local.endpoint),
        b(remote.endpoint),
        b(admin::random::<32>()?),
        a([u(if schedule.applies(c) && schedule.unsupported_suite {
            2
        } else {
            1
        })]),
        a([u(1)]),
        b(credential.raw),
        b(&engine.spec(c)?.membership),
        b(wire::hash(handshake::CONSENT, &consent)),
        b(local.reference()),
    ]))
}
fn verify_hello(
    engine: &mut Engine,
    c: Id,
    frame: &Frame,
    peer_role: u64,
    local: &Descriptor,
    remote: &Descriptor,
) -> Result<()> {
    let spec = engine.spec(c)?;
    frame.verify(
        frame.binding,
        spec.peers[peer_role as usize].cert.device_id(),
    )?;
    let f = frame.body.fields(11)?;
    let credentials = Credentials::decode(f[7].bytes()?)?;
    ensure!(
        frame.kind == 1
            && f[0].fixed::<32>()? == c
            && f[1].uint()? == peer_role
            && f[2].fixed::<32>()? == remote.endpoint
            && f[3].fixed::<32>()? == local.endpoint
            && f[4].bytes()?.len() == 32
            && f[5] == a([u(1)])
            && f[6] == a([u(1)])
            && credentials.cert_ref()? == spec.peers[peer_role as usize].cert_ref()?
            && f[8].bytes()? == spec.membership
            && f[9].fixed::<32>()?
                == wire::hash(
                    handshake::CONSENT,
                    &admin::required(&engine.store, &admin::consent_path(c))?
                )
            && f[10].fixed::<32>()? == remote.reference(),
        "OD1 Hello pins/endpoints/consent/capabilities"
    );
    local.valid_now(engine::now()?)?;
    remote.valid_now(engine::now()?)?;
    // A signed newer Root revocation is durable knowledge even if it denies
    // this very Hello. An older transcript cannot erase that knowledge later.
    let path = format!("peer-authority/{}.snapshot", credentials.cert.account_id());
    let prior = kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&admin::required(
        &engine.store,
        &path,
    )?)?;
    ensure!(
        credentials.authority.revision() >= prior.revision(),
        "authority rollback"
    );
    ensure!(
        credentials.authority.revision() != prior.revision()
            || credentials.authority.encode()? == prior.encode()?,
        "authority revision conflict"
    );
    if credentials.authority.revision() > prior.revision() {
        let mut changes = Changes::default();
        changes.put(path, credentials.authority.encode()?);
        let mut head = engine.head(c)?;
        head.bump(false)?;
        engine.save(&mut head, None, changes)?;
        engine.leases.remove(&c);
    }
    engine.policy(c)?;
    Ok(())
}
fn persist_pair(engine: &mut Engine, c: Id, pair: Vec<u8>) -> Result<()> {
    let mut changes = Changes::default();
    changes.put(format!("{}current-auth", prefix(c)), pair);
    let mut head = engine.head(c)?;
    head.bump(false)?;
    engine.save(&mut head, None, changes)
}
fn deadline(engine: &Engine, c: Id, local: &Descriptor, remote: &Descriptor) -> Result<u64> {
    Ok((engine::now()?.saturating_add(300))
        .min(engine.policy(c)?)
        .min(local.expires)
        .min(remote.expires))
}
async fn admit_a(
    shared: &Shared,
    endpoint: &Endpoint,
    c: Id,
) -> Result<(Connection, Arc<Link>, Vec<u8>)> {
    let (spec, local, remote) = {
        let engine = shared.lock().await;
        ensure!(engine.role(c)? == 0, "A dialer role");
        (
            engine.spec(c)?.clone(),
            load_descriptor(&engine, c, true)?,
            load_descriptor(&engine, c, false)?,
        )
    };
    local.valid_now(engine::now()?)?;
    remote.valid_now(engine::now()?)?;
    let connection = endpoint.connect(remote.addr()?, wire::ALPN).await?;
    ensure!(
        connection.remote_id().as_bytes() == &remote.endpoint,
        "actual TLS endpoint"
    );
    let binding = transport::binding(&connection, *endpoint.id().as_bytes(), true)?;
    let hello_a = {
        let engine = shared.lock().await;
        signed(&engine, 1, binding, hello(&engine, c, 0, &local, &remote)?)?
    };
    let hello_b = rpc(&connection, &hello_a, &spec, 1).await?;
    {
        let mut engine = shared.lock().await;
        verify_hello(&mut engine, c, &hello_b, 1, &local, &remote)?;
    }
    let hello_a_raw = hello_a.raw()?;
    let hello_b_raw = hello_b.raw()?;
    let t = handshake::transcript(binding, &hello_a_raw, &hello_b_raw)?;
    let refs = spec.cert_refs()?;
    let body = a([b(t), b(c), u(1), u(1), b(refs[0]), b(refs[1])]);
    let proof_a = {
        let engine = shared.lock().await;
        signed(&engine, 2, binding, body.clone())?
    };
    let proof_b = rpc(&connection, &proof_a, &spec, 1).await?;
    ensure!(
        proof_b.kind == 2 && proof_b.body == body,
        "B Proof transcript"
    );
    let pair =
        handshake::initial_pair(t, hello_a_raw, hello_b_raw, proof_a.raw()?, proof_b.raw()?)?;
    let admitted_body = a([b(t), b(c), u(1), u(1)]);
    let admitted = {
        let mut engine = shared.lock().await;
        persist_pair(&mut engine, c, pair.clone())?;
        signed(&engine, 3, binding, admitted_body.clone())?
    };
    let response = rpc(&connection, &admitted, &spec, 1).await?;
    ensure!(
        response.kind == 3 && response.body == admitted_body,
        "B admission confirmation"
    );
    let generation = CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed);
    let expiry = {
        let mut engine = shared.lock().await;
        let expiry = deadline(&engine, c, &local, &remote)?;
        engine.leases.insert(
            c,
            Lease {
                binding,
                generation,
                deadline: expiry,
            },
        );
        expiry
    };
    Ok((
        connection,
        Arc::new(Link {
            c,
            binding,
            generation,
            deadline: expiry,
            admitted_at: engine::now()?,
            peer_role: 1,
            hint: Mutex::new(Hint::default()),
            _context_owner: None,
        }),
        pair,
    ))
}

async fn admit_b(
    shared: &Shared,
    endpoint: &Endpoint,
    connection: &Connection,
    contexts: &BTreeMap<Id, Arc<Mutex<()>>>,
) -> Result<Arc<Link>> {
    let binding = transport::binding(connection, *endpoint.id().as_bytes(), false)?;
    let (mut send, mut receive) = connection.accept_bi().await?;
    let (hello_a, hello_a_raw) = transport::read(&mut receive).await?;
    ensure!(hello_a.kind == 1, "Hello must be first");
    let c = hello_a.body.fields(11)?[0].fixed()?;
    let context_owner = contexts
        .get(&c)
        .context("context is paused or not an inbound B context")?
        .clone()
        .try_lock_owned()
        .context("context already has an admitted or authenticating connection")?;
    let (spec, local, remote) = {
        let engine = shared.lock().await;
        ensure!(engine.role(c)? == 1, "B accept role");
        (
            engine.spec(c)?.clone(),
            load_descriptor(&engine, c, true)?,
            load_descriptor(&engine, c, false)?,
        )
    };
    ensure!(
        connection.remote_id().as_bytes() == &remote.endpoint,
        "actual TLS dialer endpoint"
    );
    hello_a.verify(binding, spec.peers[0].cert.device_id())?;
    let hello_b = {
        let mut engine = shared.lock().await;
        verify_hello(&mut engine, c, &hello_a, 0, &local, &remote)?;
        Frame::signed(
            1,
            binding,
            hello_a.request,
            hello(&engine, c, 1, &local, &remote)?,
            engine.device.identity(),
        )?
    };
    let hello_b_raw = transport::write(&mut send, &hello_b).await?;
    let t = handshake::transcript(binding, &hello_a_raw, &hello_b_raw)?;
    let refs = spec.cert_refs()?;
    let proof_body = a([b(t), b(c), u(1), u(1), b(refs[0]), b(refs[1])]);
    let (mut send, mut receive) = connection.accept_bi().await?;
    let (proof_a, proof_a_raw) = transport::read(&mut receive).await?;
    proof_a.verify(binding, spec.peers[0].cert.device_id())?;
    ensure!(
        proof_a.kind == 2 && proof_a.body == proof_body,
        "A Proof transcript"
    );
    let proof_b = {
        let engine = shared.lock().await;
        Frame::signed(
            2,
            binding,
            proof_a.request,
            proof_body,
            engine.device.identity(),
        )?
    };
    let proof_b_raw = transport::write(&mut send, &proof_b).await?;
    let pair = handshake::initial_pair(t, hello_a_raw, hello_b_raw, proof_a_raw, proof_b_raw)?;
    let (mut send, mut receive) = connection.accept_bi().await?;
    let (admitted, _) = transport::read(&mut receive).await?;
    admitted.verify(binding, spec.peers[0].cert.device_id())?;
    let admitted_body = a([b(t), b(c), u(1), u(1)]);
    ensure!(
        admitted.kind == 3 && admitted.body == admitted_body,
        "A admission confirmation"
    );
    let reply = {
        let mut engine = shared.lock().await;
        persist_pair(&mut engine, c, pair)?;
        Frame::signed(
            3,
            binding,
            admitted.request,
            admitted_body,
            engine.device.identity(),
        )?
    };
    transport::write(&mut send, &reply).await?;
    let generation = CONNECTION_GENERATION.fetch_add(1, Ordering::Relaxed);
    let expiry = {
        let mut engine = shared.lock().await;
        let expiry = deadline(&engine, c, &local, &remote)?;
        engine.leases.insert(
            c,
            Lease {
                binding,
                generation,
                deadline: expiry,
            },
        );
        expiry
    };
    Ok(Arc::new(Link {
        c,
        binding,
        generation,
        deadline: expiry,
        admitted_at: engine::now()?,
        peer_role: 0,
        hint: Mutex::new(Hint::default()),
        _context_owner: Some(context_owner),
    }))
}

async fn handshake_a(
    shared: &Shared,
    connection: &Connection,
    link: &Link,
    pair: &[u8],
) -> Result<()> {
    let c = link.c;
    let spec = { shared.lock().await.spec(c)?.clone() };
    let body = {
        let mut engine = shared.lock().await;
        let mut offer = engine.ensure_offer(c, pair, link.binding)?;
        let schedule = super::test_support::load()?;
        if schedule.applies(c) && schedule.conflicting_offer {
            // A malicious-signer test on the actual admitted TLS connection.
            // Native account/session/counters and the durable original stay put.
            let body = wire::control(&offer, 1, engine.device.identity().device_id())?;
            let mut fields = body.fields(8)?.to_vec();
            fields[4] = b(admin::random::<32>()?);
            offer = wire::signed_control(1, a(fields), engine.device.identity())?;
        }
        a([
            b(c),
            b(offer),
            b(admin::required(
                &engine.store,
                &format!("{}initial-pair", prefix(c)),
            )?),
        ])
    };
    let frame = {
        let engine = shared.lock().await;
        signed(&engine, 5, link.binding, body)?
    };
    let grant = rpc(connection, &frame, &spec, 1).await?;
    ensure!(
        grant.kind == 6 && grant.body.fields(3)?[0].fixed::<32>()? == c,
        "Grant response"
    );
    let init = {
        let mut engine = shared.lock().await;
        engine.init(c, grant.body.fields(3)?[2].bytes()?, link.binding)?
    };
    let frame = {
        let engine = shared.lock().await;
        let head = engine.head(c)?;
        signed(
            &engine,
            7,
            link.binding,
            a([
                b(c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                b(init),
            ]),
        )?
    };
    let confirm = rpc(connection, &frame, &spec, 1).await?;
    ensure!(confirm.kind == 7, "Confirm response");
    let ready = {
        let mut engine = shared.lock().await;
        engine.receive_confirm(c, confirm.body.fields(4)?[3].bytes()?, link.binding)?
    };
    let frame = {
        let engine = shared.lock().await;
        let head = engine.head(c)?;
        signed(
            &engine,
            8,
            link.binding,
            a([
                b(c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                b(ready),
            ]),
        )?
    };
    let ready = rpc(connection, &frame, &spec, 1).await?;
    ensure!(ready.kind == 8, "Ready response");
    shared
        .lock()
        .await
        .ready(c, ready.body.fields(4)?[3].bytes()?, link.binding)?;
    Ok(())
}

async fn observe_hint(shared: &Shared, link: &Link, frame: &Frame) -> Result<()> {
    // Debug network scheduler may delay authenticated cursor observations to
    // exercise future records already in flight. The Frame itself is verified.
    let schedule = super::test_support::load()?;
    if schedule.applies(link.c) && schedule.delay_cursor_updates {
        return Ok(());
    }
    let f = frame.body.list()?;
    {
        let engine = shared.lock().await;
        let head = engine.head(link.c)?;
        ensure!(
            f[0].fixed::<32>()? == link.c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e,
            "cursor context/domain/epoch"
        );
        if frame.kind == 9 {
            ensure!(
                f[3].text()? == head.sid.as_deref().context("cursor SID")?,
                "cursor SID"
            );
        }
    }
    let (send, receive, status, credit) = if frame.kind == 9 {
        (&f[5], &f[6], &f[7], &f[8])
    } else {
        (&f[3], &f[4], &f[5], &f[6])
    };
    let mut hint = link.hint.lock().await;
    hint.send = send.fields(2)?[0].uint()?;
    hint.receive = receive.fields(2)?[0].uint()?;
    ensure!(hint.send <= 512 && hint.receive <= 512, "peer cursor cap");
    hint.blocked = status.fields(2)?[0].uint()?;
    hint.ordinary = credit.fields(5)?[0].uint()?;
    ensure!(
        hint.ordinary <= 64 && hint.blocked <= 7,
        "cursor status/credit bounds"
    );
    for cursor in [send, receive] {
        let p = cursor.fields(2)?;
        let last = wire::optional_id(&p[1])?;
        ensure!(
            (p[0].uint()? == 0) == last.is_none(),
            "cursor predecessor shape"
        );
    }
    Ok(())
}

async fn dispatch(shared: &Shared, link: &Link, frame: &Frame) -> Result<Option<Frame>> {
    let mut engine = shared.lock().await;
    engine.check_lease(link.c, link.binding)?;
    let c = link.c;
    frame.verify(
        link.binding,
        engine.spec(c)?.peers[link.peer_role as usize]
            .cert
            .device_id(),
    )?;
    let head = engine.head(c)?;
    let response = match frame.kind {
        4 => {
            ensure!(
                frame.body.fields(6)?[0].fixed::<32>()? == c,
                "OpenIntent context"
            );
            None
        }
        5 => {
            let f = frame.body.fields(3)?;
            ensure!(f[0].fixed::<32>()? == c, "Offer context");
            let raw = engine.grant(c, f[1].bytes()?, f[2].bytes()?, link.binding)?;
            Some((
                6,
                a([b(c), b(wire::hash(wire::CONTROL_ID, f[1].bytes()?)), b(raw)]),
            ))
        }
        7 => {
            let f = frame.body.fields(4)?;
            ensure!(
                f[0].fixed::<32>()? == c
                    && Some(f[1].fixed::<32>()?) == head.d
                    && Some(f[2].fixed::<32>()?) == head.e,
                "Init context"
            );
            let raw = engine.confirm(c, f[3].bytes()?, link.binding)?;
            let h = engine.head(c)?;
            Some((
                7,
                a([b(c), b(h.d.context("D")?), b(h.e.context("E")?), b(raw)]),
            ))
        }
        8 => {
            let f = frame.body.fields(4)?;
            ensure!(
                f[0].fixed::<32>()? == c
                    && Some(f[1].fixed::<32>()?) == head.d
                    && Some(f[2].fixed::<32>()?) == head.e,
                "Ready context/domain/epoch"
            );
            let raw = engine.ready(c, f[3].bytes()?, link.binding)?;
            Some((
                8,
                a([
                    b(c),
                    b(head.d.context("D")?),
                    b(head.e.context("E")?),
                    b(raw),
                ]),
            ))
        }
        9 => {
            ensure!(
                frame.body.fields(9)?[0].fixed::<32>()? == c,
                "Resume context"
            );
            Some((9, engine.resume_body(c)?))
        }
        10 => {
            let result = engine.accept_data_frame(c, frame)?;
            if std::env::var_os("KILOGRAM_TEST_OD1_SCHEDULE").is_some() {
                eprintln!(
                    "od1_direct_result context={} requested={} saved={} invalid={} partial={}",
                    state::hex(&c),
                    frame.body.fields(6)?[5].list()?.len(),
                    result.ids.len(),
                    result.invalid,
                    result.error.is_some()
                );
            }
            if result.error.is_none() && !result.invalid {
                Some((
                    13,
                    a([
                        b(c),
                        b(head.d.context("D")?),
                        b(head.e.context("E")?),
                        b(frame.request),
                        a(result.ids.iter().map(b)),
                    ]),
                ))
            } else {
                Some((
                    14,
                    a([
                        b(c),
                        opt(head.d.map(b)),
                        opt(head.e.map(b)),
                        b(frame.request),
                        u(if result.invalid { 1 } else { 3 }),
                        u(if result.invalid { 6 } else { 4 }),
                        opt(None),
                        u(1),
                    ]),
                ))
            }
        }
        11 => return Ok(Some(engine.prefix_response(c, frame)?)),
        13 | 14 => None,
        15 => {
            engine.accept_ack_frame(c, frame)?;
            None
        }
        16 => return Ok(Some(engine.ack_response(c, frame)?)),
        17 => {
            ensure!(frame.body.fields(7)?[0].fixed::<32>()? == c, "Head context");
            None
        }
        _ => anyhow::bail!("unsupported admitted OD1 frame"),
    };
    let reply = response
        .map(|(kind, body)| {
            Frame::signed(
                kind,
                link.binding,
                frame.request,
                body,
                engine.device.identity(),
            )
        })
        .transpose()?;
    drop(engine);
    if frame.kind == 9 || frame.kind == 17 {
        observe_hint(shared, link, frame).await?;
    }
    Ok(reply)
}

async fn incoming(shared: Shared, connection: Connection, link: Arc<Link>) -> Result<()> {
    loop {
        let (mut send, mut receive) = connection.accept_bi().await?;
        let result = tokio::time::timeout(Duration::from_secs(15), async {
            let (frame, _) = transport::read(&mut receive).await?;
            match dispatch(&shared, &link, &frame).await {
                Ok(Some(reply)) => {
                    super::test_support::before_write(&reply).await?;
                    transport::write(&mut send, &reply).await?;
                }
                Ok(None) => {
                    let _ = send.finish();
                }
                Err(error) => {
                    eprintln!("od1_round_error={error:#}");
                    let engine = shared.lock().await;
                    let head = engine.head(link.c)?;
                    let reply = Frame::signed(
                        14,
                        link.binding,
                        frame.request,
                        a([
                            b(link.c),
                            opt(head.d.map(b)),
                            opt(head.e.map(b)),
                            b(frame.request),
                            u(2),
                            u(7),
                            opt(None),
                            u(1),
                        ]),
                        engine.device.identity(),
                    )?;
                    drop(engine);
                    transport::write(&mut send, &reply).await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .await;
        if !matches!(result, Ok(Ok(()))) {
            connection.close(1_u32.into(), b"OD1 frame failure");
            anyhow::bail!("OD1 admitted stream failed");
        }
    }
}

async fn terminal(connection: &Connection, frame: &Frame) -> Result<()> {
    let (mut send, _receive) = connection.open_bi().await?;
    transport::write(&mut send, frame).await?;
    Ok(())
}
async fn data_worker(
    shared: Shared,
    connection: Connection,
    link: Arc<Link>,
    control: Arc<RuntimeControl>,
) -> Result<()> {
    let spec = { shared.lock().await.spec(link.c)?.clone() };
    let c = link.c;
    let mut resumed = Instant::now() - Duration::from_secs(31);
    let mut repair_wait = false;
    let mut last_receive = 0;
    let mut invalid_head = false;
    let mut direct_attempted = std::collections::BTreeSet::new();
    loop {
        let schedule = super::test_support::load()?;
        ensure!(
            engine::now()? < link.deadline,
            "authorization/locator lease expired"
        );
        if engine::now()?.saturating_sub(link.admitted_at) >= 240 {
            anyhow::bail!("scheduled reauthentication");
        }
        if connection.close_reason().is_some() {
            anyhow::bail!("connection closed");
        }
        let active = { shared.lock().await.active(c)? };
        if !active {
            tokio::time::sleep(Duration::from_millis(20)).await;
            continue;
        }
        let work_waiting = {
            let engine = shared.lock().await;
            !engine.unacked(c)?.is_empty()
                || !engine
                    .store
                    .paths(&format!("{}pending/", prefix(c)))
                    .is_empty()
                || !engine
                    .store
                    .paths(&format!("{}queue/", prefix(c)))
                    .is_empty()
        };
        if resumed.elapsed() >= Duration::from_secs(if work_waiting { 1 } else { 30 }) {
            let request = {
                let engine = shared.lock().await;
                signed(&engine, 9, link.binding, engine.resume_body(c)?)?
            };
            let response = rpc(&connection, &request, &spec, link.peer_role).await?;
            ensure!(response.kind == 9, "Resume response");
            observe_hint(&shared, &link, &response).await?;
            resumed = Instant::now();
            let hint = link.hint.lock().await.clone();
            if hint.receive > last_receive && hint.ordinary > 0 {
                repair_wait = false;
            }
            last_receive = hint.receive;
        }
        if !invalid_head {
            let has_pending = {
                let engine = shared.lock().await;
                !engine
                    .store
                    .paths(&format!("{}pending/", prefix(c)))
                    .is_empty()
            };
            if has_pending {
                let drained = shared.lock().await.drain(c, link.binding)?;
                invalid_head = drained.invalid;
            }
        }
        let peer_head = link.hint.lock().await.send;
        let request = if schedule.applies(c) && schedule.suspend_prefix_requests {
            None
        } else {
            let engine = shared.lock().await;
            engine
                .prefix_request(c, peer_head)?
                .map(|body| signed(&engine, 11, link.binding, body))
                .transpose()?
        };
        if let Some(request) = request {
            let response = rpc(&connection, &request, &spec, link.peer_role).await?;
            ensure!(response.kind == 12, "PrefixResponse required");
            let f = response.body.fields(10)?;
            let expected = request.body.fields(12)?;
            ensure!(
                f[4] == expected[4] && f[5].uint()? <= expected[5].uint()?,
                "prefix response requested span"
            );
            if let Some(first) = f[8].list()?.first() {
                let e = wire::Event::parse(
                    first.bytes()?,
                    spec.peers[link.peer_role as usize].cert.device_id(),
                )?;
                ensure!(
                    wire::optional_id(&expected[6])? == e.core.previous,
                    "prefix response requested predecessor"
                );
            }
            let result = shared.lock().await.accept_data_frame(c, &response)?;
            let complete = {
                let engine = shared.lock().await;
                let head = engine.head(c)?;
                if result.error.is_none() && !result.invalid {
                    Frame::signed(
                        13,
                        link.binding,
                        response.request,
                        a([
                            b(c),
                            b(head.d.context("D")?),
                            b(head.e.context("E")?),
                            b(response.request),
                            a(result.ids.iter().map(b)),
                        ]),
                        engine.device.identity(),
                    )?
                } else {
                    Frame::signed(
                        14,
                        link.binding,
                        response.request,
                        a([
                            b(c),
                            opt(head.d.map(b)),
                            opt(head.e.map(b)),
                            b(response.request),
                            u(3),
                            u(if result.invalid { 6 } else { 4 }),
                            opt(None),
                            u(1),
                        ]),
                        engine.device.identity(),
                    )?
                }
            };
            terminal(&connection, &complete).await?;
        }
        for _ in 0..8 {
            if shared
                .lock()
                .await
                .materialize_one(c, link.binding)?
                .is_none()
            {
                break;
            }
        }
        let intent = if schedule.applies(c) && schedule.disable_ack {
            None
        } else {
            shared.lock().await.ack_intents(c, link.binding)?
        };
        if let Some((frame, ordinals)) = intent {
            shared
                .lock()
                .await
                .mark_ack_intents(c, link.binding, &ordinals, 1)?;
            terminal(&connection, &frame).await?;
            shared
                .lock()
                .await
                .mark_ack_intents(c, link.binding, &ordinals, 2)?;
        }
        let unacked = { shared.lock().await.unacked(c)? };
        if !unacked.is_empty() {
            let items = unacked.iter().take(32).copied().collect::<Vec<_>>();
            let query = {
                let engine = shared.lock().await;
                engine.ack_query(c, &items, link.binding, admin::random::<16>()?)?
            };
            let response = rpc(&connection, &query, &spec, link.peer_role).await?;
            ensure!(response.kind == 15, "AckBatch response");
            shared.lock().await.accept_ack_frame(c, &response)?;
        }
        let hint = link.hint.lock().await.clone();
        if hint.ordinary == 0 || hint.blocked == 1 || hint.blocked == 6 {
            repair_wait = true;
        }
        if !repair_wait {
            // Apply a debug transport schedule at actual dispatch, after any
            // queue/materialization or RPC awaits in this actor turn.
            let schedule = super::test_support::load()?;
            let batch = {
                let engine = shared.lock().await;
                let role = engine.role(c)?;
                engine
                    .unacked(c)?
                    .iter()
                    .filter(|(n, _)| !direct_attempted.contains(n) && !schedule.held(c, role, *n))
                    .take(schedule.direct_batch.unwrap_or(8))
                    .map(|(n, _)| engine.event_at(c, role, *n)?.context("source outbox event"))
                    .collect::<Result<Vec<_>>>()?
            };
            if !batch.is_empty() {
                let frame = {
                    let engine = shared.lock().await;
                    let frame =
                        engine.deliver_frame(c, &batch, link.binding, admin::random::<16>()?)?;
                    super::test_support::delivery(frame, engine.device.identity())?
                };
                match rpc(&connection, &frame, &spec, link.peer_role).await {
                    Ok(complete) => {
                        ensure!(complete.kind == 13, "delivery completion");
                        let f = complete.body.fields(5)?;
                        let head = shared.lock().await.head(c)?;
                        ensure!(
                            f[0].fixed::<32>()? == c
                                && Some(f[1].fixed::<32>()?) == head.d
                                && Some(f[2].fixed::<32>()?) == head.e
                                && f[3].fixed::<16>()? == frame.request,
                            "completion context/domain/epoch/request"
                        );
                        let ids = complete.body.fields(5)?[4]
                            .list()?
                            .iter()
                            .map(Value::fixed)
                            .collect::<wire::Result<Vec<Id>>>()?;
                        ensure!(
                            ids == batch
                                .iter()
                                .map(|e| e.id())
                                .collect::<wire::Result<Vec<_>>>()?,
                            "subset/changed completion"
                        );
                        direct_attempted.extend(batch.iter().map(|e| e.core.n));
                    }
                    Err(error) => {
                        eprintln!("od1_deliver_repair_wait={error:#}");
                        repair_wait = true;
                    }
                }
            }
        }
        let status = {
            let engine = shared.lock().await;
            if engine.send_blocked(c)? {
                "UpgradeRequired".to_owned()
            } else if invalid_head {
                "InvalidPrefix".to_owned()
            } else if !engine
                .store
                .paths(&format!("{}pending/", prefix(c)))
                .is_empty()
            {
                let head = engine.head(c)?;
                let next = engine
                    .native(&head)?
                    .guard
                    .as_ref()
                    .context("guard")?
                    .peer
                    .next;
                format!("WaitingPrefix({next})")
            } else if !engine.unacked(c)?.is_empty() {
                "WaitingAck".to_owned()
            } else {
                "Active".to_owned()
            }
        };
        control.statuses.lock().await.insert(c, status);
        tokio::select! {_=tokio::time::sleep(Duration::from_millis(25))=>{},_=control.wake.notified()=>{}}
    }
}

async fn established(
    shared: Shared,
    connection: Connection,
    link: Arc<Link>,
    control: Arc<RuntimeControl>,
    pair: Option<Vec<u8>>,
) -> Result<()> {
    let incoming_task = tokio::spawn(incoming(shared.clone(), connection.clone(), link.clone()));
    let result = async {
        if let Some(pair) = pair {
            handshake_a(&shared, &connection, &link, &pair).await?;
        }
        data_worker(shared.clone(), connection.clone(), link.clone(), control).await
    }
    .await;
    incoming_task.abort();
    connection.close(0_u32.into(), b"OD1 reconnect");
    shared.lock().await.close_lease(link.c, link.generation);
    result
}

pub async fn run(profile: Profile) -> Result<()> {
    profile.validate()?;
    super::test_support::validate(&profile.state_dir)?;
    let store = OrderedVault::open(StateDirectoryLock::acquire(&profile.state_dir)?)?;
    let mut engine = Engine::new(store)?;
    let enabled = profile
        .enabled_contexts
        .iter()
        .map(|c| state::parse_id(c))
        .collect::<Result<Vec<_>>>()?;
    for c in &enabled {
        engine.spec(*c)?;
    }
    let endpoint = transport::bind_loopback(profile.bind.parse()?).await?;
    let addresses = endpoint.addr().ip_addrs().copied().collect::<Vec<_>>();
    std::fs::create_dir_all(&profile.locator_directory)?;
    for c in &enabled {
        let role = engine.role(*c)?;
        let revision = match engine.store.get(&descriptor_path(*c, true))? {
            Some(raw) => Descriptor::parse(engine.spec(*c)?, role, &raw)?.revision + 1,
            None => 1,
        };
        let descriptor = Descriptor::issue(
            engine.spec(*c)?,
            role,
            *endpoint.id().as_bytes(),
            addresses.clone(),
            revision,
            engine::now()?,
            engine.device.identity(),
        )?;
        let mut changes = Changes::default();
        changes.put(descriptor_path(*c, true), descriptor.raw.clone());
        let mut head = engine.head(*c)?;
        head.bump(false)?;
        engine.save(&mut head, None, changes)?;
        let path = profile
            .locator_directory
            .join(format!("{}.locator", state::hex(c)));
        let mut temp = tempfile::NamedTempFile::new_in(&profile.locator_directory)?;
        use std::io::Write;
        temp.write_all(&descriptor.raw)?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
    }
    let account = admin::claim(&engine.store)?.1;
    let (ipc, mut work) = kilogram_runtime_ipc::RuntimeIpcServer::start(
        profile.ipc_file.clone(),
        account,
        engine.device.identity(),
    )
    .await?;
    let shared = Arc::new(Mutex::new(engine));
    let control = Arc::new(RuntimeControl::default());
    let mut tasks = Vec::new();
    let listener_endpoint = endpoint.clone();
    let listener_shared = shared.clone();
    let listener_control = control.clone();
    let inbound = enabled
        .iter()
        .filter(|c| {
            shared
                .try_lock()
                .is_ok_and(|e| e.role(**c).is_ok_and(|role| role == 1))
        })
        .map(|c| (*c, Arc::new(Mutex::new(()))))
        .collect::<BTreeMap<_, _>>();
    let inbound = Arc::new(inbound);
    let slots = Arc::new(tokio::sync::Semaphore::new(2));
    tasks.push(tokio::spawn(async move {
        while let Some(incoming) = listener_endpoint.accept().await {
            let Ok(slot) = slots.clone().try_acquire_owned() else {
                drop(incoming);
                continue;
            };
            let endpoint = listener_endpoint.clone();
            let shared = listener_shared.clone();
            let control = listener_control.clone();
            let inbound = inbound.clone();
            tokio::spawn(async move {
                let _slot = slot;
                let result = async {
                    let connection = tokio::time::timeout(Duration::from_secs(15), incoming)
                        .await
                        .context("TLS admission timeout")??;
                    let link = tokio::time::timeout(
                        Duration::from_secs(15),
                        admit_b(&shared, &endpoint, &connection, &inbound),
                    )
                    .await
                    .context("B auth deadline")??;
                    established(shared, connection, link, control, None).await
                }
                .await;
                if let Err(error) = result {
                    eprintln!("od1_listener={error:#}");
                }
            });
        }
    }));
    for c in &enabled {
        if shared.lock().await.role(*c)? != 0 {
            continue;
        }
        let c = *c;
        let shared = shared.clone();
        let endpoint = endpoint.clone();
        let control = control.clone();
        tasks.push(tokio::spawn(async move {let mut delay=1_u64;loop {
            let result=async {let (connection,link,pair)=tokio::time::timeout(Duration::from_secs(15),admit_a(&shared,&endpoint,c)).await.context("A auth deadline")??;
                delay=1;established(shared.clone(),connection,link,control.clone(),Some(pair)).await}.await;
            if let Err(error)=result {eprintln!("od1_dialer={error:#}");control.statuses.lock().await.insert(c,format!("ConnectionBackoff: {error:#}"));}
            tokio::select! {_=tokio::time::sleep(Duration::from_secs(delay))=>{},_=control.wake.notified()=>{}}
            delay=(delay*2).min(60);
        }}));
    }
    println!("profile_mode=ordered-dialog-v1");
    println!("endpoint_id={}", endpoint.id());
    println!("ipc_file={}", profile.ipc_file.display());
    println!("status=runtime-listening");
    use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse};
    while let Some(item) = work.recv().await {
        let (command, reply) = item.into_parts();
        let shutdown = matches!(command, RuntimeIpcCommand::Shutdown);
        let response = match command {
            RuntimeIpcCommand::Shutdown => RuntimeIpcResponse::ShutdownAccepted,
            RuntimeIpcCommand::Ping => {
                let engine = shared.lock().await;
                RuntimeIpcResponse::Pong {
                    account_id: account,
                    device_id: engine.device.identity().device_id(),
                }
            }
            RuntimeIpcCommand::OrderedDialog { request } => {
                match ipc_request(&shared, &control, &enabled, &request).await {
                    Ok(response) => RuntimeIpcResponse::OrderedDialog { response },
                    Err(error) => RuntimeIpcResponse::Error {
                        message: format!("{error:#}"),
                    },
                }
            }
            _ => RuntimeIpcResponse::Error {
                message: "UnsupportedProfile: OD1 requires explicit ordered commands".to_owned(),
            },
        };
        let _ = reply.send(response);
        ipc.publish_change();
        if shutdown {
            break;
        }
    }
    for task in tasks {
        task.abort();
    }
    endpoint.close().await;
    ipc.shutdown().await?;
    Ok(())
}

/// Local authenticated IPC opcodes: 1 status,2 queue,3 history,4 import locator,
/// 5 export locator,6 wake/open/sync. Native keys/pickles are never returned.
pub async fn ipc_request(
    shared: &Shared,
    control: &RuntimeControl,
    enabled: &[Id],
    raw: &[u8],
) -> Result<Vec<u8>> {
    let value = wire::decode(raw, 32 * 1024)?;
    let f = value.list()?;
    ensure!(!f.is_empty(), "IPC operation");
    let mut engine = shared.lock().await;
    let result = match f[0].uint()? {
        1 => {
            ensure!(f.len() == 1, "status arity");
            let mut rows = Vec::new();
            for c in enabled {
                let head = engine.head(*c)?;
                let pending = engine.pending_stats(*c)?;
                let unacked = engine.unacked(*c)?.len();
                let counts = if head.phase == 3 {
                    let native = engine.native(&head)?;
                    let g = native.guard.as_ref().context("guard")?;
                    a([u(g.own.next), u(g.peer.next), u(g.own.g), u(g.peer.g)])
                } else {
                    a([u(0), u(0), u(0), u(0)])
                };
                rows.push(a([
                    b(*c),
                    u(head.phase),
                    u(head.native_generation),
                    opt(head.sid.as_ref().map(|s| b(s.as_bytes()))),
                    counts,
                    u((pending.0 + usize::from(pending.2)) as u64),
                    u(unacked as u64),
                    b(if engine.send_blocked(*c)? {
                        b"UpgradeRequired".to_vec()
                    } else {
                        control
                            .statuses
                            .lock()
                            .await
                            .get(c)
                            .map(String::as_bytes)
                            .unwrap_or(b"Negotiating")
                            .to_vec()
                    }),
                    a([
                        u(pending.0 as u64),
                        u(pending.1 as u64),
                        u(u64::from(pending.2)),
                        u(if pending.2 { 0 } else { 64 * 1024 }),
                    ]),
                ]));
            }
            a([b(b"ordered-dialog-v1"), u(engine.store.commits), a(rows)])
        }
        2 => {
            let f = value.fields(4)?;
            let c = f[1].fixed()?;
            ensure!(enabled.contains(&c), "context is paused");
            let inserted = engine.queue(c, f[2].fixed()?, f[3].text()?)?;
            a([u(1), u(u64::from(inserted))])
        }
        3 => {
            let f = value.fields(4)?;
            engine.history(
                f[1].fixed()?,
                usize::try_from(f[2].uint()?)?,
                usize::try_from(f[3].uint()?)?,
            )?
        }
        4 => {
            let f = value.fields(3)?;
            let c = f[1].fixed()?;
            ensure!(enabled.contains(&c), "context is paused");
            let bytes = admin::read_public(Path::new(f[2].text()?), 8192)?;
            import_locator(&mut engine, c, &bytes)?;
            a([u(1)])
        }
        5 => {
            let f = value.fields(3)?;
            let descriptor = load_descriptor(&engine, f[1].fixed()?, true)?;
            admin::write_public(
                Path::new(f[2].text()?),
                &descriptor.raw,
                engine.store.root(),
            )?;
            a([b(descriptor.raw)])
        }
        8 => {
            let f = value.fields(2)?;
            let raw = admin::read_public(Path::new(f[1].text()?), wire::MAX_AUTH)?;
            engine.pin_authority(&raw)?;
            a([u(1)])
        }
        7 => {
            let f = value.fields(3)?;
            let c = f[1].fixed()?;
            ensure!(enabled.contains(&c), "context is paused");
            let old = load_descriptor(&engine, c, true)?;
            engine.policy(c)?;
            let descriptor = Descriptor::issue(
                engine.spec(c)?,
                engine.role(c)?,
                old.endpoint,
                old.addresses,
                old.revision
                    .checked_add(1)
                    .context("locator revision exhausted")?,
                engine::now()?,
                engine.device.identity(),
            )?;
            let mut changes = Changes::default();
            changes.put(descriptor_path(c, true), descriptor.raw.clone());
            let mut head = engine.head(c)?;
            head.bump(false)?;
            engine.save(&mut head, None, changes)?;
            engine.leases.remove(&c);
            admin::write_public(
                Path::new(f[2].text()?),
                &descriptor.raw,
                engine.store.root(),
            )?;
            a([b(descriptor.raw)])
        }
        6 => {
            let f = value.fields(2)?;
            let c = f[1].fixed()?;
            ensure!(enabled.contains(&c), "context is paused");
            a([u(1)])
        }
        _ => anyhow::bail!("unsupported OD1 IPC operation"),
    };
    drop(engine);
    control.wake.notify_waiters();
    Ok(wire::encode(&result, 48 * 1024)?)
}
