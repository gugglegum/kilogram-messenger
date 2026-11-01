//! Actual four-row Owner7 calls, original Seen and bounded failure recovery.
use super::*;
use kilogram_ratchet::{
    ordered::NativeCallCounts,
    ordered_v2::{self as g2, Seen, SeenObservation},
};

const CASE: &str = "C-s53-seen-batch-r1-01";

fn fingerprint(raw: &[u8]) -> String {
    state::hex(&wire::hash(b"", raw))
}
fn request(seed: u64, index: usize) -> [u8; 16] {
    let mut id = [0xa3; 16];
    id[..8].copy_from_slice(&seed.to_be_bytes());
    id[8..].copy_from_slice(&(index as u64).to_be_bytes());
    id
}
fn held(path: &Path) -> Result<()> {
    std::fs::write(
        path,
        br#"{"suspend_drain":true,"suspend_materialization":true}"#,
    )?;
    Ok(())
}
fn opportunity<T>(path: &Path, receive: bool, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let prior = std::fs::read(path)?;
    std::fs::write(
        path,
        serde_json::to_vec(&json!({
            "suspend_drain": !receive, "suspend_materialization": receive
        }))?,
    )?;
    let result = f();
    // Restore on both Ok and Err before the owner can run another job/tick.
    let restore = std::fs::write(path, prior);
    restore?;
    result
}
fn delta(before: NativeCallCounts, encrypt: u64, decrypt: u64) -> Result<Json> {
    let after = native_call_counts();
    ensure!(
        after.fresh_account == before.fresh_account
            && after.outbound_session == before.outbound_session
            && after.inbound_session == before.inbound_session
            && after.encrypt == before.encrypt + encrypt
            && after.decrypt == before.decrypt + decrypt,
        "exact isolated real Native calls, no constructors"
    );
    Ok(json!({"encrypt":encrypt,"decrypt":decrypt,"constructors":0}))
}
fn guard(owner: &mut Owner7) -> Result<(g2::Guard2, u64, u64)> {
    let head = owner.head(0)?;
    let native = owner.mixed_native(0, &head)?;
    Ok((
        native.guard,
        owner.repo()?.generation(),
        head.native_generation,
    ))
}
fn seen_bytes(seen: Seen) -> Vec<u8> {
    let mut raw = seen.generation.to_be_bytes().to_vec();
    raw.extend(seen.first);
    raw
}
fn catalog(owner: &mut Owner7) -> Result<BTreeMap<(u8, Id), Vec<u8>>> {
    let next = owner.repo()?.metadata(0)?.next;
    ensure!(next.iter().all(|n| *n < 128), "bounded fixture catalog");
    let mut entries = BTreeMap::new();
    for role in 0..2u8 {
        for ordinal in 0..next[role as usize] {
            let raw = owner
                .repo()?
                .event_at(0, role, ordinal)?
                .context("catalog source")?;
            let key = if wire::decode(&raw, v2::MAX_EVENT)?.fields(5)?[0].uint()? == 2 {
                v2::ParsedEvent2::parse(&raw)?.meta().key
            } else {
                Event::parse(&raw, owner.spec(0)?.peers[role as usize].cert.device_id())?.key
            };
            let value = owner
                .repo()?
                .seen(0, role, key)?
                .context("authenticated Seen")?;
            ensure!(value.len() == 40, "Seen encoding");
            entries.insert((role, key), value);
        }
        ensure!(
            entries.keys().filter(|(r, _)| *r == role).count() as u64
                == owner.repo()?.seen_count(0, role)?,
            "catalog covers every durable Seen entry"
        );
    }
    Ok(entries)
}
fn projection(owner: &mut Owner7) -> Result<Json> {
    let base = observed(owner)?;
    let counts = owner.repo()?.counts(0)?;
    ensure!(
        counts.queued <= 32 && counts.pending <= 32 && counts.debt <= 32,
        "complete bounded projection"
    );
    let queue: Vec<_> = owner
        .repo()?
        .queue_page(0, None, 32)?
        .into_iter()
        .map(|q| json!([q.sequence, q.request, fingerprint(&q.sealed)]))
        .collect();
    let pending: Vec<_> = owner
        .repo()?
        .pending_page(0, None, 32)?
        .into_iter()
        .map(|p| {
            json!([
                p.ordinal,
                fingerprint(&p.raw),
                fingerprint(&p.state),
                p.head,
                p.quarantined,
                p.original,
                p.admission
            ])
        })
        .collect();
    let seen: Vec<_> = catalog(owner)?
        .into_iter()
        .map(|((r, k), v)| json!([r, k, v]))
        .collect();
    let mut history = Vec::<Json>::new();
    let mut cursor = None;
    let mut finished = false;
    for _ in 0..8 {
        let page: Json = serde_json::from_slice(&owner.repo()?.history(0, cursor.as_ref(), 32)?)?;
        history.extend(
            page["items"]
                .as_array()
                .context("history items")?
                .iter()
                .cloned(),
        );
        if page["finished"] == true {
            finished = true;
            break;
        }
        cursor = Some(db::HistoryCursor::from_base64(
            page["cursor"].as_str().context("history cursor")?,
        )?);
    }
    ensure!(finished && history.len() < 128, "complete bounded history");
    let next = owner.repo()?.metadata(0)?.next;
    let mut sources = Vec::new();
    for role in 0..2u8 {
        for ordinal in 0..next[role as usize] + 4 {
            sources.push(json!([
                role,
                ordinal,
                owner
                    .repo()?
                    .event_at(0, role, ordinal)?
                    .map(|v| fingerprint(&v)),
                owner
                    .repo()?
                    .receipt(0, role, ordinal)?
                    .map(|v| fingerprint(&v))
            ]));
        }
    }
    let repair = owner.repo()?.repair_state(0)?;
    Ok(
        json!({"base":base,"queue":queue,"pending":pending,"seen":seen,
        "debt":owner.repo()?.debt_page(0,None,32)?,"history":history,"sources_receipts":sources,
        "next_author":owner.repo()?.next_author_sequence()?,
        "repair":[repair.repair_free,repair.outstanding,repair.pending,repair.head,repair.grant]}),
    )
}
fn witness(value: &Json) -> Result<String> {
    Ok(fingerprint(&serde_json::to_vec(value)?))
}
fn reopen(profiles: &[PathBuf; 2], c: Id) -> Result<[Owner7; 2]> {
    let mut out = Vec::new();
    for path in profiles {
        let p = Profile3::load(path)?;
        out.push(Owner7::new(p.open(path)?, &[c])?);
    }
    out.try_into()
        .map_err(|_| anyhow::anyhow!("two reopened owners"))
}
fn queue(owner: &mut Owner7, seed: u64, count: usize) -> Result<()> {
    ensure!(
        owner.repo()?.counts(0)?.queued == 0,
        "exclusive queue before batch"
    );
    for i in 0..count {
        owner.queue(0, request(seed, i), format!("seen-batch/{seed}/{i}"))?;
    }
    ensure!(
        owner.repo()?.counts(0)?.queued == count as u64,
        "whole batch queued before opportunity"
    );
    Ok(())
}
struct Batch {
    role: usize,
    seed: u64,
    events: Vec<v2::ParsedEvent2>,
    before: (g2::Guard2, u64, u64),
    evidence: Json,
}
impl Batch {
    fn first(&self) -> u64 {
        self.events[0].meta().n
    }
}
fn materialize(owner: &mut Owner7, path: &Path, seed: u64, count: usize) -> Result<Batch> {
    let before = guard(owner)?;
    let role = owner.role(0)? as usize;
    let old = catalog(owner)?;
    let debt = owner.repo()?.counts(0)?.debt;
    ensure!(
        owner.repo()?.counts(0)?.queued == count as u64,
        "all rows queued together"
    );
    let calls = native_call_counts();
    ensure!(
        opportunity(path, false, || owner.materialize(0))?,
        "actual materialize call"
    );
    let calls = delta(calls, count as u64, 0)?;
    let mut events = Vec::new();
    for i in 0..count {
        let raw = owner
            .repo()?
            .event_at(0, role as u8, before.0.own().next + i as u64)?
            .context("new committed event")?;
        events.push(v2::ParsedEvent2::parse(&raw)?);
    }
    let first = &events[0];
    let new = before.0.rotate_due();
    let retained = if new {
        ensure!(
            Some(first.meta().key) != before.0.own().key
                && !old.contains_key(&(role as u8, first.meta().key)),
            "genuine new packet key"
        );
        ensure!(
            first.meta().expected.g == before.0.own().g + 1 && first.meta().index == 0,
            "successor and actual index0"
        );
        Seen {
            generation: first.meta().expected.g,
            first: first.id(),
        }
    } else {
        let lane = before.0.own();
        let s = Seen {
            generation: lane.g,
            first: lane.first.context("original first")?,
        };
        ensure!(
            Some(first.meta().key) == lane.key && first.id() != s.first,
            "continuation of old first"
        );
        ensure!(
            old.get(&(role as u8, first.meta().key)) == Some(&seen_bytes(s)),
            "durable old Seen witness"
        );
        s
    };
    for (i, event) in events.iter().enumerate() {
        let meta = event.meta();
        ensure!(
            meta.key == first.meta().key
                && meta.expected.g == retained.generation
                && meta.index == first.meta().index + i as u64,
            "actual same-key batch indexes"
        );
        ensure!(
            owner
                .repo()?
                .request(0, request(seed, i))?
                .context("request result")?
                .event
                == Some(event.id()),
            "real request/event mapping"
        );
    }
    let mut expected = old.clone();
    expected
        .entry((role as u8, first.meta().key))
        .or_insert(seen_bytes(retained));
    ensure!(
        catalog(owner)? == expected && expected.len() == old.len() + usize::from(new),
        "all original Seen unchanged, one only for new key"
    );
    let after = guard(owner)?.0;
    let counts = owner.repo()?.counts(0)?;
    ensure!(
        after.own().next == before.0.own().next + count as u64
            && counts.queued == 0
            && counts.debt == debt + count as u64
            && after.own().first == Some(retained.first),
        "four-row committed projection"
    );
    let idle = native_call_counts();
    ensure!(
        !opportunity(path, false, || owner.materialize(0))?,
        "empty queue no-op"
    );
    delta(idle, 0, 0)?;
    let evidence = json!({"rows":count,"new_key":new,"calls":calls,"seen_before":old.len(),"seen_after":expected.len(),
        "persisted_seen":fingerprint(&seen_bytes(retained)),"guard":fingerprint(&after.encode()?),"generation":retained.generation});
    Ok(Batch {
        role,
        seed,
        events,
        before,
        evidence,
    })
}
fn pending(owner: &mut Owner7, batch: &Batch) -> Result<()> {
    ensure!(
        owner.repo()?.counts(0)?.pending == batch.events.len() as u64,
        "exact complete Pending batch"
    );
    ensure!(
        owner.repo()?.metadata(0)?.next[batch.role] == batch.first(),
        "contiguous head before drain"
    );
    for event in &batch.events {
        let p = owner
            .repo()?
            .pending(0, event.meta().n)?
            .context("contiguous actual Pending")?;
        ensure!(
            !p.quarantined && p.raw == event.raw(),
            "original source held before work"
        );
    }
    Ok(())
}
fn drain(owner: &mut Owner7, path: &Path, batch: &Batch) -> Result<Json> {
    pending(owner, batch)?;
    let old = catalog(owner)?;
    let first = &batch.events[0];
    let old_guard = guard(owner)?.0;
    let new = old_guard.peer().key != Some(first.meta().key);
    let retained = if new {
        Seen {
            generation: first.meta().expected.g,
            first: first.id(),
        }
    } else {
        Seen {
            generation: old_guard.peer().g,
            first: old_guard.peer().first.context("peer original first")?,
        }
    };
    let calls = native_call_counts();
    ensure!(
        opportunity(path, true, || owner.drain(0))?,
        "single actual drain call"
    );
    let calls = delta(calls, 0, batch.events.len() as u64)?;
    ensure!(
        owner.repo()?.counts(0)?.pending == 0
            && owner.repo()?.metadata(0)?.next[batch.role]
                == batch.first() + batch.events.len() as u64,
        "whole batch readable without quarantine"
    );
    let mut expected = old.clone();
    expected
        .entry((batch.role as u8, first.meta().key))
        .or_insert(seen_bytes(retained));
    ensure!(
        catalog(owner)? == expected && expected.len() == old.len() + usize::from(new),
        "receive preserves old Seen"
    );
    let view = projection(owner)?;
    for (i, event) in batch.events.iter().enumerate() {
        ensure!(
            owner
                .repo()?
                .event_at(0, batch.role as u8, event.meta().n)?
                .as_deref()
                == Some(event.raw())
                && owner
                    .repo()?
                    .receipt(0, batch.role as u8, event.meta().n)?
                    .is_some(),
            "readable source and real receipt"
        );
        ensure!(
            view["history"]
                .as_array()
                .context("readable history")?
                .iter()
                .any(|row| row["text"] == format!("seen-batch/{}/{i}", batch.seed)),
            "actual decrypted plaintext"
        );
    }
    Ok(
        json!({"rows":batch.events.len(),"calls":calls,"new_key":new,
        "persisted_seen":fingerprint(&seen_bytes(retained)),"durable_projection":witness(&view)?,"quarantine":0}),
    )
}
async fn deliver(net: &Net, path: &Path, batch: &Batch) -> Result<Json> {
    let receiver = 1 - batch.role;
    let key = batch.events[0].meta().key;
    if batch.evidence["new_key"] == true {
        let role = batch.role as u8;
        ensure!(
            net.handles[receiver]
                .call(move |o| Ok(o.repo()?.seen(0, role, key)?.is_none()))
                .await?,
            "receiver Absent before new-key source admission"
        );
    }
    net.deliver(batch.role, batch.first()).await?;
    let schedule = path.to_path_buf();
    let batch = Batch {
        role: batch.role,
        seed: batch.seed,
        events: batch.events.clone(),
        before: batch.before.clone(),
        evidence: batch.evidence.clone(),
    };
    let received = net.handles[receiver]
        .call(move |o| drain(o, &schedule, &batch))
        .await?;
    net.acks(1 - receiver).await?;
    Ok(received)
}
fn fault(owner: &mut Owner7, path: &Path, receive: bool) -> Result<Json> {
    let old = projection(owner)?;
    let calls = native_call_counts();
    owner
        .repo()?
        .verification_existing_io_fault("before_write", 0)?;
    let result = opportunity(path, receive, || {
        if receive {
            owner.drain(0)
        } else {
            owner.materialize(0)
        }
    });
    ensure!(
        result.is_err() && owner.repo()?.requires_reopen(),
        "real I/O failure poisons the owner"
    );
    let calls = delta(
        calls,
        if receive { 0 } else { 4 },
        if receive { 4 } else { 0 },
    )?;
    Ok(json!({"old":old,"calls":calls,"error_observed":true,"poisoned":true}))
}
fn negative_guards(receiver: &mut Owner7, batch: &Batch) -> Result<Json> {
    let (peer, root, native) = guard(receiver)?;
    let meta = batch.events[0].meta();
    let expected = Seen {
        generation: peer.peer().g,
        first: peer.peer().first.context("negative original first")?,
    };
    let wrongs = [
        Seen {
            generation: expected.generation + 1,
            ..expected
        },
        Seen {
            first: [0xff; 32],
            ..expected
        },
    ];
    ensure!(expected.first != wrongs[1].first, "distinct negative first");
    let calls = native_call_counts();
    for wrong in wrongs {
        let mut sender = batch.before.0.clone();
        let send_before = sender.encode()?;
        let step = sender.preflight_send(&meta, batch.before.1, batch.before.2)?;
        ensure!(
            sender
                .apply(
                    step,
                    &meta,
                    batch.events[0].id(),
                    SeenObservation::Present(wrong)
                )
                .is_err()
                && sender.encode()? == send_before,
            "send rejects conflicting Seen without mutation"
        );
        let before = peer.encode()?;
        ensure!(
            peer.preflight_receive(&meta, SeenObservation::Present(wrong), root, native)
                .is_err()
                && peer.encode()? == before,
            "receive rejects conflicting Seen without mutation"
        );
    }
    delta(calls, 0, 0)?;
    Ok(
        json!({"wrong_generation_rejected":true,"wrong_first_rejected":true,"guard_unchanged":true,"native_calls":0}),
    )
}

#[test]
fn s53_seen_batch_real_callers_core() -> Result<()> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(90);
    ensure!(
        std::env::var("KILOGRAM_S53_CASE")? == CASE,
        "bound case before owned thread"
    );
    let joined = std::thread::Builder::new()
        .name("s53-seen-batch-case".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            ensure!(Instant::now() < deadline, "absolute startup deadline");
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .thread_stack_size(2 * 1024 * 1024)
                .enable_all()
                .build()?;
            rt.block_on(Box::pin(work(started, deadline)))
        })?
        .join();
    match joined {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
async fn work(started: Instant, deadline: Instant) -> Result<()> {
    let binding = Binding::managed(CASE)?;
    let schedule = binding.run.join("seen-batch.schedule.json");
    let clock = binding.run.join("seen-batch.clock");
    ensure!(
        std::env::var_os("KILOGRAM_TEST_OD1_SCHEDULE").as_deref() == Some(schedule.as_os_str())
            && std::env::var_os("KILOGRAM_TEST_OD1_CLOCK_FILE").as_deref()
                == Some(clock.as_os_str())
            && std::fs::read_to_string(clock)?.trim() == "0",
        "owned existing controller plumbing"
    );
    let mut fixture = Fixture::new(&binding.run, false)?;
    let lineage = [
        Lineage::capture(&mut fixture.owners[0])?,
        Lineage::capture(&mut fixture.owners[1])?,
    ];
    fixture.owners[0].existing(0, |s| s.request_upgrade(0, request(1, 0)))?;
    fixture.complete()?;
    held(&schedule)?;
    let profiles = fixture.profiles.clone();
    let c = fixture.c;
    let mut owners = fixture.owners;
    let calls = native_call_counts();
    let role = (0..2)
        .find_map(|r| match guard(&mut owners[r]) {
            Ok((g, _, _)) if !g.rotate_due() => Some(Ok(r)),
            Err(e) => Some(Err(e)),
            _ => None,
        })
        .context("observed established-key lane")??;
    let mut evidence = Vec::new();
    queue(&mut owners[role], 10, 4)?;
    let first = materialize(&mut owners[role], &schedule, 10, 4)?;
    ensure!(first.evidence["new_key"] == false, "existing-key baseline");
    evidence.push(json!({"phase":"existing-send","result":first.evidence}));
    let net = Net::new(owners, c, deadline).await?;
    evidence
        .push(json!({"phase":"existing-receive","result":deliver(&net,&schedule,&first).await?}));
    let persisted = [
        net.handles[0].call(projection).await?,
        net.handles[1].call(projection).await?,
    ];
    drop(net);
    owners = reopen(&profiles, c)?;
    for r in 0..2 {
        ensure!(
            projection(&mut owners[r])? == persisted[r],
            "complete cold projection"
        );
        lineage[r].check(&mut owners[r])?;
    }
    evidence.push(json!({"phase":"cold-reopen","exact_old_projection":true,"witnesses":[witness(&persisted[0])?,witness(&persisted[1])?]}));
    queue(&mut owners[role], 20, 4)?;
    let second = materialize(&mut owners[role], &schedule, 20, 4)?;
    ensure!(
        second.evidence["new_key"] == false,
        "fresh overlay after reopen uses old first"
    );
    let net = Net::new(owners, c, deadline).await?;
    evidence.push(json!({"phase":"reopened-batch","sender_role":second.role,"receiver_role":1-second.role,"cold_witnesses":[witness(&persisted[0])?,witness(&persisted[1])?],"send":second.evidence,"receive":deliver(&net,&schedule,&second).await?}));
    let mut reopened_roles = [false; 2];
    reopened_roles[second.role] = true;
    drop(net);
    owners = reopen(&profiles, c)?;
    queue(&mut owners[role], 30, 4)?;
    let failed_send = fault(&mut owners[role], &schedule, false)?;
    drop(owners);
    owners = reopen(&profiles, c)?;
    ensure!(
        projection(&mut owners[role])? == failed_send["old"],
        "cold send rollback complete projection"
    );
    let retry = materialize(&mut owners[role], &schedule, 30, 4)?;
    evidence.push(json!({"phase":"send-fault","calls":failed_send["calls"],"poisoned":true,"cold_old_projection":witness(&failed_send["old"])?,"retry":retry.evidence}));
    let net = Net::new(owners, c, deadline).await?;
    net.deliver(role, retry.first()).await?;
    let path = schedule.clone();
    let receive_batch = Batch {
        role: retry.role,
        seed: retry.seed,
        events: retry.events.clone(),
        before: retry.before.clone(),
        evidence: retry.evidence.clone(),
    };
    let failed_receive = net.handles[1 - role]
        .call(move |o| {
            pending(o, &receive_batch)?;
            fault(o, &path, true)
        })
        .await?;
    drop(net);
    owners = reopen(&profiles, c)?;
    ensure!(
        projection(&mut owners[1 - role])? == failed_receive["old"],
        "cold receive rollback complete projection"
    );
    let admission_calls = native_call_counts();
    let net = Net::new(owners, c, deadline).await?;
    delta(admission_calls, 0, 0)?;
    let path = schedule.clone();
    let received = net.handles[1 - role]
        .call(move |o| drain(o, &path, &retry))
        .await?;
    net.acks(role).await?;
    evidence.push(json!({"phase":"receive-fault","calls":failed_receive["calls"],"poisoned":true,"cold_old_projection":witness(&failed_receive["old"])?,"readmission_without_native":true,"retry":received}));
    // One normal observed duplex turn, then the opposite lane's genuine rotation.
    let mut rotating = None;
    for r in 0..2 {
        if net.handles[r]
            .call(|o| Ok(guard(o)?.0.rotate_due()))
            .await?
        {
            rotating = Some(r);
            break;
        }
    }
    let warm_role = rotating.context("observed genuine duplex rotation opportunity")?;
    let path = schedule.clone();
    let warm = net.handles[warm_role]
        .call(move |o| {
            queue(o, 40, 1)?;
            materialize(o, &path, 40, 1)
        })
        .await?;
    ensure!(
        warm.evidence["new_key"] == true,
        "ordinary warm-up actually rotates"
    );
    let warm_received = deliver(&net, &schedule, &warm).await?;
    evidence.push(json!({"phase":"ordinary-duplex-warmup-excluded-from-batch-baseline","send":warm.evidence,"receive":warm_received}));
    ensure!(
        warm_role == 1 - role,
        "observed warm-up is the reverse direction"
    );
    let warm_key = warm.events[0].meta().key;
    let warm_generation = warm.events[0].meta().expected.g;
    let warm_first = warm.events[0].id();
    net.handles[warm_role]
        .call(move |o| {
            let g = guard(o)?.0;
            ensure!(
                !g.rotate_due()
                    && g.own().key == Some(warm_key)
                    && g.own().g == warm_generation
                    && g.own().first == Some(warm_first),
                "observed established reverse key before cold reopen"
            );
            Ok(())
        })
        .await?;
    let reverse_persisted = [
        net.handles[0].call(projection).await?,
        net.handles[1].call(projection).await?,
    ];
    drop(net);
    owners = reopen(&profiles, c)?;
    for r in 0..2 {
        ensure!(
            projection(&mut owners[r])? == reverse_persisted[r],
            "reverse cold projection before any locator writes"
        );
        // Retain the original pre-conversion lineage across this cold boundary.
        lineage[r].check(&mut owners[r])?;
    }
    queue(&mut owners[warm_role], 45, 4)?;
    let reverse = materialize(&mut owners[warm_role], &schedule, 45, 4)?;
    ensure!(
        reverse.role == warm_role
            && reverse.evidence["new_key"] == false
            && reverse.events[0].meta().key == warm_key
            && reverse.events[0].meta().expected.g == warm_generation
            && reverse.before.0.own().first == Some(warm_first),
        "four reverse continuations retain the actual pre-reopen key/generation/first"
    );
    let net = Net::new(owners, c, deadline).await?;
    let reverse_received = deliver(&net, &schedule, &reverse).await?;
    for handle in &net.handles {
        handle
            .call(|o| {
                let counts = o.repo()?.counts(0)?;
                ensure!(
                    counts.debt == 0 && counts.pending == 0 && counts.queued == 0,
                    "complete reverse ACK handling and no unrelated Native work"
                );
                Ok(())
            })
            .await?;
    }
    reopened_roles[reverse.role] = true;
    ensure!(
        reopened_roles == [true, true],
        "both sender roles completed reopened four-row continuations"
    );
    evidence.push(json!({"phase":"reverse-reopened-batch","sender_role":reverse.role,"receiver_role":1-reverse.role,
        "cold_witnesses":[witness(&reverse_persisted[0])?,witness(&reverse_persisted[1])?],
        "original_lineage":true,"send":reverse.evidence,"receive":reverse_received,
        "both_roles":reopened_roles,"ack_debt":0,"pending":0,"queued":0}));
    let new_role = 1 - warm_role;
    ensure!(
        net.handles[new_role]
            .call(|o| Ok(guard(o)?.0.rotate_due()))
            .await?,
        "real receive causes next rotation"
    );
    let path = schedule.clone();
    let new = net.handles[new_role]
        .call(move |o| {
            queue(o, 50, 4)?;
            materialize(o, &path, 50, 4)
        })
        .await?;
    ensure!(
        new.evidence["new_key"] == true,
        "actual new-key first plus three continuations"
    );
    evidence.push(json!({"phase":"genuine-new-key-batch","send":new.evidence,"receive":deliver(&net,&schedule,&new).await?}));
    let persisted = [
        net.handles[0].call(projection).await?,
        net.handles[1].call(projection).await?,
    ];
    drop(net);
    owners = reopen(&profiles, c)?;
    for r in 0..2 {
        ensure!(
            projection(&mut owners[r])? == persisted[r],
            "new first and old entries persist across reopen"
        );
        lineage[r].check(&mut owners[r])?;
    }
    queue(&mut owners[new_role], 60, 4)?;
    let continued = materialize(&mut owners[new_role], &schedule, 60, 4)?;
    ensure!(
        continued.evidence["new_key"] == false,
        "reopened new key continues original first"
    );
    let net = Net::new(owners, c, deadline).await?;
    evidence.push(json!({"phase":"new-key-cold-continuation","send":continued.evidence,"receive":deliver(&net,&schedule,&continued).await?,"cold_seen_preserved":true}));
    let path = schedule.clone();
    let negative = net.handles[new_role]
        .call(move |o| {
            queue(o, 70, 4)?;
            materialize(o, &path, 70, 4)
        })
        .await?;
    ensure!(
        !negative.before.0.rotate_due() && negative.evidence["new_key"] == false,
        "conflicting-input stage starts with existing-key continuations"
    );
    let receiver = 1 - new_role;
    let batch_copy = Batch {
        role: negative.role,
        seed: negative.seed,
        events: negative.events.clone(),
        before: negative.before.clone(),
        evidence: negative.evidence.clone(),
    };
    let negatives = net.handles[receiver]
        .call(move |o| negative_guards(o, &batch_copy))
        .await?;
    let originals = negative.events.clone();
    let (items, auth) = net.handles[new_role]
        .call(move |o| {
            let mut rows: Vec<_> = originals
                .iter()
                .map(|e| v2::TaggedItem {
                    format: 2,
                    raw: e.raw().to_vec(),
                })
                .collect();
            let mut core = originals[3].core().clone();
            core.expected.g += 1;
            let bad = v2::sign_event(core, &originals[3].packet(), o.device.identity())?;
            ensure!(
                bad.packet().bytes == originals[3].packet().bytes
                    && bad.meta().key == originals[3].meta().key
                    && bad.meta().index == originals[3].meta().index,
                "only core generation/resign, same real packet"
            );
            rows[3].raw = bad.raw().to_vec();
            let reference = originals[0].core().author_auth;
            Ok((
                rows,
                BTreeMap::from([(
                    reference,
                    o.repo()?
                        .auth(0, reference)?
                        .context("genuine authority catalog")?,
                )]),
            ))
        })
        .await?;
    let bound = net.binding(receiver).await?;
    net.handles[receiver]
        .call(move |o| o.receive2(0, bound, items, auth, None, false).map(|_| ()))
        .await?;
    let path = schedule.clone();
    let quarantine = net.handles[receiver].call(move |o| {
        ensure!(o.repo()?.counts(0)?.pending == 4,"four genuine staged source rows");
        for event in &negative.events[..3] {let p=o.repo()?.pending(0,event.meta().n)?.context("valid pending prefix")?;ensure!(p.raw==event.raw() && !p.quarantined,"valid original prefix");}
        let bad_ordinal = negative.first()+3;
        ensure!(o.repo()?.pending(0,bad_ordinal)?.context("signed negative pending")?.raw != negative.events[3].raw(),"original fourth never admitted");
        let old = catalog(o)?;
        let root = o.repo()?.generation();
        let before = native_call_counts();
        ensure!(opportunity(&path,true,||o.drain(0))?,"valid prefix processed");
        let calls = delta(before,0,3)?;
        ensure!(o.repo()?.generation()==root+2 && o.repo()?.metadata(0)?.next[new_role]==bad_ordinal
            && o.repo()?.counts(0)?.pending==1 && catalog(o)?==old,"prefix Native commit plus separate quarantine");
        ensure!(o.repo()?.pending(0,bad_ordinal)?.context("quarantined bad fourth")?.quarantined
            && o.repo()?.receipt(0,new_role as u8,bad_ordinal)?.is_none(),"no fourth decrypt/receipt");
        for event in &negative.events[..3] {ensure!(o.repo()?.event_at(0,new_role as u8,event.meta().n)?.as_deref()==Some(event.raw()) && o.repo()?.receipt(0,new_role as u8,event.meta().n)?.is_some(),"three committed valid sources/receipts");}
        let before = native_call_counts();
        ensure!(!opportunity(&path,true,||o.drain(0))?,"quarantined head blocks repeat work");
        delta(before,0,0)?;
        Ok(json!({"prefix":3,"calls":calls,"quarantined":1,"fourth_decrypt":0,"fourth_receipt":false,"commits":2,"repeat_native":0,"seen_unchanged":true}))
    }).await?;
    evidence.push(
        json!({"phase":"conflicting-input","guard_negatives":negatives,"quarantine":quarantine}),
    );
    let after = native_call_counts();
    ensure!(
        calls.fresh_account == after.fresh_account
            && calls.outbound_session == after.outbound_session
            && calls.inbound_session == after.inbound_session,
        "original Native throughout all subcases"
    );
    for (r, original) in lineage.into_iter().enumerate() {
        net.handles[r].call(move |o| original.check(o)).await?;
    }
    drop(net);
    ensure!(
        Instant::now() < deadline,
        "absolute whole kernel deadline includes cleanup"
    );
    binding.finish(json!({"classification":"real Owner7 batches; genuine TLS2 leases; original Native and Seen",
        "subcases":evidence,"reopened_continuation_roles":reopened_roles,"constructors_after_fixture":0,"elapsed_seconds":started.elapsed().as_secs_f64()}),||Ok(()))
}
