//! Real ALPN2 existing transport. Only original A dials; B notification uses
//! its own bidi RPC on that same A-originated connection.
use super::*;
use iroh::{Endpoint, endpoint::Connection};
use kilogram_protocol::ordered_dialog_v2 as v2;
use kilogram_transport_iroh::ordered2 as transport;
use protocol2::DispatchExisting7;
use runtime::Handle7;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone, Copy)]
struct AdmissionProgress {
    side: &'static str,
    stage: &'static str,
    operation: &'static str,
    started: Instant,
}
impl AdmissionProgress {
    fn new(side: &'static str) -> Self {
        Self {
            side,
            stage: "start",
            operation: "start",
            started: Instant::now(),
        }
    }
    fn step(&mut self, stage: &'static str, operation: &'static str) {
        self.stage = stage;
        self.operation = operation;
        self.observe(operation);
    }
    fn operation(&mut self, operation: &'static str) {
        self.step(self.stage, operation);
    }
    fn context(&self) -> String {
        format!(
            "existing admission side={} stage={} operation={} elapsed_ms={}",
            self.side,
            self.stage,
            self.operation,
            self.started.elapsed().as_millis()
        )
    }
    fn observe(&self, event: &str) {
        // Only the two managed test paths emit phase observations. No frame,
        // identity, binding, credential or private payload is printed here.
        #[cfg(test)]
        if std::env::var("KILOGRAM_S53_CASE").is_ok_and(|case| {
            matches!(
                case.as_str(),
                "C-s53-regressions-r2-01" | "C-s53-admission-r6-01"
            )
        }) {
            eprintln!("s53_admission_phase {} event={event}", self.context());
        }
        #[cfg(not(test))]
        let _ = event;
    }
}
async fn admission_call<T, F>(
    handle: &Handle7,
    progress: &mut AdmissionProgress,
    stage: &'static str,
    f: F,
) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(&mut owner::Owner7) -> Result<T> + Send + 'static,
{
    progress.step(stage, "owner-queued");
    let observation = *progress;
    let result = handle
        .call(move |owner| {
            observation.observe("owner-enter");
            let result = f(owner);
            observation.observe(if result.is_ok() {
                "owner-ok"
            } else {
                "owner-error"
            });
            result
        })
        .await;
    progress.operation("owner-replied");
    result.with_context(|| progress.context())
}
async fn raw_rpc(
    connection: &Connection,
    frame: &v2::ParsedFrame2,
    mut progress: Option<&mut AdmissionProgress>,
) -> Result<v2::ParsedFrame2> {
    let outcome = tokio::time::timeout(Duration::from_secs(15), async {
        if let Some(p) = progress.as_deref_mut() {
            p.operation("rpc-open-bidi");
        }
        let (mut send, mut receive) = connection.open_bi().await?;
        if let Some(p) = progress.as_deref_mut() {
            p.operation("rpc-write-finish");
        }
        transport::write(&mut send, frame).await?;
        if let Some(p) = progress.as_deref_mut() {
            p.operation("rpc-read-through-fin");
        }
        let reply = transport::read(&mut receive).await?;
        ensure!(
            reply.binding() == frame.binding() && reply.request() == frame.request(),
            "wire2 RPC binding/request"
        );
        Ok(reply)
    })
    .await;
    let context = || {
        progress
            .as_ref()
            .map_or_else(|| "existing RPC".to_owned(), |p| p.context())
    };
    outcome
        .with_context(|| format!("existing RPC timeout: {}", context()))?
        .with_context(context)
}
async fn before_publication(frame: &v2::ParsedFrame2) -> Result<()> {
    let name = match frame.body().kind() {
        v2::FrameKind::Propose => Some("P"),
        v2::FrameKind::Accept => Some("A"),
        v2::FrameKind::Select => Some("K"),
        v2::FrameKind::Committed => Some("Committed"),
        _ => None,
    };
    if let Some(name) = name {
        let raw = frame
            .body()
            .value()
            .fields(if name == "P" { 2 } else { 1 })?[0]
            .bytes()?;
        let facts = serde_json::json!({"control":name,"control_ref":state::hex(&v2::hash(v2::CONTROL_ID,raw)),"signed_control_blake3":blake3::hash(raw).to_hex().to_string()});
        super::super::test_support::checkpoint_facts(
            &format!("s53-{name}-before-publication"),
            Some(&facts),
        )?;
    }
    Ok(())
}
pub(super) async fn rpc(
    handle: &Handle7,
    connection: &Connection,
    c: u8,
    dispatch: &DispatchExisting7,
) -> Result<v2::ParsedFrame2> {
    before_publication(&dispatch.frame).await?;
    if handle.verification_enabled {
        let frame = dispatch.frame.clone();
        if let Some(pause) = handle
            .call(move |o| o.verification_ack(c, &frame, false))
            .await?
        {
            pause.wait(connection).await?;
        }
    }
    if super::super::test_support::load()?.trace_rpc {
        eprintln!("s53_rpc slot={c} kind={:?}", dispatch.frame.body().kind());
    }
    let binding = dispatch.frame.binding();
    handle
        .call(move |o| o.check_existing_lease(c, binding))
        .await?;
    dispatch.gate()?;
    let reply = raw_rpc(connection, &dispatch.frame, None).await?;
    let verify = reply.clone();
    handle
        .call(move |o| o.verified_frame2(c, binding, verify).map(|_| ()))
        .await?;
    Ok(reply)
}
pub(super) async fn close_binding(handle: &Handle7, connection: &Connection, binding: Id) {
    connection.close(0_u32.into(), b"existing connection complete");
    let _ = handle
        .call(move |o| {
            for c in 0..o.specs.len() {
                if o.leases[c]
                    .as_ref()
                    .is_some_and(|lease| lease.binding == binding)
                    || o.admissions[c]
                        .as_ref()
                        .is_some_and(|admission| admission.binding == binding)
                {
                    o.invalidate(c as u8);
                }
            }
            Ok(())
        })
        .await;
}
pub(super) async fn admit_a(
    handle: &Handle7,
    endpoint: &Endpoint,
    c: u8,
    deadline: Instant,
) -> Result<(Connection, owner::Lease1)> {
    let admission_deadline = (Instant::now() + Duration::from_secs(30)).min(deadline);
    let mut progress = AdmissionProgress::new("A");
    let remote = admission_call(handle, &mut progress, "route", move |o| {
        ensure!(o.role(c)? == 0, "only original A dials");
        let peer = o.locator2(c, false)?;
        o.route.remote(&peer)
    })
    .await?;
    let tls_deadline = (Instant::now() + Duration::from_secs(15)).min(admission_deadline);
    progress.step("tls", "connect");
    let connection =
        tokio::time::timeout_at(tls_deadline.into(), endpoint.connect(remote, v2::ALPN))
            .await
            .context("existing TLS deadline")??;
    let binding = transport::binding(&connection, *endpoint.id().as_bytes(), true)?;
    let result = async {
        let live = connection.clone();
        let (generation, hello) = admission_call(handle, &mut progress, "hello-begin", move |o| {
            o.begin2(c, live, binding, admission_deadline, deadline, None)
        })
        .await?;
        progress.step("hello", "rpc-start");
        let peer = raw_rpc(&connection, &hello, Some(&mut progress)).await?;
        admission_call(handle, &mut progress, "hello-accept", move |o| {
            o.observe_hello2(c, binding, &peer)?;
            o.accept_hello2(c, generation, peer)
        })
        .await?;
        let proof = admission_call(handle, &mut progress, "proof-build", move |o| {
            o.proof2(c, generation)
        })
        .await?;
        progress.step("proof", "rpc-start");
        let peer = raw_rpc(&connection, &proof, Some(&mut progress)).await?;
        admission_call(handle, &mut progress, "proof-accept", move |o| {
            o.accept_proof2(c, generation, peer)
        })
        .await?;
        let admitted = admission_call(handle, &mut progress, "admitted-build", move |o| {
            o.admitted2(c, generation, None)
        })
        .await?;
        progress.step("admitted", "rpc-start");
        let peer = raw_rpc(&connection, &admitted, Some(&mut progress)).await?;
        admission_call(handle, &mut progress, "admitted-verify", move |o| {
            o.verify_admitted2(c, generation, &peer)
        })
        .await?;
        let head = admission_call(handle, &mut progress, "head-build", move |o| {
            let body = o.head_body2(c)?;
            o.signed2(v2::FrameKind::Head, binding, None, body)
        })
        .await?;
        progress.step("head", "rpc-start");
        let peer = raw_rpc(&connection, &head, Some(&mut progress)).await?;
        admission_call(handle, &mut progress, "head-finish", move |o| {
            o.finish2(c, generation, peer)
        })
        .await
    };
    match tokio::time::timeout_at(admission_deadline.into(), result).await {
        Ok(Ok(lease)) => {
            progress.step("complete", "admitted");
            Ok((connection, lease))
        }
        outcome => {
            progress.observe("failed-before-close");
            close_binding(handle, &connection, binding).await;
            match outcome {
                Ok(Err(error)) => Err(error).with_context(|| progress.context()),
                Err(error) => Err(error).with_context(|| {
                    format!("existing admission deadline: {}", progress.context())
                }),
                _ => unreachable!(),
            }
        }
    }
}
pub(super) async fn admit_b(
    handle: &Handle7,
    endpoint: &Endpoint,
    connection: &Connection,
    slots: &BTreeMap<Id, Arc<Mutex<()>>>,
    deadline: Instant,
    opportunity_deadline: Instant,
) -> Result<(u8, owner::Lease1, tokio::sync::OwnedMutexGuard<()>)> {
    let binding = transport::binding(connection, *endpoint.id().as_bytes(), false)?;
    let mut progress = AdmissionProgress::new("B");
    let result = async {
        progress.step("hello", "accept-bidi");
        let (mut send, mut receive) = connection.accept_bi().await?;
        progress.operation("read-through-fin");
        let hello = transport::read(&mut receive).await?;
        ensure!(hello.body().kind() == v2::FrameKind::Hello, "Hello2 first");
        let context = hello.body().value().fields(12)?[0].fixed::<32>()?;
        let guard = slots
            .get(&context)
            .context("C not enabled for B")?
            .clone()
            .try_lock_owned()
            .context("C authenticating/admitted")?;
        let live = connection.clone();
        let (c, generation, reply) =
            admission_call(handle, &mut progress, "hello-accept-begin", move |o| {
                let c = o.slot(context)?;
                ensure!(o.role(c)? == 1, "original B listener");
                o.observe_hello2(c, binding, &hello)?;
                let (generation, reply) = o.begin2(
                    c,
                    live,
                    binding,
                    deadline,
                    opportunity_deadline,
                    Some(hello.request()),
                )?;
                o.accept_hello2(c, generation, hello)?;
                Ok((c, generation, reply))
            })
            .await?;
        progress.step("hello", "write-finish");
        transport::write(&mut send, &reply).await?;
        progress.step("proof", "accept-bidi");
        let (mut send, mut receive) = connection.accept_bi().await?;
        progress.operation("read-through-fin");
        let proof = transport::read(&mut receive).await?;
        let reply = admission_call(handle, &mut progress, "proof-accept-build", move |o| {
            o.accept_proof2(c, generation, proof)?;
            o.proof2(c, generation)
        })
        .await?;
        progress.step("proof", "write-finish");
        transport::write(&mut send, &reply).await?;
        progress.step("admitted", "accept-bidi");
        let (mut send, mut receive) = connection.accept_bi().await?;
        progress.operation("read-through-fin");
        let admitted = transport::read(&mut receive).await?;
        let reply = admission_call(handle, &mut progress, "admitted-verify-build", move |o| {
            o.verify_admitted2(c, generation, &admitted)?;
            o.admitted2(c, generation, Some(admitted.request()))
        })
        .await?;
        progress.step("admitted", "write-finish");
        transport::write(&mut send, &reply).await?;
        progress.step("head", "accept-bidi");
        let (mut send, mut receive) = connection.accept_bi().await?;
        progress.operation("read-through-fin");
        let head = transport::read(&mut receive).await?;
        let request = head.request();
        let (lease, reply) = admission_call(handle, &mut progress, "head-finish-build", move |o| {
            let lease = o.finish2(c, generation, head)?;
            let body = o.head_body2(c)?;
            let reply =
                o.dispatch_frame2(c, binding, v2::FrameKind::Head, Some(request), body, None)?;
            Ok((lease, reply))
        })
        .await?;
        reply.gate()?;
        progress.step("head", "write-finish");
        transport::write(&mut send, &reply.frame).await?;
        Ok((c, lease, guard))
    };
    match tokio::time::timeout_at(deadline.into(), result).await {
        Ok(Ok(result)) => {
            progress.step("complete", "admitted");
            Ok(result)
        }
        outcome => {
            progress.observe("failed-before-close");
            close_binding(handle, connection, binding).await;
            match outcome {
                Ok(Err(error)) => Err(error).with_context(|| progress.context()),
                Err(error) => Err(error).with_context(|| {
                    format!(
                        "existing listener admission deadline: {}",
                        progress.context()
                    )
                }),
                _ => unreachable!(),
            }
        }
    }
}
pub(super) async fn incoming(
    handle: Handle7,
    connection: Connection,
    c: u8,
    binding: Id,
) -> Result<()> {
    loop {
        let (mut send, mut receive) = connection.accept_bi().await?;
        tokio::time::timeout(Duration::from_secs(15), async {
            let frame = transport::read(&mut receive).await?;
            let request = frame.request();
            let result = handle.call(move |o| o.dispatch2(c, binding, frame)).await;
            match result {
                Ok(reply) => {
                    before_publication(&reply.frame).await?;
                    if handle.verification_enabled {
                        let frame = reply.frame.clone();
                        if let Some(pause) = handle
                            .call(move |o| o.verification_ack(c, &frame, true))
                            .await?
                        {
                            // The existing incoming15s bound still encloses this ACK-query reply.
                            pause.wait(&connection).await?;
                        }
                    }
                    handle
                        .call(move |o| o.check_existing_lease(c, binding))
                        .await?;
                    reply.gate()?;
                    transport::write(&mut send, &reply.frame).await?;
                }
                Err(error) => {
                    eprintln!("s53_existing_refused slot={c} {error:#}");
                    let reply = handle
                        .call(move |o| {
                            let h = o.head(c)?;
                            o.dispatch_frame2(
                                c,
                                binding,
                                v2::FrameKind::Error,
                                Some(request),
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
                                None,
                            )
                        })
                        .await?;
                    reply.gate()?;
                    transport::write(&mut send, &reply.frame).await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("existing incoming RPC deadline")??;
    }
}
pub(super) async fn worker(
    handle: &Handle7,
    connection: &Connection,
    c: u8,
    binding: Id,
) -> Result<()> {
    let role = handle.call(move |o| o.role(c)).await?;
    let mut direct_next = None;
    let mut live_retry = verification::retry::Once::default();
    let mut notified = std::collections::BTreeSet::new();
    loop {
        let state = handle
            .call(move |o| {
                o.check_existing_lease(c, binding)?;
                o.repo()?.existing_state(c).map_err(Into::into)
            })
            .await?;
        if role == 1 && state.mode == store::ExistingMode::L1 {
            let staged = handle
                .call(move |o| o.existing(c, |controls| controls.stage_notification(c)))
                .await?;
            if let Some(staged) = staged {
                let request = staged.request();
                if !notified.contains(&request) {
                    let dispatch = handle
                        .call(move |o| {
                            let request = staged.request();
                            let body = o.control_gate2(c, binding, |controls, gate| {
                                controls.handoff_notification(staged, gate)
                            })?;
                            body.map(|body| {
                                o.dispatch_frame2(
                                    c,
                                    binding,
                                    v2::FrameKind::OpenIntent,
                                    Some(request),
                                    body.value().clone(),
                                    None,
                                )
                            })
                            .transpose()
                        })
                        .await?;
                    if let Some(dispatch) = dispatch {
                        ensure!(notified.len() < 128, "bounded notification opportunities");
                        notified.insert(request);
                        let reply = rpc(handle, connection, c, &dispatch).await?;
                        ensure!(
                            reply.body().kind() == v2::FrameKind::Head,
                            "notification Head2 response"
                        );
                        handle
                            .call(move |o| o.accept_head2(c, binding, reply.body().value(), false))
                            .await?;
                    }
                }
            }
        }
        if role == 0 {
            match state.mode {
                store::ExistingMode::L1 | store::ExistingMode::Offering
                    if state.triggered() || state.mode == store::ExistingMode::Offering =>
                {
                    let propose = handle.call(move |o| o.propose_frame2(c, binding)).await?;
                    let reply = rpc(handle, connection, c, &propose).await?;
                    ensure!(
                        reply.body().kind() == v2::FrameKind::Accept,
                        "Accept2 response"
                    );
                    let raw = reply.body().value().fields(1)?[0].bytes()?.to_vec();
                    let select = handle
                        .call(move |o| o.select_frame2(c, binding, &raw))
                        .await?;
                    let reply = rpc(handle, connection, c, &select).await?;
                    ensure!(
                        reply.body().kind() == v2::FrameKind::Committed,
                        "Committed2 response"
                    );
                    let raw = reply.body().value().fields(1)?[0].bytes()?.to_vec();
                    handle
                        .call(move |o| {
                            o.control_gate2(c, binding, |controls, gate| {
                                controls.activate(c, &raw, gate)
                            })
                        })
                        .await?;
                }
                store::ExistingMode::Selected => {
                    let select = handle
                        .call(move |o| o.saved_select_frame2(c, binding))
                        .await?;
                    let reply = rpc(handle, connection, c, &select).await?;
                    ensure!(
                        reply.body().kind() == v2::FrameKind::Committed,
                        "Committed2 retry response"
                    );
                    let raw = reply.body().value().fields(1)?[0].bytes()?.to_vec();
                    handle
                        .call(move |o| {
                            o.control_gate2(c, binding, |controls, gate| {
                                controls.activate(c, &raw, gate)
                            })
                        })
                        .await?;
                }
                _ => (),
            }
        }
        // No RPC is outstanding here. The owner callback has replied and no
        // signed dispatch is retained across this long pressure-only pause.
        if handle.verification_enabled
            && let Some(mut selection) = handle
                .call(move |o| o.verification_selection(c, binding))
                .await?
        {
            if let Some(pause) = selection.pause.take() {
                pause.wait(connection).await?;
            }
            drop(selection);
            handle
                .call(move |o| o.check_existing_lease(c, binding))
                .await?;
        }
        let head = handle
            .call(move |o| {
                let body = o.head_body2(c)?;
                o.dispatch_frame2(c, binding, v2::FrameKind::Head, None, body, None)
            })
            .await?;
        let reply = rpc(handle, connection, c, &head).await?;
        ensure!(reply.body().kind() == v2::FrameKind::Head, "Head2 response");
        let body = reply.body().value().clone();
        let received = body.fields(9)?[5].fields(2)?[0].uint()?;
        let credit = body.fields(9)?[8].clone();
        handle
            .call(move |o| {
                o.accept_head2(c, binding, &body, false)?;
                // Observation only, after a genuine worker Head RPC and all
                // production validation. Never persistence or protocol authority.
                #[cfg(test)]
                {
                    o.check_existing_lease(c, binding)?;
                    let progress = &mut o.stats.worker_head_progress[usize::from(c)];
                    let before = progress
                        .filter(|(seen, _)| *seen == binding)
                        .map_or(0, |(_, n)| n);
                    *progress = Some((
                        binding,
                        before
                            .checked_add(1)
                            .context("worker Head observation overflow")?,
                    ));
                }
                Ok(())
            })
            .await?;
        if let Some((prefix, plan)) = handle.call(move |o| o.prefix2(c, binding)).await? {
            let reply = rpc(handle, connection, c, &prefix).await?;
            let pause = handle
                .call(move |o| o.accept_prefix2(c, binding, reply, &plan))
                .await?;
            if let Some(notice) = pause {
                notice.after_reply(connection).await?;
                handle
                    .call(move |o| o.check_existing_lease(c, binding))
                    .await?;
            }
        }
        let first = direct_next.unwrap_or(received).max(received);
        let retry_state = handle.verification_enabled.then(|| live_retry.clone());
        let (delivery, ticket) = handle
            .call(move |o| match retry_state {
                Some(state) => o.retry_outgoing2(c, binding, first, received, credit, &state),
                None => Ok((o.outgoing2(c, binding, first, credit)?, None)),
            })
            .await?;
        if let Some(delivery) = delivery {
            if let Some(ticket) = &ticket {
                live_retry.begin(ticket.clone())?;
            }
            let reply = rpc(handle, connection, c, &delivery.dispatch).await?;
            let request = delivery.dispatch.frame.request();
            let ids = delivery.ids;
            let next = delivery.next;
            let completion = ticket.clone();
            let completed_ids = ticket.as_ref().map(|_| ids.clone());
            handle
                .call(move |o| {
                    o.check_complete2(c, binding, reply, request, &ids)?;
                    if let Some(ticket) = completion {
                        o.retry_complete2(c, binding, &ticket, &ids, next)?;
                    }
                    Ok(())
                })
                .await?;
            if let Some(ticket) = ticket {
                live_retry.finish(
                    &ticket,
                    completed_ids
                        .as_deref()
                        .context("S54 captured delivery IDs")?,
                    next,
                )?;
            }
            direct_next = Some(next);
        }
        if let Some(ack) = handle.call(move |o| o.ack_push2(c, binding)).await? {
            let reply = rpc(handle, connection, c, &ack.dispatch).await?;
            let request = ack.dispatch.frame.request();
            let ids = ack.ids;
            let next = ack.next;
            handle
                .call(move |o| {
                    o.check_complete2(c, binding, reply, request, &ids)?;
                    o.commit_data2(c, Some(binding), store::ExistingData7::AckProgress { next })
                        .map(|_| ())
                })
                .await?;
        }
        if let Some(query) = handle.call(move |o| o.ack_query2(c, binding)).await? {
            let reply = rpc(handle, connection, c, &query).await?;
            handle
                .call(move |o| o.accept_acks2(c, binding, reply))
                .await?;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
pub(super) async fn established(
    handle: Handle7,
    connection: Connection,
    c: u8,
    lease: owner::Lease1,
) -> Result<()> {
    let binding = lease.binding;
    let mut receiving = tokio::spawn(incoming(handle.clone(), connection.clone(), c, binding));
    let (result, incoming_done) = tokio::select! {
        result=worker(&handle,&connection,c,binding)=>(result,false),
        result=&mut receiving=>(match result{Ok(result)=>result,Err(error)=>Err(error.into())},true),
    };
    connection.close(0_u32.into(), b"existing lease ended");
    if !incoming_done {
        receiving.abort();
        let _ = receiving.await;
    }
    close_binding(&handle, &connection, binding).await;
    result
}

pub(super) async fn coordinate(
    handle: Handle7,
    endpoint: Endpoint,
    c: u8,
    permits: Arc<Semaphore>,
    resources: Arc<resources::Resources7>,
) -> Result<()> {
    use control::{ExistingScheduler7, ScheduleDecision7 as D};
    let mut scheduler = ExistingScheduler7::default();
    let trace = super::super::test_support::load()?.trace_rpc;
    let started = Instant::now();
    let mut prior_l1 = false;
    loop {
        let (mode, trigger, eligible, l1) = handle
            .call(move |o| {
                let state = o.repo()?.existing_state(c)?;
                let eligible = o.current_existing(c).is_ok()
                    && o.locator2(c, true).is_ok()
                    && o.locator2(c, false).is_ok();
                let l1 = state.mode == store::ExistingMode::L1 && o.current(c).is_ok();
                Ok((state.mode, state.triggered(), eligible, l1))
            })
            .await?;
        let now = Instant::now();
        scheduler.eligibility(now, eligible)?;
        if l1 && !prior_l1 && eligible {
            scheduler.l1_returned(now)?;
        }
        prior_l1 = l1;
        let decision = if mode != store::ExistingMode::L1 {
            D::Existing
        } else if !eligible {
            D::Ordinary
        } else {
            scheduler.decision(now, mode, trigger, l1, false)
        };
        match decision {
            D::Wait => tokio::time::sleep(Duration::from_millis(100)).await,
            D::ResolveOrdinary => {
                scheduler.ordinary_finished(Instant::now())?;
            }
            D::Ordinary => {
                if trace {
                    eprintln!(
                        "s53_scheduler slot={c} phase=ordinary-start monotonic_ms={}",
                        started.elapsed().as_millis()
                    );
                }
                let bounds = scheduler.ordinary_bounds().unwrap_or((
                    Instant::now() + control::ORDINARY_FLOOR7,
                    Instant::now() + control::ORDINARY_WHOLE7,
                ));
                let turn = network::OrdinaryTurn7 {
                    floor: bounds.0,
                    whole: bounds.1,
                    no_trigger_due: scheduler.discovery_due().unwrap_or(bounds.0),
                    preempt_intent: eligible && !scheduler.ordinary_after_probe(),
                };
                let result = async {
                    let permit =
                        tokio::time::timeout_at(bounds.1.into(), permits.clone().acquire_owned())
                            .await
                            .context("ordinary permit deadline")??;
                    let accounting = resources.acquired();
                    let admission = (Instant::now() + Duration::from_secs(30)).min(bounds.1);
                    let (connection, link, pair) = tokio::time::timeout_at(
                        admission.into(),
                        network::admit_a(&handle, &endpoint, c),
                    )
                    .await
                    .context("ordinary admission deadline")??;
                    let result =
                        network::established_turn(handle.clone(), connection, link, pair, turn)
                            .await;
                    drop(accounting);
                    drop(permit);
                    result
                }
                .await;
                handle
                    .call(move |o| {
                        o.invalidate(c);
                        Ok(())
                    })
                    .await?;
                if eligible {
                    scheduler.ordinary_finished(Instant::now())?;
                }
                if trace {
                    eprintln!(
                        "s53_scheduler slot={c} phase=ordinary-finished monotonic_ms={}",
                        started.elapsed().as_millis()
                    );
                }
                if let Err(error) = result {
                    eprintln!("s53_ordinary_attempt slot={c} {error:#}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
            D::Probe => {
                let token = scheduler.start(Instant::now(), mode, trigger, l1, false)?;
                if trace {
                    eprintln!(
                        "s53_scheduler slot={c} phase=probe-start monotonic_ms={}",
                        started.elapsed().as_millis()
                    );
                }
                let result=async{
                    let permit=tokio::time::timeout_at(token.deadline.into(),permits.clone().acquire_owned()).await.context("probe permit deadline")??;
                    let accounting=resources.acquired();let(connection,lease)=admit_a(&handle,&endpoint,c,token.deadline).await?;
                    // A committed transition wins the old probe timer. The timer
                    // resolves the serialized durable outcome before any L1 turn.
                    let live=connection.clone();let bound=lease.binding;
                    let task=established(handle.clone(),connection,c,lease);
                    tokio::pin!(task);
                    let result=tokio::select!{
                        result=&mut task=>result,
                        _=tokio::time::sleep_until(token.deadline.into())=>{
                            let current=handle.call(move|o|o.repo()?.existing_state(c).map_err(Into::into)).await?;
                            scheduler.gate(token,Instant::now(),current.mode).ok();
                            if current.mode==store::ExistingMode::L1 {close_binding(&handle,&live,bound).await;task.await}
                            else {task.await}
                        }
                    };
                    drop(accounting);drop(permit);result
                }.await;
                let durable = handle
                    .call(move |o| {
                        o.invalidate(c);
                        Ok(o.repo()?.existing_state(c)?.mode)
                    })
                    .await?;
                scheduler.resolved(token, Instant::now(), durable)?;
                if trace {
                    eprintln!(
                        "s53_scheduler slot={c} phase=probe-resolved mode={durable:?} monotonic_ms={}",
                        started.elapsed().as_millis()
                    );
                }
                if let Err(error) = result {
                    eprintln!("s53_discovery_closed slot={c} {error:#}");
                }
            }
            D::Existing => {
                let result = async {
                    let permit = permits.clone().acquire_owned().await?;
                    let accounting = resources.acquired();
                    let (connection, lease) = admit_a(
                        &handle,
                        &endpoint,
                        c,
                        Instant::now() + control::PROBE_WHOLE7,
                    )
                    .await?;
                    let result = established(handle.clone(), connection, c, lease).await;
                    drop(accounting);
                    drop(permit);
                    result
                }
                .await;
                if let Err(error) = result {
                    eprintln!("s53_existing_wait slot={c} {error:#}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
}
