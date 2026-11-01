//! Predicates shared by the inactive controller and pure negative checks.
use super::*;

pub(super) fn prerequisites(
    next: usize,
    live: [bool; 2],
    requests: usize,
    pending: &[observations::Pending],
    ids: &[(u64, old::Id)],
) -> Result<()> {
    ensure!(
        next == 9
            && live == [true; 2]
            && requests == 1344
            && pending.len() == 64
            && ids.len() == 256,
        "same owned incomplete9/12 and original request/pending witnesses"
    );
    ensure!(
        pending
            .iter()
            .zip(&ids[1..65])
            .all(|(p, (n, id))| p.context == 0
                && p.role == 0
                && p.ordinal == *n
                && p.event == *id
                && p.generation > 0),
        "original pending386..449 exact P7 sources"
    );
    Ok(())
}
pub(super) fn held_schedule(c: &control::Controls, owner: usize) -> Result<()> {
    ensure!(
        c.old["disable_ack"] == true
            && c.old["suspend_prefix_requests"] == true
            && c.current["phase"] == "catch_up"
            && c.current["suspend_live"] == true
            && c.current["suspend_prefix"] == true
            && c.current["hold_source64"] == false
            && c.current["source64_after"].is_null()
            && c.current["hold_ack"] == false
            && c.current["hold_ack_from"].is_null()
            && c.current["pause_after_normal_page"].is_null()
            && c.current["retry_live"].is_null()
            && c.current["hold_ordinals"] == json!([if owner == 0 { 385 } else { 640 }]),
        "valid held CatchUp schedule preserves original source holds"
    );
    Ok(())
}
pub(super) fn progress(row: &Value, end: u64) -> Result<bool> {
    let next = row["next"][0].as_u64().context("actual B peer frontier")?;
    ensure!(
        next <= end
            && row["next"][1] == 641
            && row["counts"]["debt"] == 1
            && row["counts"]["queued"] == 0,
        "no barrier overshoot/B640 debt loss"
    );
    Ok(next == end && row["counts"]["pending"] == 0 && row["head_pending"].is_null())
}
pub(super) fn barrier(
    page: &observations::PageObservation,
    key: oracle::Incarnation,
    local: u64,
    progress: u64,
    target: u64,
) -> Result<()> {
    ensure!(
        page.stamp.incarnation == key
            && page.context == 0
            && !page.head
            && page.local_page == local
            && progress == target,
        "exact actual page target/barrier"
    );
    Ok(())
}
pub(super) fn final_c1(rows: &[Value; 2], debt: u64) -> Result<()> {
    ensure!(
        rows[0]["next"] == json!([641, 640])
            && rows[1]["next"] == json!([641, 641])
            && rows[0]["counts"]["debt"] == debt
            && rows[1]["counts"]["debt"] == 1
            && rows.iter().all(|r| r["counts"]["queued"] == 0
                && r["counts"]["pending"] == 0
                && r["head_pending"].is_null()),
        "actual caught-up frontiers/debts/queues/head obligations"
    );
    Ok(())
}
pub(super) fn ordered_history(
    rows: &[Value],
    owner: usize,
    ids: &[(u64, old::Id)],
    held: &Value,
) -> Result<BTreeMap<String, Value>> {
    ensure!(
        owner < 2 && rows.len() == 1279 + owner,
        "actual C1 A1279/B1280 Text histories while B640 held"
    );
    let mut next = [1u64; 2];
    for r in rows {
        let role = r["role"].as_u64().context("actual history role")? as usize;
        ensure!(
            role < 2 && r["ordinal"] == next[role],
            "actual per-lane history ordering/contiguity"
        );
        let n = next[role];
        let request = oracle::Request::new(0, role as u8, n.try_into()?)?;
        ensure!(
            r["request"] == source::request_id(request)
                && r["text"] == request.body()
                && r["format"] == if n < 25 { 1 } else { 2 },
            "actual history request/body/format"
        );
        if role == 0 && n >= 385 {
            ensure!(
                ids.get((n - 385) as usize)
                    == Some(&(n, state::parse_id(r["id"].as_str().context("history ID")?)?)),
                "same original P7 source in public history"
            );
        }
        if role == 1 && n == 640 {
            ensure!(r["id"] == held["id"], "same original held B640");
        }
        next[role] += 1;
    }
    ensure!(
        next == if owner == 0 { [641, 640] } else { [641, 641] },
        "exact complete lane frontiers in history"
    );
    source::history_map(rows)
}
pub(super) fn history_pair(
    a: &BTreeMap<String, Value>,
    b: &BTreeMap<String, Value>,
    held: &Value,
) -> Result<()> {
    let id = held["id"].as_str().context("original held B640 ID")?;
    ensure!(
        a.len() == 1279
            && b.len() == 1280
            && !a.contains_key(id)
            && b.contains_key(id)
            && a.iter().all(|(id, row)| b.get(id) == Some(row)),
        "one exact B640 difference, same remaining sources/bodies"
    );
    Ok(())
}
pub(super) fn ack_sources(
    ack: &observations::AckObservation,
    map: &BTreeMap<String, Value>,
    ids: &[(u64, old::Id)],
) -> Result<()> {
    ensure!(
        ack.context == 0
            && !ack.query_reply
            && ack.lane == 0
            && ack.from == Some(385)
            && !ack.receipts.is_empty()
            && ack.receipts.iter().any(|r| r.ordinal >= 385),
        "actual targeted offline B push/lane0, not a query or old-only duplicate"
    );
    for r in &ack.receipts {
        let actual = map
            .get(&state::hex(&r.event))
            .context("every ACK tuple has real source history, including older duplicates")?;
        ensure!(
            r.lane == 0
                && actual["role"] == 0
                && actual["ordinal"] == r.ordinal
                && actual["format"] == r.source_format,
            "actual mixed receipt/source association"
        );
        if r.ordinal >= 385 {
            ensure!(
                r.ordinal <= 640
                    && r.encoding == 2
                    && ids.get((r.ordinal - 385) as usize) == Some(&(r.ordinal, r.event)),
                "eligible ACK refers to original outstanding P7 source"
            );
        }
    }
    Ok(())
}
pub(super) fn replay(
    old: &observations::AckObservation,
    new: &observations::AckObservation,
) -> Result<()> {
    ensure!(
        old.binding != new.binding
            && old.request != new.request
            && old.stamp.incarnation == new.stamp.incarnation
            && old.stamp.line < new.stamp.line
            && old.context == new.context
            && old.lane == new.lane
            && old.from == new.from
            && old.receipts == new.receipts
            && old.digest == new.digest,
        "new dispatch/binding/request retains exactly saved receipt tuples and digest"
    );
    Ok(())
}
pub(super) struct Work {
    baseline: [oracle::Counts; 2],
    open: [bool; 2],
    pieces: Vec<(usize, oracle::Counts)>,
}
impl Work {
    pub fn new(baseline: [oracle::Counts; 2]) -> Self {
        Self {
            baseline,
            open: [true; 2],
            pieces: vec![],
        }
    }
    pub fn close(&mut self, owner: usize, actual: oracle::Counts) -> Result<()> {
        ensure!(
            owner < 2 && self.open[owner] && self.pieces.len() < 8,
            "bounded unique reconciled incarnation close"
        );
        let d = actual.delta(self.baseline[owner])?;
        ensure!(
            d.encrypt == 0
                && d.aborted == 0
                && d.ambiguous == 0
                && d.accounts == 0
                && d.outbound == 0
                && d.inbound == 0
                && d.attempts == d.decrypt
                && (owner == 1 || d.decrypt == 0),
            "actual P9-only packet effects without constructors/encrypt/A/C2 work"
        );
        self.pieces.push((owner, d));
        self.open[owner] = false;
        Ok(())
    }
    pub fn reopen(&mut self, owner: usize, actual: oracle::Counts) -> Result<()> {
        ensure!(
            owner < 2 && !self.open[owner] && actual == oracle::Counts::default(),
            "actual new held incarnation has zero packet/constructor effects"
        );
        self.baseline[owner] = actual;
        self.open[owner] = true;
        Ok(())
    }
    pub fn terminal(&self) -> Result<Value> {
        ensure!(
            self.open == [false; 2],
            "both reconciled stops required before terminal totals"
        );
        let decrypt = self.pieces.iter().try_fold(0u64, |n, (_, d)| {
            n.checked_add(d.decrypt).context("actual work overflow")
        })?;
        ensure!(
            decrypt == 256,
            "P9 actual B decrypt256 across observed/reconciled incarnations"
        );
        Ok(
            json!({"actual_decrypt":decrypt,"incarnations":self.pieces.iter().map(|(o,d)|json!({"owner":o,"encrypt":d.encrypt,"decrypt":d.decrypt,"attempts":d.attempts,"aborted":d.aborted,"ambiguous":d.ambiguous,"accounts":d.accounts,"outbound":d.outbound,"inbound":d.inbound})).collect::<Vec<_>>() }),
        )
    }
}
