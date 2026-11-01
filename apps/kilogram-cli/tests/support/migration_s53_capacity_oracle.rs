//! Small G2-only observations; no exhausted fixture or process execution in tests.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Item {
    pub format: u8,
    pub id: String,
    pub role: u8,
    pub ordinal: u64,
    pub sequence: u64,
    pub request: String,
    pub text: String,
    pub proof_kind: String,
    pub delivered: bool,
}
pub(super) type History = BTreeMap<String, Item>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    version: u8,
    view: String,
    #[serde(rename = "F")]
    frontier: [u64; 2],
    finished: bool,
    cursor: String,
    items: Vec<Item>,
}
pub(super) fn history(raw: Value, next: [u64; 2], count: usize) -> Result<History> {
    let page: Page = serde_json::from_value(raw)?;
    state::parse_id(&page.view)?;
    db::HistoryCursor::from_base64(&page.cursor)?;
    ensure!(
        page.version == 3 && page.frontier == next && page.finished,
        "G2 full tiny history"
    );
    items(page.items, next, count)
}
fn items(items: Vec<Item>, next: [u64; 2], count: usize) -> Result<History> {
    ensure!(items.len() == count && count <= 4, "G2 exact history count");
    let mut result = History::new();
    for row in items {
        state::parse_id(&row.id)?;
        ensure!(
            row.role < 2 && row.ordinal > 0 && row.ordinal < next[row.role as usize],
            "G2 history scope"
        );
        ensure!(
            row.request.len() == 32
                && row
                    .request
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "G2 request encoding"
        );
        ensure!(
            row.delivered && matches!(row.format, 1 | 2),
            "G2 delivered history item"
        );
        ensure!(
            result.insert(row.id.clone(), row).is_none(),
            "G2 duplicate history ID"
        );
    }
    Ok(result)
}
pub(super) fn preserved(before: &History, after: &History) -> Result<()> {
    for (id, row) in before {
        ensure!(
            after.get(id) == Some(row),
            "G2 earlier history changed: {id}"
        );
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Request {
    pub queue: u64,
    pub event: Option<String>,
    pub delivered: bool,
}
pub(super) fn request(value: Value) -> Result<Option<Request>> {
    let tuples: Vec<(Vec<u64>, Vec<String>, u64)> = serde_json::from_value(value)?;
    ensure!(tuples.len() <= 1, "G2 request option cardinality");
    let Some((queue, event, delivered)) = tuples.into_iter().next() else {
        return Ok(None);
    };
    ensure!(
        queue.len() == 1 && event.len() <= 1 && delivered <= 1,
        "G2 own request shape/Some(queue)"
    );
    if let Some(id) = event.first() {
        state::parse_id(id)?;
    }
    ensure!(
        delivered == 0 || event.len() == 1,
        "G2 delivered request needs ID"
    );
    Ok(Some(Request {
        queue: queue[0],
        event: event.into_iter().next(),
        delivered: delivered == 1,
    }))
}
pub(super) fn stable(before: &Request, after: &Request) -> Result<()> {
    ensure!(
        before.queue == after.queue && (!before.delivered || after.delivered),
        "G2 stable queue/delivery"
    );
    if let Some(id) = &before.event {
        ensure!(after.event.as_ref() == Some(id), "G2 stable EventId");
    }
    Ok(())
}
pub(super) fn send_result(raw: &str) -> Result<Request> {
    let (queue, event): (Vec<u64>, Vec<String>) = serde_json::from_str(raw)?;
    ensure!(queue.len() == 1 && event.len() <= 1, "G2 queue-call shape");
    if let Some(id) = event.first() {
        state::parse_id(id)?;
    }
    Ok(Request {
        queue: queue[0],
        event: event.into_iter().next(),
        delivered: false,
    })
}
pub(super) fn message(
    history: &History,
    request: &Request,
    round: usize,
    role: u8,
    n: u64,
) -> Result<()> {
    let id = request.event.as_ref().context("G2 delivered EventId")?;
    ensure!(request.delivered, "G2 actual receipt flag");
    let row = history
        .get(id)
        .context("G2 missing delivery history item")?;
    ensure!(
        row.format == 2
            && row.proof_kind == "FullMixed"
            && row.role == role
            && row.ordinal == n
            && row.request == request_id(round)
            && row.text == body(round)
            && row.delivered,
        "G2 exact delivered message"
    );
    ensure!(
        history
            .values()
            .filter(|row| row.request == request_id(round))
            .count()
            == 1,
        "G2 duplicate request"
    );
    Ok(())
}
pub(super) fn request_id(round: usize) -> String {
    format!("fa00000000000000{:016x}", 0xc200 + round)
}
pub(super) fn request_bytes(round: usize) -> [u8; 16] {
    let mut id = [0; 16];
    id[0] = 0xfa;
    id[8..].copy_from_slice(&(0xc200 + round as u64).to_be_bytes());
    id
}
pub(super) fn body(round: usize) -> &'static str {
    if round == 0 {
        "G2 capacity-committed delivery"
    } else {
        "G2 profile-staged delivery"
    }
}
pub(super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value
        .get(name)
        .with_context(|| format!("G2 required field {name}"))
}
pub(super) fn number(value: &Value, name: &str) -> Result<u64> {
    field(value, name)?
        .as_u64()
        .with_context(|| format!("G2 unsigned {name}"))
}
pub(super) fn lineage(row: &Value) -> Result<Value> {
    let mut result = serde_json::Map::new();
    for name in ["context", "d", "e", "q", "native_hash"] {
        let v = field(row, name)?.as_str().context("G2 lineage ID")?;
        state::parse_id(v)?;
        if name != "native_hash" {
            result.insert(name.into(), json!(v));
        }
    }
    result.insert(
        "sid".into(),
        json!(field(row, "sid")?.as_str().context("G2 SID")?),
    );
    let cuts: [u64; 2] = serde_json::from_value(field(row, "cuts")?.clone())?;
    let keys: Vec<String> = serde_json::from_value(field(field(row, "native")?, "keys")?.clone())?;
    ensure!(!keys.is_empty(), "G2 Native keys present");
    for key in &keys {
        state::parse_id(key)?;
    }
    result.insert("cuts".into(), json!(cuts));
    result.insert("keys".into(), json!(keys));
    number(row, "native_generation")?;
    Ok(Value::Object(result))
}
pub(super) fn admitted(status: &Value, node: &Node) -> Result<Value> {
    let row = node.context(status)?;
    lineage(row)?;
    ensure!(
        field(row, "mode")?.as_str() == Some("Active")
            && number(row, "lease_family")? == 2
            && number(row, "role")? < 2,
        "G2 selected family2"
    );
    for flag in [
        "enabled",
        "active",
        "lease_current",
        "publication_ready",
        "existing_origin_ready",
    ] {
        ensure!(
            field(row, flag)?.as_bool() == Some(true),
            "G2 admission {flag}"
        );
    }
    for error in [
        "policy_error",
        "handoff_policy_error",
        "publication_error",
        "origin_publication_error",
    ] {
        ensure!(field(row, error)?.is_null(), "G2 admission {error}");
    }
    for name in ["queued", "pending", "debt"] {
        ensure!(
            number(field(row, "counts")?, name)? == 0,
            "G2 quiescence {name}"
        );
    }
    ensure!(
        field(row, "control_debt")?.as_bool() == Some(false),
        "G2 control quiescence"
    );
    Ok(row.clone())
}
pub(super) fn zero_work(status: &Value) -> Result<()> {
    for field in [
        "packet_attempts",
        "committed_encrypt",
        "committed_decrypt",
        "materialized",
        "readable",
        "aborted_packet_calls",
        "ambiguous_packet_calls",
        "account_constructors",
        "outbound_constructors",
        "inbound_constructors",
    ] {
        ensure!(
            number(self::field(status, "stats")?, field)? == 0,
            "G2 fresh-process no work {field}"
        );
    }
    Ok(())
}
pub(super) fn locator_revision(old: u64, new: u64, old_raw: &[u8], new_raw: &[u8]) -> Result<()> {
    ensure!(
        new >= old && (new != old || old_raw == new_raw),
        "G2 locator non-regression/equal-revision identity"
    );
    Ok(())
}
pub(super) fn exact(before: &Value, after: &Value) -> Result<()> {
    exact_at("G2 stopped", before, after)
}
pub(super) fn exact_at(path: &str, before: &Value, after: &Value) -> Result<()> {
    fn short(value: Option<&Value>) -> String {
        value.map_or_else(
            || "<missing>".into(),
            |v| v.to_string().chars().take(256).collect(),
        )
    }
    fn difference(
        path: &str,
        a: Option<&Value>,
        b: Option<&Value>,
        depth: usize,
    ) -> Option<String> {
        if a == b {
            return None;
        }
        if depth < 12 {
            match (a, b) {
                (Some(Value::Object(a)), Some(Value::Object(b))) => {
                    for key in a
                        .keys()
                        .chain(b.keys())
                        .collect::<std::collections::BTreeSet<_>>()
                    {
                        if let Some(diff) =
                            difference(&format!("{path}/{key}"), a.get(key), b.get(key), depth + 1)
                        {
                            return Some(diff);
                        }
                    }
                }
                (Some(Value::Array(a)), Some(Value::Array(b))) => {
                    for index in 0..a.len().max(b.len()) {
                        if let Some(diff) = difference(
                            &format!("{path}/{index}"),
                            a.get(index),
                            b.get(index),
                            depth + 1,
                        ) {
                            return Some(diff);
                        }
                    }
                }
                _ => {}
            }
        }
        Some(format!("{path}: before={} after={}", short(a), short(b)))
    }
    if let Some(diff) = difference(path, Some(before), Some(after), 0) {
        anyhow::bail!("G2 mismatch {diff}");
    }
    Ok(())
}

pub(super) fn ledger(credit: &Value) -> Result<Value> {
    let mut result = serde_json::Map::new();
    for key in ["repair_free", "grant_unissued", "pending"] {
        let value: [u64; 2] = serde_json::from_value(field(credit, key)?.clone())?;
        result.insert(key.into(), json!(value));
    }
    let head: [bool; 2] = serde_json::from_value(field(credit, "head_in_use")?.clone())?;
    result.insert("head_in_use".into(), json!(head));
    Ok(Value::Object(result))
}
pub(super) fn accounting(report: &Value, owner: usize) -> Result<store::L1Accounting> {
    let value: store::L1Accounting = serde_json::from_value(field(report, "accounting")?.clone())?;
    let credit = field(report, "credits")?;
    let used: [u64; 2] = serde_json::from_value(field(credit, "used")?.clone())?;
    let reserved: [u64; 2] = serde_json::from_value(field(credit, "reserved")?.clone())?;
    let quotas: [u64; 2] =
        serde_json::from_value(field(field(report, "capacity")?, "quotas")?.clone())?;
    let capacity = field(report, "capacity")?;
    exact_at(
        &format!("owner{owner}/physical-quota"),
        &json!(number(capacity, "configured_physical")?),
        &json!(number(capacity, "physical")?),
    )?;
    exact_at(
        &format!("owner{owner}/ledger"),
        field(field(report, "stable")?, "obligations")?,
        &ledger(credit)?,
    )?;
    for lane in 0..2 {
        let total_used = value.runtime.used[lane]
            .checked_add(value.prepaid_metadata.used[lane])
            .with_context(|| format!("G2 owner{owner}/aggregate/used/{lane}: overflow"))?;
        let total_reserved = value.runtime.reserved[lane]
            .checked_add(value.prepaid_metadata.reserved[lane])
            .with_context(|| format!("G2 owner{owner}/aggregate/reserved/{lane}: overflow"))?;
        exact_at(
            &format!("owner{owner}/aggregate/used/{lane}"),
            &json!(used[lane]),
            &json!(total_used),
        )?;
        exact_at(
            &format!("owner{owner}/aggregate/reserved/{lane}"),
            &json!(reserved[lane]),
            &json!(total_reserved),
        )?;
        let total = total_used
            .checked_add(total_reserved)
            .with_context(|| format!("G2 owner{owner}/quota/{lane}: total overflow"))?;
        ensure!(
            total <= quotas[lane],
            "G2 owner{owner}/quota/{lane}: funded={total} quota={}",
            quotas[lane]
        );
    }
    Ok(value)
}
pub(super) fn funded(group: &store::L1AccountingGroup) -> Result<[u64; 2]> {
    Ok([
        group.used[0]
            .checked_add(group.reserved[0])
            .context("G2 metadata funding overflow lane0")?,
        group.used[1]
            .checked_add(group.reserved[1])
            .context("G2 metadata funding overflow lane1")?,
    ])
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct LiveSnapshot {
    pub history: [History; 2],
    pub requests: Vec<Request>,
    roles: [u8; 2],
    native: [(String, u64, [u64; 2]); 2],
}
impl LiveSnapshot {
    // Called only with the real IPC observations already collected by live().
    pub(super) fn capture(
        pair: &Pair,
        full: &[Value; 2],
        history: [History; 2],
        requests: &[Request],
    ) -> Result<Self> {
        let mut roles = [0; 2];
        let mut native = Vec::new();
        for (local, status) in full.iter().enumerate() {
            let row = admitted(status, &pair.nodes[local])?;
            roles[local] = number(&row, "role")? as u8;
            native.push((
                field(&row, "native_hash")?
                    .as_str()
                    .context("G2 live Native hash")?
                    .into(),
                number(&row, "native_generation")?,
                serde_json::from_value(field(&row, "next")?.clone())?,
            ));
        }
        ensure!(roles[0] != roles[1], "G2 distinct owner roles");
        Ok(Self {
            history,
            requests: requests.to_vec(),
            roles,
            native: native
                .try_into()
                .map_err(|_| anyhow::anyhow!("G2 two Native observations"))?,
        })
    }
    pub(super) fn messages(&self, original: &Self) -> Result<()> {
        ensure!(
            self.roles == original.roles
                && original.requests.is_empty()
                && self.requests.len() <= 2,
            "G2 original/live scope"
        );
        for history in &self.history {
            ensure!(
                history.len() == 2 + self.requests.len(),
                "G2 complete original-plus-delivery history"
            );
        }
        for (local, history) in self.history.iter().enumerate() {
            preserved(&original.history[local], history)?;
            for (round, request) in self.requests.iter().enumerate() {
                let ordinal = original.native[0].2[self.roles[0] as usize]
                    .checked_add(round as u64)
                    .context("G2 author ordinal")?;
                message(history, request, round, self.roles[0], ordinal)?;
            }
        }
        Ok(())
    }
    pub(super) fn same(&self, reopened: &Self) -> Result<()> {
        exact_at(
            "saved-live/reopened",
            &serde_json::to_value(self)?,
            &serde_json::to_value(reopened)?,
        )
    }
    pub(super) fn join(&self, stopped: &[Value; 2]) -> Result<()> {
        for (local, report) in stopped.iter().enumerate() {
            let saved = field(report, "stable")?;
            let history: History = serde_json::from_value(field(saved, "history")?.clone())?;
            exact_at(
                &format!("owner{local}/live-stopped/history"),
                &serde_json::to_value(&self.history[local])?,
                &serde_json::to_value(history)?,
            )?;
            let native = (
                field(saved, "native_hash")?
                    .as_str()
                    .context("G2 stopped Native hash")?
                    .to_owned(),
                number(saved, "native_generation")?,
                serde_json::from_value::<[u64; 2]>(field(saved, "next")?.clone())?,
            );
            exact_at(
                &format!("owner{local}/live-stopped/native"),
                &json!(self.native[local]),
                &json!(native),
            )?;
            let own: BTreeMap<String, Request> =
                serde_json::from_value(field(saved, "delivered_requests")?.clone())?;
            let expected = if local == 0 {
                self.requests
                    .iter()
                    .enumerate()
                    .map(|(r, v)| (request_id(r), v.clone()))
                    .collect()
            } else {
                BTreeMap::new()
            };
            exact_at(
                &format!("owner{local}/live-stopped/requests"),
                &serde_json::to_value(expected)?,
                &serde_json::to_value(own)?,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item() -> Item {
        Item {
            format: 2,
            id: "11".repeat(32),
            role: 0,
            ordinal: 2,
            sequence: 2,
            request: request_id(0),
            text: body(0).into(),
            proof_kind: "FullMixed".into(),
            delivered: true,
        }
    }
    #[test]
    fn g2a_request_queue_and_id_are_stable() -> Result<()> {
        let a = request(json!([[[7], [], 0]]))?.context("pending")?;
        let b = request(json!([[[7], ["11".repeat(32)], 1]]))?.context("delivered")?;
        stable(&a, &b)?;
        for bad in [
            json!([[[], ["11".repeat(32)], 1]]),
            json!([[[7], [], 1]]),
            json!([[[7], null, 0]]),
        ] {
            assert!(request(bad).is_err());
        }
        assert!(
            stable(
                &b,
                &Request {
                    queue: 8,
                    ..b.clone()
                }
            )
            .is_err()
        );
        assert!(
            stable(
                &b,
                &Request {
                    event: Some("22".repeat(32)),
                    ..b.clone()
                }
            )
            .is_err()
        );
        assert!(stable(&b, &a).is_err());
        Ok(())
    }
    #[test]
    fn g2a_history_rejects_missing_wrong_and_duplicate_message() -> Result<()> {
        let row = item();
        let req = Request {
            queue: 7,
            event: Some(row.id.clone()),
            delivered: true,
        };
        let good = History::from([(row.id.clone(), row.clone())]);
        message(&good, &req, 0, 0, 2)?;
        assert!(message(&History::new(), &req, 0, 0, 2).is_err());
        let mut bad = good.clone();
        bad.get_mut(&row.id).context("row")?.text = "wrong".into();
        assert!(message(&bad, &req, 0, 0, 2).is_err());
        assert!(preserved(&good, &bad).is_err());
        let mut duplicate = row.clone();
        duplicate.id = "22".repeat(32);
        bad = good.clone();
        bad.insert(duplicate.id.clone(), duplicate);
        assert!(message(&bad, &req, 0, 0, 2).is_err());
        assert!(items(vec![row.clone()], [3, 2], 1).is_ok());
        assert!(items(vec![row.clone(), row], [3, 2], 2).is_err());
        let mut missing = serde_json::to_value(item())?;
        missing.as_object_mut().context("item")?.remove("text");
        assert!(serde_json::from_value::<Item>(missing).is_err());
        Ok(())
    }
    #[test]
    fn g2a_locator_valid_reuse_and_regression() -> Result<()> {
        locator_revision(4, 4, b"same", b"same")?;
        locator_revision(4, 5, b"old", b"new")?;
        assert!(locator_revision(4, 3, b"old", b"new").is_err());
        assert!(locator_revision(4, 4, b"old", b"new").is_err());
        Ok(())
    }
    fn snapshots() -> (LiveSnapshot, LiveSnapshot, [Value; 2]) {
        let mut a = item();
        a.format = 1;
        a.id = "22".repeat(32);
        a.ordinal = 1;
        a.text = "original A".into();
        a.request = "ec000000000000000000000000000001".into();
        a.proof_kind = "LegacySnapshot".into();
        let mut b = a.clone();
        b.id = "33".repeat(32);
        b.role = 1;
        b.text = "original B".into();
        b.request = "ec000000000000000000000000000002".into();
        let history = History::from([(a.id.clone(), a), (b.id.clone(), b)]);
        let original = LiveSnapshot {
            history: [history.clone(), history],
            requests: vec![],
            roles: [0, 1],
            native: [("aa".repeat(32), 3, [2, 2]), ("bb".repeat(32), 3, [2, 2])],
        };
        let mut live = original.clone();
        let row = item();
        for h in &mut live.history {
            h.insert(row.id.clone(), row.clone());
        }
        live.requests.push(Request {
            queue: 7,
            event: Some(row.id),
            delivered: true,
        });
        live.native = [("cc".repeat(32), 4, [3, 2]), ("dd".repeat(32), 4, [3, 2])];
        let stopped = std::array::from_fn(
            |local| json!({"stable":{"history":live.history[local],"native_hash":live.native[local].0,"native_generation":live.native[local].1,"next":live.native[local].2,"delivered_requests":if local==0 {BTreeMap::from([(request_id(0),live.requests[0].clone())])}else{BTreeMap::new()}}}),
        );
        (original, live, stopped)
    }
    #[test]
    fn g2a_live_stopped_join_rejects_one_sided_history_native_and_request() -> Result<()> {
        let (original, live, stopped) = snapshots();
        live.messages(&original)?;
        live.join(&stopped)?;
        for id in ["22".repeat(32), "11".repeat(32)] {
            let mut bad = stopped.clone();
            bad[1]["stable"]["history"][&id]["text"] = json!("changed only in stopped");
            assert!(live.join(&bad).is_err());
        }
        let mut missing = stopped.clone();
        missing[0]["stable"]["history"]
            .as_object_mut()
            .context("history")?
            .remove(&"22".repeat(32));
        assert!(live.join(&missing).is_err());
        let mut native = stopped.clone();
        native[1]["stable"]["native_generation"] = json!(5);
        assert!(live.join(&native).is_err());
        for (key, value) in [("queue", json!(8)), ("event", json!("44".repeat(32)))] {
            let mut bad = stopped.clone();
            bad[0]["stable"]["delivered_requests"][request_id(0)][key] = value;
            assert!(live.join(&bad).is_err());
        }
        Ok(())
    }
    #[test]
    fn g2a_saved_reopened_join_reapplies_original_and_exact_message() -> Result<()> {
        let (original, saved, _) = snapshots();
        saved.same(&saved)?;
        for id in ["22".repeat(32), "11".repeat(32)] {
            let mut reopened = saved.clone();
            reopened.history[1].get_mut(&id).context("row")?.text =
                "changed in reopened live".into();
            assert!(saved.same(&reopened).is_err());
            assert!(reopened.messages(&original).is_err());
        }
        let mut missing = saved.clone();
        missing.history[0].remove(&"22".repeat(32));
        assert!(saved.same(&missing).is_err());
        assert!(missing.messages(&original).is_err());
        let mut request = saved.clone();
        request.requests[0].event = Some("44".repeat(32));
        assert!(saved.same(&request).is_err());
        assert!(request.messages(&original).is_err());
        Ok(())
    }
    fn accounting_reports() -> Result<[Value; 2]> {
        let credit = json!({"used":[40,10],"reserved":[110,5],"repair_free":[64,0],"grant_unissued":[0,0],"pending":[0,0],"head_in_use":[false,false]});
        let report = json!({"stable":{"obligations":ledger(&credit)?},"shared_records":[],"profile":"public-profile-hash","capacity":{"quotas":[1000,1000],"physical":10000,"configured_physical":10000,"revision":1},"credits":credit,"accounting":{"runtime":{"records":4,"used":[30,10],"reserved":[20,5]},"prepaid_metadata":{"records":3,"used":[10,0],"reserved":[90,0]}}});
        Ok([report.clone(), report])
    }
    #[test]
    fn g2a_accounting_accepts_funded_redistribution_but_not_reopen_drift() -> Result<()> {
        let before = accounting_reports()?;
        witness::check(&before)?;
        witness::persisted(&before, &before)?;
        for delta in [1, 7, 45] {
            let mut after = before.clone();
            after[0]["accounting"]["prepaid_metadata"]["used"][0] = json!(10 + delta);
            after[0]["accounting"]["prepaid_metadata"]["reserved"][0] = json!(90 - delta);
            after[0]["credits"]["used"][0] = json!(40 + delta);
            after[0]["credits"]["reserved"][0] = json!(110 - delta);
            witness::capacity_only(&before, &after)?;
            assert!(witness::resumed(&before, &after).is_err());
            assert!(witness::persisted(&before, &after).is_err());
        }
        Ok(())
    }
    #[test]
    fn g2a_accounting_rejects_runtime_ledger_reconciliation_and_funding_changes() -> Result<()> {
        let before = accounting_reports()?;
        let mut runtime = before.clone();
        runtime[1]["accounting"]["runtime"]["reserved"][0] = json!(21);
        runtime[1]["credits"]["reserved"][0] = json!(111);
        let error = witness::capacity_only(&before, &runtime)
            .err()
            .context("nonmetadata reserve must fail")?
            .to_string();
        assert!(error.contains("owner1/runtime-accounting/reserved/0"));
        let mut changed = before.clone();
        changed[0]["credits"]["pending"][0] = json!(1);
        changed[0]["stable"]["obligations"]["pending"][0] = json!(1);
        assert!(witness::capacity_only(&before, &changed).is_err());
        let mut bad = before.clone();
        bad[0]["credits"]["used"][0] = json!(41);
        assert!(witness::check(&bad).is_err());
        let mut overflow = before.clone();
        overflow[0]["accounting"]["runtime"]["used"][0] = json!(u64::MAX);
        assert!(witness::check(&overflow).is_err());
        let mut quota = before.clone();
        quota[0]["capacity"]["quotas"][0] = json!(149);
        assert!(witness::check(&quota).is_err());
        let mut physical = before.clone();
        physical[0]["capacity"]["physical"] = json!(10001);
        assert!(witness::check(&physical).is_err());
        let mut funding = before.clone();
        funding[0]["accounting"]["prepaid_metadata"]["reserved"][0] = json!(91);
        funding[0]["credits"]["reserved"][0] = json!(111);
        assert!(witness::capacity_only(&before, &funding).is_err());
        let mut count = before.clone();
        count[0]["accounting"]["prepaid_metadata"]["records"] = json!(4);
        assert!(witness::capacity_only(&before, &count).is_err());
        Ok(())
    }
    #[test]
    fn g2a_mismatch_diagnostic_has_owner_path_and_bounded_public_values() -> Result<()> {
        let error = exact_at(
            "owner1/stable",
            &json!({"public":"a".repeat(10000)}),
            &json!({"public":"b".repeat(10000)}),
        )
        .err()
        .context("different public values")?
        .to_string();
        assert!(
            error.contains("owner1/stable/public")
                && error.contains("before=")
                && error.contains("after=")
        );
        assert!(error.len() < 1024);
        Ok(())
    }
}
