//! Bounded two-process transport and authenticated IPC adapter for Owner2.
use super::diagnostics::{Counters, DialStage, Event};
use super::*;
use admission::Link2;
use discovery::AdmissionPermit2;
use iroh::{
    Endpoint,
    endpoint::{Connection, IncomingAddr},
};
use kilogram_runtime_ipc::lab_diagnostics::{Kind as DiagnosticKind, Sink as DiagnosticSink};
use kilogram_runtime_ipc::ordered2::{
    DiscoveryStatus2, DiscoveryView2, HISTORY_ANCHOR_OPCODE, HISTORY_FOLLOW_OPCODE,
    HISTORY_POSITION_REPLY_CAP, LanDiscovery2,
};
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse, RuntimeIpcServer};
use kilogram_transport_iroh::ordered2 as transport;
use network::{AckProgress2, Dispatch2, PrefixPlan2};
use owner::Owner2;
use std::{
    net::IpAddr,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Semaphore, mpsc, oneshot};

pub(super) type Job2 = Box<dyn FnOnce(&mut Owner2) + Send>;
#[derive(Clone)]
pub(super) struct Handle2 {
    sender: mpsc::Sender<Job2>,
    stop: Arc<AtomicBool>,
    resources: Arc<resources::Resources2>,
    pub(super) diagnostics: Arc<Counters>,
}
impl Handle2 {
    pub async fn call<T: Send + 'static, F: FnOnce(&mut Owner2) -> Result<T> + Send + 'static>(
        &self,
        f: F,
    ) -> Result<T> {
        let submit = timing::span(timing::Metric::MailboxSubmit);
        let permit = self.sender.reserve().await.context("owner stopped")?;
        drop(submit);
        let queued = timing::instant();
        let (tx, rx) = oneshot::channel();
        self.resources
            .observed_mailbox(self.sender.max_capacity() - self.sender.capacity());
        permit.send(Box::new(move |owner| {
            timing::record(timing::Metric::MailboxQueue, queued);
            let _timing = timing::span(timing::Metric::OwnerJob);
            let _ = tx.send(f(owner));
        }));
        rx.await.context("owner response cancelled")?
    }
}
fn owner_thread(mut owner: Owner2) -> Result<(Handle2, std::thread::JoinHandle<()>)> {
    let (sender, mut receiver) = mpsc::channel::<Job2>(16);
    let stop = Arc::new(AtomicBool::new(false));
    owner.resources.bind_mailbox(&sender);
    let handle = Handle2 {
        sender,
        stop: stop.clone(),
        resources: owner.resources.clone(),
        diagnostics: owner.diagnostics.clone(),
    };
    let join = std::thread::Builder::new()
        .name("kilogram-od2-owner".into())
        .spawn(move || {
            let mut next = 0;
            let mut receive_turn = [false; 2];
            let mut retry_at = [Instant::now(); 2];
            let mut last_error: [Option<String>; 2] = [None, None];
            while !stop.load(Ordering::Acquire) {
                owner.expire_public_import();
                let mut worked = false;
                match receiver.try_recv() {
                    Ok(job) => {
                        job(&mut owner);
                        worked = true;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                    Err(_) => {}
                }
                if owner.repository.requires_reopen() {
                    owner.needs_reopen = true;
                }
                if owner.needs_reopen
                    && let Err(e) = owner.recover()
                {
                    eprintln!("od2_recovery_failed {e:#}");
                    stop.store(true, Ordering::Release);
                    break;
                }
                // Each data turn has at most four native packets. All C share this round-robin.
                let c = owner.enabled[next % owner.enabled.len()];
                next = (next + 1) % owner.enabled.len();
                if Instant::now() < retry_at[usize::from(c)] {
                    timing::idle_sleep(Duration::from_millis(2));
                    continue;
                }
                // Empty turns use poison-checked authenticated root/ledger counters.
                // Nonempty work still loads its installed snapshot and checks phase,
                // current policy/time/provider/lease before the closed commit gate.
                let result = if receive_turn[usize::from(c)] {
                    owner.drain(c)
                } else {
                    owner.materialize(c)
                };
                receive_turn[usize::from(c)] = !receive_turn[usize::from(c)];
                match result {
                    Ok(v) => {
                        worked |= v;
                        last_error[usize::from(c)] = None;
                    }
                    Err(error) => {
                        let message = format!("{error:#}").chars().take(512).collect::<String>();
                        if last_error[usize::from(c)].as_ref() != Some(&message) {
                            eprintln!("od2_context_wait context={c} {message}");
                            last_error[usize::from(c)] = Some(message);
                        }
                        retry_at[usize::from(c)] = Instant::now() + Duration::from_millis(100);
                    }
                }
                if owner.needs_reopen
                    && let Err(e) = owner.recover()
                {
                    eprintln!("od2_recovery_failed {e:#}");
                    stop.store(true, Ordering::Release);
                    break;
                }
                std::thread::yield_now();
                if !worked {
                    timing::idle_sleep(Duration::from_millis(2));
                }
            }
            owner.withdraw_lan_discovery();
            owner.invalidate();
        })
        .context("spawn OD2 owner")?;
    Ok((handle, join))
}
async fn read_round(
    connection: &Connection,
) -> Result<(iroh::endpoint::SendStream, wire::ParsedFrame2)> {
    let _timing = timing::span(timing::Metric::ReadRound);
    let (send, mut receive) = connection.accept_bi().await?;
    let frame = transport::read(&mut receive).await?;
    // Retain only the response half; EOF of the input is required by the decoder.
    send.set_priority(0)?;
    Ok((send, frame))
}
async fn rpc(connection: &Connection, dispatch: Dispatch2) -> Result<wire::ParsedFrame2> {
    let _timing = timing::span(timing::Metric::Rpc);
    tokio::time::timeout(Duration::from_secs(15), async {
        let evidence = ack_evidence(&dispatch.frame)?;
        hooks::transport_barrier(dispatch.c, dispatch.frame.body().kind(), &evidence).await?;
        dispatch.check(connection)?;
        let request = dispatch.frame.request();
        let (mut send, mut receive) = connection.open_bi().await?;
        dispatch.check(connection)?;
        transport::write(&mut send, &dispatch.frame).await?;
        let reply = transport::read(&mut receive).await?;
        ensure!(reply.request() == request, "round correlation");
        Ok(reply)
    })
    .await
    .context("OD2 round deadline")?
}
fn ack_evidence(frame: &wire::ParsedFrame2) -> Result<Vec<Id>> {
    if frame.body().kind() != wire::FrameKind::AckBatch {
        return Ok(vec![]);
    }
    Ok(
        wire::tagged_items(&frame.body().value().fields(8)?[6], wire::MAX_RECEIPT)?
            .into_iter()
            .map(|item| wire::ParsedReceipt2::parse(&item.raw).map(|r| r.id()))
            .collect::<wire::Result<Vec<_>>>()?,
    )
}
async fn admission_rpc(
    connection: &Connection,
    frame: &wire::ParsedFrame2,
    deadline: Instant,
    permit: Option<&AdmissionPermit2>,
) -> Result<wire::ParsedFrame2> {
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
        let (mut send, mut receive) =
            admission_stream(permit, async { Ok(connection.open_bi().await?) }).await?;
        write_admission(&mut send, frame, permit).await?;
        let reply = transport::read(&mut receive).await?;
        ensure!(reply.request() == frame.request(), "admission correlation");
        Ok(reply)
    })
    .await
    .context("whole admission deadline")?
}

async fn admission_stream<T>(
    permit: Option<&AdmissionPermit2>,
    open: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    if let Some(permit) = permit {
        permit.check()?;
    }
    let stream = open.await?;
    if let Some(permit) = permit {
        permit.check()?;
    }
    Ok(stream)
}

async fn write_admission(
    send: &mut iroh::endpoint::SendStream,
    frame: &wire::ParsedFrame2,
    permit: Option<&AdmissionPermit2>,
) -> Result<()> {
    let Some(permit) = permit else {
        return transport::write(send, frame).await;
    };
    let result = transport::write_checked(send, frame, || permit.check()).await;
    if result.is_err() {
        let _ = send.reset(0u32.into());
    }
    result
}
fn dial_peer(
    owner: &mut Owner2,
    c: u8,
) -> Result<(
    iroh::EndpointAddr,
    kilogram_transport_iroh::RoutePolicy,
    Option<Arc<discovery::LocatorPair2>>,
)> {
    let (s, current) = owner.require(c, false)?;
    let pair = owner.pair_for(&s, now()?)?.cloned();
    let locator = wire::ParsedRuntimeLocator2::parse(
        s.spec
            .peer_locator
            .as_deref()
            .context("peer locator missing")?,
    )?;
    if owner.route.lan().is_some() {
        locator.verify(
            s.head.context,
            &current.peers[usize::from(1 - s.head.role)],
            &current.peers[usize::from(s.head.role)],
        )?;
        let wall = now()?;
        ensure!(
            locator.fields().not_before <= wall && wall < locator.fields().expires,
            "LAN peer locator interval"
        );
        if !owner.auto_discovery() {
            owner.route.advertisement(
                owner
                    .lan_endpoint
                    .as_ref()
                    .context("LAN interface not currently verified")?,
            )?;
        }
    }
    Ok((
        owner.route.remote(&locator)?,
        owner.route.connection_policy(&locator)?,
        pair,
    ))
}
async fn admit_a(
    handle: &Handle2,
    endpoint: &Endpoint,
    c: u8,
    deadline: Instant,
) -> Result<(Connection, Link2)> {
    handle.diagnostics.stage(c, DialStage::PeerSelection);
    let (remote, route, pair) = handle.call(move |owner| dial_peer(owner, c)).await?;
    handle.diagnostics.stage(c, DialStage::Connect);
    let connection = tokio::time::timeout(
        Duration::from_secs(15),
        endpoint.connect(remote, wire::ALPN),
    )
    .await
    .context("TLS/connect deadline")??;
    let mut closing = CloseOnFailure(connection.clone(), true);
    handle.diagnostics.stage(c, DialStage::RoutePolicy);
    kilogram_transport_iroh::await_route_policy(
        &connection,
        route,
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(15)),
    )
    .await?;
    let binding = transport::binding(&connection, *endpoint.id().as_bytes(), true)?;
    let conn = connection.clone();
    handle.diagnostics.stage(c, DialStage::BeginAdmission);
    let (g, hello, permit) = handle
        .call(move |owner| owner.begin_admission(c, conn, binding, deadline, None, pair.as_ref()))
        .await?;
    handle.diagnostics.stage(c, DialStage::Hello);
    let reply = admission_rpc(&connection, &hello, deadline, permit.as_ref()).await?;
    handle
        .call(move |owner| owner.accept_hello(c, g, reply))
        .await?;
    handle.diagnostics.stage(c, DialStage::Proof);
    let proof = handle
        .call(move |owner| owner.admission_proof(c, g))
        .await?;
    let reply = admission_rpc(&connection, &proof, deadline, permit.as_ref()).await?;
    handle
        .call(move |owner| owner.accept_proof(c, g, reply))
        .await?;
    handle.diagnostics.stage(c, DialStage::Admitted);
    let admitted = handle
        .call(move |owner| owner.admitted_frame(c, g, None))
        .await?;
    let reply = admission_rpc(&connection, &admitted, deadline, permit.as_ref()).await?;
    let link = handle
        .call(move |owner| owner.accept_admitted(c, g, reply))
        .await?;
    closing.1 = false;
    handle.diagnostics.stage(c, DialStage::AuthenticatedLease);
    handle.diagnostics.lease(c);
    Ok((connection, link))
}
struct CloseOnFailure(Connection, bool);
impl Drop for CloseOnFailure {
    fn drop(&mut self) {
        if self.1 {
            self.0.close(0u32.into(), b"admission failed/cancelled");
        }
    }
}
async fn admit_b(
    handle: &Handle2,
    endpoint: &Endpoint,
    connection: &Connection,
    deadline: Instant,
    pair: Option<Arc<discovery::LocatorPair2>>,
) -> Result<Link2> {
    let remote = *connection.remote_id().as_bytes();
    let expected = pair.clone();
    let route = handle
        .call(move |owner| owner.route_for_remote(remote, expected.as_ref()))
        .await?;
    kilogram_transport_iroh::await_route_policy(
        connection,
        route,
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(15)),
    )
    .await?;
    let binding = transport::binding(connection, *endpoint.id().as_bytes(), false)?;
    let (mut send, hello) = read_round(connection).await?;
    ensure!(hello.body().kind() == wire::FrameKind::Hello, "Hello first");
    let id = hello.body().value().fields(12)?[0].fixed::<32>()?;
    let request = hello.request();
    let conn = connection.clone();
    let (c, g, own, permit) = handle
        .call(move |owner| {
            let c = owner.slot(id)?;
            ensure!(owner.snapshot(c)?.head.role == 1, "B listener role");
            owner.observe_hello_authority(c, &conn, binding, &hello, pair.as_ref(), deadline)?;
            let (g, own, permit) =
                owner.begin_admission(c, conn, binding, deadline, Some(request), pair.as_ref())?;
            owner.accept_hello(c, g, hello)?;
            Ok((c, g, own, permit))
        })
        .await?;
    write_admission(&mut send, &own, permit.as_ref()).await?;
    let (mut send, proof) = read_round(connection).await?;
    handle
        .call(move |owner| owner.accept_proof(c, g, proof))
        .await?;
    let own = handle
        .call(move |owner| owner.admission_proof(c, g))
        .await?;
    write_admission(&mut send, &own, permit.as_ref()).await?;
    let (mut send, admitted) = read_round(connection).await?;
    let request = admitted.request();
    let own = handle
        .call(move |owner| owner.admitted_frame(c, g, Some(request)))
        .await?;
    let link = handle
        .call(move |owner| owner.accept_admitted(c, g, admitted))
        .await?;
    write_admission(&mut send, &own, permit.as_ref()).await?;
    handle.diagnostics.event(Event::InboundLease);
    handle.diagnostics.lease(c);
    Ok(link)
}
async fn verify(handle: &Handle2, link: &Link2, frame: &wire::ParsedFrame2) -> Result<()> {
    let link = link.clone();
    let frame = frame.clone();
    handle
        .call(move |owner| owner.verify_frame(&link, &frame))
        .await
}
async fn sign(
    handle: &Handle2,
    link: &Link2,
    kind: wire::FrameKind,
    body: Value,
    request: [u8; 16],
) -> Result<Dispatch2> {
    let link = link.clone();
    handle
        .call(move |owner| owner.frame(&link, kind, body, request, None))
        .await
}
async fn sign_prepared(
    handle: &Handle2,
    link: &Link2,
    kind: wire::FrameKind,
    prepared: network::PreparedBody2,
    request: [u8; 16],
) -> Result<Dispatch2> {
    let link = link.clone();
    handle
        .call(move |owner| {
            owner.frame(
                &link,
                kind,
                prepared.body,
                request,
                Some(prepared.references),
            )
        })
        .await
}
async fn fresh_a(handle: &Handle2, connection: &Connection, link: &Link2) -> Result<()> {
    let c = link.c;
    eprintln!("od2_fresh_begin context={c}");
    let body = handle
        .call(move |owner| {
            let (offer, pair) = owner.offer(c)?;
            Ok(a([b(owner.snapshot(c)?.head.context), b(offer), b(pair)]))
        })
        .await?;
    eprintln!("od2_fresh_offer_ready context={c}");
    let reply = rpc(
        connection,
        sign(
            handle,
            link,
            wire::FrameKind::Offer,
            body,
            shared_admin::random()?,
        )
        .await?,
    )
    .await?;
    verify(handle, link, &reply).await?;
    ensure!(reply.body().kind() == wire::FrameKind::Grant, "Grant reply");
    let grant = reply.body().value().fields(3)?[2].bytes()?.to_vec();
    let body = handle
        .call(move |owner| {
            let init = owner.init(c, grant)?;
            let s = owner.snapshot(c)?;
            Ok(a([
                b(s.head.context),
                b(s.head.dialog.context("D")?),
                b(s.head.epoch.context("E")?),
                u(2),
                b(init),
            ]))
        })
        .await?;
    let reply = rpc(
        connection,
        sign(
            handle,
            link,
            wire::FrameKind::EpochCipher,
            body,
            shared_admin::random()?,
        )
        .await?,
    )
    .await?;
    verify(handle, link, &reply).await?;
    ensure!(
        reply.body().kind() == wire::FrameKind::EpochCipher,
        "Confirm reply"
    );
    let raw = reply.body().value().fields(5)?[4].bytes()?.to_vec();
    handle
        .call(move |owner| owner.receive_genesis(c, raw))
        .await?;
    let body = handle
        .call(move |owner| {
            let ready = owner.ready(c)?;
            let s = owner.snapshot(c)?;
            Ok(a([
                b(s.head.context),
                b(s.head.dialog.context("D")?),
                b(s.head.epoch.context("E")?),
                b(ready),
            ]))
        })
        .await?;
    let reply = rpc(
        connection,
        sign(
            handle,
            link,
            wire::FrameKind::Ready,
            body,
            shared_admin::random()?,
        )
        .await?,
    )
    .await?;
    verify(handle, link, &reply).await?;
    ensure!(reply.body().kind() == wire::FrameKind::Ready, "Ready reply");
    let raw = reply.body().value().fields(4)?[3].bytes()?.to_vec();
    handle.call(move |owner| owner.accept_ready(c, raw)).await?;
    Ok(())
}
async fn grant_for_frontier(handle: &Handle2, c: u8) -> Result<()> {
    handle.call(move |owner| owner.grant_repair(c)).await
}
async fn take_prefix(
    handle: &Handle2,
    link: &Link2,
    body: Value,
    plan: PrefixPlan2,
) -> Result<Vec<Id>> {
    let _timing = timing::span(timing::Metric::TakePrefix);
    let c = link.c;
    let head = plan.head;
    let grant = plan.grant;
    let start = plan.start;
    let (items, records) = handle
        .call(move |owner| owner.prefix_items(c, &body, &plan))
        .await?;
    let records = Arc::new(records);
    let mut ids = Vec::with_capacity(items.len());
    for raw in items {
        let records = records.clone();
        ids.push(
            handle
                .call(move |owner| owner.ingest(c, raw, &records, head, grant))
                .await?,
        );
    }
    if !ids.is_empty() {
        let count = ids.len();
        handle
            .call(move |owner| {
                if head {
                    owner.stats.prefix_head_pages[usize::from(c)] += 1;
                } else {
                    owner.stats.prefix_pages[usize::from(c)] += 1;
                }
                eprintln!("od2_prefix_saved context={c} head={head} start={start} count={count}");
                if !head {
                    hooks::prefix_saved(
                        c,
                        owner.stats.prefix_pages[usize::from(c)],
                        serde_json::json!({"start":start,"count":count}),
                    )?;
                }
                Ok(())
            })
            .await?;
    }
    Ok(ids)
}
async fn take_ack(handle: &Handle2, c: u8, body: Value) -> Result<()> {
    let _timing = timing::span(timing::Metric::TakeAck);
    let (items, records) = handle.call(move |owner| owner.ack_items(c, &body)).await?;
    let records = Arc::new(records);
    for raw in items {
        let records = records.clone();
        handle
            .call(move |owner| owner.acknowledge(c, raw, &records))
            .await?;
    }
    Ok(())
}
async fn take_delivery(handle: &Handle2, c: u8, body: Value) -> Result<Vec<Id>> {
    let _timing = timing::span(timing::Metric::TakeDelivery);
    let (items, records) = handle
        .call(move |owner| owner.delivery_items(c, &body))
        .await?;
    let records = Arc::new(records);
    let mut ids = Vec::with_capacity(items.len());
    for raw in items {
        let records = records.clone();
        ids.push(
            handle
                .call(move |owner| owner.ingest(c, raw, &records, false, None))
                .await?,
        );
    }
    Ok(ids)
}
async fn push_live_a(
    handle: &Handle2,
    connection: &Connection,
    link: &Link2,
    cursor: &mut u64,
) -> Result<()> {
    let _timing = timing::span(timing::Metric::PushLive);
    let c = link.c;
    let first = *cursor;
    let Some(window) = handle
        .call(move |owner| owner.live_window(c, first))
        .await?
    else {
        *cursor = 1;
        return Ok(());
    };
    let body = window.bodies.first().context("live window page")?;
    let last = wire::tagged_items(&body.fields(7)?[6], wire::MAX_EVENT)?
        .pop()
        .context("live page tail")?;
    let next = wire::ParsedEvent2::parse(&last.raw)?
        .core()
        .n
        .checked_add(1)
        .context("live cursor")?;
    tokio::time::timeout(Duration::from_secs(15), async {
        hooks::transport_barrier(c, wire::FrameKind::Deliver, &window.ids).await?;
        for body in window.bodies.iter().take(1) {
            let expected = wire::tagged_items(&body.fields(7)?[6], wire::MAX_EVENT)?
                .into_iter()
                .map(|i| wire::ParsedEvent2::parse(&i.raw).map(|e| e.id()))
                .collect::<wire::Result<Vec<_>>>()?;
            let request = shared_admin::random()?;
            let reply = rpc(
                connection,
                sign(
                    handle,
                    link,
                    wire::FrameKind::Deliver,
                    body.clone(),
                    request,
                )
                .await?,
            )
            .await?;
            verify(handle, link, &reply).await?;
            ensure!(
                reply.body().kind() == wire::FrameKind::Complete,
                "Deliver Complete"
            );
            let f = reply.body().value().fields(6)?;
            ensure!(
                f[4].fixed::<16>()? == request && f[5] == a(expected.iter().map(b)),
                "Deliver Complete exact all IDs"
            );
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("live window deadline")??;
    *cursor = next;
    Ok(())
}
async fn data_round_a(
    handle: &Handle2,
    connection: &Connection,
    link: &Link2,
    live_cursor: &mut u64,
) -> Result<()> {
    let _timing = timing::span(timing::Metric::DataRound);
    push_live_a(handle, connection, link, live_cursor).await?;
    let c = link.c;
    let body = handle.call(move |owner| owner.head_body(c)).await?;
    let reply = rpc(
        connection,
        sign(
            handle,
            link,
            wire::FrameKind::Head,
            body,
            shared_admin::random()?,
        )
        .await?,
    )
    .await?;
    verify(handle, link, &reply).await?;
    match reply.body().kind() {
        wire::FrameKind::PrefixReq => {
            let request = reply.request();
            let remote = reply.body().value().clone();
            let body = handle
                .call(move |owner| owner.prefix_response(c, &remote))
                .await?;
            let expected = wire::tagged_items(&body.body.fields(11)?[9], wire::MAX_EVENT)?
                .into_iter()
                .map(|i| wire::ParsedEvent2::parse(&i.raw).map(|e| e.id()))
                .collect::<wire::Result<Vec<_>>>()?;
            let reply = rpc(
                connection,
                sign_prepared(handle, link, wire::FrameKind::PrefixResp, body, request).await?,
            )
            .await?;
            verify(handle, link, &reply).await?;
            ensure!(
                reply.body().kind() == wire::FrameKind::Complete,
                "prefix Complete"
            );
            let f = reply.body().value().fields(6)?;
            ensure!(
                f[4].fixed::<16>()? == request && f[5] == a(expected.iter().map(b)),
                "Complete exact entire batch"
            );
        }
        wire::FrameKind::Head => {
            let body = reply.body().value().clone();
            handle
                .call(move |owner| owner.accept_head(c, &body))
                .await?;
            grant_for_frontier(handle, c).await?;
            let request = shared_admin::random()?;
            if let Some((body, plan)) = handle
                .call(move |owner| owner.prefix_request(c, request))
                .await?
            {
                let reply = rpc(
                    connection,
                    sign(handle, link, wire::FrameKind::PrefixReq, body, request).await?,
                )
                .await?;
                verify(handle, link, &reply).await?;
                ensure!(
                    reply.body().kind() == wire::FrameKind::PrefixResp,
                    "PrefixResp reply"
                );
                take_prefix(handle, link, reply.body().value().clone(), plan).await?;
            }
        }
        wire::FrameKind::Deliver => {
            let request = reply.request();
            let ids = take_delivery(handle, c, reply.body().value().clone()).await?;
            let body = handle
                .call(move |owner| owner.complete_body(c, request, &ids))
                .await?;
            let response = rpc(
                connection,
                sign(
                    handle,
                    link,
                    wire::FrameKind::Complete,
                    body.clone(),
                    request,
                )
                .await?,
            )
            .await?;
            verify(handle, link, &response).await?;
            ensure!(
                response.body().kind() == wire::FrameKind::Complete
                    && response.body().value() == &body,
                "Complete confirmation"
            );
        }
        _ => anyhow::bail!("Head response kind"),
    }
    let (body, progress) = handle.call(move |owner| owner.ack_body(c, None)).await?;
    let reply = rpc(
        connection,
        sign(
            handle,
            link,
            wire::FrameKind::AckBatch,
            body,
            shared_admin::random()?,
        )
        .await?,
    )
    .await?;
    verify(handle, link, &reply).await?;
    ensure!(
        reply.body().kind() == wire::FrameKind::AckBatch,
        "AckBatch reply"
    );
    take_ack(handle, c, reply.body().value().clone()).await?;
    if let Some(progress) = progress {
        handle
            .call(move |owner| owner.ack_progress(c, progress))
            .await?;
    }
    if let Some(body) = handle.call(move |owner| owner.ack_query(c)).await? {
        let reply = rpc(
            connection,
            sign_prepared(
                handle,
                link,
                wire::FrameKind::AckQuery,
                body,
                shared_admin::random()?,
            )
            .await?,
        )
        .await?;
        verify(handle, link, &reply).await?;
        ensure!(
            reply.body().kind() == wire::FrameKind::AckBatch,
            "AckQuery response"
        );
        take_ack(handle, c, reply.body().value().clone()).await?;
    }
    Ok(())
}
type PendingDelivery2 = ([u8; 16], Vec<Id>, Vec<Id>, window::RefPermit2, Instant);
async fn serve_b(handle: &Handle2, connection: &Connection, link: &Link2) -> Result<()> {
    let _timing = timing::span(timing::Metric::ServerSession);
    let c = link.c;
    let mut pending: Option<PrefixPlan2> = None;
    let mut live_cursor = 1;
    let mut send_live = true;
    let mut pending_delivery: Option<PendingDelivery2> = None;
    loop {
        tokio::time::timeout(Duration::from_secs(15), async {
            let (mut send, frame) = read_round(connection).await?;
            verify(handle, link, &frame).await?;
            let request = frame.request();
            let body = frame.body().value().clone();
            ensure!(
                pending_delivery
                    .as_ref()
                    .is_none_or(|d| Instant::now() < d.4),
                "Complete deadline"
            );
            let mut ack_progress: Option<AckProgress2> = None;
            let mut references = None;
            let (kind, response) = match frame.body().kind() {
                wire::FrameKind::Offer => {
                    let f = body.fields(3)?;
                    let offer = f[1].bytes()?.to_vec();
                    let pair = f[2].bytes()?.to_vec();
                    let id = wire::ParsedControl2::parse(&offer)?.id();
                    let response = handle
                        .call(move |owner| {
                            let grant = owner.grant(c, offer, pair)?;
                            Ok(a([b(owner.snapshot(c)?.head.context), b(id), b(grant)]))
                        })
                        .await?;
                    (wire::FrameKind::Grant, response)
                }
                wire::FrameKind::EpochCipher => {
                    let f = body.fields(5)?;
                    ensure!(f[3].uint()? == 2, "UpgradeRequired");
                    let raw = f[4].bytes()?.to_vec();
                    let response = handle
                        .call(move |owner| {
                            let confirm = owner.receive_genesis(c, raw)?;
                            let s = owner.snapshot(c)?;
                            Ok(a([
                                b(s.head.context),
                                b(s.head.dialog.context("D")?),
                                b(s.head.epoch.context("E")?),
                                u(2),
                                b(confirm),
                            ]))
                        })
                        .await?;
                    (wire::FrameKind::EpochCipher, response)
                }
                wire::FrameKind::Ready => {
                    let raw = body.fields(4)?[3].bytes()?.to_vec();
                    handle.call(move |owner| owner.accept_ready(c, raw)).await?;
                    let response = handle
                        .call(move |owner| {
                            let ready = owner.ready(c)?;
                            let s = owner.snapshot(c)?;
                            Ok(a([
                                b(s.head.context),
                                b(s.head.dialog.context("D")?),
                                b(s.head.epoch.context("E")?),
                                b(ready),
                            ]))
                        })
                        .await?;
                    (wire::FrameKind::Ready, response)
                }
                wire::FrameKind::Head => {
                    handle
                        .call(move |owner| owner.accept_head(c, &body))
                        .await?;
                    grant_for_frontier(handle, c).await?;
                    let tried_live = send_live && pending_delivery.is_none();
                    let live = if tried_live {
                        let first = live_cursor;
                        handle
                            .call(move |owner| owner.live_window(c, first))
                            .await?
                    } else {
                        None
                    };
                    send_live = !send_live;
                    if let Some(mut live) = live {
                        live_cursor = live.next;
                        // B sends one bounded page as a response; remaining suffix is sought next turn.
                        let body = live.bodies.remove(0);
                        let items = wire::tagged_items(&body.fields(7)?[6], wire::MAX_EVENT)?;
                        if !live.bodies.is_empty() {
                            live_cursor =
                                wire::ParsedEvent2::parse(&items.last().context("live tail")?.raw)?
                                    .core()
                                    .n
                                    .checked_add(1)
                                    .context("live cursor")?;
                        }
                        let expected = items
                            .iter()
                            .map(|i| wire::ParsedEvent2::parse(&i.raw).map(|e| e.id()))
                            .collect::<wire::Result<Vec<_>>>()?;
                        hooks::transport_barrier(c, wire::FrameKind::Deliver, &live.ids).await?;
                        pending_delivery = Some((
                            request,
                            expected,
                            live.ids,
                            live._references,
                            Instant::now() + Duration::from_secs(15),
                        ));
                        (wire::FrameKind::Deliver, body)
                    } else {
                        if tried_live {
                            live_cursor = 1;
                        }
                        if let Some((body, plan)) = handle
                            .call(move |owner| owner.prefix_request(c, request))
                            .await?
                        {
                            pending = Some(plan);
                            (wire::FrameKind::PrefixReq, body)
                        } else {
                            (
                                wire::FrameKind::Head,
                                handle.call(move |owner| owner.head_body(c)).await?,
                            )
                        }
                    }
                }
                wire::FrameKind::PrefixReq => {
                    let prepared = handle
                        .call(move |owner| owner.prefix_response(c, &body))
                        .await?;
                    references = Some(prepared.references);
                    (wire::FrameKind::PrefixResp, prepared.body)
                }
                wire::FrameKind::PrefixResp => {
                    let plan = pending.take().context("unsolicited prefix response")?;
                    ensure!(plan.request == request, "prefix correlation");
                    let ids = take_prefix(handle, link, body, plan).await?;
                    (
                        wire::FrameKind::Complete,
                        handle
                            .call(move |owner| owner.complete_body(c, request, &ids))
                            .await?,
                    )
                }
                wire::FrameKind::Deliver => {
                    let ids = take_delivery(handle, c, body).await?;
                    (
                        wire::FrameKind::Complete,
                        handle
                            .call(move |owner| owner.complete_body(c, request, &ids))
                            .await?,
                    )
                }
                wire::FrameKind::Complete => {
                    let f = body.fields(6)?;
                    let fields = f.to_vec();
                    handle
                        .call(move |owner| owner.check_scope(c, &fields))
                        .await?;
                    let pending = pending_delivery.take().context("unsolicited Complete")?;
                    ensure!(
                        pending.0 == request
                            && f[4].fixed::<16>()? == request
                            && f[5] == a(pending.1.iter().map(b)),
                        "Complete entire requested batch"
                    );
                    (wire::FrameKind::Complete, body)
                }
                wire::FrameKind::AckBatch => {
                    take_ack(handle, c, body).await?;
                    let (response, progress) =
                        handle.call(move |owner| owner.ack_body(c, None)).await?;
                    ack_progress = progress;
                    (wire::FrameKind::AckBatch, response)
                }
                wire::FrameKind::AckQuery => {
                    let (response, _) = handle
                        .call(move |owner| owner.ack_body(c, Some(&body)))
                        .await?;
                    (wire::FrameKind::AckBatch, response)
                }
                _ => anyhow::bail!("UnsupportedOperation"),
            };
            let link_owned = link.clone();
            let dispatch = handle
                .call(move |owner| owner.frame(&link_owned, kind, response, request, references))
                .await?;
            let evidence = ack_evidence(&dispatch.frame)?;
            hooks::transport_barrier(c, kind, &evidence).await?;
            dispatch.check(connection)?;
            transport::write(&mut send, &dispatch.frame).await?;
            if let Some(progress) = ack_progress {
                handle
                    .call(move |owner| owner.ack_progress(c, progress))
                    .await?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("whole server round deadline")??;
    }
}
fn replace_public(path: &Path, raw: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("artifact parent")?)?;
    temp.write_all(raw)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn unpublish_previous_instance(owner: &mut Owner2, directory: &Path) -> Result<()> {
    let profile = owner.repository.profile()?;
    for c in 0..2 {
        let path = directory.join(format!(
            "{}.locator",
            hex(&profile.contexts[usize::from(c)])
        ));
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
            Ok(_) => {}
        }
        let raw = shared_admin::read_public(&path, 8192)?;
        let locator = wire::ParsedRuntimeLocator2::parse(&raw)?;
        let s = owner.repository.snapshot(c)?;
        let credentials = s
            .spec
            .credentials
            .iter()
            .map(|raw| wire::ParsedCredentialBundle::parse(raw)?.verify_root())
            .collect::<wire::Result<Vec<_>>>()?;
        locator.verify(
            s.head.context,
            &credentials[usize::from(s.head.role)],
            &credentials[usize::from(1 - s.head.role)],
        )?;
        // Only a verified publication by this exact saved Device/context is ours.
        // The former endpoint is never exported as the newly starting instance.
        std::fs::remove_file(path)?;
    }
    Ok(())
}
pub(super) fn start_transport(
    tasks: &mut tokio::task::JoinSet<()>,
    handle: &Handle2,
    endpoint: &Endpoint,
    lan: Option<wire::DirectLan2>,
    enabled: &[u8],
    target: Option<discovery::TargetChanges2>,
    stop: tokio::sync::watch::Receiver<bool>,
) {
    let resources = handle.resources.clone();
    let permits = Arc::new(Semaphore::new(2));
    let listener_endpoint = endpoint.clone();
    let listener_handle = handle.clone();
    let listener_permits = permits.clone();
    let listener_resources = resources.clone();
    let mut listener_stop = stop.clone();
    tasks.spawn(async move{
        let mut connections=tokio::task::JoinSet::new();
        loop{while connections.try_join_next().is_some(){} tokio::select!{
            _ = listener_stop.wait_for(|stop| *stop) => break,
            incoming=listener_endpoint.accept()=>{let Some(incoming)=incoming else{break};
                listener_handle.diagnostics.event(Event::ListenerIncoming);
                if let Some(lan)=lan
                    && !matches!(incoming.remote_addr(), IncomingAddr::Ip(socket) if socket.ip() == IpAddr::V4(lan.peer()))
                { listener_handle.diagnostics.event(Event::ListenerPeerRejected);incoming.ignore(); continue; }
                let Ok(permit)=listener_permits.clone().try_acquire_owned()else{listener_resources.refused();incoming.refuse();continue};
                let accounting=listener_resources.acquired();
                let endpoint=listener_endpoint.clone();let handle=listener_handle.clone();let deadline=Instant::now()+Duration::from_secs(30);
                connections.spawn(async move{let _permit=permit;let _accounting=accounting;
                    let result=async{
                        let connected = tokio::time::timeout(Duration::from_secs(15), async {
                            let pair = if lan.is_some() {
                                let source = incoming.remote_addr();
                                handle.call(move |owner| {
                                    if !owner.auto_discovery() { return Ok(None); }
                                    let IncomingAddr::Ip(socket) = source else {
                                        anyhow::bail!("LAN discovery requires an IP source");
                                    };
                                    owner.listener_pair(socket)
                                }).await?
                            } else { None };
                            Ok::<_, anyhow::Error>((incoming.await?, pair))
                        }).await.context("TLS admission deadline");
                        if !matches!(&connected,Ok(Ok(_))) { handle.diagnostics.event(Event::ListenerTlsError); }
                        let (connection, pair) = connected??;
                        let admitted=tokio::time::timeout_at(tokio::time::Instant::from_std(deadline),admit_b(&handle,&endpoint,&connection,deadline,pair)).await;
                        if !matches!(&admitted,Ok(Ok(_))) { handle.diagnostics.event(Event::InboundAdmissionError); }
                        let result=match admitted{Ok(Ok(link))=>serve_b(&handle,&connection,&link).await,Ok(Err(e))=>Err(e),Err(e)=>Err(e.into())};
                        connection.close(0u32.into(),b"bounded connection ended");result}.await;
                    if let Err(e)=result{eprintln!("od2_inbound_end {e:#}");}
                });
            },_ =connections.join_next(),if !connections.is_empty()=>{},
        }}
        connections.abort_all();
        while connections.join_next().await.is_some() {}
    });
    for &c in enabled {
        if let Some(target) = &target {
            let mut stop = stop.clone();
            let dial = auto_dial(
                c,
                endpoint.clone(),
                handle.clone(),
                permits.clone(),
                target.clone(),
            );
            tasks.spawn(async move {
                tokio::select! {
                    _ = stop.wait_for(|stop| *stop) => {}
                    _ = dial => {}
                }
            });
            continue;
        }
        let endpoint = endpoint.clone();
        let handle = handle.clone();
        let permits = permits.clone();
        tasks.spawn(async move {
            loop {
                if handle.stop.load(Ordering::Acquire) {
                    break;
                }
                let role = handle
                    .call(move |owner| {
                        if !owner.repository.installed(c)? {
                            return Ok(None);
                        }
                        Ok(Some(owner.snapshot(c)?.head.role))
                    })
                    .await;
                if !matches!(role, Ok(Some(0))) {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                if let Ok(permit) = permits.clone().try_acquire_owned() {
                    let _permit = permit;
                    outbound(c, &handle, &endpoint).await;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }
}

async fn outbound(c: u8, handle: &Handle2, endpoint: &Endpoint) {
    handle.diagnostics.attempt(c);
    let _accounting = handle.resources.acquired();
    let deadline = Instant::now() + Duration::from_secs(30);
    let outcome = tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        admit_a(handle, endpoint, c, deadline),
    )
    .await;
    if let Ok(Ok((connection, link))) = outcome {
        let _close = CloseOnFailure(connection.clone(), true);
        let outcome: Result<()> = async {
            handle.diagnostics.stage(c, DialStage::FreshProtocol);
            let fresh = fresh_a(handle, &connection, &link).await;
            handle.diagnostics.fresh(c, fresh.is_ok());
            fresh?;
            handle.diagnostics.stage(c, DialStage::Serving);
            let mut cursor = 1;
            loop {
                data_round_a(handle, &connection, &link, &mut cursor).await?;
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
        .await;
        if let Err(error) = outcome {
            eprintln!("od2_outbound_end context={c} {error:#}");
        }
        handle.diagnostics.stage(c, DialStage::Ended);
    } else {
        handle.diagnostics.admission_error(c, outcome.is_err());
        match outcome {
            Ok(Err(error)) => handle
                .diagnostics
                .error(Event::OutboundAdmissionError, &error),
            Err(_) => handle.diagnostics.event(Event::OutboundAdmissionTimeout),
            _ => {}
        }
    }
}

fn retry_delay(failures: u32, jitter: u16) -> Duration {
    let upper = 1000u64
        .checked_shl(failures.min(4))
        .unwrap_or(16_000)
        .min(15_000);
    let lower = upper / 2;
    Duration::from_millis(lower + u64::from(jitter) % (upper - lower + 1))
}

async fn auto_dial(
    slot: u8,
    endpoint: Endpoint,
    handle: Handle2,
    permits: Arc<Semaphore>,
    mut target: discovery::TargetChanges2,
) {
    handle.diagnostics.stage(slot, DialStage::RoleCheck);
    let role = handle
        .call(move |owner| Ok(owner.snapshot(slot)?.head.role))
        .await;
    if !matches!(role, Ok(0)) {
        handle.diagnostics.stage(slot, DialStage::NotDialer);
        return;
    }
    handle.diagnostics.stage(slot, DialStage::WaitingTarget);
    'target: loop {
        let Ok(observed) = target.wait_for(Option::is_some).await else {
            return;
        };
        drop(observed);
        let mut failures = 0u32;
        loop {
            tokio::select! {
                biased;
                next = target.changed() => {
                    if next.is_err() { return; }
                    continue 'target;
                }
                permit = permits.clone().acquire_owned() => {
                    let Ok(_permit) = permit else { return };
                    tokio::select! {
                        biased;
                        next = target.changed() => {
                            if next.is_err() { return; }
                            continue 'target;
                        }
                        _ = outbound(slot, &handle, &endpoint) => {}
                    }
                }
            }
            let jitter = shared_admin::random::<2>()
                .map(u16::from_ne_bytes)
                .unwrap_or(0);
            let delay = retry_delay(failures, jitter);
            failures = failures.saturating_add(1);
            tokio::select! {
                biased;
                next = target.changed() => {
                    if next.is_err() { return; }
                    continue 'target;
                }
                _ = tokio::time::sleep(delay) => {}
            }
        }
    }
}

pub async fn run(profile: config::Profile2) -> Result<()> {
    let _diagnostic_report = timing::Report;
    profile.validate()?;
    let lan = profile.route.lan();
    let auto = !profile.lan_discovery.is_manual();
    if !auto && let Some(lan) = lan {
        lan::verify_interface(lan)?;
    }
    let mut repository = db::RuntimeRepository::open(
        kilogram_state::StateDirectoryLock::acquire(&profile.state_dir)?,
        profile.owner()?,
        profile.db_config(),
    )?;
    let saved = repository.profile()?;
    ensure!(
        saved.enrollment.device == parse_id(&profile.device)?
            && saved.contexts
                == [
                    parse_id(&profile.contexts[0])?,
                    parse_id(&profile.contexts[1])?
                ],
        "profile Device/context pins"
    );
    let mut owner = Owner2::new(repository, profile.route.clone(), profile.enabled.clone())?;
    let diagnostic_sink = profile
        .ipc_file
        .parent()
        .and_then(Path::parent)
        .and_then(DiagnosticSink::open);
    owner.diagnostics = Arc::new(Counters::new(diagnostic_sink.is_some()));
    std::fs::create_dir_all(&profile.locator_directory)?;
    unpublish_previous_instance(&mut owner, &profile.locator_directory)?;
    let instance = shared_admin::random()?;
    let endpoint = if auto {
        owner.select_auto_discovery();
        None
    } else {
        let endpoint = transport::bind_fresh(&profile.route).await?;
        if lan.is_some() {
            owner.observe_lan_interface(Some(endpoint.clone()));
        }
        for c in &profile.enabled {
            // Local readable history remains available when current messaging permission is blocked.
            if let Err(error) = owner.require(*c, false) {
                eprintln!("od2_publication_pending context={c} {error:#}");
                continue;
            }
            owner.rotate_locator(*c, instance, &endpoint.addr())?;
            let raw = owner.export_locator(*c, instance, &endpoint.addr())?;
            let id = owner.snapshot(*c)?.head.context;
            match replace_public(
                &profile
                    .locator_directory
                    .join(format!("{}.locator", hex(&id))),
                &raw,
            ) {
                Ok(()) => owner.mark_exported(*c, &raw)?,
                Err(e) => eprintln!("od2_export_pending context={c} {e:#}"),
            }
        }
        Some(endpoint)
    };
    let account = owner.account()?;
    let (ipc, mut work) =
        RuntimeIpcServer::start_ordered2(profile.ipc_file.clone(), account, owner.device()).await?;
    let (handle, join) = owner_thread(owner)?;
    let mut tasks = tokio::task::JoinSet::new();
    let (stop_auto, auto_stop) = tokio::sync::watch::channel(false);
    #[cfg(windows)]
    let auto_task = auto.then(|| {
        tokio::spawn(super::lan_runtime::run(
            profile.clone(),
            instance,
            handle.clone(),
            auto_stop,
        ))
    });
    #[cfg(not(windows))]
    let _ = auto_stop;
    if let Some(endpoint) = &endpoint {
        let publication_endpoint = endpoint.clone();
        let publication_route = profile.route.clone();
        let publication_handle = handle.clone();
        let directory = profile.locator_directory.clone();
        let enabled = profile.enabled.clone();
        tasks.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                if let Some(lan) = lan {
                    let current = lan::verify_interface(lan).is_ok()
                        && publication_route
                            .advertisement(&publication_endpoint)
                            .is_ok();
                    let verified = current.then(|| publication_endpoint.clone());
                    if publication_handle
                        .call(move |owner| {
                            owner.observe_lan_interface(verified);
                            Ok(())
                        })
                        .await
                        .is_err()
                        || !current
                    {
                        continue;
                    }
                }
                for c in &enabled {
                    let c = *c;
                    let addr = publication_endpoint.addr();
                    let publication = publication_handle
                        .call(move |owner| {
                            if owner.locator_needs_rotation(c, instance, &addr)? {
                                owner.rotate_locator(c, instance, &addr)?;
                            }
                            let raw = owner.export_locator(c, instance, &addr)?;
                            let id = owner.snapshot(c)?.head.context;
                            if owner.exported[usize::from(c)]
                                == Some(wire::hash(wire::DESCRIPTOR, &raw))
                            {
                                return Ok(None);
                            }
                            Ok(Some((id, raw)))
                        })
                        .await;
                    if let Ok(Some((id, raw))) = publication
                        && replace_public(&directory.join(format!("{}.locator", hex(&id))), &raw)
                            .is_ok()
                    {
                        let _ = publication_handle
                            .call(move |owner| owner.mark_exported(c, &raw))
                            .await;
                    }
                }
            }
        });
        start_transport(
            &mut tasks,
            &handle,
            endpoint,
            lan,
            &profile.enabled,
            None,
            stop_auto.subscribe(),
        );
        eprintln!(
            "od2_runtime_ready pid={} endpoint={}",
            std::process::id(),
            endpoint.id()
        );
    } else {
        eprintln!(
            "od2_runtime_ready pid={} endpoint=waiting-network",
            std::process::id()
        );
    }
    loop {
        let work = tokio::select! {work=work.recv()=>work,_=tokio::time::sleep(Duration::from_millis(100))=>{if handle.stop.load(Ordering::Acquire){break;}continue;}};
        let Some(work) = work else { break };
        let (command, response) = work.into_parts();
        let shutdown = matches!(command, RuntimeIpcCommand::Shutdown);
        let result = match command {
            RuntimeIpcCommand::Shutdown => RuntimeIpcResponse::ShutdownAccepted,
            RuntimeIpcCommand::OrderedDialog { request } => {
                let manual = endpoint.as_ref().map(Endpoint::addr);
                let setting = profile.lan_discovery.clone();
                let outcome = handle
                    .call(move |owner| {
                        let addr = manual
                            .or_else(|| owner.endpoint.as_ref().map(|(_, addr)| addr.clone()));
                        ipc2(owner, &request, instance, addr.as_ref(), &setting)
                    })
                    .await;
                match outcome {
                    Ok(response) => RuntimeIpcResponse::OrderedDialog { response },
                    Err(e) => RuntimeIpcResponse::Error {
                        message: format!("{e:#}"),
                    },
                }
            }
            _ => RuntimeIpcResponse::Error {
                message: "UnsupportedProfile: ordinary IPC forbidden in runtime2".into(),
            },
        };
        let _ = response.send(result);
        if shutdown {
            break;
        }
    }
    if let Some(sink) = &diagnostic_sink {
        let _ = sink.record(
            DiagnosticKind::RuntimeStopping,
            Some(std::process::id()),
            Some(&hex(&instance)),
            &handle.diagnostics.snapshot(),
        );
    }
    stop_auto.send_replace(true);
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    if let Some(endpoint) = endpoint {
        endpoint.close().await;
    }
    #[cfg(windows)]
    let mut network_join_ok = true;
    #[cfg(windows)]
    let network_shutdown = match auto_task {
        Some(task) => {
            let result = task.await;
            network_join_ok = result.is_ok();
            result
                .context("LAN Auto supervisor failed")
                .and_then(|result| result)
        }
        None => Ok(()),
    };
    #[cfg(windows)]
    if let Some(sink) = &diagnostic_sink {
        let _ = sink.record(DiagnosticKind::RuntimeNetworkStopped, Some(std::process::id()), Some(&hex(&instance)),
            &serde_json::json!({"network_ok":network_shutdown.is_ok(),"network_join_ok":network_join_ok,"counters":handle.diagnostics.snapshot()}));
    }
    handle.stop.store(true, Ordering::Release);
    let ipc_shutdown = ipc.shutdown().await;
    let owner_join = join.join().map_err(|_| anyhow::anyhow!("owner panicked"));
    if let Some(sink) = &diagnostic_sink {
        #[cfg(windows)]
        let network_ok = network_shutdown.is_ok();
        #[cfg(not(windows))]
        let network_ok = true;
        #[cfg(not(windows))]
        let network_join_ok = true;
        let _ = sink.record(DiagnosticKind::RuntimeFinal, Some(std::process::id()), Some(&hex(&instance)),
            &serde_json::json!({"network_ok":network_ok,"network_join_ok":network_join_ok,"ipc_ok":ipc_shutdown.is_ok(),"owner_join_ok":owner_join.is_ok(),"counters":handle.diagnostics.snapshot()}));
    }
    owner_join?;
    ipc_shutdown?;
    #[cfg(windows)]
    network_shutdown?;
    Ok(())
}
fn ipc2(
    owner: &mut Owner2,
    raw: &[u8],
    instance: Id,
    addr: Option<&iroh::EndpointAddr>,
    setting: &LanDiscovery2,
) -> Result<Vec<u8>> {
    let value = wire::decode(raw, 32 * 1024)?;
    let f = value.list()?;
    ensure!(
        f.len() >= 2 && f[0].uint()? == 2,
        "UnsupportedProfile: nested IPC version"
    );
    let opcode = f[1].uint()?;
    let c = |value: &Value| -> Result<u8> {
        let c = value.uint()?;
        ensure!(c < 2, "context slot");
        Ok(c as u8)
    };
    let result = match opcode {
        1 => {
            let _timing = timing::span(timing::Metric::Status);
            value.fields(2)?;
            let auto = owner.auto_discovery();
            let mut lan_current =
                addr.is_some_and(|addr| owner.check_lan_advertisement(addr).is_ok());
            if !auto && !lan_current {
                owner.observe_lan_interface(None);
            }
            let mut contexts = Vec::new();
            for c in owner.enabled.clone() {
                if !owner.repository.installed(c)? {
                    contexts.push(serde_json::json!({"slot":c,"installed":false,"active":false,"publication_ready":false,"counts":owner.repository.counts(c)?}));
                    continue;
                }
                let s = owner.snapshot(c)?;
                let counts = owner.repository.counts(c)?;
                let lease = owner
                    .require(c, false)
                    .ok()
                    .and_then(|(verified, _)| owner.live_lease(c, &verified).ok());
                let live_admitted = lease.is_some();
                let lease_expires = lease.map(|(deadline, _)| deadline);
                let publication_ready =
                    addr.is_some_and(|addr| owner.export_locator(c, instance, addr).is_ok());
                let policy_error = now()
                    .and_then(|wall| owner.current(&s, wall))
                    .err()
                    .map(|e| format!("{e:#}"));
                let native = if s.head.phase == 3 {
                    let n = owner.native(c, &s)?;
                    let g = n.guard()?;
                    serde_json::json!({"own_next":g.own().next,"peer_next":g.peer().next,"own_g":g.own().g,"peer_g":g.peer().g,"own_index":g.own().index,"peer_index":g.peer().index,"own_first":g.own().first.map(|id|hex(&id)),"rotate_due":g.rotate_due()})
                } else {
                    serde_json::Value::Null
                };
                let details = serde_json::json!({"native":native,"native_hash":s.head.native_hash.map(|id|hex(&id)),"offer":s.head.offer.map(|id|hex(&id)),"grant":s.head.grant.map(|id|hex(&id)),"readies":s.head.ready.iter().map(|r|r.as_ref().map(|r|hex(&wire::hash(b"",r)))).collect::<Vec<_>>(),"current_consent":wire::ParsedConsent2::parse(&s.spec.consent).ok().map(|v|hex(&v.reference())),"selected_hash":hex(&wire::hash(b"",&s.selection.proof)),"policy_generation":s.profile.policy_generation,"wall_high_water":s.profile.wall_high_water});
                let mut row = serde_json::json!({"slot":c,"live_admitted":live_admitted,"lease_expires":lease_expires,"context":hex(&s.head.context),"role":s.head.role,"phase":s.head.phase,"active":s.head.active,"native_generation":s.head.native_generation,"d":s.head.dialog.map(|x|hex(&x)),"e":s.head.epoch.map(|x|hex(&x)),"sid":s.head.sid,"q":s.head.mode_ref.map(|x|hex(&x)),"blocked":s.head.blocked,"own_locator_revision":s.spec.own_locator_revision,"peer_locator_revision":s.spec.peer_locator_revision,"publication_ready":publication_ready,"file_exported":owner.exported[usize::from(c)].is_some(),"policy_error":policy_error,"counts":counts,"details":details});
                if let Some((_, Some(socket))) = lease {
                    row["selected_lan_peer"] = serde_json::json!(socket);
                }
                contexts.push(row);
            }
            owner.stats.aborted_packet_calls = owner.stats.packet_attempts.saturating_sub(
                owner.stats.committed_encrypt
                    + owner.stats.committed_decrypt
                    + owner.stats.ambiguous_packet_calls,
            );
            let mut discovery = None;
            if auto {
                lan_current &= owner
                    .network_freshness()
                    .is_some_and(|token| token.is_current());
                if !lan_current {
                    for row in &mut contexts {
                        if let Some(fields) = row.as_object_mut()
                            && fields.contains_key("live_admitted")
                        {
                            fields.insert("live_admitted".into(), false.into());
                            fields.insert("lease_expires".into(), serde_json::Value::Null);
                            fields.remove("selected_lan_peer");
                            fields.insert("publication_ready".into(), false.into());
                        }
                    }
                }
                let state = if !lan_current {
                    DiscoveryStatus2::WaitingNetwork
                } else if contexts
                    .iter()
                    .all(|row| row["active"] == true && row["live_admitted"] == true)
                {
                    DiscoveryStatus2::Connected
                } else if contexts
                    .iter()
                    .any(|row| !row["policy_error"].is_null() || row["publication_ready"] != true)
                {
                    DiscoveryStatus2::WaitingPolicy
                } else if owner
                    .snapshot(0)
                    .and_then(|s| owner.pair_for(&s, now()?).map(|pair| pair.is_some()))
                    .unwrap_or(false)
                {
                    DiscoveryStatus2::Retrying
                } else {
                    DiscoveryStatus2::Discovering
                };
                discovery = Some(DiscoveryView2 {
                    selection: setting.clone(),
                    state,
                });
            }
            let observed_endpoint = if owner.route.lan().is_some() {
                addr.filter(|_| lan_current)
            } else {
                owner.endpoint.as_ref().map(|(_, addr)| addr)
            };
            let mut report = serde_json::json!({"version":2,"runtime_pid":std::process::id(),"runtime_instance":hex(&instance),"contexts":contexts,"stats":owner.stats,"window":owner.window.snapshot(),"resources":owner.resources.snapshot(),"endpoint":observed_endpoint.map(|addr|serde_json::json!({"id":hex(addr.id.as_bytes()),"ips":addr.ip_addrs().collect::<Vec<_>>(),"relays":addr.relay_urls().map(|u|u.as_str()).collect::<Vec<_>>()})),"route":config::profile_route(&owner.route),"root_generation":owner.repository.generation(),"backend":owner.repository.backend_stats()?});
            if owner.diagnostics.enabled() {
                report["stats"]["lab_diagnostics_v1"] = owner.diagnostics.snapshot();
            }
            if let Some(discovery) = discovery {
                report["lan_discovery"] = serde_json::to_value(discovery)?;
            }
            b(serde_json::to_vec(&report)?)
        }
        2 => {
            let f = value.fields(5)?;
            let outcome = owner.queue(c(&f[2])?, f[3].fixed()?, f[4].text()?.to_owned())?;
            a([u(outcome.queue_sequence), opt(outcome.event.map(b))])
        }
        3 => {
            let f = value.fields(4)?;
            let c = c(&f[2])?;
            let request = f[3].fixed()?;
            let outcome = owner.repository.request(c, request)?;
            let delivery = owner.repository.request_delivery(c, request)?;
            opt(match outcome {
                Some((o, _)) => Some(a([
                    u(o.queue_sequence),
                    opt(o.event.map(b)),
                    u(u64::from(delivery.context("request delivery missing")?)),
                ])),
                None => None,
            })
        }
        4 => {
            let f = value.fields(5)?;
            let cursor = f[3]
                .option()?
                .map(|v| -> Result<db::HistoryCursor> {
                    Ok(db::HistoryCursor::from_bytes(v.bytes()?)?)
                })
                .transpose()?;
            b(owner
                .repository
                .history(c(&f[2])?, cursor.as_ref(), usize::try_from(f[4].uint()?)?)?
                .encode_json()?)
        }
        5 => {
            let f = value.fields(4)?;
            owner.import_locator(c(&f[2])?, f[3].bytes()?.to_vec())?;
            a([])
        }
        6 => {
            let f = value.fields(3)?;
            b(owner.export_locator(c(&f[2])?, instance, addr.context("LocatorUnavailable")?)?)
        }
        7 => {
            let f = value.fields(3)?;
            owner.snapshot(c(&f[2])?)?;
            a([])
        }
        8 => {
            let f = value.fields(3)?;
            owner.import_authority(f[2].bytes()?.to_vec())?;
            a([])
        }
        9 => {
            let f = value.fields(6)?;
            ensure!(f[5].uint()? <= 1, "consent import kind");
            owner.import_consent(
                c(&f[2])?,
                f[4].bytes()?.to_vec(),
                f[3].fixed()?,
                f[5].uint()? == 1,
            )?;
            a([])
        }
        10 => {
            let f = value.fields(3)?;
            let before = owner.repository.generation();
            admin::install(&mut owner.repository, f[2].bytes()?)?;
            if owner.repository.generation() != before {
                owner.invalidate();
            }
            a([])
        }
        12 => {
            let f = value.fields(4)?;
            b(owner.public_import_begin(usize::try_from(f[2].uint()?)?, f[3].fixed()?)?)
        }
        13 => {
            let f = value.fields(5)?;
            owner.public_import_part(
                f[2].fixed()?,
                usize::try_from(f[3].uint()?)?,
                f[4].bytes()?,
            )?;
            a([])
        }
        14 => {
            let f = value.fields(3)?;
            owner.public_import_finish(f[2].fixed()?)?;
            a([])
        }
        15 => {
            let f = value.fields(5)?;
            let c = c(&f[2])?;
            let raw = owner.export_locator(c, instance, addr.context("LocatorUnavailable")?)?;
            ensure!(
                wire::hash(b"", &raw) == f[4].fixed::<32>()?,
                "locator changed before export"
            );
            let path = Path::new(f[3].text()?);
            shared_admin::write_public(
                path,
                &raw,
                &owner.repository.state_directory().canonicalize()?,
            )?;
            a([])
        }
        HISTORY_ANCHOR_OPCODE | HISTORY_FOLLOW_OPCODE => {
            let (slot, completed) = if opcode == HISTORY_ANCHOR_OPCODE {
                (c(&value.fields(3)?[2])?, None)
            } else {
                let fields = value.fields(4)?;
                (
                    c(&fields[2])?,
                    Some(db::HistoryCursor::from_bytes(fields[3].bytes()?)?),
                )
            };
            let position = owner
                .repository
                .history_position(slot, completed.as_ref())?;
            a([
                b(position.context),
                b(position.view),
                a(position.frontiers.map(u)),
                a(position.next.map(u)),
                b(position.cursor.as_bytes()),
            ])
        }
        _ => anyhow::bail!("UnsupportedOperation"),
    };
    let cap = if matches!(opcode, HISTORY_ANCHOR_OPCODE | HISTORY_FOLLOW_OPCODE) {
        HISTORY_POSITION_REPLY_CAP
    } else {
        48 * 1024
    };
    Ok(wire::encode(&a([u(2), u(opcode), result]), cap)?)
}
#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
