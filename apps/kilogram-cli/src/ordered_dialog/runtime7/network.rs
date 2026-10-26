//! ALPN1 only. Public wire frames cross awaits; Native work stays in Owner7 jobs.
use super::*;
use iroh::{Endpoint, endpoint::Connection};
use kilogram_transport_iroh::ordered as transport;
use owner::{Lease1, Owner7};
use runtime::Handle7;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::Mutex;
static CONNECTION: AtomicU64 = AtomicU64::new(1);
pub(super) struct Link7 {
    pub c: u8,
    pub context: Id,
    pub binding: Id,
    pub peer: u64,
    pub generation: u64,
}

impl Owner7 {
    fn hello(&mut self, c: u8, local: &Descriptor, remote: &Descriptor) -> Result<Value> {
        self.current(c)?;
        let role = self.role(c)?;
        let credential = self.credentials(c, role)?;
        let consent = self.control(c, store::L1Control::Consent)?;
        Ok(a([
            b(self.spec(c)?.c),
            u(role),
            b(local.endpoint),
            b(remote.endpoint),
            b(admin::random::<32>()?),
            a([u(1)]),
            a([u(1)]),
            b(credential.raw),
            b(&self.spec(c)?.membership),
            b(wire::hash(handshake::CONSENT, &consent)),
            b(local.reference()),
        ]))
    }
    fn verify_hello(
        &mut self,
        c: u8,
        frame: &Frame,
        local: &Descriptor,
        remote: &Descriptor,
    ) -> Result<()> {
        let role = 1 - self.role(c)?;
        let spec = self.spec(c)?.clone();
        frame.verify(frame.binding, spec.peers[role as usize].cert.device_id())?;
        let f = frame.body.fields(11)?;
        let credential = Credentials::decode(f[7].bytes()?)?;
        let consent = self.control(c, store::L1Control::Consent)?;
        ensure!(
            frame.kind == 1
                && f[0].fixed::<32>()? == spec.c
                && f[1].uint()? == role
                && f[2].fixed::<32>()? == remote.endpoint
                && f[3].fixed::<32>()? == local.endpoint
                && f[4].bytes()?.len() == 32
                && f[5] == a([u(1)])
                && f[6] == a([u(1)])
                && credential.cert_ref()? == spec.peers[role as usize].cert_ref()?
                && f[8].bytes()? == spec.membership
                && f[9].fixed::<32>()? == wire::hash(handshake::CONSENT, &consent)
                && f[10].fixed::<32>()? == remote.reference(),
            "Hello1 original pins/endpoints/consent/limits"
        );
        local.valid_now(now()?)?;
        remote.valid_now(now()?)?;
        self.pin_authority(&credential.authority.encode()?)?;
        Credentials::parse(&credential.raw)?;
        self.current(c)?;
        Ok(())
    }
    fn finish_admission(
        &mut self,
        c: u8,
        connection: Connection,
        binding: Id,
        pair: Vec<u8>,
        generation: u64,
    ) -> Result<Lease1> {
        let consent = self.control(c, store::L1Control::Consent)?;
        handshake::verify_initial_pair(
            self.spec(c)?,
            &pair,
            Some(wire::hash(handshake::CONSENT, &consent)),
        )?;
        let role = self.role(c)?;
        let value = wire::decode(&pair, 192 * 1024)?;
        let peer = Frame::parse(value.fields(6)?[2 + (1 - role) as usize].bytes()?)?;
        let own = Frame::parse(value.fields(6)?[2 + role as usize].bytes()?)?;
        let local = self.descriptor(c, true)?;
        let remote = self.descriptor(c, false)?;
        ensure!(
            connection.remote_id().as_bytes() == &remote.endpoint
                && peer.binding == binding
                && own.binding == binding
                && own.body.fields(11)?[7].bytes()? == self.credentials(c, role)?.raw,
            "admission actual TLS/current own credential"
        );
        self.verify_hello(c, &peer, &local, &remote)?;
        let peer_credential = peer.body.fields(11)?[7].bytes()?.to_vec();
        let live = connection.clone();
        let mut gate = || {
            if live.close_reason().is_none() {
                Ok(())
            } else {
                Err(db::Error::PolicyRejected("closed before admission commit"))
            }
        };
        self.repo()?.commit(
            store::L1Operation::AdmissionPair { context: c, pair },
            Some(&mut gate),
        )?;
        self.admit(c, connection, binding, peer_credential, generation)
    }
}

async fn rpc(
    connection: &Connection,
    frame: &Frame,
    spec: &Spec,
    peer: u64,
    gate: Option<&Lease1>,
) -> Result<Frame> {
    let schedule = super::super::test_support::load()?;
    let trace = schedule.trace_rpc && schedule.applies(spec.c);
    let started = std::time::Instant::now();
    if trace {
        eprintln!(
            "l1_rpc kind={} stage=begin request={}",
            frame.kind,
            state::hex(&frame.request)
        );
    }
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (mut send, mut receive) = connection.open_bi().await?;
        super::super::test_support::before_write(frame).await?;
        if let Some(lease) = gate {
            lease.gate()?;
        }
        transport::write(&mut send, frame).await?;
        if trace {
            eprintln!(
                "l1_rpc kind={} stage=written request={}",
                frame.kind,
                state::hex(&frame.request)
            );
        }
        let (reply, _) = transport::read(&mut receive).await?;
        reply.verify(frame.binding, spec.peers[peer as usize].cert.device_id())?;
        ensure!(reply.request == frame.request, "wire1 response correlation");
        ensure!(reply.kind != 14, "peer Rejected/Busy wire1 response");
        Ok(reply)
    })
    .await
    .context("wire1 round timeout")?;
    if trace {
        eprintln!(
            "l1_rpc kind={} stage=finished millis={} result={}",
            frame.kind,
            started.elapsed().as_millis(),
            match &result {
                Ok(reply) => format!("reply{}", reply.kind),
                Err(error) => format!("{error:#}"),
            }
        );
    }
    result
}
async fn checked_rpc(
    handle: &Handle7,
    connection: &Connection,
    link: &Link7,
    frame: &Frame,
    spec: &Spec,
) -> Result<Frame> {
    let c = link.c;
    let binding = link.binding;
    let schedule = super::super::test_support::load()?;
    let trace = schedule.trace_rpc && schedule.applies(link.context);
    let started = std::time::Instant::now();
    if trace {
        eprintln!(
            "l1_rpc kind={} stage=lease_begin tick_ms={}",
            frame.kind,
            trace_tick()
        );
    }
    let lease = handle.call(move |o| o.check_lease(c, binding)).await?;
    if trace {
        eprintln!(
            "l1_rpc kind={} stage=lease_end tick_ms={} owner_ms={}",
            frame.kind,
            trace_tick(),
            started.elapsed().as_millis()
        );
    }
    lease.gate()?;
    rpc(connection, frame, spec, link.peer, Some(&lease)).await
}
fn trace_tick() -> u128 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis()
}

pub(super) async fn admit_a(
    handle: &Handle7,
    endpoint: &Endpoint,
    c: u8,
) -> Result<(Connection, Arc<Link7>, Vec<u8>)> {
    let generation = handle
        .call(move |o| {
            o.current(c)?;
            Ok(o.repo()?.metadata(c)?.policy_generation)
        })
        .await?;
    let attempt = admit_a_once(handle, endpoint, c);
    tokio::pin!(attempt);
    loop {
        tokio::select! {
            result = &mut attempt => return result,
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                let actual = handle.call(move |o| Ok(o.repo()?.metadata(c)?.policy_generation)).await?;
                ensure!(actual == generation, "L1 pending admission policy changed");
            }
        }
    }
}

async fn admit_a_once(
    handle: &Handle7,
    endpoint: &Endpoint,
    c: u8,
) -> Result<(Connection, Arc<Link7>, Vec<u8>)> {
    let (spec, local, remote) = handle
        .call(move |o| {
            o.current(c)?;
            ensure!(o.role(c)? == 0, "A dialer");
            Ok((
                o.spec(c)?.clone(),
                o.descriptor(c, true)?,
                o.descriptor(c, false)?,
            ))
        })
        .await?;
    local.valid_now(now()?)?;
    remote.valid_now(now()?)?;
    let connection = endpoint.connect(remote.addr()?, wire::ALPN).await?;
    ensure!(
        connection.remote_id().as_bytes() == &remote.endpoint,
        "actual TLS endpoint"
    );
    let binding = transport::binding(&connection, *endpoint.id().as_bytes(), true)?;
    let local2 = local.clone();
    let remote2 = remote.clone();
    let hello_a = handle
        .call(move |o| {
            let body = o.hello(c, &local2, &remote2)?;
            o.signed(1, binding, body)
        })
        .await?;
    let hello_b = rpc(&connection, &hello_a, &spec, 1, None).await?;
    let peer_hello = hello_b.clone();
    handle
        .call(move |o| o.verify_hello(c, &peer_hello, &local, &remote))
        .await?;
    let hello_a_raw = hello_a.raw()?;
    let hello_b_raw = hello_b.raw()?;
    let transcript = handshake::transcript(binding, &hello_a_raw, &hello_b_raw)?;
    let certs = spec.cert_refs()?;
    let proof_body = a([
        b(transcript),
        b(spec.c),
        u(1),
        u(1),
        b(certs[0]),
        b(certs[1]),
    ]);
    let proof_a_body = proof_body.clone();
    let proof_a = handle
        .call(move |o| o.signed(2, binding, proof_a_body))
        .await?;
    let proof_b = rpc(&connection, &proof_a, &spec, 1, None).await?;
    ensure!(
        proof_b.kind == 2 && proof_b.body == proof_body,
        "Proof1 B transcript"
    );
    let pair = handshake::initial_pair(
        transcript,
        hello_a_raw,
        hello_b_raw,
        proof_a.raw()?,
        proof_b.raw()?,
    )?;
    let context = spec.c;
    let body = a([b(transcript), b(context), u(1), u(1)]);
    let request_body = body.clone();
    let admitted = handle
        .call(move |o| o.signed(3, binding, request_body))
        .await?;
    let response = rpc(&connection, &admitted, &spec, 1, None).await?;
    ensure!(
        response.kind == 3 && response.body == body,
        "B admitted confirmation"
    );
    let generation = CONNECTION.fetch_add(1, Ordering::Relaxed);
    let live = connection.clone();
    let saved_pair = pair.clone();
    handle
        .call(move |o| o.finish_admission(c, live, binding, saved_pair, generation))
        .await?;
    Ok((
        connection,
        Arc::new(Link7 {
            c,
            context,
            binding,
            peer: 1,
            generation,
        }),
        pair,
    ))
}
pub(super) async fn admit_b(
    handle: &Handle7,
    endpoint: &Endpoint,
    connection: &Connection,
    slots: &std::collections::BTreeMap<Id, Arc<Mutex<()>>>,
) -> Result<(Arc<Link7>, tokio::sync::OwnedMutexGuard<()>)> {
    let binding = transport::binding(connection, *endpoint.id().as_bytes(), false)?;
    let (mut send, mut receive) = connection.accept_bi().await?;
    let (hello_a, hello_a_raw) = transport::read(&mut receive).await?;
    ensure!(hello_a.kind == 1, "Hello1 must be first");
    let context = hello_a.body.fields(11)?[0].fixed::<32>()?;
    let guard = slots
        .get(&context)
        .context("C not enabled for B")?
        .clone()
        .try_lock_owned()
        .context("C already authenticating/admitted")?;
    let remote_id = *connection.remote_id().as_bytes();
    let first = hello_a.clone();
    let (c, spec, hello_b) = handle
        .call(move |o| {
            let c = o.slot(context)?;
            ensure!(o.role(c)? == 1, "B accept role");
            let local = o.descriptor(c, true)?;
            let remote = o.descriptor(c, false)?;
            ensure!(remote.endpoint == remote_id, "actual TLS dialer");
            o.verify_hello(c, &first, &local, &remote)?;
            let hello = o.hello(c, &local, &remote)?;
            Ok((
                c,
                o.spec(c)?.clone(),
                Frame::signed(1, binding, first.request, hello, o.device.identity())?,
            ))
        })
        .await?;
    let hello_b_raw = transport::write(&mut send, &hello_b).await?;
    let transcript = handshake::transcript(binding, &hello_a_raw, &hello_b_raw)?;
    let certs = spec.cert_refs()?;
    let proof_body = a([
        b(transcript),
        b(context),
        u(1),
        u(1),
        b(certs[0]),
        b(certs[1]),
    ]);
    let (mut send, mut receive) = connection.accept_bi().await?;
    let (proof_a, proof_a_raw) = transport::read(&mut receive).await?;
    proof_a.verify(binding, spec.peers[0].cert.device_id())?;
    ensure!(
        proof_a.kind == 2 && proof_a.body == proof_body,
        "A Proof1 transcript"
    );
    let proof_request = proof_a.request;
    let proof_b = handle
        .call(move |o| {
            Ok(Frame::signed(
                2,
                binding,
                proof_request,
                proof_body,
                o.device.identity(),
            )?)
        })
        .await?;
    let proof_b_raw = transport::write(&mut send, &proof_b).await?;
    let pair = handshake::initial_pair(
        transcript,
        hello_a_raw,
        hello_b_raw,
        proof_a_raw,
        proof_b_raw,
    )?;
    let (mut send, mut receive) = connection.accept_bi().await?;
    let (admitted, _) = transport::read(&mut receive).await?;
    admitted.verify(binding, spec.peers[0].cert.device_id())?;
    let body = a([b(transcript), b(context), u(1), u(1)]);
    ensure!(
        admitted.kind == 3 && admitted.body == body,
        "A admission confirmation"
    );
    let generation = CONNECTION.fetch_add(1, Ordering::Relaxed);
    let live = connection.clone();
    let request = admitted.request;
    let reply = handle
        .call(move |o| {
            o.finish_admission(c, live, binding, pair, generation)?;
            Ok(Frame::signed(
                3,
                binding,
                request,
                body,
                o.device.identity(),
            )?)
        })
        .await?;
    transport::write(&mut send, &reply).await?;
    Ok((
        Arc::new(Link7 {
            c,
            context,
            binding,
            peer: 0,
            generation,
        }),
        guard,
    ))
}
async fn handshake_a(
    handle: &Handle7,
    connection: &Connection,
    link: &Link7,
    pair: Vec<u8>,
    spec: &Spec,
) -> Result<()> {
    let c = link.c;
    let binding = link.binding;
    let context = link.context;
    let offer = handle
        .call(move |o| {
            let offer = o.ensure_offer(c, &pair, binding)?;
            let original = o.control(c, store::L1Control::InitialPair)?;
            o.signed(5, binding, a([b(context), b(offer), b(original)]))
        })
        .await?;
    let grant = checked_rpc(handle, connection, link, &offer, spec).await?;
    ensure!(
        grant.kind == 6 && grant.body.fields(3)?[0].fixed::<32>()? == context,
        "Grant response"
    );
    let grant = grant.body.fields(3)?[2].bytes()?.to_vec();
    let init = handle
        .call(move |o| {
            let init = o.init(c, &grant, binding)?;
            let h = o.head(c)?;
            o.signed(
                7,
                binding,
                a([
                    b(context),
                    b(h.d.context("D")?),
                    b(h.e.context("E")?),
                    b(init),
                ]),
            )
        })
        .await?;
    let confirm = checked_rpc(handle, connection, link, &init, spec).await?;
    ensure!(confirm.kind == 7, "Confirm response");
    let confirm = confirm.body.fields(4)?[3].bytes()?.to_vec();
    let ready = handle
        .call(move |o| {
            let ready = o.receive_confirm(c, &confirm, binding)?;
            let h = o.head(c)?;
            o.signed(
                8,
                binding,
                a([
                    b(context),
                    b(h.d.context("D")?),
                    b(h.e.context("E")?),
                    b(ready),
                ]),
            )
        })
        .await?;
    let peer_ready = checked_rpc(handle, connection, link, &ready, spec).await?;
    ensure!(peer_ready.kind == 8, "Ready response");
    let ready = peer_ready.body.fields(4)?[3].bytes()?.to_vec();
    handle.call(move |o| o.ready(c, &ready, binding)).await?;
    Ok(())
}
async fn incoming(handle: Handle7, connection: Connection, link: Arc<Link7>) -> Result<()> {
    loop {
        let (mut send, mut receive) = connection.accept_bi().await?;
        let result = tokio::time::timeout(Duration::from_secs(15), async {
            let (frame, _) = transport::read(&mut receive).await?;
            let request = frame.request;
            let c = link.c;
            let binding = link.binding;
            match handle.call(move |o| o.dispatch(c, frame)).await {
                Ok(Some(reply)) => {
                    let lease = handle.call(move |o| o.check_lease(c, binding)).await?;
                    lease.gate()?;
                    super::super::test_support::before_write(&reply.frame).await?;
                    lease.gate()?;
                    transport::write(&mut send, &reply.frame).await?;
                    drop(reply.refs);
                }
                Ok(None) => {
                    send.finish()?;
                }
                Err(error) => {
                    let message = format!("{error:#}");
                    eprintln!("l1_dispatch_rejected {message}");
                    let reject = handle
                        .call(move |o| {
                            let h = o.head(c)?;
                            Frame::signed(
                                14,
                                binding,
                                request,
                                a([
                                    b(h.c),
                                    opt(h.d.map(b)),
                                    opt(h.e.map(b)),
                                    b(request),
                                    u(2),
                                    u(7),
                                    opt(None),
                                    u(1),
                                ]),
                                o.device.identity(),
                            )
                            .map_err(Into::into)
                        })
                        .await?;
                    transport::write(&mut send, &reject).await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("wire1 incoming round timeout")?;
        result?;
    }
}
async fn worker(
    handle: &Handle7,
    connection: &Connection,
    link: &Link7,
    spec: &Spec,
) -> Result<()> {
    let c = link.c;
    let binding = link.binding;
    let mut direct_after = None;
    loop {
        let active = handle
            .call(move |o| {
                o.check_lease(c, binding)?;
                o.active(c)
            })
            .await?;
        if active {
            let resume = handle
                .call(move |o| {
                    let body = o.resume_body(c)?;
                    o.signed(9, binding, body)
                })
                .await?;
            let reply = checked_rpc(handle, connection, link, &resume, spec).await?;
            ensure!(reply.kind == 9, "Resume response");
            handle.call(move |o| o.observe_hint(c, &reply)).await?;
            if let Some(request) = handle.call(move |o| o.prefix_request(c, binding)).await? {
                let reply = checked_rpc(handle, connection, link, &request, spec).await?;
                ensure!(reply.kind == 12, "PrefixBatch response");
                handle.call(move |o| o.accept_data_frame(c, &reply)).await?;
            }
            let schedule = super::super::test_support::load()?;
            let trace = schedule.trace_rpc && schedule.applies(link.context);
            let started = std::time::Instant::now();
            if trace {
                eprintln!(
                    "l1_delivery c={c} stage=owner_begin after={direct_after:?} tick_ms={}",
                    trace_tick()
                );
            }
            let delivery = handle
                .call(move |o| o.outgoing(c, binding, direct_after))
                .await?;
            if trace {
                eprintln!(
                    "l1_delivery c={c} stage=owner_end after={direct_after:?} tick_ms={} owner_ms={} selected={}",
                    trace_tick(),
                    started.elapsed().as_millis(),
                    delivery.is_some()
                );
            }
            if let Some(delivery) = delivery {
                let reply = checked_rpc(handle, connection, link, &delivery.frame, spec).await?;
                ensure!(reply.kind == 13, "Deliver Complete response");
                let f = reply.body.fields(5)?;
                ensure!(
                    f[0].fixed::<32>()? == link.context
                        && f[1] == delivery.frame.body.fields(6)?[1]
                        && f[2] == delivery.frame.body.fields(6)?[2]
                        && f[3].fixed::<16>()? == delivery.frame.request,
                    "Complete C/request"
                );
                let sent = delivery.frame.body.fields(6)?[5]
                    .list()?
                    .iter()
                    .map(|v| Ok(wire::hash(wire::EVENT_ID, v.bytes()?)))
                    .collect::<wire::Result<Vec<_>>>()?;
                let received = f[4]
                    .list()?
                    .iter()
                    .map(Value::fixed)
                    .collect::<wire::Result<Vec<Id>>>()?;
                ensure!(sent == received, "Complete exact possession IDs");
                let schedule = super::super::test_support::load()?;
                if schedule.trace_rpc && schedule.applies(link.context) {
                    eprintln!(
                        "l1_complete c={c} after={direct_after:?} tick_ms={} exact_ids={:?}",
                        trace_tick(),
                        received.iter().map(|id| state::hex(id)).collect::<Vec<_>>()
                    );
                }
                direct_after = delivery.frame.body.fields(6)?[5]
                    .list()?
                    .last()
                    .map(|v| -> Result<u64> {
                        let value = wire::decode(v.bytes()?, wire::MAX_EVENT)?;
                        Ok(wire::Core::parse(&value.fields(5)?[1])?.n)
                    })
                    .transpose()?;
                drop(delivery.refs);
            } else {
                direct_after = None;
            }
            if let Some(query) = handle.call(move |o| o.ack_query(c, binding)).await? {
                let reply = checked_rpc(handle, connection, link, &query, spec).await?;
                ensure!(reply.kind == 15, "AckBatch response");
                handle.call(move |o| o.accept_ack_frame(c, &reply)).await?;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
pub(super) async fn established(
    handle: Handle7,
    connection: Connection,
    link: Arc<Link7>,
    pair: Option<Vec<u8>>,
) -> Result<()> {
    let c = link.c;
    let spec = handle.call(move |o| Ok(o.spec(c)?.clone())).await?;
    let result = async {
        let mut incoming = tokio::spawn(incoming(handle.clone(), connection.clone(), link.clone()));
        let outbound = async {
            if let Some(pair) = pair {
                handshake_a(&handle, &connection, &link, pair, &spec).await?;
            }
            worker(&handle, &connection, &link, &spec).await
        };
        let (result,incoming_done) = tokio::select! {
            result=outbound=>(result,false),
            result=&mut incoming=>(match result {Ok(result)=>result,Err(error)=>Err(error.into())},true)
        };
        if !incoming_done { incoming.abort(); let _=incoming.await; }
        result
    }
    .await;
    connection.close(0_u32.into(), b"L1 reconnect");
    let generation = link.generation;
    let _ = handle
        .call(move |o| {
            if o.leases[usize::from(c)]
                .as_ref()
                .is_some_and(|l| l.generation == generation)
            {
                o.invalidate(c);
            }
            Ok(())
        })
        .await;
    result
}
