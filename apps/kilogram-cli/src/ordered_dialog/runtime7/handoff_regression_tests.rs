//! One charged genuine lineage for review1's integrated permission/data/reopen
//! regressions. Availability hooks delay real work, never invent Native/debt.
use super::*;

fn request(n: u64) -> [u8; 16] {
    let mut out = [0x92; 16];
    out[8..].copy_from_slice(&n.to_be_bytes());
    out
}
fn consent1(owner: &mut Owner7, roots: &[PathBuf; 2], ttl: u64) -> Result<Vec<u8>> {
    let original = owner.control(0, store::L1Control::Consent)?;
    let value = wire::decode(&original, 4096)?;
    let mut fields = value.fields(3)?[0].fields(8)?.to_vec();
    let wall = now()?;
    fields[6] = u(wall - 1);
    fields[7] = u(wall + ttl);
    let body = a(fields);
    let raw = wire::encode(&body, 4096)?;
    Ok(wire::encode(
        &a([
            body,
            b(AccountRootState::load(&roots[0])?.sign_ordered_context_consent(&raw)),
            b(AccountRootState::load(&roots[1])?.sign_ordered_context_consent(&raw)),
        ]),
        4096,
    )?)
}
fn renew1(owner: &mut Owner7, raw: Vec<u8>) -> Result<()> {
    let current = owner.control(0, store::L1Control::Consent)?;
    owner.renew_consent(
        0,
        Some(wire::hash(
            super::super::super::handshake::CONSENT,
            &current,
        )),
        raw,
    )
}
fn schedule(path: &Path, held: bool) -> Result<()> {
    std::fs::write(path, serde_json::to_vec(&json!({"suspend_drain":held}))?)?;
    Ok(())
}
fn reopen(profiles: &[PathBuf; 2], c: Id) -> Result<[Owner7; 2]> {
    let mut owners = Vec::new();
    for path in profiles {
        let p = Profile3::load(path)?;
        owners.push(Owner7::new(p.open(path)?, &[c])?);
    }
    owners
        .try_into()
        .map_err(|_| anyhow::anyhow!("two real reopened owners"))
}
async fn worker_heads(net: &Net, binding: Id) -> Result<u64> {
    net.handles[1]
        .call(move |o| {
            Ok(o.stats.worker_head_progress[0]
                .filter(|(seen, _)| *seen == binding)
                .map_or(0, |(_, n)| n))
        })
        .await
}
async fn repeated_worker_opportunity(
    net: &Net,
    binding: Id,
    baseline: u64,
    notifications: u64,
) -> Result<u64> {
    let deadline = (Instant::now() + Duration::from_secs(12)).min(net.deadline);
    tokio::time::timeout_at(deadline.into(), async {
        loop {
            let completed = worker_heads(net, binding).await?;
            if completed >= baseline + 2 {
                ensure!(
                    net.binding(1).await? == binding,
                    "same current exporter at observed worker barrier"
                );
                ensure!(
                    net.handles[0]
                        .call(|o| Ok(o.stats.open_intent_received))
                        .await?
                        == notifications,
                    "exact notification count after at least two real validated worker Heads"
                );
                return Ok(completed);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("bounded actual worker Head progress barrier")?
}

const CASE_STACK_BYTES: usize = 8 * 1024 * 1024;
const WORKER_STACK_BYTES: usize = 2 * 1024 * 1024;
fn regression_phase(started: Instant, phase: &str) {
    eprintln!(
        "s53_regressions_phase phase={phase} elapsed_ms={} thread={}",
        started.elapsed().as_millis(),
        std::thread::current().name().unwrap_or("unnamed")
    );
}
#[test]
fn s53_regressions_real_native_and_cold_pending_core() -> Result<()> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(330);
    ensure!(
        std::env::var("KILOGRAM_S53_CASE")? == "C-s53-regressions-r2-01",
        "managed exact case before thread/runtime creation"
    );
    regression_phase(started, "entry");
    let joined = std::thread::Builder::new()
        .name("s53-regressions-case".into())
        .stack_size(CASE_STACK_BYTES)
        .spawn(move || regressions_on_owned_thread(started, deadline))
        .context("spawn bounded regression controller thread")?
        .join();
    match joined {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
#[inline(never)]
fn regressions_on_owned_thread(started: Instant, deadline: Instant) -> Result<()> {
    regression_phase(started, "owned-thread-enter");
    ensure!(
        Instant::now() < deadline,
        "case deadline includes thread startup"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(WORKER_STACK_BYTES)
        .enable_all()
        .build()?;
    regression_phase(started, "runtime-built");
    // Construct and pin only on the bounded owned thread. The libtest entry
    // never owns this large future; block_on polls it here, not on a worker.
    let future = regressions_work(started, deadline);
    eprintln!(
        "s53_regressions_layout future_bytes={} fixture_bytes={} owner_bytes={} net_bytes={} configured_case_stack_bytes={} configured_worker_stack_bytes={} workers=4",
        std::mem::size_of_val(&future),
        std::mem::size_of::<Fixture>(),
        std::mem::size_of::<Owner7>(),
        std::mem::size_of::<Net>(),
        CASE_STACK_BYTES,
        WORKER_STACK_BYTES
    );
    let future = Box::pin(future);
    regression_phase(started, "future-boxed");
    runtime.block_on(future)
}
async fn regressions_work(started: Instant, deadline: Instant) -> Result<()> {
    regression_phase(started, "poll-enter");
    ensure!(
        Instant::now() < deadline,
        "case deadline includes runtime/future setup"
    );
    let binding = Binding::managed("C-s53-regressions-r2-01")?;
    let clock_path = binding.run.join("regressions.clock");
    let schedule_path = binding.run.join("regressions.schedule.json");
    ensure!(
        std::env::var_os("KILOGRAM_TEST_OD1_CLOCK_FILE").as_deref() == Some(clock_path.as_os_str())
            && std::fs::read_to_string(&clock_path)?.trim() == "0",
        "owned zero-offset clock before fixture"
    );
    ensure!(
        std::env::var_os("KILOGRAM_TEST_OD1_SCHEDULE").as_deref()
            == Some(schedule_path.as_os_str()),
        "owned availability schedule before fixture"
    );
    regression_phase(started, "fixture-start");
    let mut f = Fixture::new_with_timing(&binding.run, false, Some(("regressions-r5", started)))?;
    regression_phase(started, "fixture-ready");
    let short = consent1(&mut f.owners[0], &f.roots, 60)?;
    for owner in &mut f.owners {
        renew1(owner, short.clone())?;
    }
    let original = [
        Lineage::capture(&mut f.owners[0])?,
        Lineage::capture(&mut f.owners[1])?,
    ];
    let calls = native_call_counts();
    let profiles = f.profiles.clone();
    let roots = f.roots.clone();
    let c = f.c;
    let mut net = Net::new(f.owners, c, deadline).await?;
    regression_phase(started, "tls2-ready");
    net.handles[0]
        .call(|o| {
            o.queue(
                0,
                request(1),
                "Native1 waits for its own permission family".into(),
            )
        })
        .await?;
    let clock = ClockOffset::install(&clock_path, 120)?;
    let before = net.handles[0].call(observed).await?;
    let before_calls = native_call_counts();
    let refused = net.handles[0]
        .call(|o| Ok(o.materialize(0).is_err()))
        .await?;
    ensure!(
        refused
            && before == net.handles[0].call(observed).await?
            && before_calls == native_call_counts(),
        "R1 Consent2-only probe cannot encrypt Native1 or change queue/frontier"
    );
    ensure!(
        net.handles[0]
            .call(|o| Ok(o.leases[0].as_ref().context("still admitted probe")?.family))
            .await?
            == 2,
        "receive/repair admission remains available"
    );

    net.handles[1]
        .call(|o| o.existing(0, |s| s.request_upgrade(0, [0x93; 16])))
        .await?;
    let handle = net.handles[1].clone();
    let connection = net.connections.as_ref().context("real binding")?[1].clone();
    let bound = net.binding(1).await?;
    let first_binding = bound;
    let first_head_baseline = worker_heads(&net, bound).await?;
    net.tasks
        .spawn(async move { network2::worker(&handle, &connection, 0, bound).await });
    let first_heads = repeated_worker_opportunity(&net, bound, first_head_baseline, 1).await?;
    net.reconnect().await?;
    let handle = net.handles[1].clone();
    let connection = net.connections.as_ref().context("new binding")?[1].clone();
    let bound = net.binding(1).await?;
    ensure!(
        bound != first_binding,
        "actual fresh exporter for notification retry"
    );
    let fresh_head_baseline = worker_heads(&net, bound).await?;
    net.tasks
        .spawn(async move { network2::worker(&handle, &connection, 0, bound).await });
    let fresh_heads = repeated_worker_opportunity(&net, bound, fresh_head_baseline, 2).await?;
    regression_phase(started, "two-real-notification-barriers");
    let roots_for_sign = roots.clone();
    let renewed = net.handles[0]
        .call(move |o| consent1(o, &roots_for_sign, 86400))
        .await?;
    for role in 0..2 {
        let raw = renewed.clone();
        net.handles[role].call(move |o| renew1(o, raw)).await?;
    }
    for role in 0..2 {
        ensure!(
            net.handles[role]
                .call(|o| Ok(o.leases[0].is_none()))
                .await?,
            "renewal actually invalidated prior leases before owned disconnect and fresh L1 admission"
        );
    }
    net.legacy().await?;
    let next = before["meta"]["next"][0]
        .as_u64()
        .context("original send frontier")?;
    net.wait(0, |v| {
        v["counts"]["queued"] == 0 && v["meta"]["next"][0] == next + 1
    })
    .await?;
    net.wait(1, |v| v["meta"]["next"][0] == next + 1).await?;
    net.wait(0, |v| v["counts"]["debt"] == 0).await?;
    let after_l1 = native_call_counts();
    ensure!(
        after_l1.encrypt == before_calls.encrypt + 1
            && after_l1.decrypt == before_calls.decrypt + 1,
        "R1 exactly one real Native1 materialization after real L1 admission"
    );
    regression_phase(started, "native1-positive-complete");
    drop(net);
    let mut f = Fixture {
        owners: reopen(&profiles, c)?,
        profiles: profiles.clone(),
        roots: roots.clone(),
        consent: f.consent,
        c,
        timing: None,
    };
    f.owners[0].existing(0, |s| s.request_upgrade(0, [0x94; 16]))?;
    f.complete()?;
    regression_phase(started, "active-conversion-complete");
    let cut = f.owners[0].repo()?.existing_state(0)?.cuts[0].context("actual cut")?;
    let offline_calls = native_call_counts();
    for chunk in 0..33_u64 {
        let count = (131 - chunk * 4).min(4);
        for index in 0..count {
            let n = chunk * 4 + index;
            f.owners[0].queue(0, request(1000 + n), format!("real offline Event2 {n}"))?;
        }
        ensure!(
            f.owners[0].materialize(0)?,
            "R2 bounded local Active materialization without a connection"
        );
    }
    ensure!(
        f.owners[0].repo()?.counts(0)?.debt == 131
            && f.owners[0].repo()?.metadata(0)?.next[0] == cut + 131,
        "R2 genuine offline debt crosses127 through131"
    );
    ensure!(
        native_call_counts().encrypt == offline_calls.encrypt + 131,
        "R2 actual Native encryptions, no debt injection"
    );
    regression_phase(started, "offline131-complete");
    drop(f.owners);
    let owners = reopen(&profiles, c)?;
    let net = Net::new(owners, c, deadline).await?;
    ensure!(
        net.handles[0]
            .call(|o| Ok(o.repo()?.counts(0)?.debt))
            .await?
            == 131,
        "R2 disk debt survives cold reopen"
    );
    net.head(1).await?;
    for first in [cut + 1, cut + 17, cut + 33, cut + 49] {
        net.deliver(0, first).await?;
    }
    net.wait(1, |v| v["counts"]["pending"] == 64).await?;
    schedule(&schedule_path, true)?;
    let bound = net.binding(1).await?;
    let before_pool = net.handles[1]
        .call(|o| Ok(o.repo()?.repair_state(0)?.repair_free))
        .await?;
    let (dispatch, plan) = net.handles[1]
        .call(move |o| o.prefix2(0, bound))
        .await?
        .context("real funded-head request")?;
    let watermark = net.handles[1]
        .call(|o| Ok(o.repo()?.metadata(0)?.peer_source_next))
        .await?;
    net.handles[0]
        .call(|o| {
            o.queue(
                0,
                request(2000),
                "actual append between Head and PrefixReq".into(),
            )
        })
        .await?;
    net.wait(0, |v| v["meta"]["next"][0] == watermark + 1)
        .await?;
    let response = net.send(1, &dispatch).await?;
    // Keep the genuine outer signer and exact request, but cross the immutable
    // source's C without re-signing its Event2. No rejected bytes may persist.
    let mut corrupt = response.body().value().fields(11)?.to_vec();
    let mut items = v2::tagged_items(&corrupt[9], v2::MAX_EVENT)?;
    let raw = wire::decode(&items[0].raw, v2::MAX_EVENT)?;
    let mut event = raw.fields(5)?.to_vec();
    let mut body = event[1].fields(15)?.to_vec();
    body[1] = b([0xfe; 32]);
    event[1] = a(body);
    items[0].raw = wire::encode(&a(event), v2::MAX_EVENT)?;
    corrupt[9] = a(items
        .iter()
        .map(|item| a([u(item.format as u64), b(&item.raw)])));
    let rid = response.request();
    let ab = net.binding(0).await?;
    let corrupt = net.handles[0]
        .call(move |o| {
            o.dispatch_frame2(
                0,
                ab,
                v2::FrameKind::PrefixResp,
                Some(rid),
                a(corrupt),
                None,
            )
        })
        .await?
        .frame;
    let intact = net.handles[1].call(observed).await?;
    let rejected_plan = plan.clone();
    let native_before = native_call_counts();
    ensure!(
        net.handles[1]
            .call(move |o| Ok(o.accept_prefix2(0, bound, corrupt, &rejected_plan).is_err()))
            .await?
            && intact == net.handles[1].call(observed).await?
            && native_before == native_call_counts(),
        "R3 cross-scope/corrupt Event2 refuses before Pending and Native"
    );
    let mut bad = response.body().value().fields(11)?.to_vec();
    let genuine_cursor = bad[7].fields(2)?;
    ensure!(
        genuine_cursor[0].uint()? == watermark + 1 && bad[6].uint()? < watermark,
        "R4 genuine advanced cursor and prefix end below saved watermark"
    );
    let conflicting_tail = wire::optional_id(&genuine_cursor[1])?
        .context("R4 real concurrently appended source tail")?;
    net.handles[0]
        .call(move |o| {
            let raw = o
                .repo()?
                .event_at(0, 0, watermark)?
                .context("R4 actual concurrent append source")?;
            ensure!(
                v2::ParsedEvent2::parse(&raw)?.id() == conflicting_tail,
                "R4 conflicting tail is the real committed append"
            );
            Ok(())
        })
        .await?;
    net.handles[1]
        .call(move |o| {
            let meta = o.repo()?.metadata(0)?;
            ensure!(
                meta.peer_source_next == watermark
                    && meta.peer_source_last != Some(conflicting_tail),
                "R4 receiver retains equal watermark and a different actual tail"
            );
            Ok(())
        })
        .await?;
    bad[7] = a([u(watermark), opt(Some(b(conflicting_tail)))]);
    let request_id = response.request();
    let ab = net.binding(0).await?;
    let native_before_r4 = native_call_counts();
    let bad = net.handles[0]
        .call(move |o| {
            o.dispatch_frame2(
                0,
                ab,
                v2::FrameKind::PrefixResp,
                Some(request_id),
                a(bad),
                None,
            )
        })
        .await
        .context("R4 schema-valid PrefixResp construction/signing")?
        .frame;
    ensure!(
        bad.body().kind() == v2::FrameKind::PrefixResp && bad.request() == request_id,
        "R4 genuine signer preserves response identity"
    );
    regression_phase(started, "r4-schema-valid-negative-built");
    let prior = net.handles[1].call(observed).await?;
    let rejected_plan = plan.clone();
    let r4_rejection = net.handles[1]
        .call(move |o| {
            let source = bad.body().value().fields(11)?[7].fields(2)?;
            ensure!(
                source[0].uint()? == o.repo()?.metadata(0)?.peer_source_next
                    && source[0].uint()? == watermark,
                "R4 source_next equals saved receiver watermark"
            );
            o.accept_prefix2(0, bound, bad, &rejected_plan)
                .err()
                .context("R4 expected receiver prefix conflict")
        })
        .await
        .context("R4 receiver acceptance path")?
        .to_string();
    let unchanged = prior == net.handles[1].call(observed).await?;
    let native_after_r4 = native_call_counts();
    ensure!(
        r4_rejection == "prefix equal watermark tail conflict",
        "R4 wrong rejection path: {r4_rejection}"
    );
    ensure!(
        unchanged && native_before_r4 == native_after_r4,
        "R4 exact cursor conflict leaves durable state and global Native counts unchanged"
    );
    regression_phase(started, "r4-equal-watermark-tail-rejected");
    net.handles[1]
        .call(move |o| o.accept_prefix2(0, bound, response, &plan))
        .await?;
    let pending = net.wait(1, |v| v["counts"]["pending"] == 65).await?;
    regression_phase(started, "pending65-before-close");
    let paid_pool = net.handles[1]
        .call(|o| Ok(o.repo()?.repair_state(0)?.repair_free))
        .await?;
    ensure!(
        net.handles[1]
            .call(|o| Ok(o.repo()?.metadata(0)?.peer_source_next))
            .await?
            == watermark + 1,
        "R4 authenticated newer frontier advances monotonically"
    );
    drop(net);
    let mut owners = reopen(&profiles, c)?;
    ensure!(
        owners[1].repo()?.counts(0)?.pending == 65
            && owners[1].repo()?.repair_state(0)?.head == Some(cut)
            && owners[1].repo()?.repair_state(0)?.repair_free == paid_pool,
        "R3 ordinary plus funded-head Event2 survive cold reopen without funding again"
    );
    regression_phase(started, "pending65-reopened");
    let net = Net::new(owners, c, deadline).await?;
    schedule(&schedule_path, false)?;
    net.wait(1, |v| {
        v["counts"]["pending"] == 0 && v["meta"]["next"][0] == cut + 65
    })
    .await?;
    let final_pool = net.handles[1]
        .call(|o| Ok(o.repo()?.repair_state(0)?.repair_free))
        .await?;
    regression_phase(started, "drain65-complete");
    ensure!(
        final_pool < before_pool && final_pool > paid_pool,
        "R3 funded frame retired exactly once, with only its unused remainder refunded"
    );
    for _ in 0..5 {
        net.acks(0).await?;
    }
    ensure!(
        net.handles[0]
            .call(|o| Ok(o.repo()?.counts(0)?.debt))
            .await?
            == 67,
        "actual acknowledgements retire exactly65 of132 sources"
    );
    for (role, lineage) in original.into_iter().enumerate() {
        net.handles[role].call(move |o| lineage.check(o)).await?;
    }
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
        "no account/session constructors in any reviewed transition"
    );
    let final_rows = [
        net.handles[0].call(observed).await?,
        net.handles[1].call(observed).await?,
    ];
    drop(net);
    drop(clock);
    ensure!(
        Instant::now() < deadline,
        "absolute review2 component deadline"
    );
    regression_phase(started, "functional-assertions-complete");
    binding.finish(json!({"classification":"one genuine lineage: real TLS permission families and notification retries, actual offline Native debt, concurrent prefix and cold ordinary/head pending","notification_worker_heads":{"first_baseline":first_head_baseline,"first_completed":first_heads,"fresh_baseline":fresh_head_baseline,"fresh_completed":fresh_heads,"different_exporters":true},"r1_refusal":before,"offline_debt":131,"ordinary_pending":64,"funded_head_pending":1,"cold_pending":pending,"watermark_before":watermark,"watermark_after":watermark+1,"r4_rejection":r4_rejection,"r4_state_unchanged":unchanged,"r4_native_before":[native_before_r4.fresh_account,native_before_r4.outbound_session,native_before_r4.inbound_session,native_before_r4.encrypt,native_before_r4.decrypt],"r4_native_after":[native_after_r4.fresh_account,native_after_r4.outbound_session,native_after_r4.inbound_session,native_after_r4.encrypt,native_after_r4.decrypt],"pool_before_grant":before_pool,"pool_pending":paid_pool,"pool_after_drain":final_pool,"final":final_rows}),||Ok(()))
}

/// A separately selectable gate for the failure before tls2-ready. It creates
/// the same genuine old dialog/migrations/short Consent1, without treating this
/// narrow admission result as completion of the full regression above.
#[test]
fn s53_initial_existing_admission_core() -> Result<()> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(90);
    ensure!(
        std::env::var("KILOGRAM_S53_CASE")? == "C-s53-admission-r6-01",
        "managed exact admission case before thread/runtime creation"
    );
    regression_phase(started, "admission-entry");
    let joined = std::thread::Builder::new()
        .name("s53-admission-case".into())
        .stack_size(CASE_STACK_BYTES)
        .spawn(move || admission_on_owned_thread(started, deadline))
        .context("spawn bounded admission controller thread")?
        .join();
    match joined {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
#[inline(never)]
fn admission_on_owned_thread(started: Instant, deadline: Instant) -> Result<()> {
    ensure!(Instant::now() < deadline, "admission startup deadline");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(WORKER_STACK_BYTES)
        .enable_all()
        .build()?;
    let future = admission_work(started, deadline);
    eprintln!(
        "s53_admission_layout future_bytes={} configured_case_stack_bytes={} configured_worker_stack_bytes={} workers=4",
        std::mem::size_of_val(&future),
        CASE_STACK_BYTES,
        WORKER_STACK_BYTES
    );
    let future = Box::pin(future);
    runtime.block_on(future)
}
async fn admission_work(started: Instant, deadline: Instant) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "admission runtime setup deadline"
    );
    let binding = Binding::managed("C-s53-admission-r6-01")?;
    let clock_path = binding.run.join("regressions.clock");
    let schedule_path = binding.run.join("regressions.schedule.json");
    ensure!(
        std::env::var_os("KILOGRAM_TEST_OD1_CLOCK_FILE").as_deref() == Some(clock_path.as_os_str())
            && std::fs::read_to_string(&clock_path)?.trim() == "0"
            && std::env::var_os("KILOGRAM_TEST_OD1_SCHEDULE").as_deref()
                == Some(schedule_path.as_os_str())
            && std::fs::read_to_string(&schedule_path)?.trim() == "{}",
        "same owned zero-offset clock and availability as the initial full regression"
    );
    let mut f = Fixture::new_with_timing(&binding.run, false, Some(("admission-r6", started)))?;
    regression_phase(started, "admission-fixture-ready");
    let short = consent1(&mut f.owners[0], &f.roots, 60)?;
    for owner in &mut f.owners {
        renew1(owner, short.clone())?;
    }
    let original = [
        Lineage::capture(&mut f.owners[0])?,
        Lineage::capture(&mut f.owners[1])?,
    ];
    let before = native_call_counts();
    let net = Net::new(f.owners, f.c, deadline).await?;
    regression_phase(started, "admission-tls2-ready");
    let shared_binding = net.binding(0).await?;
    ensure!(
        shared_binding == net.binding(1).await?,
        "actual shared TLS2 exporter"
    );
    for (role, lineage) in original.into_iter().enumerate() {
        net.handles[role]
            .call(move |owner| {
                let lease = owner.check_existing_lease(0, shared_binding)?;
                ensure!(
                    lease.family == 2
                        && owner.repo()?.existing_state(0)?.mode == store::ExistingMode::L1,
                    "current ALPN2 admission remains pre-P L1"
                );
                lineage.check_legacy(owner)
            })
            .await?;
    }
    ensure!(
        before == native_call_counts(),
        "admission preserves actual Native constructors and encrypt/decrypt counts"
    );
    drop(net);
    ensure!(
        Instant::now() < deadline,
        "absolute initial admission deadline includes cleanup"
    );
    regression_phase(started, "admission-assertions-complete");
    binding.finish(json!({"classification":"initial existing admission only; full regression remains unverified", "real_tls2_both_sides":true,"current_lease_family":2,"mode":"L1","same_original_native_lineage":true,"native_calls_unchanged":true}), || Ok(()))
}
