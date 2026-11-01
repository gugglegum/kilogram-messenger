//! G1's two new messages and stopped/restarted witnesses only.
use super::*;
use kilogram_cli::ordered_dialog::{runtime7::Profile3, state};
use kilogram_protocol::{ordered_dialog_v1 as old, ordered_dialog_v2 as wire};
use kilogram_state::indexed_dialog::{self as db, migration7::live1 as store};
use std::collections::{BTreeMap, BTreeSet};

fn request(role: usize) -> String {
    format!("fa00000000000000{:016x}", 600 + role)
}
fn request_bytes(role: usize) -> [u8; 16] {
    let mut id = [0; 16];
    id[0] = 0xfa;
    id[8..].copy_from_slice(&(600 + role as u64).to_be_bytes());
    id
}
fn hash(raw: &[u8]) -> String {
    blake3::hash(raw).to_hex().to_string()
}
fn check(until: Instant) -> Result<()> {
    s53_deadline::check()?;
    ensure!(Instant::now() < until, "G1 message observation deadline");
    Ok(())
}
fn live(pair: &mut Pair) -> Result<[Value; 2]> {
    for child in &mut pair.children {
        child.as_mut().context("owned G1 process")?.alive()?;
    }
    Ok([pair.nodes[0].status()?, pair.nodes[1].status()?])
}
fn context(pair: &Pair, full: &[Value; 2]) -> Result<[Value; 2]> {
    Ok([
        pair.nodes[0].context(&full[0])?.clone(),
        pair.nodes[1].context(&full[1])?.clone(),
    ])
}
fn constructors(full: &Value) -> Result<()> {
    for f in [
        "account_constructors",
        "outbound_constructors",
        "inbound_constructors",
    ] {
        ensure!(
            full["stats"].get(f).and_then(Value::as_u64) == Some(0),
            "G1 zero process {f}"
        );
    }
    Ok(())
}
fn progress(row: &Value, count: usize) -> bool {
    row["next"] == json!([513, 18 + (count - 1)])
        && row["counts"]["sources"] == 530 + count
        && row["counts"]["receipts"] == json!([512, 17 + (count - 1)])
        && ["queued", "pending", "debt"]
            .iter()
            .all(|f| row["counts"][*f] == 0)
        && row["control_debt"] == false
}
fn own_status(pair: &Pair, role: usize) -> Result<Value> {
    Ok(serde_json::from_str(&pair.command(
        role,
        "request-status3",
        &["--request-id".into(), request(role)],
    )?)?)
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct NewRequest {
    queue: u64,
    event: Option<String>,
    delivered: bool,
}
impl NewRequest {
    fn delivered_id(&self) -> Option<&str> {
        self.delivered.then_some(self.event.as_deref()).flatten()
    }
}
fn request_status(value: &Value) -> Result<Option<NewRequest>> {
    let outer = value.as_array().context("G1 request-status option array")?;
    ensure!(outer.len() <= 1, "G1 request-status option cardinality");
    let Some(tuple) = outer.first() else {
        return Ok(None);
    };
    let f = tuple.as_array().context("G1 request-status tuple")?;
    ensure!(f.len() == 3, "G1 request-status tuple width");
    let queue = f[0].as_array().context("G1 request queue option")?;
    let event = f[1].as_array().context("G1 request event option")?;
    ensure!(
        queue.len() <= 1 && event.len() <= 1,
        "G1 request inner option cardinality"
    );
    // New own sends retain their historical queue identity after the Queue
    // ticket is removed. Imported legacy requests have a separate None path.
    let queue = queue
        .first()
        .context("G1 new request queue identity required")?
        .as_u64()
        .context("G1 request queue identity u64")?;
    let event = event
        .first()
        .map(|id| -> Result<String> {
            let id = id.as_str().context("G1 request EventId string")?;
            ensure!(
                id.len() == 64
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "G1 request canonical EventId"
            );
            Ok(id.to_owned())
        })
        .transpose()?;
    let delivered = f[2].as_u64().context("G1 request delivered u64")?;
    ensure!(delivered <= 1, "G1 request delivered flag");
    ensure!(
        delivered == 0 || event.is_some(),
        "G1 delivered request missing EventId"
    );
    Ok(Some(NewRequest {
        queue,
        event,
        delivered: delivered == 1,
    }))
}
fn stable_request(previous: &NewRequest, current: &NewRequest) -> Result<()> {
    ensure!(
        previous.queue == current.queue,
        "G1 retained queue identity changed"
    );
    if previous.event.is_some() {
        ensure!(
            previous.event == current.event,
            "G1 materialized request EventId changed"
        );
    }
    ensure!(
        !previous.delivered || current.delivered,
        "G1 delivered request regressed"
    );
    Ok(())
}
fn persisted_request(
    own: &store::L1Request,
    live: &NewRequest,
    id: old::Id,
    text: &str,
) -> Result<()> {
    ensure!(
        live.delivered_id() == Some(state::hex(&id).as_str())
            && own.event == Some(id)
            && own.queue_sequence == Some(live.queue)
            && own.text_hash == old::hash(b"kg:od1:request-text\0", text.as_bytes()),
        "G1 own request exact live queue/source/body"
    );
    Ok(())
}

// Same finite public pagination bounds as G1b1. This case-local collection also
// returns the new rows; G1b1's independent original map/frozen cursors stay intact.
fn history(
    node: &Node,
    next: [u64; 2],
    total: usize,
    until: Instant,
) -> Result<BTreeMap<String, Value>> {
    let mut cursor = None;
    let mut cursors = BTreeSet::new();
    let mut rows = BTreeMap::new();
    let mut bytes = 0;
    let mut view = None;
    for _ in 0..34 {
        check(until)?;
        let page = node.history(32, cursor.as_deref())?;
        let size = serde_json::to_vec(&page)?.len();
        bytes += size;
        ensure!(
            size <= 48 * 1024 && bytes <= 4 * 1024 * 1024,
            "G1 complete history byte bounds"
        );
        ensure!(
            page["version"] == 3 && page["F"] == json!(next),
            "G1 complete history frozen frontier"
        );
        let current = page["view"].as_str().context("G1 history view")?;
        ensure!(current.len() == 64, "G1 view encoding");
        if let Some(old) = &view {
            ensure!(old == current, "G1 pagination view drift");
        } else {
            view = Some(current.to_owned());
        }
        let items = page["items"].as_array().context("G1 history items")?;
        ensure!(
            !items.is_empty() && items.len() <= 32 && rows.len() + items.len() <= 1024,
            "G1 pagination progress/row cap"
        );
        for row in items {
            let id = row["id"].as_str().context("G1 history ID")?.to_owned();
            ensure!(
                rows.insert(id, row.clone()).is_none(),
                "G1 duplicate history source"
            );
        }
        let encoded = page["cursor"].as_str().context("G1 history cursor")?;
        db::HistoryCursor::from_base64(encoded)?;
        ensure!(
            cursors.insert(encoded.to_owned()),
            "G1 cursor did not progress"
        );
        if page["finished"] == true {
            ensure!(rows.len() == total, "G1 complete history count");
            check(until)?;
            return Ok(rows);
        }
        ensure!(page["finished"] == false, "G1 history terminal flag");
        cursor = Some(encoded.to_owned());
    }
    anyhow::bail!("G1 history page cap")
}
fn new_rows(rows: &BTreeMap<String, Value>, ids: &[String], count: usize) -> Result<()> {
    ensure!(
        rows.len() == 528 + count && rows.values().filter(|r| r["format"] == 1).count() == 528,
        "G1 complete original/new format counts"
    );
    let mixed = rows
        .values()
        .filter(|r| r["format"] == 2)
        .collect::<Vec<_>>();
    ensure!(mixed.len() == count, "G1 exact new format2 membership");
    for role in 0..count {
        let row = rows.get(&ids[role]).context("G1 unique delivered new ID")?;
        ensure!(
            row["format"] == 2
                && row["role"] == role
                && row["ordinal"] == [512, 18][role]
                && row["request"] == request(role)
                && row["text"] == g1_admission::FIRST_TEXT
                && row["proof_kind"] == "FullMixed"
                && row["delivered"] == true,
            "G1 exact delivered Event2 projection: {row}"
        );
        ensure!(
            rows.values()
                .filter(|r| r["request"] == request(role))
                .count()
                == 1,
            "G1 request appears exactly once"
        );
    }
    if count == 1 {
        ensure!(
            rows.values().all(|r| r["request"] != request(1)),
            "B601 absent until A ack"
        );
    }
    Ok(())
}
pub(super) struct Delivery {
    full: [Value; 2],
    rows: [Value; 2],
    history: [BTreeMap<String, Value>; 2],
    requests: [Value; 2],
    pub(super) ids: Vec<String>,
    profiles: [String; 2],
}
impl Delivery {
    pub(super) fn status(&self) -> &[Value; 2] {
        &self.full
    }
    pub(super) fn histories(&self) -> &[BTreeMap<String, Value>; 2] {
        &self.history
    }
    pub(super) fn report(&self) -> Result<Value> {
        Ok(
            json!({"status":self.full,"own_requests":self.requests,"source_ids":self.ids,
            "history_rows":self.history[0].len(),"history_digests":[hash(&serde_json::to_vec(&self.history[0])?),hash(&serde_json::to_vec(&self.history[1])?)],
            "profile_hashes":self.profiles}),
        )
    }
    pub(super) fn rows(&self) -> [Value; 2] {
        self.rows.clone()
    }
}
pub(super) fn wait_delivery(
    pair: &mut Pair,
    original: &[Value; 2],
    count: usize,
) -> Result<Delivery> {
    ensure!((1..=2).contains(&count), "G1 two sequential sends only");
    let until = Instant::now() + Duration::from_secs(90);
    let mut seen: [Option<NewRequest>; 2] = [None, None];
    let requests = loop {
        check(until)?;
        let full = live(pair)?;
        let rows = context(pair, &full)?;
        for role in 0..2 {
            same_origin(&original[role], &rows[role])?;
            constructors(&full[role])?;
            ensure!(
                rows[role]["mode"] == "Active"
                    && rows[role]["q"] == original[role]["q"]
                    && rows[role]["cuts"] == json!([512, 18]),
                "G1 same Active Q/cuts during delivery"
            );
        }
        let requests = [own_status(pair, 0)?, own_status(pair, 1)?];
        let observed = [request_status(&requests[0])?, request_status(&requests[1])?];
        if count == 1 {
            ensure!(
                observed[1].is_none(),
                "G1 B601 request exists before B send"
            );
        }
        for role in 0..count {
            if let Some(previous) = &seen[role] {
                stable_request(
                    previous,
                    observed[role]
                        .as_ref()
                        .context("G1 own request disappeared")?,
                )?;
            }
            seen[role] = observed[role].clone();
        }
        if rows.iter().all(|r| progress(r, count))
            && observed[..count]
                .iter()
                .all(|r| r.as_ref().and_then(NewRequest::delivered_id).is_some())
        {
            break requests;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let ids = (0..count)
        .map(|role| {
            request_status(&requests[role])?
                .and_then(|r| r.delivered_id().map(str::to_owned))
                .context("G1 delivered own source ID")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        ids.iter().all(|s| s.len() == 64) && ids.iter().collect::<BTreeSet<_>>().len() == count,
        "G1 distinct new IDs"
    );
    let next = [513, 18 + (count as u64 - 1)];
    let histories = [
        history(&pair.nodes[0], next, 528 + count, until)?,
        history(&pair.nodes[1], next, 528 + count, until)?,
    ];
    for h in &histories {
        new_rows(h, &ids, count)?;
    }
    for id in &ids {
        ensure!(
            histories[0].get(id) == histories[1].get(id),
            "G1 same delivered projection on both replicas"
        );
    }
    let full = live(pair)?;
    let rows = context(pair, &full)?;
    let final_requests = [own_status(pair, 0)?, own_status(pair, 1)?];
    ensure!(
        final_requests == requests,
        "G1 stable delivered requests across full history"
    );
    for role in 0..2 {
        ensure!(
            progress(&rows[role], count),
            "G1 completed delivery remains quiescent"
        );
        constructors(&full[role])?;
        same_origin(&original[role], &rows[role])?;
    }
    let profiles = [
        hash(&std::fs::read(&pair.nodes[0].profile)?),
        hash(&std::fs::read(&pair.nodes[1].profile)?),
    ];
    Ok(Delivery {
        full,
        rows,
        history: histories,
        requests,
        ids,
        profiles,
    })
}
pub(super) fn compare_delivery(before: &Delivery, after: &Delivery) -> Result<()> {
    ensure!(
        before.ids == after.ids
            && before.history == after.history
            && before.requests == after.requests
            && before.profiles == after.profiles,
        "G1 exact persisted history/request/profile/new source witnesses"
    );
    for role in 0..2 {
        for f in [
            "q",
            "cuts",
            "next",
            "counts",
            "native_generation",
            "native_hash",
            "native",
            "native_encoding",
        ] {
            ensure!(
                before.rows[role][f] == after.rows[role][f],
                "G1 actual post-data persisted {f} role{role}"
            );
        }
        for f in [
            "materialized",
            "readable",
            "account_constructors",
            "outbound_constructors",
            "inbound_constructors",
        ] {
            ensure!(
                after.full[role]["stats"].get(f).and_then(Value::as_u64) == Some(0),
                "G1 reopened process no repeated {f}"
            );
        }
    }
    Ok(())
}
pub(super) fn a_before_b(first: &Delivery, complete: &Delivery) -> Result<()> {
    ensure!(
        first.ids.len() == 1
            && complete.ids.len() == 2
            && first.ids[0] == complete.ids[0]
            && first.requests[1] == json!([])
            && first.requests[0] == complete.requests[0],
        "G1 A ack observed before B request"
    );
    for local in 0..2 {
        ensure!(
            first.history[local].get(&first.ids[0]) == complete.history[local].get(&first.ids[0]),
            "G1 first A delivered row preserved after B"
        );
    }
    Ok(())
}
pub(super) fn stop(pair: &mut Pair) -> Result<Value> {
    let pids = pair
        .children
        .each_ref()
        .map(|c| c.as_ref().map(|c| c.0.id()));
    ensure!(
        pids.iter().all(Option::is_some),
        "G1 two live owned processes before stop"
    );
    pair.stop()?;
    ensure!(
        pair.children.iter().all(Option::is_none),
        "G1 stop/wait both owners complete"
    );
    Ok(json!({"pids":pids,"both_owned_kill_wait_completed":true}))
}

fn credential(
    repo: &mut store::L1Repository,
    reference: old::Id,
) -> Result<(Vec<u8>, wire::RootVerifiedCredential)> {
    let raw = repo.auth(0, reference)?.context("G1 captured credential")?;
    let credential = wire::ParsedCredentialBundle::parse(&raw)?.verify_root()?;
    ensure!(
        credential.reference() == reference,
        "G1 captured AuthRecord hash"
    );
    let floor = repo.authority(*credential.certificate().account_id().as_bytes())?;
    let parsed = kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&floor)?;
    ensure!(
        credential.authority().revision() >= parsed.revision()
            && (credential.authority().revision() != parsed.revision()
                || credential.authority().encode()? == floor),
        "G1 new credential current stored floor"
    );
    Ok((raw, credential))
}
fn selection(
    repo: &mut store::L1Repository,
    origin: &[u8],
    expected: &Value,
) -> Result<(wire::StaticSelection2, String)> {
    let decoded = old::decode(origin, wire::MAX_EXISTING_ORIGIN)?;
    let f = decoded.fields(7)?;
    ensure!(
        f[1].bytes()? == repo.spec(0)?
            && f[4].bytes()?
                == repo
                    .control(0, store::L1Control::InitialPair)?
                    .context("G1 original pair")?,
        "G1 stored signed original scope"
    );
    for part in [2, 3] {
        let raw = f[part].bytes()?;
        ensure!(
            repo.signed_control(0, old::hash(old::CONTROL_ID, raw))? == raw,
            "G1 stored old Offer/Grant"
        );
    }
    for role in 0..2 {
        ensure!(
            f[5].fields(2)?[role].bytes()?
                == repo.event_at(0, role as u8, 0)?.context("G1 old genesis")?
                && f[6].fields(2)?[role].bytes()?
                    == repo
                        .control(0, store::L1Control::Ready(role as u8))?
                        .context("G1 old Ready")?,
            "G1 stored old genesis/Ready"
        );
    }
    let scope = wire::VerifiedExistingScope1::verify_export(origin)?;
    let state = repo.existing_state(0)?;
    let q = state.q.context("G1 stored Q")?;
    ensure!(
        state.mode == store::ExistingMode::Active
            && state.cuts == [Some(512), Some(18)]
            && state::hex(&q) == expected["q"],
        "G1 actual stored Active selection"
    );
    let raw = repo.existing_control(0, q)?;
    let decoded = old::decode(&raw, wire::MAX_PROOF)?;
    let p = decoded.fields(6)?;
    for (part, reference) in [(2, state.propose), (3, state.accept), (4, state.select)] {
        let control = wire::ParsedControl2::parse(p[part].bytes()?)?;
        ensure!(
            Some(control.id()) == reference
                && repo.existing_control(0, control.id())? == control.raw(),
            "G1 stored selected signed P/A/K"
        );
    }
    let consent = wire::ParsedConsent2::parse(p[5].bytes()?)?;
    ensure!(
        Some(consent.reference()) == state.selected_consent
            && state.current_consent == state.selected_consent
            && repo.existing_control(0, consent.reference())? == consent.raw(),
        "G1 stored selected/current Consent2"
    );
    let parsed_selection = wire::ParsedModeProof2::parse(&raw)?;
    ensure!(
        parsed_selection.cuts() == [512, 18],
        "G1 signed proof exact exhausted cuts"
    );
    let selected = scope.selection(parsed_selection)?;
    ensure!(
        selected.proof().q() == q
            && state::hex(&selected.c()) == expected["context"]
            && state::hex(&selected.d()) == expected["d"]
            && state::hex(&selected.e()) == expected["e"]
            && selected.sid() == expected["sid"].as_str(),
        "G1 verified original selected domain"
    );
    let committed = repo.existing_control(0, state.committed.context("G1 stored Committed")?)?;
    let committed_control = wire::ParsedControl2::parse(&committed)?;
    committed_control.require_kind(21)?;
    ensure!(
        Some(committed_control.id()) == state.committed,
        "G1 stored Committed exact reference"
    );
    committed_control.verify_committed(&selected, &scope.peers()[1])?;
    Ok((selected, hash(&[origin, &raw, &committed].concat())))
}
fn verify_new(
    selected: &wire::StaticSelection2,
    raw: &[u8],
    author: &wire::RootVerifiedCredential,
    receipt: &[u8],
    receiver: &wire::RootVerifiedCredential,
    expected: (usize, u64, [u8; 16]),
) -> Result<wire::VerifiedEvent2> {
    let (role, n, request) = expected;
    let verified = wire::ParsedEvent2::parse(raw)?.verify(selected, author)?;
    let c = verified.event().core();
    ensure!(
        c.role == role as u8 && c.n == n && c.kind == 3 && c.request == request,
        "G1 exact signed new source role/ordinal/request"
    );
    wire::ParsedReceipt2::parse(receipt)?.verify(&verified, selected, receiver)?;
    Ok(verified)
}
pub(super) struct Stopped {
    report: Value,
    history: [BTreeMap<String, Value>; 2],
    rows: [Value; 2],
    requests: [Value; 2],
    ids: Vec<String>,
}
impl Stopped {
    pub(super) fn report(&self) -> Value {
        self.report.clone()
    }
    pub(super) fn bind(&self, delivery: &Delivery) -> Result<()> {
        ensure!(
            self.ids == delivery.ids && self.requests == delivery.requests,
            "C2 stopped/live request identities"
        );
        for local in 0..2 {
            for (id, row) in &self.history[local] {
                ensure!(
                    delivery.history[local].get(id) == Some(row),
                    "C2 stopped signed row equals live history"
                );
            }
            for field in [
                "q",
                "cuts",
                "next",
                "counts",
                "native_generation",
                "native_hash",
            ] {
                ensure!(
                    self.rows[local][field] == delivery.rows[local][field],
                    "C2 stopped/live {field}"
                );
            }
        }
        Ok(())
    }
    pub(super) fn preserve_prefix(&self, later: &Self) -> Result<()> {
        ensure!(
            self.ids.len() <= later.ids.len() && later.ids.starts_with(&self.ids),
            "C2 stopped prefix identities"
        );
        for local in 0..2 {
            for i in 0..self.ids.len() {
                ensure!(
                    self.report[local]["records"][i] == later.report[local]["records"][i],
                    "C2 stopped prefix raw records preserved"
                );
            }
        }
        Ok(())
    }
}
fn stopped_ids(requests: &[Value; 2], count: usize) -> Result<Vec<String>> {
    ensure!((1..=2).contains(&count), "C2 stopped count1/count2 only");
    if count == 1 {
        ensure!(
            requests[1] == json!([]),
            "C2 B request absent before B send"
        );
    }
    (0..count)
        .map(|role| {
            request_status(&requests[role])?
                .and_then(|r| r.delivered_id().map(str::to_owned))
                .context("C2 stopped delivered request ID")
        })
        .collect()
}
pub(super) fn stopped_a(
    pair: &Pair,
    origin: &[u8],
    rows: &[Value; 2],
    requests: &[Value; 2],
) -> Result<Stopped> {
    let ids = stopped_ids(requests, 1)?;
    stopped_raw(pair, origin, rows, requests, &ids)
}
pub(super) fn stopped_delivery(pair: &Pair, delivery: &Delivery) -> Result<Stopped> {
    let origin = kilogram_cli::ordered_dialog::admin::read_public(
        &pair.base.join("exhausted.origin.cbor"),
        wire::MAX_EXISTING_ORIGIN,
    )?;
    let ids = stopped_ids(&delivery.requests, 2)?;
    let stopped = stopped_raw(pair, &origin, &delivery.rows, &delivery.requests, &ids)?;
    stopped.bind(delivery)?;
    Ok(stopped)
}
pub(super) fn stopped_records(pair: &Pair, delivery: &Delivery) -> Result<Value> {
    Ok(stopped_delivery(pair, delivery)?.report())
}
fn raw_identity(source: &wire::VerifiedEvent2, expected: &str, previous: old::Id) -> Result<()> {
    ensure!(
        state::hex(&source.event().id()) == expected
            && source.event().core().previous == Some(previous),
        "G1 exact first new source/previous signed legacy tail"
    );
    Ok(())
}
fn stopped_raw(
    pair: &Pair,
    origin: &[u8],
    rows: &[Value; 2],
    requests: &[Value; 2],
    ids: &[String],
) -> Result<Stopped> {
    ensure!(
        (1..=2).contains(&ids.len()),
        "C2 stopped raw count1/count2 only"
    );
    ensure!(
        pair.children.iter().all(Option::is_none),
        "G1 new-record read only after both owned waits"
    );
    let mut reports = Vec::new();
    let mut history = [BTreeMap::new(), BTreeMap::new()];
    for local in 0..2 {
        let node = &pair.nodes[local];
        let profile = Profile3::load(&node.profile)?;
        let mut repo = profile.open(&node.profile)?;
        let (selected, selection_hash) = selection(&mut repo, origin, &rows[local])?;
        let spec = kilogram_cli::ordered_dialog::handshake::Spec::parse(&repo.spec(0)?)?;
        let head = state::Head::parse(
            &repo.head(0)?,
            selected.c(),
            spec.cert_refs()?[local],
            local as u64,
        )?;
        ensure!(
            json!(head.native_generation) == rows[local]["native_generation"]
                && head.blob.map(|id| state::hex(&id))
                    == rows[local]["native_hash"].as_str().map(str::to_owned),
            "G1 stopped actual post-data Native"
        );
        let snapshot = repo.snapshot(0)?;
        ensure!(
            snapshot.head == repo.head(0)? && snapshot.native.is_some(),
            "G1 stopped authenticated Mixed7 Native pointer"
        );
        let native_raw_hash = hash(
            snapshot
                .native
                .as_deref()
                .context("G1 stopped opaque Native")?,
        );
        let native = snapshot.native.as_deref().context("C2 opaque Mixed7")?;
        ensure!(
            head.blob == Some(old::hash(store::MIXED_NATIVE_ID, native)),
            "C2 Mixed7 raw hash/domain"
        );
        let envelope = old::decode(native, state::MAX_NATIVE)?;
        let f = envelope.fields(5)?;
        ensure!(
            f[0].uint()? == 7 && f[1].uint()? == 3 && f[2].uint()? == head.native_generation,
            "C2 Mixed7 public envelope/generation"
        );
        let mut records = Vec::new();
        for role in 0..ids.len() {
            s53_deadline::check()?;
            let n = [512, 18][role];
            let raw = repo.event_at(0, role as u8, n)?.context("G1 new source")?;
            let parsed = wire::ParsedEvent2::parse(&raw)?;
            let id = parsed.id();
            let (author_raw, author) = credential(&mut repo, parsed.core().author_auth)?;
            let receipt = repo.receipt(0, role as u8, n)?.context("G1 new Receipt2")?;
            let receiver_ref = wire::ParsedReceipt2::parse(&receipt)?
                .fields()
                .receiver_auth;
            let (receiver_raw, receiver) = credential(&mut repo, receiver_ref)?;
            let verified = verify_new(
                &selected,
                &raw,
                &author,
                &receipt,
                &receiver,
                (role, n, request_bytes(role)),
            )?;
            let old_raw = repo
                .event_at(0, role as u8, n - 1)?
                .context("G1 preceding legacy source")?;
            let previous = old::Event::parse(&old_raw, author.certificate().device_id())?.id()?;
            raw_identity(&verified, &ids[role], previous)?;
            let projection = repo.projection(0, id)?;
            ensure!(
                projection.text == g1_admission::FIRST_TEXT,
                "G1 exact saved new body"
            );
            let binding = old::decode(&projection.binding, 2304)?;
            let f = binding.fields(5)?;
            let pointer = f[3].fields(2)?;
            let key = db::Key::hash(
                0,
                db::Namespace::Control,
                old::hash(b"kg:od2:full-mixed-proof/v7\0", &id),
            )?;
            ensure!(
                f[0].uint()? == 7
                    && f[1].fixed::<32>()? == repo.migration_id()
                    && f[2].fixed::<32>()? == repo.source_id()
                    && f[4].fixed::<16>()? == request_bytes(role)
                    && pointer[0].uint()? == 2
                    && pointer[1].fixed::<32>()? == key.bytes,
                "G1 saved FullMixed projection binding"
            );
            let proof = repo
                .data_proof(0, id)?
                .context("G1 full mixed proof required")?;
            let mut captured = Vec::new();
            for reference in [
                Some(proof.original),
                proof.admission,
                proof.final_auth,
                proof.receiver,
                proof.receiver_overlay,
            ]
            .into_iter()
            .flatten()
            {
                let (raw, cred) = credential(&mut repo, reference)?;
                ensure!(
                    selected.proof().certs().contains(&cred.cert_ref()),
                    "G1 mixed proof peer credential"
                );
                captured.push(hash(&raw));
            }
            let own = if role == local {
                let own = repo
                    .request(0, request_bytes(role))?
                    .context("G1 new own request")?;
                let live =
                    request_status(&requests[local])?.context("G1 live own request witness")?;
                persisted_request(&own, &live, id, g1_admission::FIRST_TEXT)?;
                Some(hash(&serde_json::to_vec(&own)?))
            } else {
                None
            };
            let expected = json!({"format":2,"id":state::hex(&id),"role":role,"ordinal":n,"sequence":verified.event().core().sequence,
                "request":request(role),"text":projection.text,"proof_kind":"FullMixed","delivered":true});
            history[local].insert(state::hex(&id), expected);
            records.push(json!({"role":role,"ordinal":n,"id":state::hex(&id),"q":state::hex(&selected.proof().q()),
                "source_raw":hash(&raw),"author_auth":hash(&author_raw),"projection":hash(&projection.encode()?),
                "proof":hash(&serde_json::to_vec(&proof)?),"proof_credentials":captured,"receipt_raw":hash(&receipt),
                "receipt_id":state::hex(&wire::ParsedReceipt2::parse(&receipt)?.id()),"receiver_auth":hash(&receiver_raw),"own_request":own}));
        }
        reports.push(json!({"local":local,"selection":selection_hash,"opaque_native_bytes":native_raw_hash,"native_hash":rows[local]["native_hash"],
            "native_generation":head.native_generation,"records":records}));
    }
    let a_records = reports[0]["records"]
        .as_array()
        .context("G1 A new records")?;
    let b_records = reports[1]["records"]
        .as_array()
        .context("G1 B new records")?;
    for (a, b) in a_records.iter().zip(b_records) {
        for field in ["id", "source_raw", "receipt_raw", "receipt_id"] {
            ensure!(a[field] == b[field], "G1 both replicas exact new {field}");
        }
    }
    Ok(Stopped {
        report: json!(reports),
        history,
        rows: rows.clone(),
        requests: requests.clone(),
        ids: ids.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(dead_code)]
    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/kilogram-protocol/tests/support/ordered_dialog_v2.rs"
        ));
    }
    fn built<T>(result: support::TestResult<T>) -> Result<T> {
        result.map_err(|e| anyhow::anyhow!("{e}"))
    }
    #[test]
    fn g1c2_independent_signed_a_and_count_preconditions() -> Result<()> {
        use kilogram_ratchet::ordered::OrderedAccount;
        let actors = built(support::Actors::new([94; 32]))?;
        let (a, _) = OrderedAccount::fresh(false);
        let (mut b, otk) = OrderedAccount::fresh(true);
        let otk = otk.context("C2 genuine OTK")?;
        let proof = built(actors.fresh(a.curve_key(), b.curve_key(), otk))?;
        let mut sender = a.outbound(b.curve_key(), otk)?;
        let sid = sender.sid();
        let selected = wire::StaticSelection2::new(proof.proof.clone(), Some(sid.clone()))?;
        let setup = sender.encrypt(b"C2 native session setup")?;
        let (mut receiver, _) = b.inbound(a.curve_key(), &setup)?;
        let ack = receiver.encrypt(b"C2 native session reply")?;
        sender.decrypt(&ack)?;
        let header = sender.fork().encrypt(b"header trial")?.header()?;
        let previous = [19; 32];
        let core = wire::Core2 {
            c: actors.c,
            d: selected.d(),
            e: selected.e(),
            sid,
            role: 0,
            n: 512,
            previous: Some(previous),
            sequence: 1564,
            kind: 3,
            request: request_bytes(0),
            parents: vec![previous],
            expected: wire::Expected {
                g: 0,
                index: header.index,
                parent: None,
                tail: None,
            },
            author_auth: actors.credentials[0].reference(),
            q: selected.proof().q(),
        };
        let plain = wire::inner2(
            &core,
            wire::a([wire::b(g1_admission::FIRST_TEXT.as_bytes())]),
        )?;
        let packet = sender.encrypt(&plain)?;
        let event = wire::sign_event(core.clone(), &packet, &actors.devices[0])?;
        let decrypted = receiver.decrypt(&packet)?;
        let receipt = wire::ParsedReceipt2::sign(
            wire::ReceiptFields2 {
                c: core.c,
                d: core.d,
                e: core.e,
                sid: core.sid.clone(),
                q: core.q,
                format: 2,
                lane: 0,
                n: 512,
                event: event.id(),
                receiver_cert: actors.credentials[1].cert_ref(),
                receiver_auth: actors.credentials[1].reference(),
            },
            &actors.devices[1],
        )?;
        let verify = |raw: &[u8], ack: &[u8]| {
            verify_new(
                &selected,
                raw,
                &actors.credentials[0],
                ack,
                &actors.credentials[1],
                (0, 512, request_bytes(0)),
            )
        };
        let verified = verify(event.raw(), receipt.raw())?;
        verified.verify_inner(&decrypted)?;
        let id = state::hex(&event.id());
        raw_identity(&verified, &id, previous)?;
        ensure!(
            raw_identity(&verified, &"0".repeat(64), previous).is_err()
                && raw_identity(&verified, &id, [20; 32]).is_err(),
            "C2 exact signed ID/legacy tail"
        );
        for mutate_receipt in [false, true] {
            let mut raw = if mutate_receipt {
                receipt.raw().to_vec()
            } else {
                event.raw().to_vec()
            };
            let end = raw.len() - 1;
            raw[end] ^= 1;
            ensure!(
                if mutate_receipt {
                    verify(event.raw(), &raw).is_err()
                } else {
                    verify(&raw, receipt.raw()).is_err()
                },
                "C2 genuine signature corruption"
            );
        }
        let requests = [json!([[[1564], [id], 1]]), json!([])];
        ensure!(
            stopped_ids(&requests, 1)? == vec![id],
            "C2 independent count1 identity"
        );
        ensure!(
            stopped_ids(&requests, 2).is_err()
                && stopped_ids(&requests, 0).is_err()
                && stopped_ids(&requests, 3).is_err(),
            "C2 count2 cannot precede B and counts are bounded"
        );
        let live = request_status(&requests[0])?.context("C2 own identity")?;
        let own = store::L1Request {
            queue_sequence: Some(1564),
            event: Some(event.id()),
            text_hash: old::hash(
                b"kg:od1:request-text\0",
                g1_admission::FIRST_TEXT.as_bytes(),
            ),
        };
        persisted_request(&own, &live, event.id(), g1_admission::FIRST_TEXT)?;
        let mut changed = own;
        changed.queue_sequence = Some(1565);
        ensure!(
            persisted_request(&changed, &live, event.id(), g1_admission::FIRST_TEXT).is_err(),
            "C2 historical retained queue must match raw request"
        );
        Ok(())
    }
    #[test]
    fn g1_request_observer_actual_retained_sequence_and_negatives() -> Result<()> {
        // Actual attempt01 command0525 reply and command1116 delivered status.
        // No dependency on its private replica; these are public RPC bytes.
        let accepted: Value = serde_json::from_str("[[1564],[]]")?;
        let raw =
            r#"[[[1564],["36c4b673903a55449df7e4dfc28d4de8d36bc0299b2ef4814dd4a91dd835f800"],1]]"#;
        let value: Value = serde_json::from_str(raw)?;
        let live = request_status(&value)?.context("actual delivered request")?;
        ensure!(
            Some(live.queue) == accepted[0][0].as_u64() && live.delivered_id().is_some(),
            "actual send/status queue identity"
        );
        let id = state::parse_id(live.delivered_id().context("actual EventId")?)?;
        let own = store::L1Request {
            queue_sequence: Some(live.queue),
            event: Some(id),
            text_hash: old::hash(
                b"kg:od1:request-text\0",
                g1_admission::FIRST_TEXT.as_bytes(),
            ),
        };
        persisted_request(&own, &live, id, g1_admission::FIRST_TEXT)?;
        let queued = request_status(&json!([[[live.queue], [], 0]]))?.context("queued")?;
        let pending =
            request_status(&json!([[[live.queue], [state::hex(&id)], 0]]))?.context("unacked")?;
        ensure!(
            queued.delivered_id().is_none() && pending.delivered_id().is_none(),
            "ordinary pending never delivered"
        );
        stable_request(&queued, &pending)?;
        stable_request(&pending, &live)?;
        stable_request(&live, &live)?;
        ensure!(request_status(&json!([]))?.is_none(), "absent option");
        let mut malformed = vec![
            json!(null),
            json!({}),
            json!([[], []]),
            json!([[]]),
            json!([[[1], [], 0, 0]]),
            json!([[[], [state::hex(&id)], 1]]),
            json!([[[1, 2], [state::hex(&id)], 1]]),
            json!([[[-1], [state::hex(&id)], 1]]),
            json!([[["1564"], [state::hex(&id)], 1]]),
            json!([[[true], [state::hex(&id)], 1]]),
            json!([[[1.5], [state::hex(&id)], 1]]),
            json!([[[1], [], 1]]),
            json!([[[1], [state::hex(&id), state::hex(&id)], 1]]),
            json!([[[1], [1], 1]]),
            json!([[[1], ["f"], 1]]),
            json!([[[1], ["z".repeat(64)], 1]]),
            json!([[[1], [state::hex(&id).to_uppercase()], 1]]),
            json!([[[1], [state::hex(&id)], 2]]),
            json!([[[1], [state::hex(&id)], true]]),
        ];
        malformed.push(json!([[[1], [state::hex(&id)], -1]]));
        for bad in malformed {
            ensure!(
                request_status(&bad).is_err(),
                "malformed terminal status must error: {bad}"
            );
        }
        for (queue, event, text) in [
            (None, Some(id), own.text_hash),
            (Some(live.queue + 1), Some(id), own.text_hash),
            (Some(live.queue), None, own.text_hash),
            (Some(live.queue), Some([7; 32]), own.text_hash),
            (Some(live.queue), Some(id), [0; 32]),
        ] {
            let bad = store::L1Request {
                queue_sequence: queue,
                event,
                text_hash: text,
            };
            ensure!(
                persisted_request(&bad, &live, id, g1_admission::FIRST_TEXT).is_err(),
                "persisted queue/event/body mismatch"
            );
        }
        let changed = NewRequest {
            queue: live.queue + 1,
            ..live.clone()
        };
        ensure!(
            stable_request(&live, &changed).is_err(),
            "live queue stability"
        );
        let changed = NewRequest {
            event: Some(state::hex(&[7; 32])),
            ..live.clone()
        };
        ensure!(
            stable_request(&live, &changed).is_err(),
            "live source stability"
        );
        ensure!(
            stable_request(&live, &pending).is_err(),
            "delivered cannot regress across A-before-B/restart"
        );
        Ok(())
    }
    #[test]
    fn g1b3_genuine_event_receipt_selection_negatives() -> Result<()> {
        use kilogram_ratchet::ordered::OrderedAccount;
        let actors = built(support::Actors::new([93; 32]))?;
        let (a, _) = OrderedAccount::fresh(false);
        let (mut b, otk) = OrderedAccount::fresh(true);
        let otk = otk.context("genuine OTK")?;
        let proof = built(actors.fresh(a.curve_key(), b.curve_key(), otk))?;
        let mut sender = a.outbound(b.curve_key(), otk)?;
        let sid = sender.sid();
        let selected = wire::StaticSelection2::new(proof.proof.clone(), Some(sid.clone()))?;
        let first = sender.encrypt(b"native setup")?;
        let (mut receiver, _) = b.inbound(a.curve_key(), &first)?;
        let ack = receiver.encrypt(b"native ack")?;
        sender.decrypt(&ack)?;
        // Native may rotate its sender key after receiving the reply. Discover
        // the real next index on a fork, without advancing the sender under test.
        let next_header = sender.fork().encrypt(b"header-only trial")?.header()?;
        let core = wire::Core2 {
            c: actors.c,
            d: selected.d(),
            e: selected.e(),
            sid,
            role: 0,
            n: 1,
            previous: Some([19; 32]),
            sequence: 1,
            kind: 3,
            request: request_bytes(0),
            parents: vec![],
            expected: wire::Expected {
                g: 0,
                index: next_header.index,
                parent: None,
                tail: None,
            },
            author_auth: actors.credentials[0].reference(),
            q: selected.proof().q(),
        };
        let plaintext = wire::inner2(
            &core,
            wire::a([wire::b(g1_admission::FIRST_TEXT.as_bytes())]),
        )?;
        let packet = sender.encrypt(&plaintext)?;
        let event = wire::sign_event(core.clone(), &packet, &actors.devices[0])?;
        let receipt = wire::ParsedReceipt2::sign(
            wire::ReceiptFields2 {
                c: core.c,
                d: core.d,
                e: core.e,
                sid: core.sid.clone(),
                q: core.q,
                format: 2,
                lane: 0,
                n: 1,
                event: event.id(),
                receiver_cert: actors.credentials[1].cert_ref(),
                receiver_auth: actors.credentials[1].reference(),
            },
            &actors.devices[1],
        )?;
        let verified = verify_new(
            &selected,
            event.raw(),
            &actors.credentials[0],
            receipt.raw(),
            &actors.credentials[1],
            (0, 1, core.request),
        )?;
        verified.verify_inner(&receiver.decrypt(&packet)?)?;
        let mut bad = event.raw().to_vec();
        let end = bad.len() - 1;
        bad[end] ^= 1;
        wire::ParsedEvent2::parse(&bad)?;
        ensure!(
            verify_new(
                &selected,
                &bad,
                &actors.credentials[0],
                receipt.raw(),
                &actors.credentials[1],
                (0, 1, core.request)
            )
            .is_err(),
            "changed Event2 signature refused"
        );
        let mut bad_receipt = receipt.raw().to_vec();
        let end = bad_receipt.len() - 1;
        bad_receipt[end] ^= 1;
        wire::ParsedReceipt2::parse(&bad_receipt)?;
        ensure!(
            verify_new(
                &selected,
                event.raw(),
                &actors.credentials[0],
                &bad_receipt,
                &actors.credentials[1],
                (0, 1, core.request)
            )
            .is_err(),
            "changed Receipt2 signature refused"
        );
        let mut wrong_q = core.clone();
        wrong_q.q = [17; 32];
        let wrong = wire::sign_event(wrong_q, &packet, &actors.devices[0])?;
        let error = verify_new(
            &selected,
            wrong.raw(),
            &actors.credentials[0],
            receipt.raw(),
            &actors.credentials[1],
            (0, 1, core.request),
        )
        .err()
        .context("wrong Q accepted")?;
        ensure!(
            error.to_string().contains("event scope/Q"),
            "exact signed wrong-Q premise"
        );
        ensure!(
            verify_new(
                &selected,
                event.raw(),
                &actors.credentials[0],
                receipt.raw(),
                &actors.credentials[0],
                (0, 1, core.request)
            )
            .is_err(),
            "wrong receiver refused"
        );
        ensure!(
            verify_new(
                &selected,
                event.raw(),
                &actors.credentials[0],
                receipt.raw(),
                &actors.credentials[1],
                (0, 1, request_bytes(1))
            )
            .is_err(),
            "wrong expected request refused"
        );
        let mut bad_proof = proof.raw.clone();
        let end = bad_proof.len() - 1;
        bad_proof[end] ^= 1;
        let parsed = wire::ParsedModeProof2::parse(&bad_proof)?;
        ensure!(
            parsed
                .verify(&wire::SelectionEvidence2 {
                    peers: actors.credentials.clone(),
                    membership: actors.membership.clone(),
                    initial_pair: Some(proof.pair),
                    previous: None
                })
                .is_err(),
            "invalid signed selection never becomes a token"
        );
        Ok(())
    }
}
