//! One controlled pre-admission B request. No scheduler or product overrides.
use super::*;
use std::io::Read;

const CASE: &str = "F-s53-b-before-alpn2-two-c-01";
const TRACE_CAP: usize = 1024 * 1024;

const STATUS_CAP: usize = 48 * 1024;

fn status_identity(v: &Value, m: &oracle::Mapping) -> Result<()> {
    ensure!(
        v.is_object() && v["version"] == 3,
        "upgrade status object version3"
    );
    ensure!(
        v["context"] == m.context && v["slot"] == m.slot,
        "actual upgrade context/slot"
    );
    Ok(())
}
fn unselected(v: &Value) -> Result<()> {
    ensure!(
        v["mode"] == "L1"
            && v["lease_current"] == false
            && oracle::field(v, "policy_error")?.is_null(),
        "L1 without current family2 admission"
    );
    for k in ["p", "a", "k", "committed"] {
        ensure!(
            oracle::field(&v["controls"], k)?.is_null(),
            "no selected control before imports"
        );
    }
    ensure!(
        oracle::field(v, "q")?.is_null()
            && v["cuts"] == json!([null, null])
            && v["freeze"] == false
            && v["control_debt"] == false,
        "unselected status without control debt"
    );
    Ok(())
}
/// UpgradeStatus prints its UTF8 JSON object directly (ordered3_cli.rs).
fn parse_upgrade_status(raw: &str, m: &oracle::Mapping) -> Result<Value> {
    ensure!(raw.len() <= STATUS_CAP, "bounded UpgradeStatus object");
    let v: Value = serde_json::from_str(raw)?;
    status_identity(&v, m)?;
    ensure!(
        m.owner == 0 && m.role == 0 && v["status"] == "NoLocalIntent",
        "pre-B actual Y/A has no local intent"
    );
    unselected(&v)?;
    Ok(v)
}
/// Upgrade falls through the CLI Value::B -> hex JSON string printer.
fn decode_upgrade_reply(raw: &str, m: &oracle::Mapping) -> Result<Value> {
    ensure!(raw.len() <= 2 * STATUS_CAP + 3, "bounded Upgrade hex reply");
    let encoded: Value = serde_json::from_str(raw)?;
    let s = encoded.as_str().context("upgrade JSON hex")?;
    ensure!(
        !s.is_empty() && s.len().is_multiple_of(2),
        "upgrade hex length"
    );
    let bytes = s
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Ok(u8::from_str_radix(std::str::from_utf8(p)?, 16)?))
        .collect::<Result<Vec<_>>>()?;
    let v = serde_json::from_slice(&bytes)?;
    status_identity(&v, m)?;
    Ok(v)
}
fn pending(v: &Value, m: &oracle::Mapping) -> Result<()> {
    ensure!(m.owner == 1 && m.role == 1, "actual C1 B owner");
    status_identity(v, m)?;
    unselected(v)?;
    ensure!(
        v["status"] == "NotificationPending",
        "serialized pre-admission B intent"
    );
    Ok(())
}
fn selected(full: &[Option<Value>; 3], maps: &[oracle::Mapping], c: &str) -> Result<()> {
    let peers = maps.iter().filter(|m| m.context == c).collect::<Vec<_>>();
    ensure!(
        peers.len() == 2 && peers[0].role != peers[1].role,
        "selected mapped peers"
    );
    let a = full[peers[0].owner].as_ref().context("selected owner A")?;
    let b = full[peers[1].owner].as_ref().context("selected owner B")?;
    oracle::admitted(a, c)?;
    oracle::admitted(b, c)?;
    let a = oracle::row(a, c)?;
    let b = oracle::row(b, c)?;
    for row in [a, b] {
        ensure!(
            row["control_debt"] == false,
            "selected control debt drained"
        );
        for count in ["queued", "pending", "debt"] {
            ensure!(row["counts"][count] == 0, "selected {count} drained");
        }
    }
    for k in ["d", "e", "sid", "q", "cuts", "controls", "current_consent2"] {
        let v = oracle::field(a, k)?;
        ensure!(!v.is_null(), "selected identity field {k}");
        oracle::exact(k, v, oracle::field(b, k)?)?;
    }
    for k in ["p", "a", "k", "committed"] {
        ensure!(
            a["controls"][k].is_string(),
            "complete selected control {k}"
        );
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct Trace {
    owner: usize,
    path: PathBuf,
    complete_prefix: String,
    complete_bytes: usize,
    partial_tail_bytes: usize,
    prefix_blake3: String,
    records: Vec<Value>,
}
fn parse_trace(raw: &[u8], owner: usize, path: &Path, maps: &[oracle::Mapping]) -> Result<Trace> {
    ensure!(raw.len() <= TRACE_CAP, "bounded current-process trace");
    let end = raw.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let prefix = std::str::from_utf8(&raw[..end])?;
    let mut records = Vec::new();
    let mut stamps = std::collections::BTreeMap::<u8, u64>::new();
    for line in prefix.lines() {
        let f = line.split_ascii_whitespace().collect::<Vec<_>>();
        if !matches!(f.first().copied(), Some("s53_scheduler" | "s53_rpc")) {
            continue;
        }
        let slot: u8 = f
            .get(1)
            .and_then(|s| s.strip_prefix("slot="))
            .context("trace slot")?
            .parse()?;
        let matches = maps
            .iter()
            .filter(|m| m.owner == owner && m.slot == slot)
            .collect::<Vec<_>>();
        ensure!(matches.len() == 1, "unique trace owner/context/slot");
        let m = matches[0];
        if f[0] == "s53_rpc" {
            ensure!(f.len() == 3, "exact RPC trace record");
            let kind = f[2]
                .strip_prefix("kind=")
                .filter(|s| !s.is_empty())
                .context("RPC kind")?;
            records.push(json!({"context":m.context,"slot":slot,"kind":kind}));
        } else {
            let phase = f
                .get(2)
                .and_then(|s| s.strip_prefix("phase="))
                .context("scheduler phase")?;
            let resolved = phase == "probe-resolved";
            ensure!(
                matches!(
                    phase,
                    "ordinary-start" | "ordinary-finished" | "probe-start" | "probe-resolved"
                ) && f.len() == if resolved { 5 } else { 4 },
                "scheduler record shape"
            );
            if resolved {
                ensure!(
                    f[3].strip_prefix("mode=").is_some_and(|s| !s.is_empty()),
                    "resolved mode"
                );
            }
            let ms: u64 = f
                .last()
                .and_then(|s| s.strip_prefix("monotonic_ms="))
                .context("trace monotonic stamp")?
                .parse()?;
            ensure!(
                stamps.get(&slot).is_none_or(|p| ms >= *p),
                "monotonic per-coordinator trace; no restarted epoch"
            );
            stamps.insert(slot, ms);
            records.push(json!({"context":m.context,"slot":slot,"phase":phase,"monotonic_ms":ms,"line":line}));
        }
    }
    Ok(Trace {
        owner,
        path: path.into(),
        complete_prefix: prefix.into(),
        complete_bytes: end,
        partial_tail_bytes: raw.len() - end,
        prefix_blake3: hash(&raw[..end]),
        records,
    })
}
fn traces(nodes: &[Node; 3], maps: &[oracle::Mapping]) -> Result<Vec<Trace>> {
    nodes
        .iter()
        .enumerate()
        .map(|(owner, n)| {
            let path = n.branch.base.join(format!("selected-{owner}.stderr.log"));
            let mut raw = Vec::new();
            std::fs::File::open(&path)?
                .take((TRACE_CAP + 1) as u64)
                .read_to_end(&mut raw)?;
            parse_trace(&raw, owner, &path, maps)
        })
        .collect()
}
fn notification(traces: &[Trace], c: &str, before: u64, after: u64) -> Result<()> {
    ensure!(after > before, "attributed Y OpenIntent receipt increase");
    ensure!(
        traces.iter().any(|t| t.owner == 1
            && t.records
                .iter()
                .any(|r| r["context"] == c && r["kind"] == "OpenIntent")),
        "actual C1 B OpenIntent attempt"
    );
    ensure!(
        traces.iter().any(|t| t.owner == 0
            && t.records
                .iter()
                .any(|r| r["context"] == c && r["phase"] == "probe-start")),
        "actual Y C1 discovery"
    );
    Ok(())
}
fn receipt(s: &Value) -> Result<u64> {
    oracle::number(&s["stats"], "open_intent_received")
}

#[derive(Default, serde::Serialize)]
struct Observed {
    phase: String,
    statuses: [Option<Value>; 3],
    traces: Vec<Trace>,
    transcript: Vec<Value>,
    tb_ms: Option<u128>,
    tr_ms: Option<u128>,
    active_ms: Option<u128>,
    last_error: Option<String>,
}
fn persist(run: &Path, name: &str, mut value: Value) -> Result<()> {
    // Also usable after a local deadline: cached public evidence only, no IPC.
    public_fingerprints(&mut value)?;
    let raw = serde_json::to_vec(&value)?;
    ensure!(
        raw.len() <= 6 * TRACE_CAP,
        "bounded B-start public evidence"
    );
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(run.join(name))?;
    f.write_all(&raw)?;
    f.sync_all()?;
    Ok(())
}
fn record(managed: &Managed, o: &Observed, name: &str) -> Result<()> {
    persist(
        &managed.run,
        &format!("b-start-{name}.json"),
        serde_json::to_value(o)?,
    )
}
fn poll(f: &mut Initial, o: &mut Observed) -> Result<()> {
    f.owners.alive()?;
    // Update one at a time: if the next command blocks, earlier samples survive.
    for (i, n) in f.nodes.iter().enumerate() {
        o.statuses[i] = Some(n.status()?);
    }
    o.traces = traces(&f.nodes, &f.maps)?;
    s53_deadline::check()?;
    Ok(())
}
fn upgrade(
    pair: &Pair,
    role: usize,
    id: u8,
    o: &mut Observed,
    m: &oracle::Mapping,
) -> Result<Value> {
    let n = &pair.nodes[role];
    o.transcript.push(json!({"command":"upgrade","context":pair.c,"slot":n.slot,"ipc":n.ipc,"request":format!("{id:02x}").repeat(16)}));
    let raw = pair.command(
        role,
        "upgrade",
        &["--request-id".into(), format!("{id:02x}").repeat(16)],
    )?;
    o.transcript.push(json!({"raw_upgrade_reply":raw}));
    let v = decode_upgrade_reply(&raw, m)?;
    o.transcript.push(json!({"reply":v}));
    Ok(v)
}
fn exchange_recorded(f: &mut Initial, first: bool, o: &mut Observed) -> Result<()> {
    let pair = if first { &mut f.first } else { &mut f.second };
    f.owners.wait(15, || {
        Ok(pair
            .nodes
            .iter()
            .all(|n| n.locators.join(format!("{}.locator2", pair.c)).is_file())
            .then_some(()))
    })?;
    // Preserve original A-import-then-B-import ordering. The B reply gate is
    // checked by the caller before this function is first reachable for C1.
    for role in 0..2 {
        let peer = &pair.nodes[1 - role];
        let path = peer.locators.join(format!("{}.locator2", pair.c));
        let raw = std::fs::read(&path)?;
        let parsed = wire::ParsedRuntimeLocator2::parse(&raw)?;
        let fields = parsed.fields();
        ensure!(
            fields.c == state::parse_id(&pair.c)?,
            "actual locator context"
        );
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        ensure!(
            fields.not_before <= wall && wall < fields.expires,
            "current public locator interval"
        );
        o.transcript.push(json!({"command":"locator2-import","context":pair.c,"role":role,"slot":pair.nodes[role].slot,"ipc":pair.nodes[role].ipc,"path":path,"hash":hash(&raw),"endpoint":state::hex(&fields.endpoint),"revision":fields.revision}));
        let result = pair.command(
            role,
            "locator2-import",
            &["--existing".into(), "--locator-file".into(), text(&path)],
        )?;
        o.transcript.push(json!({"import_reply":result}));
        o.traces = traces(&f.nodes, &f.maps)?;
    }
    Ok(())
}
fn unchanged_c2(a: &Live, b: &Live, maps: &[oracle::Mapping], c: &str) -> Result<()> {
    for m in maps.iter().filter(|m| m.context == c) {
        oracle::exact(
            "C2 selected/frontier",
            &oracle::stable(oracle::row(a.status(m.owner)?, c)?)?,
            &oracle::stable(oracle::row(b.status(m.owner)?, c)?)?,
        )?;
        ensure!(
            a.history[&key(m.owner, c)] == b.history[&key(m.owner, c)],
            "no C1 leakage into C2 history"
        );
    }
    Ok(())
}
fn execute(
    managed: &Managed,
    f: &mut Initial,
    o: &mut Observed,
    started: Instant,
    preparation: Instant,
) -> Result<Value> {
    let prep = s53_deadline::Phase::until(preparation)?;
    o.phase = "pre-B".into();
    poll(f, o)?;
    for (owner, c, role) in [(0, 0, 0), (1, 0, 1), (1, 1, 0), (2, 1, 1)] {
        ensure!(
            mapping(&f.maps, owner, &f.contexts[c])?.role == role,
            "original Y-X-Z role ordering"
        );
    }
    let y = &f.first.nodes[0];
    let y_raw = f.first.command(0, "upgrade-status", &[])?;
    o.transcript.push(json!({"command":"upgrade-status","context":f.contexts[0],"slot":y.slot,"ipc":y.ipc,"raw_reply":y_raw}));
    record(managed, o, "Y-status-raw")?;
    let y_before = parse_upgrade_status(&y_raw, mapping(&f.maps, 0, &f.contexts[0])?)?;
    o.transcript
        .push(json!({"Y_upgrade_status":y_before,"C1_peer_imports_so_far":0}));
    record(managed, o, "pre-B")?;
    ensure!(
        y_before["context"] == f.contexts[0]
            && y_before["status"] == "NoLocalIntent"
            && y_before["lease_current"] == false,
        "Y has no local intent or ALPN2"
    );
    ensure!(
        y.slot == mapping(&f.maps, 0, &f.contexts[0])?.slot,
        "actual Y C1 slot"
    );
    let before = receipt(o.statuses[0].as_ref().context("Y baseline")?)?;
    // No C1 import is called in initial(), ceremony(), or this prefix.
    let b = upgrade(&f.first, 1, 51, o, mapping(&f.maps, 1, &f.contexts[0])?)?;
    o.tb_ms = Some(started.elapsed().as_millis());
    record(managed, o, "B-serialized-reply")?;
    pending(&b, mapping(&f.maps, 1, &f.contexts[0])?)?;
    upgrade(&f.second, 0, 52, o, mapping(&f.maps, 1, &f.contexts[1])?)?;
    exchange_recorded(f, true, o)?;
    let tr = Instant::now();
    o.tr_ms = Some(started.elapsed().as_millis());
    drop(prep);
    o.phase = "Active".into();
    record(managed, o, "C1-imports-complete")?;
    // This phase encloses C2 imports too; no slow C2/status command can escape.
    {
        let _active = s53_deadline::Phase::until(tr + Duration::from_secs(240))?;
        exchange_recorded(f, false, o)?;
        loop {
            poll(f, o)?;
            let ready = f
                .contexts
                .iter()
                .all(|c| selected(&o.statuses, &f.maps, c).is_ok());
            if ready {
                break;
            }
            s53_deadline::check()?;
            std::thread::sleep(Duration::from_millis(100));
        }
        o.active_ms = Some(started.elapsed().as_millis());
        record(managed, o, "Active-observed")?;
        notification(
            &o.traces,
            &f.contexts[0],
            before,
            receipt(o.statuses[0].as_ref().context("Y receipt")?)?,
        )?;
        s53_deadline::check()?;
    }
    let _data = s53_deadline::Phase::until(Instant::now() + Duration::from_secs(30))?;
    o.phase = "data".into();
    let initial = live(
        &mut f.owners,
        &f.nodes,
        &f.maps,
        managed,
        "b-start-selected",
    )?;
    original(&initial, &f.maps, &f.contexts)?;
    let m = mapping(&f.maps, 1, &f.contexts[0])?;
    let sender = view(&f.nodes, &f.maps, 1, &f.contexts[0])?;
    ensure!(
        request(&sender, 601)?.is_none(),
        "fresh B-start data request"
    );
    let own = &initial.history[&key(1, &f.contexts[0])];
    let previous = own
        .values()
        .filter(|v| v.role == m.role)
        .max_by_key(|v| v.ordinal)
        .context("original B tail")?;
    let sequence = initial
        .history
        .iter()
        .filter(|(k, _)| k.starts_with("1/"))
        .flat_map(|(_, h)| h.values())
        .map(|v| v.sequence)
        .max()
        .context("original author frontier")?
        .checked_add(1)
        .context("author sequence overflow")?;
    let next: [u64; 2] =
        serde_json::from_value(oracle::row(initial.status(1)?, &f.contexts[0])?["next"].clone())?;
    let body = "B-start C1 B to A exactly once";
    let sent = sender.send(601, body)?;
    o.transcript.push(json!({"command":"send","context":f.contexts[0],"author":1,"request":oracle::request_id(601),"reply":sent}));
    record(managed, o, "send-accepted")?;
    let mut saved = oracle::sent(&sent)?;
    loop {
        f.owners.alive()?;
        let current = request(&sender, 601)?.context("B-start request retained")?;
        oracle::request_stable(&saved, &current)?;
        saved = current;
        if saved.delivered {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let directed = oracle::Directed {
        context: f.contexts[0].clone(),
        author: 1,
        role: m.role,
        id: 601,
        body: body.into(),
        ordinal: next[m.role as usize],
        sequence,
        previous: Some(state::parse_id(&previous.id)?),
        saved,
    };
    let after = live(
        &mut f.owners,
        &f.nodes,
        &f.maps,
        managed,
        "b-start-delivered",
    )?;
    o.statuses = after.status.clone();
    phase(
        managed,
        "b-start-data-observed",
        json!({"directed":directed,"send":sent,"after":after}),
    )?;
    oracle::progress(&initial.status, &after.status, &f.maps, &directed)?;
    for m in f.maps.iter().filter(|m| m.context == f.contexts[0]) {
        directed.history(
            &initial.history[&key(m.owner, &m.context)],
            &after.history[&key(m.owner, &m.context)],
        )?;
    }
    unchanged_c2(&initial, &after, &f.maps, &f.contexts[1])?;
    s53_deadline::check()?;
    drop(_data);
    let _stopped = s53_deadline::Phase::until(Instant::now() + Duration::from_secs(30))?;
    o.phase = "stopped".into();
    let final_live = live(&mut f.owners, &f.nodes, &f.maps, managed, "b-start-final")?;
    for m in &f.maps {
        ensure!(
            after.history[&key(m.owner, &m.context)]
                == final_live.history[&key(m.owner, &m.context)],
            "no duplicate after drain"
        );
    }
    let saved = stop_read(
        managed,
        "b-start-final",
        &mut f.owners,
        &f.nodes,
        &f.maps,
        &f.contexts,
        std::slice::from_ref(&directed),
    )?;
    joined(&final_live, &saved, &f.maps)?;
    s53_deadline::check()?;
    Ok(
        json!({"scope":"one pre-admission B-only C1 startup, three actual owners/two contexts; no universal SLA","observed":o,"mapping":f.maps,"directed":directed,"stopped":saved}),
    )
}
pub(super) fn run() -> Result<()> {
    let managed = Managed::new(CASE, 480)?;
    let started = Instant::now();
    let mut o = Observed::default();
    let preparation = started + Duration::from_secs(150);
    let prepared = (|| {
        let _phase = s53_deadline::Phase::until(preparation)?;
        initial(&managed, Some(started))
    })();
    let mut f = match prepared {
        Ok(f) => f,
        Err(e) => {
            persist(
                &managed.run,
                "b-start-preparation-failure.json",
                json!({"phase":"initial","error":format!("{e:#}"),"elapsed_ms":started.elapsed().as_millis(),"cached_evidence":"g3-legacy/ready/mapping and owner start phase files; existing RAII and independent outer cleanup"}),
            )?;
            return Err(e);
        }
    };
    let result = execute(&managed, &mut f, &mut o, started, preparation);
    match result {
        Ok(v) => managed.finish(v),
        Err(e) => {
            o.last_error = Some(format!("{e:#}"));
            let liveness=f.owners.children.iter_mut().enumerate().map(|(owner,c)|json!({"owner":owner,"pid":c.as_ref().map(|c|c.0.id()),"try_wait":c.as_mut().map(|c|format!("{:?}",c.0.try_wait()))})).collect::<Vec<_>>();
            persist(
                &managed.run,
                "b-start-failure.json",
                json!({"cached":o,"liveness":liveness,"elapsed_ms":started.elapsed().as_millis(),"unobserved_internal_cause":"eligibility/Owner7/permit cause unobserved; see actual trace/errors"}),
            )?;
            f.owners.stop()?;
            Err(e)
        }
    }
}

#[cfg(test)]
#[path = "migration_s53_b_start_tests.rs"]
mod tests;
