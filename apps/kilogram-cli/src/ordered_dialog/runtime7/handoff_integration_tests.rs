//! Managed, isolated integrated components. No private exhausted fixture access.
use super::super::{network2, protocol2, runtime};
use super::*;
use iroh::{Endpoint, endpoint::Connection};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::Ordering},
};
use tokio::sync::{Mutex, Semaphore, oneshot};
use zeroize::Zeroizing;
#[path = "history_busy_tests.rs"]
mod history_busy;
#[path = "handoff_regression_tests.rs"]
mod review2;
#[path = "handoff_seen_batch_tests.rs"]
mod seen_batch;

struct Lineage {
    head: Head,
    key: Zeroizing<Id>,
    keys: [Id; 3],
    account: Id,
    reservation: Option<(Id, Id, Id, u64)>,
}
impl Lineage {
    fn capture(owner: &mut Owner7) -> Result<Self> {
        let head = owner.head(0)?;
        let native = owner.native(0, &head)?;
        Ok(Self {
            head,
            key: Zeroizing::new(*native.key),
            keys: native.session.as_ref().context("old session")?.keys(),
            account: native.account.curve_key(),
            reservation: native.reservation,
        })
    }
    fn check(&self, owner: &mut Owner7) -> Result<()> {
        let head = owner.head(0)?;
        let native = owner.mixed_native(0, &head)?;
        ensure!(
            head.c == self.head.c
                && head.d == self.head.d
                && head.e == self.head.e
                && head.sid == self.head.sid
                && head.local_cert == self.head.local_cert,
            "same original Head identity"
        );
        ensure!(
            *native.key == *self.key
                && native.session.keys() == self.keys
                && native.account.curve_key() == self.account
                && native.reservation == self.reservation,
            "same original Native lineage"
        );
        Ok(())
    }
    fn check_legacy(&self, owner: &mut Owner7) -> Result<()> {
        let head = owner.head(0)?;
        let native = owner.native(0, &head)?;
        let session = native.session.as_ref().context("original legacy session")?;
        ensure!(
            head.c == self.head.c
                && head.d == self.head.d
                && head.e == self.head.e
                && head.sid == self.head.sid
                && head.local_cert == self.head.local_cert,
            "same original Head identity"
        );
        ensure!(
            *native.key == *self.key
                && session.keys() == self.keys
                && native.account.curve_key() == self.account
                && native.reservation == self.reservation,
            "same original Native lineage"
        );
        Ok(())
    }
}

struct Net {
    handles: [runtime::Handle7; 2],
    joins: [Option<std::thread::JoinHandle<()>>; 2],
    endpoints: [Endpoint; 2],
    connections: Option<[Connection; 2]>,
    // Captured from real admission, independent of a later consent invalidation.
    owned_bindings: Option<[Id; 2]>,
    tasks: tokio::task::JoinSet<Result<()>>,
    c: Id,
    slot: u8,
    ordinary_cleanup: bool,
    deadline: Instant,
}
impl Drop for Net {
    fn drop(&mut self) {
        self.tasks.abort_all();
        if let Some(connections) = &self.connections {
            for connection in connections {
                connection.close(0_u32.into(), b"managed component cleanup");
            }
        }
        for handle in &self.handles {
            handle.stop.store(true, Ordering::Release);
        }
        for join in &mut self.joins {
            if let Some(join) = join.take() {
                let _ = join.join();
            }
        }
    }
}
impl Net {
    async fn legacy_admission_only(&mut self) -> Result<()> {
        let slot = self.slot;
        self.disconnect().await?;
        let result = async {
            let a = self.handles[0]
                .call(move |o| o.publish_locator(slot))
                .await?;
            let b = self.handles[1]
                .call(move |o| o.publish_locator(slot))
                .await?;
            self.handles[0]
                .call(move |o| o.import_locator(slot, b))
                .await?;
            self.handles[1]
                .call(move |o| o.import_locator(slot, a))
                .await?;
            let endpoint = self.endpoints[1].clone();
            let handle = self.handles[1].clone();
            let context = self.c;
            let deadline = self.deadline;
            let (sender, receiver) = oneshot::channel();
            self.tasks.spawn(async move {
                tokio::time::timeout_at(deadline.into(), async {
                    let incoming = endpoint.accept().await.context("mixed L1 listener")?;
                    let connection = tokio::time::timeout(Duration::from_secs(15), incoming)
                        .await
                        .context("mixed B L1 TLS deadline")??;
                    let slots = BTreeMap::from([(context, Arc::new(Mutex::new(())))]);
                    let (link, guard) =
                        super::super::network::admit_b(&handle, &endpoint, &connection, &slots)
                            .await
                            .context("mixed B genuine L1 admission")?;
                    sender
                        .send((connection.clone(), link.binding))
                        .map_err(|_| anyhow::anyhow!("mixed B L1 admission receiver closed"))?;
                    // Retain the real admission lock while application delivery is held.
                    connection.closed().await;
                    drop(guard);
                    Ok(())
                })
                .await
                .context("mixed B held L1 absolute deadline")?
            });
            let admission = tokio::time::timeout_at(
                self.deadline.into(),
                super::super::network::admit_a(&self.handles[0], &self.endpoints[0], slot),
            );
            tokio::pin!(admission);
            let (connection, link, _pair) = tokio::select! {
                biased;
                peer = self.tasks.join_next() => return Err(Self::peer_completed(peer)),
                result = &mut admission => result.context("mixed A L1 absolute deadline")?
                    .context("mixed A genuine L1 admission")?,
            };
            let received = tokio::select! {
                biased;
                peer = self.tasks.join_next() => Err(Self::peer_completed(peer)),
                result = tokio::time::timeout_at(self.deadline.into(), receiver) =>
                    result.context("mixed B L1 receipt deadline")?
                        .context("mixed B L1 receipt cancelled"),
            };
            let (other, other_binding) = match received {
                Ok(value) => value,
                Err(error) => {
                    connection.close(0_u32.into(), b"mixed L1 admission failed");
                    return Err(error);
                }
            };
            self.connections = Some([connection, other]);
            self.owned_bindings = Some([link.binding, other_binding]);
            ensure!(
                link.binding == other_binding,
                "mixed real shared L1 binding"
            );
            Ok(())
        }
        .await;
        if result.is_err() {
            self.disconnect()
                .await
                .context("mixed failed L1 task drain")?;
            // Admission may have installed a real lease before its peer failed.
            for handle in &self.handles {
                if let Some(lease) = handle
                    .call(move |o| Ok(o.leases[usize::from(slot)].clone()))
                    .await?
                {
                    network2::close_binding(handle, &lease.connection, lease.binding).await;
                }
            }
        }
        result
    }
    fn peer_completed(
        result: Option<std::result::Result<Result<()>, tokio::task::JoinError>>,
    ) -> anyhow::Error {
        match result {
            Some(Ok(Err(error))) => error.context("managed B peer task failed during admission"),
            Some(Err(error)) => anyhow::Error::new(error).context("managed B peer task join"),
            Some(Ok(Ok(()))) => anyhow::anyhow!("managed B peer task ended during admission"),
            None => anyhow::anyhow!("managed B peer task missing during admission"),
        }
    }
    async fn legacy(&mut self) -> Result<()> {
        let slot = self.slot;
        self.disconnect().await?;
        let a = self.handles[0]
            .call(move |owner| owner.publish_locator(slot))
            .await?;
        let b = self.handles[1]
            .call(move |owner| owner.publish_locator(slot))
            .await?;
        self.handles[0]
            .call(move |owner| owner.import_locator(slot, b))
            .await?;
        self.handles[1]
            .call(move |owner| owner.import_locator(slot, a))
            .await?;
        let endpoint = self.endpoints[1].clone();
        let handle = self.handles[1].clone();
        let context = self.c;
        let (sender, receiver) = oneshot::channel();
        let deadline = self.deadline;
        self.tasks.spawn(async move {
            let incoming = endpoint.accept().await.context("real L1 listener")?;
            let connection = tokio::time::timeout(Duration::from_secs(15), incoming).await??;
            let slots = BTreeMap::from([(context, Arc::new(Mutex::new(())))]);
            let (link, _guard) = tokio::time::timeout_at(
                deadline.into(),
                super::super::network::admit_b(&handle, &endpoint, &connection, &slots),
            )
            .await??;
            let _ = sender.send((connection.clone(), link.binding));
            super::super::network::established(handle, connection, link, None).await
        });
        let (connection, link, pair) = tokio::time::timeout_at(
            self.deadline.into(),
            super::super::network::admit_a(&self.handles[0], &self.endpoints[0], slot),
        )
        .await??;
        let (other, other_binding) =
            tokio::time::timeout_at(self.deadline.into(), receiver).await??;
        let binding = link.binding;
        ensure!(binding == other_binding, "real shared L1 admission binding");
        self.tasks.spawn(super::super::network::established(
            self.handles[0].clone(),
            connection.clone(),
            link,
            Some(pair),
        ));
        self.connections = Some([connection, other]);
        self.owned_bindings = Some([binding, other_binding]);
        Ok(())
    }
    async fn new(owners: [Owner7; 2], c: Id, deadline: Instant) -> Result<Self> {
        Self::new_at_slot(owners, c, 0, deadline).await
    }
    async fn new_at_slot(
        mut owners: [Owner7; 2],
        c: Id,
        slot: u8,
        deadline: Instant,
    ) -> Result<Self> {
        for (role, owner) in owners.iter().enumerate() {
            ensure!(
                owner.specs.get(usize::from(slot)).is_some_and(|s| s.c == c)
                    && owner.role(slot)? == role as u64,
                "resolved component slot/role"
            );
        }
        let endpoints = [
            kilogram_transport_iroh::ordered2::bind_existing(&owners[0].route).await?,
            kilogram_transport_iroh::ordered2::bind_existing(&owners[1].route).await?,
        ];
        let mut locators = Vec::new();
        for (role, owner) in owners.iter_mut().enumerate() {
            owner.endpoint = Some(endpoints[role].addr());
            locators.push(owner.publish_locator2(slot)?);
        }
        owners[0].import_locator2(slot, locators[1].clone())?;
        owners[1].import_locator2(slot, locators[0].clone())?;
        let [a, b] = owners;
        let (a, ja) = runtime::actor(a)?;
        let (b, jb) = runtime::actor(b)?;
        let mut net = Self {
            handles: [a, b],
            joins: [Some(ja), Some(jb)],
            endpoints,
            connections: None,
            owned_bindings: None,
            tasks: tokio::task::JoinSet::new(),
            c,
            slot,
            ordinary_cleanup: false,
            deadline,
        };
        net.connect().await?;
        Ok(net)
    }
    async fn connect(&mut self) -> Result<()> {
        let slot = self.slot;
        let endpoint = self.endpoints[1].clone();
        let handle = self.handles[1].clone();
        let context = self.c;
        let deadline = self.deadline;
        let (sender, receiver) = oneshot::channel();
        self.tasks.spawn(async move {
            let result = async {
                eprintln!("s53_admission_peer side=B event=listener-start");
                let permits = Arc::new(Semaphore::new(2));
                let incoming = endpoint.accept().await.context("component listener")?;
                let _permit = permits.acquire_owned().await?;
                let started = Instant::now();
                eprintln!("s53_admission_peer side=B event=tls-start");
                let connection = tokio::time::timeout(Duration::from_secs(15), incoming)
                    .await
                    .context("managed B TLS deadline")?
                    .context("managed B TLS accept")?;
                let slots = BTreeMap::from([(context, Arc::new(Mutex::new(())))]);
                let (c, lease, _guard) = network2::admit_b(
                    &handle,
                    &endpoint,
                    &connection,
                    &slots,
                    (started + Duration::from_secs(30)).min(deadline),
                    (started + PROBE_WHOLE7).min(deadline),
                )
                .await?;
                sender
                    .send((connection.clone(), lease.binding))
                    .map_err(|_| anyhow::anyhow!("managed B admission receiver closed"))?;
                eprintln!("s53_admission_peer side=B event=admitted-incoming-start");
                network2::incoming(handle, connection, c, lease.binding).await
            }
            .await;
            if let Err(error) = &result {
                eprintln!("s53_admission_peer side=B event=error error={error:#}");
            }
            result
        });
        let admission = async {
            let result = network2::admit_a(
                &self.handles[0],
                &self.endpoints[0],
                slot,
                (Instant::now() + PROBE_WHOLE7).min(self.deadline),
            )
            .await;
            if std::env::var_os("KILOGRAM_S53_CASE").is_none()
                && let Err(error) = &result
            {
                eprintln!("s54_history_owner_admission side=A error={error:#}");
            }
            result
        };
        tokio::pin!(admission);
        // Observe the owned listener concurrently. Its real error/panic must not
        // disappear behind A's later timeout; Net still owns and aborts/joins tasks.
        let (connection, lease) = tokio::select! {
            biased;
            peer = self.tasks.join_next() => return Err(Self::peer_completed(peer)),
            result = &mut admission => result.context("managed A admission")?,
        };
        let receiver_deadline = (Instant::now() + Duration::from_secs(30)).min(self.deadline);
        let (other, other_binding) = tokio::select! {
            biased;
            peer = self.tasks.join_next() => return Err(Self::peer_completed(peer)),
            result = tokio::time::timeout_at(receiver_deadline.into(), receiver) => result.context("managed B admission receipt deadline")?.context("managed B admission receipt cancelled")?,
        };
        ensure!(
            lease.binding == other_binding,
            "real shared TLS2 admission binding"
        );
        let handle = self.handles[0].clone();
        let incoming = connection.clone();
        self.tasks
            .spawn(network2::incoming(handle, incoming, slot, lease.binding));
        self.connections = Some([connection, other]);
        self.owned_bindings = Some([lease.binding, other_binding]);
        Ok(())
    }
    async fn reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }
    async fn disconnect(&mut self) -> Result<()> {
        if self.ordinary_cleanup {
            return self.disconnect_ordinary().await;
        }
        if let Some(connections) = self.connections.take() {
            let bindings = self
                .owned_bindings
                .take()
                .context("owned actual admission bindings")?;
            for (role, connection) in connections.iter().enumerate() {
                network2::close_binding(&self.handles[role], connection, bindings[role]).await;
            }
        }
        self.tasks.abort_all();
        while self.tasks.join_next().await.is_some() {}
        Ok(())
    }
    fn ordinary_join(
        joined: std::result::Result<Result<()>, tokio::task::JoinError>,
        abort_requested: bool,
        owned_close: bool,
    ) -> Result<()> {
        match joined {
            Err(error) if abort_requested && error.is_cancelled() => Ok(()),
            Err(error) => Err(error).context("ordinary background task join"),
            Ok(Err(error)) => {
                // Only the typed outcome of our owned close is benign. No substring matching.
                let expected = owned_close
                    && error
                        .downcast_ref::<iroh::endpoint::ConnectionError>()
                        .is_some_and(|reason| match reason {
                            iroh::endpoint::ConnectionError::LocallyClosed => true,
                            iroh::endpoint::ConnectionError::ApplicationClosed(close) => {
                                close.error_code == 0_u32.into()
                                    && close.reason.as_ref() == b"existing connection complete"
                            }
                            _ => false,
                        });
                if expected {
                    Ok(())
                } else {
                    Err(error).context("ordinary background task failed")
                }
            }
            Ok(Ok(())) => Ok(()),
        }
    }
    async fn drain_ordinary(
        tasks: &mut tokio::task::JoinSet<Result<()>>,
        owned_close: bool,
    ) -> Result<()> {
        let mut result = Ok(());
        // Already completed cancellation was not caused by this cleanup's abort_all.
        while let Some(joined) = tasks.try_join_next() {
            result = result.and(Self::ordinary_join(joined, false, owned_close));
        }
        tasks.abort_all();
        while let Some(joined) = tasks.join_next().await {
            result = result.and(Self::ordinary_join(joined, true, owned_close));
        }
        result
    }
    async fn disconnect_ordinary(&mut self) -> Result<()> {
        let mut result = Ok(());
        let owned_close = self.connections.is_some();
        if let Some(connections) = self.connections.take() {
            if let Some(bindings) = self.owned_bindings.take() {
                for (role, connection) in connections.iter().enumerate() {
                    network2::close_binding(&self.handles[role], connection, bindings[role]).await;
                }
            } else {
                for connection in &connections {
                    connection.close(0_u32.into(), b"existing connection complete");
                }
                result = Err(anyhow::anyhow!("owned actual admission bindings missing"));
            }
        }
        // Never return before all tasks have been drained, even after an earlier failure.
        result.and(Self::drain_ordinary(&mut self.tasks, owned_close).await)
    }
    /// Ordinary A cases explicitly propagate cleanup/join errors; managed defaults keep Drop.
    async fn finish(mut self) -> Result<()> {
        let mut result = self.disconnect_ordinary().await;
        for handle in &self.handles {
            let window = handle
                .call(|o| Ok(o.window.snapshot().1))
                .await
                .and_then(|current| {
                    ensure!(current == [0, 0], "owned references returned to baseline");
                    Ok(())
                });
            result = result.and(window);
            handle.stop.store(true, Ordering::Release);
        }
        for join in &mut self.joins {
            if let Some(join) = join.take() {
                result = result.and(
                    join.join()
                        .map_err(|_| anyhow::anyhow!("owned history actor panic")),
                );
            }
        }
        for endpoint in &self.endpoints {
            endpoint.close().await;
        }
        result.and_then(|()| {
            ensure!(
                self.tasks.is_empty() && self.connections.is_none(),
                "owned history tasks drained"
            );
            Ok(())
        })
    }
    async fn binding(&self, role: usize) -> Result<Id> {
        let slot = self.slot;
        self.handles[role]
            .call(move |o| {
                Ok(o.leases[usize::from(slot)]
                    .as_ref()
                    .context("actual component lease")?
                    .binding)
            })
            .await
    }
    async fn send(
        &self,
        role: usize,
        dispatch: &protocol2::DispatchExisting7,
    ) -> Result<v2::ParsedFrame2> {
        let slot = self.slot;
        tokio::time::timeout_at(
            self.deadline.into(),
            network2::rpc(
                &self.handles[role],
                &self.connections.as_ref().context("component connected")?[role],
                slot,
                dispatch,
            ),
        )
        .await
        .context("absolute mixed component deadline")?
    }
    async fn wait(&self, role: usize, what: impl Fn(&Json) -> bool) -> Result<Json> {
        let slot = self.slot;
        let start = Instant::now();
        loop {
            let value = self.handles[role]
                .call(move |o| observed_at(o, slot))
                .await?;
            if what(&value) {
                return Ok(value);
            }
            ensure!(
                start.elapsed() < Duration::from_secs(25) && Instant::now() < self.deadline,
                "bounded component observation"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    async fn head(&self, role: usize) -> Result<()> {
        let slot = self.slot;
        let binding = self.binding(role).await?;
        let dispatch = self.handles[role]
            .call(move |o| {
                let body = o.head_body2(slot)?;
                o.dispatch_frame2(slot, binding, v2::FrameKind::Head, None, body, None)
            })
            .await?;
        let reply = self.send(role, &dispatch).await?;
        ensure!(
            reply.body().kind() == v2::FrameKind::Head,
            "real Head2 response"
        );
        self.handles[role]
            .call(move |o| o.accept_head2(slot, binding, reply.body().value(), false))
            .await
    }
    async fn deliver(&self, role: usize, first: u64) -> Result<()> {
        let slot = self.slot;
        let credit = self.handles[1 - role]
            .call(move |o| o.credit2(slot))
            .await?;
        let binding = self.binding(role).await?;
        let delivery = self.handles[role]
            .call(move |o| o.outgoing2(slot, binding, first, credit))
            .await?
            .context("genuine immutable delivery")?;
        let reply = self.send(role, &delivery.dispatch).await?;
        let request = delivery.dispatch.frame.request();
        let ids = delivery.ids;
        self.handles[role]
            .call(move |o| o.check_complete2(slot, binding, reply, request, &ids))
            .await
    }
    async fn repair(&self, role: usize) -> Result<()> {
        let slot = self.slot;
        let binding = self.binding(role).await?;
        let (request, plan) = self.handles[role]
            .call(move |o| o.prefix2(slot, binding))
            .await?
            .context("real missing-prefix plan")?;
        let response = self.send(role, &request).await?;
        self.handles[role]
            .call(move |o| o.accept_prefix2(slot, binding, response, &plan).map(|_| ()))
            .await
    }
    async fn acks(&self, role: usize) -> Result<()> {
        let slot = self.slot;
        let binding = self.binding(role).await?;
        if let Some(query) = self.handles[role]
            .call(move |o| o.ack_query2(slot, binding))
            .await?
        {
            let response = self.send(role, &query).await?;
            self.handles[role]
                .call(move |o| o.accept_acks2(slot, binding, response).map(|_| ()))
                .await?;
        }
        Ok(())
    }
}

fn remaining(owner: &mut Owner7, cursor: &str) -> Result<Vec<Json>> {
    let mut cursor = db::HistoryCursor::from_base64(cursor)?;
    let mut rows = Vec::new();
    for _ in 0..64 {
        let raw = owner.repo()?.history(0, Some(&cursor), 32)?;
        ensure!(raw.len() <= 48 * 1024 - 64, "actual escaped history bytes");
        let value: Json = serde_json::from_slice(&raw)?;
        rows.extend(
            value["items"]
                .as_array()
                .context("history rows")?
                .iter()
                .cloned(),
        );
        if value["finished"] == true {
            return Ok(rows);
        }
        cursor = db::HistoryCursor::from_base64(value["cursor"].as_str().context("next cursor")?)?;
    }
    anyhow::bail!("bounded component snapshot pages")
}

#[test]
fn s53_mixed_real_tls_native_history_core() -> Result<()> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(140);
    ensure!(
        std::env::var("KILOGRAM_S53_CASE")? == "C-s53-mixed-integrated-01",
        "managed mixed case before owned thread/runtime creation"
    );
    let joined = std::thread::Builder::new()
        .name("s53-mixed-case".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || mixed_on_owned_thread(started, deadline))
        .context("spawn bounded mixed controller thread")?
        .join();
    match joined {
        Ok(result) => {
            result?;
            ensure!(
                Instant::now() < deadline,
                "absolute mixed deadline includes runtime teardown and thread join"
            );
            Ok(())
        }
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
#[inline(never)]
fn mixed_on_owned_thread(started: Instant, deadline: Instant) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "mixed owned-thread startup deadline"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()?;
    let future = mixed_work(deadline);
    eprintln!(
        "s53_mixed_layout future_bytes={} case_stack_bytes={} worker_stack_bytes={} workers=4 elapsed_ms={}",
        std::mem::size_of_val(&future),
        8 * 1024 * 1024,
        2 * 1024 * 1024,
        started.elapsed().as_millis()
    );
    let result = runtime.block_on(Box::pin(future));
    drop(runtime);
    result
}
async fn mixed_work(deadline: Instant) -> Result<()> {
    let binding = Binding::managed("C-s53-mixed-integrated-01")?;
    let mut fixture = Fixture::new(&binding.run, true)?;
    let original = [
        Lineage::capture(&mut fixture.owners[0])?,
        Lineage::capture(&mut fixture.owners[1])?,
    ];
    let old_receipt = fixture.owners[0]
        .repo()?
        .receipt(0, 1, 1)?
        .context("genuine saved Receipt1")?;
    let page: Json = serde_json::from_slice(&fixture.owners[1].repo()?.history(0, None, 1)?)?;
    let cursor = page["cursor"]
        .as_str()
        .context("pre-Q HVI1 cursor")?
        .to_owned();
    let old_suffix = remaining(&mut fixture.owners[1], &cursor)?;
    let calls = native_call_counts();
    let profiles = fixture.profiles.clone();
    ensure!(
        Instant::now() < deadline,
        "absolute mixed component setup deadline"
    );
    let mut net = Net::new(fixture.owners, fixture.c, deadline).await?;
    let initial_binding = net.binding(0).await?;
    eprintln!("s53_mixed phase=legacy-admission-only event=start");
    net.legacy_admission_only()
        .await
        .context("mixed genuine L1 admission-only phase")?;
    let l1_binding = net.binding(0).await?;
    ensure!(
        l1_binding == net.binding(1).await? && l1_binding != initial_binding,
        "fresh real common L1 exporter"
    );
    let mut l1_gates = Vec::new();
    for (role, lineage) in original.iter().enumerate() {
        let expected = Lineage {
            head: lineage.head.clone(),
            key: Zeroizing::new(*lineage.key),
            keys: lineage.keys,
            account: lineage.account,
            reservation: lineage.reservation,
        };
        l1_gates.push(
            net.handles[role]
                .call(move |o| {
                    expected.check_legacy(o)?;
                    let lease = o.native1_materialize_gate(0, l1_binding)?;
                    ensure!(
                        lease.family == 1 && lease.binding == l1_binding,
                        "mixed actual current L1 materialization authority"
                    );
                    lease.gate()?;
                    Ok(lease)
                })
                .await
                .context("mixed L1 authority and original lineage")?,
        );
    }
    ensure!(
        calls == native_call_counts(),
        "mixed admissions invoke zero Native calls"
    );
    let a_before = net.handles[0].call(observed).await?;
    let (b_before, pending_before) = net.handles[1]
        .call(|o| {
            let before = observed(o)?;
            let gap = before["meta"]["next"][0]
                .as_u64()
                .context("original B gap frontier")?;
            let pending = o
                .repo()?
                .pending_page(0, None, 32)?
                .into_iter()
                .map(|p| {
                    (
                        p.ordinal,
                        p.raw,
                        p.state,
                        p.head,
                        p.quarantined,
                        p.original,
                        p.admission,
                    )
                })
                .collect::<Vec<_>>();
            ensure!(
                before["counts"]["queued"] == 0
                    && pending.len() == 1
                    && pending[0].0 == gap + 1
                    && o.event_at(0, 0, gap)?.is_none(),
                "mixed original genuine pending source behind missing A gap"
            );
            Ok((before, pending))
        })
        .await?;
    let b_next = b_before["meta"]["next"][1]
        .as_u64()
        .context("B pre-Text1 frontier")?;
    eprintln!("s53_mixed phase=held-native1 event=start");
    let held_source = net.handles[1]
        .call(move |o| {
            let before = observed(o)?;
            let before_calls = native_call_counts();
            o.native1_materialize_gate(0, l1_binding)?;
            let queued = o.queue(0, [211; 16], "last real B Text1".into())?;
            ensure!(
                queued.event.is_none() && queued.queue_sequence.is_some(),
                "genuine B Text1 queue"
            );
            ensure!(
                o.materialize(0)?,
                "mixed real B Native1 committed in same owner job"
            );
            let event = o
                .event_at(0, 1, b_next)?
                .context("committed held B Event1")?;
            let id = event.id()?;
            let raw = event.raw()?;
            let after = observed(o)?;
            let after_calls = native_call_counts();
            ensure!(
                event.core.role == 1
                    && event.core.n == b_next
                    && event.core.kind == 3
                    && o.repo()?
                        .request(0, [211; 16])?
                        .context("committed B request211")?
                        .event
                        == Some(id)
                    && o.repo()?.event(0, id)?.as_deref() == Some(raw.as_slice())
                    && o.repo()?
                        .debt_page(0, None, 32)?
                        .iter()
                        .any(|(_, debt)| *debt == id),
                "genuine immutable held B Text1 source/request/debt"
            );
            ensure!(
                after["counts"]["queued"] == 0
                    && after["counts"]["sources"].as_u64()
                        == before["counts"]["sources"].as_u64().map(|n| n + 1)
                    && after["counts"]["debt"].as_u64()
                        == before["counts"]["debt"].as_u64().map(|n| n + 1)
                    && after["counts"]["pending"] == before["counts"]["pending"]
                    && after["counts"]["receipts"] == before["counts"]["receipts"]
                    && after["meta"]["next"][0] == before["meta"]["next"][0]
                    && after["meta"]["next"][1] == b_next + 1,
                "exact one held B source/frontier/debt and no gap consumption"
            );
            ensure!(
                after_calls.encrypt == before_calls.encrypt + 1
                    && after_calls.decrypt == before_calls.decrypt
                    && (
                        after_calls.fresh_account,
                        after_calls.outbound_session,
                        after_calls.inbound_session
                    ) == (
                        before_calls.fresh_account,
                        before_calls.outbound_session,
                        before_calls.inbound_session
                    ),
                "one real Native1 encrypt, zero decrypt/constructors"
            );
            Ok((id, raw))
        })
        .await
        .context("mixed held Native1 materialization")?;
    let b_old = net.handles[1].call(observed).await?;
    let old_b_next = b_old["meta"]["next"][1]
        .as_u64()
        .context("B old frontier")?;
    net.disconnect()
        .await
        .context("mixed owned L1 disconnect and holding-task drain")?;
    ensure!(
        l1_gates.iter().all(|g| g.gate().is_err()) && net.tasks.is_empty(),
        "old actual L1 gates revoked and held admission guard drained"
    );
    let before_tls2 = native_call_counts();
    ensure!(
        before_tls2.encrypt == calls.encrypt + 1 && before_tls2.decrypt == calls.decrypt,
        "held B Text1 is the only pre-P Native work"
    );
    net.connect()
        .await
        .context("mixed fresh ALPN2 after held genuine Native1")?;
    let ab = net.binding(0).await?;
    let bb = net.binding(1).await?;
    ensure!(
        ab == bb && ab != l1_binding && ab != initial_binding,
        "real fresh common TLS2 exporter binding"
    );
    ensure!(
        before_tls2 == native_call_counts(),
        "L1 to fresh TLS2 adds zero Native calls"
    );
    for role in 0..2 {
        net.handles[role]
            .call(move |o| {
                let lease = o.check_existing_lease(0, ab)?;
                ensure!(
                    lease.family == 2
                        && lease.binding == ab
                        && o.repo()?.existing_state(0)?.mode == store::ExistingMode::L1,
                    "mixed actual fresh family2 while still L1 before P"
                );
                Ok(())
            })
            .await?;
    }
    let receipt = old_receipt.clone();
    net.handles[0]
        .call(move |o| {
            let after = observed(o)?;
            ensure!(
                after["meta"]["next"] == a_before["meta"]["next"]
                    && after["counts"] == a_before["counts"]
                    && o.event_at(0, 1, b_next)?.is_none()
                    && o.repo()?.receipt(0, 1, b_next)?.is_none()
                    && o.repo()?.receipt(0, 1, 1)?.as_deref() == Some(receipt.as_slice()),
                "A has no held B source/consumption/receipt; original Receipt1 unchanged"
            );
            Ok(())
        })
        .await?;
    net.handles[1].call(move |o| {
        let after = observed(o)?;
        let gap = b_before["meta"]["next"][0].as_u64().context("original B gap")?;
        let pending = o.repo()?.pending_page(0, None, 32)?.into_iter()
            .map(|p| (p.ordinal, p.raw, p.state, p.head, p.quarantined, p.original, p.admission))
            .collect::<Vec<_>>();
        ensure!(pending == pending_before && after["meta"]["next"][0] == gap
            && o.event_at(0, 0, gap)?.is_none()
            && o.repo()?.event(0, held_source.0)?.as_deref() == Some(held_source.1.as_slice())
            && o.repo()?.receipt(0, 1, b_next)?.is_none(),
            "fresh TLS2 retains exact pending gap and immutable undelivered B source without receipt");
        Ok(())
    }).await?;
    eprintln!("s53_mixed phase=fresh-alpn2-held-native1 event=ready-for-P");
    net.handles[0]
        .call(|o| o.existing(0, |s| s.request_upgrade(0, [220; 16])))
        .await?;
    let p = net.handles[0]
        .call(move |o| o.propose_frame2(0, ab))
        .await?;
    let accept_reply = net.send(0, &p).await?;
    net.handles[0]
        .call(|o| o.queue(0, [210; 16], "queued while Offering \"\\\n".repeat(32)))
        .await?;
    ensure!(
        accept_reply.body().kind() == v2::FrameKind::Accept,
        "real Accept2"
    );
    let raw = accept_reply.body().value().fields(1)?[0].bytes()?.to_vec();
    let held_k = net.handles[0]
        .call(move |o| o.select_frame2(0, ab, &raw))
        .await?;
    let exact_k = held_k.frame.body().value().fields(1)?[0].bytes()?.to_vec();
    net.head(1).await?;
    let prepared = net.handles[1].call(observed).await?;
    ensure!(
        prepared["state"]["mode"] == "Prepared"
            && prepared["state"]["q"].is_null()
            && !prepared["state"]["peer_q_hint"].is_null(),
        "authenticated opaque hint without local Q"
    );
    net.handles[1]
        .call(|o| o.queue(0, [212; 16], "B first genuine Event2".into()))
        .await?;
    let before = net.handles[1].call(observed).await?;
    let bad = net.handles[0]
        .call(move |o| {
            let h = o.head_body2(0)?;
            let mut f = h.fields(9)?[..4].to_vec();
            f.extend([u(0), a([]), a([a([u(2), b([])])])]);
            o.dispatch_frame2(0, ab, v2::FrameKind::Deliver, None, a(f), None)
        })
        .await?;
    let rejected = net.send(0, &bad).await?;
    ensure!(
        rejected.body().kind() == v2::FrameKind::Error,
        "tagged2 pre-Q refusal"
    );
    ensure!(
        before == net.handles[1].call(observed).await?,
        "pre-Q format2 refuses before any write/Native"
    );
    net.repair(1).await?;
    net.wait(1, |v| v["counts"]["pending"] == 0).await?;
    net.deliver(1, old_b_next - 1).await?;
    net.wait(0, |v| v["meta"]["next"][1] == old_b_next).await?;
    let late = net.handles[0]
        .call(move |o| {
            let raw = o
                .repo()?
                .receipt(0, 1, old_b_next - 1)?
                .context("late real receipt")?;
            let v = wire::decode(&raw, 1024)?;
            let r = v.fields(2)?[0].fields(12)?;
            ensure!(
                r[0].uint()? == 2 && r[6].uint()? == 1,
                "late legacy readable creates Receipt2(format1)"
            );
            Ok(raw)
        })
        .await?;
    let before = net.handles[1].call(observed).await?;
    let refused = net.handles[0]
        .call(move |o| {
            let v = wire::decode(&late, 1024)?;
            let r = v.fields(2)?[0].fields(12)?;
            let id = r[11].fixed::<32>()?;
            let capture = o.repo()?.auth(0, id)?.context("exact receiver")?;
            let h = o.head_body2(0)?;
            let mut f = h.fields(9)?[..4].to_vec();
            f.extend([
                u(1),
                a([a([b(id), b(capture)])]),
                a([a([u(2), b(late)])]),
                a([]),
            ]);
            o.dispatch_frame2(0, ab, v2::FrameKind::AckBatch, None, a(f), None)
        })
        .await?;
    ensure!(
        net.send(0, &refused).await?.body().kind() == v2::FrameKind::Error,
        "Prepared peer cannot consume Receipt2"
    );
    ensure!(
        before == net.handles[1].call(observed).await?,
        "deferred ACK preserves actual old debt"
    );
    let before_calls = native_call_counts();
    net.reconnect().await?;
    let after_calls = native_call_counts();
    ensure!(
        before_calls == after_calls,
        "fresh Selected/Prepared reconnect invokes no Native"
    );
    ensure!(
        held_k.gate().is_err(),
        "former connection token revoked before successor"
    );
    let ab = net.binding(0).await?;
    let k = net.handles[0]
        .call(move |o| o.saved_select_frame2(0, ab))
        .await?;
    ensure!(
        k.frame.body().value().fields(1)?[0].bytes()? == exact_k,
        "same K on new exporter"
    );
    let committed = net.send(0, &k).await?;
    ensure!(
        committed.body().kind() == v2::FrameKind::Committed,
        "real committed response held by caller"
    );
    let q = net.handles[0]
        .call(|o| o.repo()?.existing_state(0).map_err(Into::into))
        .await?
        .q;
    let b = net.wait(1, |v| v["counts"]["queued"] == 0).await?;
    let cut_b = b["state"]["cuts"][1].as_u64().context("B cut")?;
    ensure!(
        net.handles[0]
            .call(|o| Ok(o.leases[0].as_ref().context("lease")?.peer_stage))
            .await?
            == 3,
        "A cached peer remains Prepared"
    );
    net.deliver(1, cut_b).await?;
    net.wait(0, |v| v["meta"]["next"][1] == cut_b + 1).await?;
    let selected = net.handles[0].call(observed).await?;
    ensure!(
        selected["state"]["mode"] == "Selected" && selected["counts"]["queued"] == 1,
        "Selected A receives real Event2 while own Text remains frozen"
    );
    net.handles[1]
        .call(|o| o.queue(0, [213; 16], "real selected repair Event2".into()))
        .await?;
    net.wait(1, |v| v["counts"]["queued"] == 0).await?;
    net.head(1).await?;
    let pool_before = net.handles[0]
        .call(|o| Ok(o.repo()?.repair_state(0)?.repair_free))
        .await?;
    net.repair(0).await?;
    net.wait(0, |v| v["counts"]["pending"] == 0).await?;
    let pool_after = net.handles[0]
        .call(|o| Ok(o.repo()?.repair_state(0)?.repair_free))
        .await?;
    ensure!(
        pool_after < pool_before,
        "actual repair consumes funded pool without refill"
    );
    let raw = committed.body().value().fields(1)?[0].bytes()?.to_vec();
    net.handles[0]
        .call(move |o| o.control_gate2(0, ab, |s, gate| s.activate(0, &raw, gate)))
        .await?;
    net.wait(0, |v| v["counts"]["queued"] == 0).await?;
    let cut_a = selected["state"]["cuts"][0].as_u64().context("A cut")?;
    net.head(0).await?;
    net.deliver(0, cut_a).await?;
    net.wait(1, |v| v["counts"]["pending"] == 0).await?;
    net.acks(0).await?;
    net.acks(1).await?;
    for role in 0..2 {
        net.wait(role, |v| {
            v["counts"]["queued"] == 0 && v["counts"]["pending"] == 0 && v["counts"]["debt"] == 0
        })
        .await?;
    }
    let before_dup = net.handles[1].call(observed).await?;
    let dup = net.handles[0]
        .call(move |o| o.saved_select_frame2(0, ab))
        .await?;
    let duplicate = net.send(0, &dup).await?;
    ensure!(
        duplicate.body().value() == committed.body().value(),
        "saved Committed on duplicate K"
    );
    ensure!(
        before_dup == net.handles[1].call(observed).await?,
        "duplicate does not reseal or refill"
    );
    let after = native_call_counts();
    ensure!(
        (
            after.fresh_account,
            after.outbound_session,
            after.inbound_session
        ) == (
            calls.fresh_account,
            calls.outbound_session,
            calls.inbound_session
        ),
        "zero constructors during handoff/mixed work"
    );
    ensure!(
        (after.encrypt - calls.encrypt, after.decrypt - calls.decrypt) == (4, 6),
        "exact genuine Native encrypt/decrypt calls without replay"
    );
    let summary = [
        net.handles[0].call(observed).await?,
        net.handles[1].call(observed).await?,
    ];
    let handles = net.handles.clone();
    for (role, lineage) in original.into_iter().enumerate() {
        handles[role].call(move |o| lineage.check(o)).await?;
    }
    let suffix = cursor.clone();
    let actual = net.handles[1].call(move |o| remaining(o, &suffix)).await?;
    ensure!(actual == old_suffix, "exact old cursor suffix after append");
    let r = old_receipt.clone();
    net.handles[0]
        .call(move |o| {
            ensure!(
                o.repo()?.receipt(0, 1, 1)?.as_deref() == Some(r.as_slice()),
                "original Receipt1 bytes immutable"
            );
            Ok(())
        })
        .await?;
    drop(net);
    let mut reopened = Vec::new();
    for profile_path in profiles {
        let p = Profile3::load(&profile_path)?;
        reopened.push(Owner7::new(p.open(&profile_path)?, &[fixture.c])?);
    }
    ensure!(
        remaining(&mut reopened[1], &cursor)? == old_suffix,
        "same HVI1 suffix after clean reopen"
    );
    ensure!(
        reopened[0].repo()?.existing_state(0)?.q == q
            && reopened[0].repo()?.repair_state(0)?.repair_free == pool_after,
        "Q and spent pool survive reopen"
    );
    drop(reopened);
    ensure!(
        Instant::now() < deadline,
        "absolute mixed component completion deadline"
    );
    binding.finish(json!({"classification":"real TLS integrated component; no process/scheduler claim","scope":"mixed Native/data/receipt/history","same_original_lineage":true,"zero_constructors":true,"encrypt_calls":4,"decrypt_calls":6,"opaque_prepared_hint":true,"early_selected_receive":true,"old_receipt_immutable":true,"frozen_suffix_reopen":true,"repair_pool_before":pool_before,"repair_pool_after":pool_after,"final":summary}),||Ok(()))
}

struct ClockOffset {
    path: PathBuf,
    prior: Vec<u8>,
}
impl ClockOffset {
    fn install(path: &Path, seconds: u64) -> Result<Self> {
        ensure!(
            std::env::var_os("KILOGRAM_TEST_OD1_CLOCK_FILE").as_deref() == Some(path.as_os_str()),
            "managed policy clock required before fixture creation"
        );
        let prior = std::fs::read(path)?;
        std::fs::write(path, seconds.to_string())?;
        Ok(Self {
            path: path.into(),
            prior,
        })
    }
}
impl Drop for ClockOffset {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.path, &self.prior);
    }
}
fn ceremony(f: &mut Fixture, ttl: u64) -> Result<Vec<u8>> {
    use super::super::existing_admin::{self, ExistingConsentInputs};
    let scope = f.owners[0].scope2(0)?;
    let origin = scope.raw();
    let authorities = [
        AccountRootState::load(&f.roots[0])?
            .authority_snapshot()?
            .encode()?,
        AccountRootState::load(&f.roots[1])?
            .authority_snapshot()?
            .encode()?,
    ];
    let inputs = |role: usize| ExistingConsentInputs {
        origin,
        authorities: [&authorities[0], &authorities[1]],
        minimum_authorities: [&authorities[0], &authorities[1]],
        context: f.c,
        account: scope.peers()[role].certificate().account_id(),
        peer: scope.peers()[1 - role].certificate().account_id(),
    };
    let wall = now()?;
    let proposal = existing_admin::propose(inputs(0), &f.roots[0], wall - 1, ttl, wall)?;
    let old = scope.peers()[0].authority().encode()?;
    if old != authorities[0] {
        let mut stale = inputs(0);
        stale.authorities[0] = &old;
        stale.minimum_authorities[0] = &old;
        ensure!(
            existing_admin::propose(stale, &f.roots[0], wall - 1, ttl, wall).is_err(),
            "own current Root cannot be replaced by old authority witness"
        );
        let mut below_floor = inputs(1);
        below_floor.authorities[0] = &old;
        ensure!(
            existing_admin::cosign(below_floor, &proposal, &f.roots[1], wall).is_err(),
            "remote authority witness below explicit current floor"
        );
    }
    ensure!(
        proposal.len() <= 4096,
        "bounded public single-Root proposal"
    );
    ensure!(
        existing_admin::propose(inputs(1), &f.roots[1], wall - 1, ttl, wall).is_err(),
        "A proposal requires its own original role"
    );
    ensure!(
        existing_admin::cosign(inputs(1), &proposal, &f.roots[0], wall).is_err(),
        "B step cannot use A Root"
    );
    let mut wrong = inputs(1);
    wrong.context = [0xfe; 32];
    ensure!(
        existing_admin::cosign(wrong, &proposal, &f.roots[1], wall).is_err(),
        "exact expected existing context"
    );
    let mut changed = wire::decode(&proposal, 4096)?.fields(5)?.to_vec();
    changed[1] = b([0xaa; 32]);
    ensure!(
        existing_admin::cosign(
            inputs(1),
            &wire::encode(&a(changed), 4096)?,
            &f.roots[1],
            wall
        )
        .is_err(),
        "proposal bound to exact signed export"
    );
    existing_admin::cosign(inputs(1), &proposal, &f.roots[1], wall)
}
fn ipc_value(owner: &mut Owner7, value: Value, directory: &Path) -> Result<Value> {
    let request = wire::encode(&value, 32 * 1024)?;
    Ok(wire::decode(
        &runtime::ipc(owner, &request, directory)?,
        48 * 1024,
    )?)
}

#[test]
fn s53_current_policy_public_ceremony_ipc_core() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(105);
    let binding = Binding::managed("C-s53-policy-integrated-01")?;
    let clock_path = binding.run.join("policy.clock");
    ensure!(
        std::env::var_os("KILOGRAM_TEST_OD1_CLOCK_FILE").as_deref() == Some(clock_path.as_os_str())
            && std::fs::read_to_string(&clock_path)?.trim() == "0",
        "managed zero-offset clock before any fixture"
    );
    let mut f = Fixture::new(&binding.run, false)?;
    let original = [
        Lineage::capture(&mut f.owners[0])?,
        Lineage::capture(&mut f.owners[1])?,
    ];
    let calls = native_call_counts();
    let route = kilogram_transport_iroh::ordered2::RouteConfig2 {
        policy: kilogram_transport_iroh::RoutePolicy::RelayOnly,
        bind: None,
        relay: Some("http://127.0.0.1:3340/".into()),
    };
    ensure!(
        f.owners[0].configure_route(route.clone()).is_err(),
        "non-Active existing scope forbids relay"
    );
    let old_authority = AccountRootState::load(&f.roots[0])?
        .authority_snapshot()?
        .encode()?;
    let root = AccountRootState::load(&f.roots[0])?;
    let extra = kilogram_identity::DeviceIdentity::generate()?;
    let encryption = kilogram_crypto::DeviceEncryptionIdentity::generate()?;
    root.issue_device_certificate(
        extra.device_id(),
        encryption.public_key(),
        &kilogram_identity::DeviceCapability::MESSAGING,
    )?;
    let current_authority = root.authority_snapshot()?.encode()?;
    drop(root);
    for owner in &mut f.owners {
        owner.pin_authority(&current_authority)?;
    }
    let unchanged = observed(&mut f.owners[0])?;
    ensure!(
        f.owners[0].pin_authority(&old_authority).is_err(),
        "real current authority rollback refused"
    );
    ensure!(
        observed(&mut f.owners[0])? == unchanged,
        "rollback has no durable effect"
    );
    let selected_consent = ceremony(&mut f, 3600)?;
    let selected_ref = v2::ParsedConsent2::parse(&selected_consent)?.reference();
    for owner in &mut f.owners {
        let expected = owner.repo()?.existing_state(0)?.current_consent;
        ipc_value(
            owner,
            a([
                u(3),
                u(7),
                u(0),
                opt(expected.map(b)),
                b(&selected_consent),
                u(0),
            ]),
            &binding.run,
        )?;
    }
    f.consent = selected_consent.clone();
    for value in [
        a([u(2), u(8), u(0), b([1; 16])]),
        a([u(3), u(7)]),
        a([u(3), u(7), u(0), opt(None), b(&selected_consent), u(2)]),
        a([u(3), u(8), u(2), b([1; 16])]),
        a([u(3), u(9), u(0), u(0)]),
        a([u(3), u(10), u(0), b([1; 15])]),
        a([u(3), u(5), u(0), u(0), u(0), opt(Some(b([])))]),
    ] {
        let before = observed(&mut f.owners[0])?;
        ensure!(
            ipc_value(&mut f.owners[0], value, &binding.run).is_err(),
            "malformed IPC3 rejected"
        );
        ensure!(
            observed(&mut f.owners[0])? == before,
            "malformed IPC3 before writes"
        );
    }
    let before = observed(&mut f.owners[0])?;
    ensure!(
        f.owners[0]
            .existing(0, |s| s.import_consent(
                0,
                Some([9; 32]),
                selected_consent.clone(),
                false
            ))
            .is_err(),
        "current Consent2 CAS"
    );
    ensure!(
        observed(&mut f.owners[0])? == before,
        "CAS refusal has no effect"
    );
    ipc_value(
        &mut f.owners[0],
        a([u(3), u(8), u(0), b([231; 16])]),
        &binding.run,
    )?;
    let controls = f.complete()?;
    let selected = [
        f.owners[0].repo()?.existing_state(0)?,
        f.owners[1].repo()?.existing_state(0)?,
    ];
    let native = [
        observed(&mut f.owners[0])?["native"].clone(),
        observed(&mut f.owners[1])?["native"].clone(),
    ];
    let pools = [
        f.owners[0].repo()?.repair_state(0)?.repair_free,
        f.owners[1].repo()?.repair_state(0)?.repair_free,
    ];
    let renewed = ceremony(&mut f, 3 * 86400)?;
    let renewed_ref = v2::ParsedConsent2::parse(&renewed)?.reference();
    for owner in &mut f.owners {
        owner.existing(0, |s| {
            s.import_consent(0, Some(selected_ref), renewed.clone(), false)
        })?;
    }
    let clock = ClockOffset::install(&clock_path, 2 * 3600)?;
    for role in 0..2 {
        f.owners[role].current_existing(0)?;
        f.owners[role].selection2(0)?;
        let state = f.owners[role].repo()?.existing_state(0)?;
        ensure!(
            state.current_consent == Some(renewed_ref)
                && state.q == selected[role].q
                && state.cuts == selected[role].cuts
                && state.propose == selected[role].propose
                && state.select == selected[role].select,
            "current renewal preserves exact selected historical controls/Q/cuts"
        );
        let before = observed(&mut f.owners[role])?;
        ensure!(
            f.owners[role]
                .existing(0, |s| s.import_consent(
                    0,
                    Some(renewed_ref),
                    selected_consent.clone(),
                    false
                ))
                .is_err(),
            "expired selected consent is not current permission"
        );
        ensure!(
            observed(&mut f.owners[role])? == before,
            "expired current import has no effect"
        );
        f.owners[role].existing(0, |s| {
            s.import_consent(0, None, selected_consent.clone(), true)
        })?;
        f.owners[role].configure_route(route.clone())?;
        f.reopen(role)?;
        f.owners[role].current_existing(0)?;
        f.owners[role].selection2(0)?;
        ensure!(
            observed(&mut f.owners[role])?["native"] == native[role]
                && f.owners[role].repo()?.repair_state(0)?.repair_free == pools[role],
            "renew/archive/reopen preserves Native and escrow"
        );
        let reply = ipc_value(&mut f.owners[role], a([u(3), u(9), u(0)]), &binding.run)?;
        let status: Json = serde_json::from_slice(reply.fields(3)?[2].bytes()?)?;
        ensure!(
            status["mode"] == "Active"
                && status["policy_paused"] == false
                && status["lease_current"] == false,
            "public status distinguishes current policy from absent network lease"
        );
    }
    for (role, lineage) in original.into_iter().enumerate() {
        lineage.check(&mut f.owners[role])?;
    }
    ensure!(
        native_call_counts() == calls,
        "ceremony/policy/conversion/reopen makes no Native packet or constructor call"
    );
    drop(f);
    drop(clock);
    ensure!(
        Instant::now() < deadline,
        "absolute policy component deadline"
    );
    binding.finish(json!({"classification":"single-owner public ceremony/policy/IPC component; no TLS/process claim","separate_own_root_steps":true,"malformed_ipc_no_write":true,"current_vs_historical":true,"selected_controls_immutable":controls.iter().map(|v|state::hex(&wire::hash(b"",v))).collect::<Vec<_>>(),"same_original_native":true,"constructors_and_packet_calls":0}),||Ok(()))
}
