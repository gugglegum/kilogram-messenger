//! Ordinary A regression: real storage/Owner7/IPC-handler, no CLI/Controller claim.
use super::super::super::data2;
use super::*;
use std::collections::BTreeSet;

fn run_case(name: &'static str, full: bool) -> Result<()> {
    run_checked_case(name, full, false)
}
fn run_checked_case(name: &'static str, full: bool, check_cleanup: bool) -> Result<()> {
    ensure!(
        std::env::var_os("KILOGRAM_S53_CASE").is_none(),
        "ordinary A test is not a managed matrix case"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let parent = root.join(".tmp/s54-block4aw2-history-owner-01");
    std::fs::create_dir_all(&parent)?;
    let run = tempfile::Builder::new()
        .prefix(name)
        .tempdir_in(parent)?
        .keep();
    let deadline = Instant::now() + Duration::from_secs(240);
    let thread = std::thread::Builder::new()
        .name(name.into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(2 * 1024 * 1024)
                .enable_all()
                .build()?;
            let result = rt.block_on(async {
                tokio::time::timeout_at(
                    deadline.into(),
                    Box::pin(scenario(&run, full, check_cleanup, deadline)),
                )
                .await
                .context("bounded ordinary history owner case")?
            });
            drop(rt);
            result
        })?;
    thread
        .join()
        .map_err(|_| anyhow::anyhow!("ordinary history owner thread panic"))??;
    ensure!(
        Instant::now() < deadline,
        "history case includes cleanup/join"
    );
    Ok(())
}

#[test]
fn s54_history_owner_real_dispatch_first_suffix_and_independent_context() -> Result<()> {
    run_case("owner-history-real-", true)
}
#[test]
fn s54_history_owner_default_fixture_and_slot_zero_compatibility() -> Result<()> {
    run_case("owner-history-default-", false)
}
#[test]
fn s54_history_owner_task_panic_error_and_owned_cancellation() -> Result<()> {
    run_checked_case("owner-history-cleanup-", false, true)
}

async fn cleanup_regression(mut net: Net) -> Result<()> {
    // Real completed Err tasks must fail, including a string impersonating a benign close.
    for owned_close in [false, true] {
        let mut tasks = tokio::task::JoinSet::new();
        let done = tasks.spawn(async {
            Err(anyhow::anyhow!(
                "closed by peer: existing connection complete (code 0)"
            ))
        });
        while !done.is_finished() {
            tokio::task::yield_now().await;
        }
        ensure!(
            Net::drain_ordinary(&mut tasks, owned_close).await.is_err() && tasks.is_empty(),
            "unexpected task Err must propagate after drain"
        );
    }
    let mut tasks = tokio::task::JoinSet::new();
    let cancelled = tasks.spawn(std::future::pending::<Result<()>>());
    cancelled.abort();
    // Await the actual JoinError instead of racing is_finished against JoinSet notification.
    let joined = tasks
        .join_next()
        .await
        .context("actual cancelled task join")?;
    ensure!(
        Net::ordinary_join(joined, false, false).is_err() && tasks.is_empty(),
        "preexisting cancellation is not our cleanup abort"
    );

    struct TaskDrop(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for TaskDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let guard = TaskDrop(dropped.clone());
    let (ready, started) = oneshot::channel();
    net.tasks.spawn(async move {
        let _guard = guard;
        let _ = ready.send(());
        std::future::pending::<Result<()>>().await
    });
    started.await?;
    // resume_unwind causes a genuine Tokio task panic/JoinError without using panic!.
    let panicked = net.tasks.spawn(async {
        std::panic::resume_unwind(Box::new("ordinary cleanup deliberate panic"));
    });
    while !panicked.is_finished() {
        tokio::task::yield_now().await;
    }
    let handles = net.handles.clone();
    let endpoints = net.endpoints.clone();
    let error = net
        .finish()
        .await
        .err()
        .context("real task panic must fail finish")?;
    ensure!(
        error
            .downcast_ref::<tokio::task::JoinError>()
            .is_some_and(|e| e.is_panic()),
        "finish preserves actual task panic after cleanup"
    );
    ensure!(
        dropped.load(Ordering::Acquire)
            && handles.iter().all(|h| h.stop.load(Ordering::Acquire))
            && endpoints.iter().all(Endpoint::is_closed),
        "panic still drains intentional cancellation, stops owners and closes endpoints"
    );
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(std::future::pending::<Result<()>>());
    Net::drain_ordinary(&mut tasks, false).await?;
    ensure!(
        tasks.is_empty(),
        "our deliberate abort is successful cleanup"
    );
    println!(
        "s54_history_owner_cleanup real_task_panic=rejected unexpected_error=rejected preexisting_cancel=rejected owned_abort=accepted drain=complete owners=stopped endpoints=closed"
    );
    Ok(())
}

async fn page(
    net: &Net,
    role: usize,
    slot: u8,
    cursor: Option<db::HistoryCursor>,
    limit: u64,
) -> Result<Json> {
    net.handles[role]
        .call(move |o| {
            let value = ipc_value(
                o,
                a([
                    u(3),
                    u(4),
                    u(slot.into()),
                    opt(cursor.as_ref().map(|c| b(c.as_bytes()))),
                    u(limit),
                ]),
                Path::new("."),
            )?;
            let fields = value.fields(3)?;
            ensure!(
                fields[0].uint()? == 3 && fields[1].uint()? == 4,
                "actual IPC3 history reply"
            );
            Ok(serde_json::from_slice(fields[2].bytes()?)?)
        })
        .await
}
async fn busy(
    net: &Net,
    role: usize,
    slot: u8,
    cursor: Option<db::HistoryCursor>,
    limit: u64,
) -> Result<()> {
    let error = page(net, role, slot, cursor, limit)
        .await
        .err()
        .context("held dispatch must reject actual history admission")?;
    ensure!(
        error.to_string() == "SchedulingWindowBusy",
        "exact Owner7 scheduling rejection"
    );
    Ok(())
}
async fn windows(net: &Net, role: usize, current: [usize; 2]) -> Result<()> {
    let expected = if net.slot == 1 { [64, 64] } else { [128, 0] };
    let snapshot = net.handles[role].call(|o| Ok(o.window.snapshot())).await?;
    ensure!(
        snapshot.0 == expected && snapshot.1 == current,
        "actual window shares/current refs"
    );
    Ok(())
}
async fn stats(net: &Net) -> Result<[Json; 2]> {
    let mut out = Vec::new();
    for handle in &net.handles {
        out.push(handle.call(|o| Ok(serde_json::to_value(&o.stats)?)).await?);
    }
    out.try_into()
        .map_err(|_| anyhow::anyhow!("two actual owner stats"))
}
async fn clear_debt(net: &Net) -> Result<()> {
    for _ in 0..8 {
        net.acks(0).await?;
        net.acks(1).await?;
        // Slot is passed into the owned callbacks, never borrowed across actor calls.
        let slot = net.slot;
        let a = net.handles[0]
            .call(move |o| Ok(o.repo()?.counts(slot)?.debt))
            .await?;
        let b = net.handles[1]
            .call(move |o| Ok(o.repo()?.counts(slot)?.debt))
            .await?;
        if a == 0 && b == 0 {
            return Ok(());
        }
    }
    anyhow::bail!("real ACK queries must clear debt")
}
async fn scenario(parent: &Path, full: bool, check_cleanup: bool, deadline: Instant) -> Result<()> {
    let fixture = if full {
        Fixture::new_history(parent, 1)?
    } else {
        Fixture::new(parent, false)?
    };
    println!("s54_history_owner_A stage=migrated ordinary=true two_contexts={full}");
    let slot = if full { 1 } else { 0 };
    let consent = fixture.consent.clone();
    let mut net = if full {
        Net::new_at_slot(fixture.owners, fixture.c, slot, deadline).await?
    } else {
        Net::new(fixture.owners, fixture.c, deadline).await?
    };
    // The managed/default callers retain their original permissive disconnect path.
    net.ordinary_cleanup = true;
    if check_cleanup {
        return cleanup_regression(net).await;
    }
    // All retained dispatches are scoped inside this future and drop before Net cleanup.
    let result = exercise(&mut net, full, consent).await;
    let cleanup = net.finish().await;
    cleanup.context("history owned cleanup")?;
    result?;
    println!("s54_history_owner_A cleanup=joined references=0 tasks=0 endpoints=closed");
    Ok(())
}

async fn exercise(net: &mut Net, full: bool, consent: Vec<u8>) -> Result<()> {
    let slot = net.slot;
    if !full {
        for role in 0..2 {
            let row = net
                .wait(role, |v| v["meta"]["next"] == json!([3, 3]))
                .await?;
            ensure!(row["counts"]["queued"] == 0, "default fixture queue");
            windows(net, role, [0, 0]).await?;
        }
        let before = native_call_counts();
        let counters = stats(net).await?;
        let rows = page(net, 1, 0, None, 16).await?;
        ensure!(
            rows["items"].as_array().is_some_and(|v| v.len() == 4) && rows["finished"] == true,
            "default roleB/slot0 real history4"
        );
        ensure!(
            native_call_counts() == before && stats(net).await? == counters,
            "default reads invoke zero native calls"
        );
        println!(
            "s54_history_owner_A default=true slot=0 role=1 share=128 genuine_rows=4 managed_gates_unchanged=true"
        );
        return Ok(());
    }
    net.legacy().await?;
    for role in 0..2 {
        net.wait(role, |v| {
            v["meta"]["next"] == json!([5, 3])
                && v["counts"]["queued"] == 0
                && v["counts"]["debt"] == 0
                && v["counts"]["pending"] == 0
        })
        .await?;
    }
    net.disconnect().await?;
    net.handles[0]
        .call(move |o| o.existing(slot, |s| s.request_upgrade(slot, [209; 16])))
        .await?;
    let p = net.handles[0]
        .call(move |o| o.existing(slot, |s| s.propose(slot, &mut || Ok(()))))
        .await?;
    let a = net.handles[1]
        .call(move |o| o.existing(slot, |s| s.prepare(slot, &p, &consent, &mut || Ok(()))))
        .await?;
    let k = net.handles[0]
        .call(move |o| o.existing(slot, |s| s.select(slot, &a, &mut || Ok(()))))
        .await?;
    let committed = net.handles[1]
        .call(move |o| o.existing(slot, |s| s.commit_b(slot, &k, &mut || Ok(()))))
        .await?;
    net.handles[0]
        .call(move |o| o.existing(slot, |s| s.activate(slot, &committed, &mut || Ok(()))))
        .await?;
    net.connect().await?;
    net.head(0).await?;
    net.head(1).await?;
    for role in 0..2 {
        net.handles[role]
            .call(move |o| {
                let mode = o.repo()?.existing_state(slot)?;
                ensure!(
                    mode.mode == store::ExistingMode::Active
                        && mode.cuts == [Some(5), Some(3)]
                        && o.repo()?.metadata(slot)?.next == [5, 3],
                    "genuine Active cut5/3"
                );
                Ok(())
            })
            .await?;
    }
    for (role, first, last) in [(0, 5u8, 6u8), (1, 3, 34)] {
        for n in first..=last {
            net.handles[role]
                .call(move |o| {
                    let request = [n + if role == 0 { 40 } else { 100 }; 16];
                    o.queue(slot, request, "genuine current history source".into())?;
                    Ok(())
                })
                .await?;
        }
        let next = if role == 0 { 7 } else { 35 };
        net.wait(role, move |v| {
            v["meta"]["next"][role] == next && v["counts"]["queued"] == 0
        })
        .await?;
        let mut cursor = u64::from(first);
        while cursor < next {
            net.deliver(role, cursor).await?;
            cursor = net.handles[1 - role]
                .call(move |o| Ok(o.repo()?.metadata(slot)?.next[role]))
                .await?;
        }
    }
    let binding = net.binding(0).await?;
    let mut held = Some(
        net.handles[0]
            .call(move |o| o.ack_push2(slot, binding))
            .await?
            .context("real A ACK push eligibility")?,
    );
    ensure!(
        !held.as_ref().context("owned ACK")?.ids.is_empty()
            && held.as_ref().context("owned ACK")?.dispatch._refs.is_some(),
        "genuine nonempty ACK owns permit"
    );
    windows(net, 0, [0, 32]).await?;
    clear_debt(net).await?;
    for role in 0..2 {
        net.wait(role, |v| {
            v["meta"]["next"] == json!([7, 35])
                && v["counts"]["queued"] == 0
                && v["counts"]["debt"] == 0
                && v["counts"]["pending"] == 0
        })
        .await?;
    }
    let mut originals = Vec::new();
    for owner in 0..2 {
        originals.push(
            net.handles[owner]
                .call(move |o| {
                    let mut out = BTreeMap::new();
                    for (role, last) in [(0, 6), (1, 34)] {
                        for n in 1..=last {
                            let raw = o
                                .repo()?
                                .event_at(slot, role, n)?
                                .context("genuine stored source")?;
                            let format = data2::declared(&raw)?.0;
                            let event = o.verify_source2(
                                slot,
                                &v2::TaggedItem { format, raw },
                                &BTreeMap::new(),
                            )?;
                            let facts = data2::facts(&event)?;
                            ensure!(
                                facts.meta.role == role
                                    && facts.meta.n == n
                                    && format
                                        == if n < if role == 0 { 5 } else { 3 } {
                                            1
                                        } else {
                                            2
                                        },
                                "actual source role/ordinal/format"
                            );
                            ensure!(
                                out.insert(state::hex(&facts.id), (role, n, format))
                                    .is_none(),
                                "unique original source"
                            );
                        }
                    }
                    ensure!(out.len() == 40, "forty genuine sources");
                    Ok(out)
                })
                .await?,
        );
    }
    ensure!(
        originals.len() == 2 && originals[0] == originals[1],
        "both owners retain same forty authenticated originals"
    );
    let expected = originals.pop().context("actual original source map")?;
    let before = native_call_counts();
    let counters = stats(net).await?;
    let c2_before = net.handles[0].call(|o| observed_at(o, 0)).await?;
    let c2_b_before = net.handles[1].call(|o| observed_at(o, 0)).await?;
    busy(net, 0, 1, None, 16).await?;
    let c2 = page(net, 0, 0, None, 16).await?;
    ensure!(
        c2["F"] == json!([1, 1])
            && c2["finished"] == true
            && c2["items"].as_array().is_some_and(Vec::is_empty),
        "independent C2 genuine empty history"
    );
    windows(net, 0, [0, 32]).await?;
    drop(held.take());
    windows(net, 0, [0, 0]).await?;
    let first = page(net, 0, 1, None, 16).await?;
    ensure!(
        first["items"].as_array().is_some_and(|v| v.len() == 16)
            && first["finished"] == false
            && first["F"] == json!([7, 35]),
        "genuine first16 of40"
    );
    let cursor =
        db::HistoryCursor::from_base64(first["cursor"].as_str().context("actual history cursor")?)?;
    held = Some(
        net.handles[0]
            .call(move |o| o.ack_push2(slot, binding))
            .await?
            .context("real repeated ACK eligibility")?,
    );
    windows(net, 0, [0, 32]).await?;
    busy(net, 0, 1, Some(cursor.clone()), 32).await?;
    drop(held.take());
    windows(net, 0, [0, 0]).await?;
    let suffix = page(net, 0, 1, Some(cursor), 32).await?;
    ensure!(
        suffix["items"].as_array().is_some_and(|v| v.len() == 24)
            && suffix["finished"] == true
            && suffix["F"] == first["F"]
            && suffix["view"] == first["view"],
        "genuine retained24 same view/frontier"
    );
    let mut actual = BTreeMap::new();
    let mut proof_kinds = BTreeSet::new();
    for row in first["items"]
        .as_array()
        .context("first actual rows")?
        .iter()
        .chain(suffix["items"].as_array().context("actual suffix rows")?)
    {
        let id = row["id"].as_str().context("actual source ID")?;
        let value = (
            row["role"].as_u64().context("role")? as u8,
            row["ordinal"].as_u64().context("ordinal")?,
            row["format"].as_u64().context("format")? as u8,
        );
        ensure!(
            expected.get(id) == Some(&value) && actual.insert(id.to_owned(), value).is_none(),
            "returned exact original source"
        );
        proof_kinds.insert(
            row["proof_kind"]
                .as_str()
                .context("actual proof kind")?
                .to_owned(),
        );
    }
    ensure!(
        actual == expected
            && proof_kinds
                == BTreeSet::from(["LegacySnapshot".into(), "FullL1".into(), "FullMixed".into()]),
        "actual mixed source proofs and exact40"
    );
    // Opposite role has a real eligible ACK; sibling physical0 stays independent.
    let binding = net.binding(1).await?;
    held = Some(
        net.handles[1]
            .call(move |o| o.ack_push2(slot, binding))
            .await?
            .context("real roleB ACK eligibility")?,
    );
    windows(net, 1, [0, 32]).await?;
    busy(net, 1, 1, None, 16).await?;
    let c2 = page(net, 1, 0, None, 16).await?;
    ensure!(
        c2["F"] == json!([1, 1]),
        "roleB sibling physical0 sensitivity"
    );
    drop(held.take());
    windows(net, 1, [0, 0]).await?;
    ensure!(
        page(net, 1, 1, None, 16).await?["items"]
            .as_array()
            .is_some_and(|v| v.len() == 16),
        "roleB free-window history"
    );
    ensure!(
        net.handles[0].call(|o| observed_at(o, 0)).await? == c2_before,
        "C2 storage unchanged"
    );
    ensure!(
        net.handles[1].call(|o| observed_at(o, 0)).await? == c2_b_before,
        "roleB C2 storage unchanged"
    );
    ensure!(
        native_call_counts() == before && stats(net).await? == counters,
        "history admission/read/release invokes zero native work"
    );
    println!(
        "s54_history_owner_A real_storage=true IPC_handler=true C1_slot=1 C2_slot=0 roles=0,1 shares=64,64 held=32 released=0 first=16 suffix=24 genuine_sources=40 cuts=5,3 frontier=7,35 native_delta=0 CLI_proof=false Controller_proof=false"
    );
    Ok(())
}
