//! G3 public association predicates; cryptographic verification stays in protocol APIs.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Mapping {
    pub owner: usize,
    pub context: String,
    pub slot: u8,
    pub role: u8,
}
pub(super) fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value.get(key).with_context(|| format!("G3 required {key}"))
}
pub(super) fn number(value: &Value, key: &str) -> Result<u64> {
    field(value, key)?
        .as_u64()
        .with_context(|| format!("G3 unsigned {key}"))
}
pub(super) fn row<'a>(status: &'a Value, context: &str) -> Result<&'a Value> {
    let rows = field(status, "contexts")?
        .as_array()
        .context("G3 contexts array")?;
    let found = rows
        .iter()
        .filter(|v| v["context"] == context)
        .collect::<Vec<_>>();
    ensure!(found.len() == 1, "G3 unique context {context}");
    Ok(found[0])
}
pub(super) fn mappings(status: &[Value; 3], contexts: &[String; 2]) -> Result<Vec<Mapping>> {
    ensure!(contexts[0] != contexts[1], "G3 distinct contexts");
    let mut result = Vec::new();
    for (owner, wanted) in [(0, vec![0]), (1, vec![0, 1]), (2, vec![1])] {
        let rows = field(&status[owner], "contexts")?
            .as_array()
            .context("G3 contexts")?;
        ensure!(
            rows.len() == wanted.len(),
            "G3 owner{owner} exact context set"
        );
        let mut slots = BTreeSet::new();
        for index in wanted {
            state::parse_id(&contexts[index])?;
            let r = row(&status[owner], &contexts[index])?;
            let slot = u8::try_from(number(r, "slot")?)?;
            let role = u8::try_from(number(r, "role")?)?;
            ensure!(
                slot < 2 && role < 2 && slots.insert(slot) && r["enabled"] == true,
                "G3 owner{owner} slot/role/enabled"
            );
            result.push(Mapping {
                owner,
                context: contexts[index].clone(),
                slot,
                role,
            });
        }
    }
    for context in contexts {
        let roles = result
            .iter()
            .filter(|m| &m.context == context)
            .map(|m| m.role)
            .collect::<BTreeSet<_>>();
        ensure!(
            roles == BTreeSet::from([0, 1]),
            "G3 opposite roles per context"
        );
    }
    Ok(result)
}
pub(super) fn ready_mappings(ready: &[Value], maps: &[Mapping]) -> Result<()> {
    ensure!(ready.len() == 3, "G3 Ready owner count");
    for (owner, output) in ready.iter().enumerate() {
        ensure!(
            output["storage_stage"] == "Ready" && output["zero_migration_native_calls"] == true,
            "G3 owner{owner} real Ready"
        );
        let rows = field(output, "contexts")?
            .as_array()
            .context("G3 Ready contexts")?;
        ensure!(
            rows.len() == maps.iter().filter(|m| m.owner == owner).count(),
            "G3 owner{owner} Ready context count"
        );
        for (slot, row) in rows.iter().enumerate() {
            let m = maps
                .iter()
                .find(|m| m.owner == owner && usize::from(m.slot) == slot)
                .context("G3 Ready resolved slot")?;
            ensure!(
                row["context"] == m.context && number(row, "role")? == u64::from(m.role),
                "G3 owner{owner}/slot{slot} Ready/status mapping"
            );
        }
    }
    Ok(())
}
pub(super) fn stopped_owners(present: [bool; 3]) -> Result<()> {
    ensure!(
        !present.into_iter().any(|p| p),
        "G3 actual owner set must be stopped/reaped; Pair views are not ownership"
    );
    Ok(())
}
pub(super) fn owner_presence(present: [bool; 3], required: [bool; 3]) -> Result<()> {
    ensure!(
        present == required,
        "G3 real handles must match the required owner subset"
    );
    Ok(())
}
pub(super) fn exact(path: &str, before: &Value, after: &Value) -> Result<()> {
    if before == after {
        return Ok(());
    }
    if let (Some(a), Some(b)) = (before.as_object(), after.as_object()) {
        for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
            exact(
                &format!("{path}/{key}"),
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
            )?;
        }
    } else if let (Some(a), Some(b)) = (before.as_array(), after.as_array()) {
        for i in 0..a.len().max(b.len()) {
            exact(
                &format!("{path}/{i}"),
                a.get(i).unwrap_or(&Value::Null),
                b.get(i).unwrap_or(&Value::Null),
            )?;
        }
    }
    anyhow::bail!(
        "G3 {path}: before={} after={}",
        before.to_string().chars().take(192).collect::<String>(),
        after.to_string().chars().take(192).collect::<String>()
    )
}
pub(super) fn stable(row: &Value) -> Result<Value> {
    let mut result = serde_json::Map::new();
    for key in [
        "context",
        "role",
        "d",
        "e",
        "sid",
        "q",
        "cuts",
        "controls",
        "current_consent",
        "current_consent2",
        "mode",
        "native_hash",
        "native_generation",
        "next",
        "counts",
    ] {
        result.insert(key.into(), field(row, key)?.clone());
    }
    Ok(Value::Object(result))
}
pub(super) fn paused(row: &Value) -> bool {
    row["lease_current"] == false
        && row
            .get("handoff_policy_error")
            .is_some_and(|v| !v.is_null())
        && row.get("policy_error").is_some_and(|v| !v.is_null())
}
pub(super) fn pin(before: &Value, after: &Value, contexts: &[String; 2]) -> Result<()> {
    for context in contexts {
        exact(
            &format!("X/{context}/pin/data"),
            &stable(row(before, context)?)?,
            &stable(row(after, context)?)?,
        )?;
    }
    let a = row(before, &contexts[0])?;
    let b = row(after, &contexts[0])?;
    ensure!(paused(b), "G3 X/C1 current pin pause and lease loss");
    ensure!(
        number(a, "policy_generation")?.checked_add(1) == Some(number(b, "policy_generation")?),
        "G3 C1 affected policy transition"
    );
    for key in ["policy_generation", "repair_free", "head_pending"] {
        exact(
            &format!("X/C2/pin/{key}"),
            field(row(before, &contexts[1])?, key)?,
            field(row(after, &contexts[1])?, key)?,
        )?;
    }
    admitted(after, &contexts[1])?;
    Ok(())
}
pub(super) fn admitted(status: &Value, context: &str) -> Result<()> {
    let r = row(status, context)?;
    for key in [
        "enabled",
        "active",
        "lease_current",
        "publication_ready",
        "existing_origin_ready",
    ] {
        ensure!(r[key] == true, "G3 {context} admission {key}");
    }
    ensure!(
        r["mode"] == "Active" && r["lease_family"] == 2,
        "G3 active selected admission"
    );
    for key in [
        "policy_error",
        "handoff_policy_error",
        "publication_error",
        "origin_publication_error",
    ] {
        ensure!(field(r, key)?.is_null(), "G3 {context} admission {key}");
    }
    Ok(())
}
pub(super) fn resources(status: &Value, enabled: &[u8]) -> Result<()> {
    ensure!(
        !enabled.is_empty()
            && enabled.len() <= 2
            && enabled.iter().copied().collect::<BTreeSet<_>>().len() == enabled.len(),
        "G3 resource owner enabled set"
    );
    let window: [[u64; 2]; 3] = serde_json::from_value(field(status, "window")?.clone())?;
    let mut limits = [0; 2];
    for slot in enabled {
        ensure!(*slot < 2, "G3 resource slot");
        limits[*slot as usize] = if enabled.len() == 2 { 64 } else { 128 };
    }
    ensure!(window[0] == limits, "G3 owner-scoped reference limits");
    for (slot, limit) in limits.into_iter().enumerate() {
        ensure!(
            window[1][slot] <= limit && window[2][slot] <= limit,
            "G3 current/peak reference slot{slot}"
        );
    }
    let r = field(status, "resources")?;
    for (current, peak, limit, cap) in [
        ("connections", "peak_connections", "connection_limit", 2),
        ("mailbox", "peak_mailbox", "mailbox_limit", 16),
    ] {
        ensure!(
            number(r, limit)? == cap && number(r, current)? <= cap && number(r, peak)? <= cap,
            "G3 owner {current}/{peak} bound"
        );
    }
    Ok(())
}

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
pub(super) fn history(raw: Value, frontier: [u64; 2]) -> Result<History> {
    let page: Page = serde_json::from_value(raw)?;
    state::parse_id(&page.view)?;
    db::HistoryCursor::from_base64(&page.cursor)?;
    ensure!(
        page.version == 3 && page.finished && page.frontier == frontier && page.items.len() <= 4,
        "G3 full tiny history"
    );
    let mut result = History::new();
    for row in page.items {
        state::parse_id(&row.id)?;
        ensure!(
            row.role < 2
                && row.ordinal > 0
                && row.ordinal < frontier[row.role as usize]
                && row.delivered
                && matches!(row.format, 1 | 2),
            "G3 readable history scope"
        );
        ensure!(
            result.insert(row.id.clone(), row).is_none(),
            "G3 duplicate history ID"
        );
    }
    Ok(result)
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct Request {
    pub queue: u64,
    pub event: Option<String>,
    pub delivered: bool,
}
pub(super) fn request(raw: Value) -> Result<Option<Request>> {
    let rows: Vec<(Vec<u64>, Vec<String>, u64)> = serde_json::from_value(raw)?;
    ensure!(rows.len() <= 1, "G3 option cardinality");
    let Some((q, event, delivered)) = rows.into_iter().next() else {
        return Ok(None);
    };
    ensure!(
        q.len() == 1 && event.len() <= 1 && delivered <= 1 && (delivered == 0 || event.len() == 1),
        "G3 request Some(queue)/EventId/delivery"
    );
    if let Some(id) = event.first() {
        state::parse_id(id)?;
    }
    Ok(Some(Request {
        queue: q[0],
        event: event.into_iter().next(),
        delivered: delivered == 1,
    }))
}
pub(super) fn sent(raw: &str) -> Result<Request> {
    let (q, e): (Vec<u64>, Vec<String>) = serde_json::from_str(raw)?;
    request(json!([[q, e, 0]]))?.context("G3 accepted send")
}
pub(super) fn request_id(id: u64) -> String {
    format!("fa00000000000000{id:016x}")
}
pub(super) fn request_bytes(id: u64) -> [u8; 16] {
    let mut out = [0; 16];
    out[0] = 0xfa;
    out[8..].copy_from_slice(&id.to_be_bytes());
    out
}
pub(super) fn request_stable(before: &Request, after: &Request) -> Result<()> {
    ensure!(
        before.queue == after.queue
            && (!before.delivered || after.delivered)
            && before
                .event
                .as_ref()
                .is_none_or(|id| after.event.as_ref() == Some(id)),
        "G3 stable request queue/EventId/delivery"
    );
    Ok(())
}
#[derive(Clone, Debug, Serialize)]
pub(super) struct Directed {
    pub context: String,
    pub author: usize,
    pub role: u8,
    pub id: u64,
    pub body: String,
    pub ordinal: u64,
    pub sequence: u64,
    pub previous: Option<old::Id>,
    pub saved: Request,
}
impl Directed {
    pub fn history(&self, before: &History, after: &History) -> Result<()> {
        ensure!(after.len() == before.len() + 1, "G3 one directed row");
        for (id, row) in before {
            ensure!(after.get(id) == Some(row), "G3 preserved earlier row {id}");
        }
        ensure!(self.saved.delivered, "G3 real receipt flag");
        let id = self.saved.event.as_ref().context("G3 delivered ID")?;
        let row = after.get(id).context("G3 delivered history row")?;
        ensure!(
            row.format == 2
                && row.proof_kind == "FullMixed"
                && row.role == self.role
                && row.ordinal == self.ordinal
                && row.sequence == self.sequence
                && row.request == request_id(self.id)
                && row.text == self.body
                && row.delivered,
            "G3 exact directed history association"
        );
        Ok(())
    }
    pub fn core(&self, c: &wire::Core2) -> Result<()> {
        ensure!(
            c.c == state::parse_id(&self.context)?
                && c.role == self.role
                && c.n == self.ordinal
                && c.sequence == self.sequence
                && c.previous == self.previous
                && c.request == request_bytes(self.id)
                && c.kind == 3,
            "G3 signed directed core association"
        );
        Ok(())
    }
}
pub(super) fn proof_joins(
    proof: &store::L1ProofRefs,
    author: old::Id,
    receiver: old::Id,
    receipt: old::Id,
    own: bool,
) -> Result<()> {
    ensure!(
        proof.original == author
            && proof.receipt_overlay == Some(receipt)
            && proof.receiver_overlay == Some(receiver),
        "G3 actual proof/source/receipt/receiver association"
    );
    ensure!(
        if own {
            proof.admission.is_none()
                && proof.final_auth.is_some()
                && proof.receiver.is_none()
                && proof.receipt.is_none()
        } else {
            proof.admission.is_some()
                && proof.final_auth.is_some()
                && proof.receiver == Some(receiver)
                && proof.receipt == Some(receipt)
        },
        "G3 applicable full proof captures"
    );
    Ok(())
}
// Imported completed Legacy1 requests retain only EventId, not a live queue order.
// Current Event2 queue identity is additionally checked by stored_request below.
pub(super) fn stored_own_request(
    saved: &store::L1Request,
    event: old::Id,
    text: &str,
    format: u8,
) -> Result<()> {
    let queue = match format {
        1 => saved.queue_sequence.is_none(),
        2 => saved.queue_sequence.is_some(),
        _ => false,
    };
    ensure!(
        queue
            && saved.event == Some(event)
            && saved.text_hash == old::hash(b"kg:od1:request-text\0", text.as_bytes()),
        "G3 stored own request/body/source"
    );
    Ok(())
}
pub(super) fn stored_request(saved: &store::L1Request, wanted: &Directed) -> Result<()> {
    ensure!(
        saved.queue_sequence == Some(wanted.saved.queue)
            && saved.event
                == wanted
                    .saved
                    .event
                    .as_deref()
                    .map(state::parse_id)
                    .transpose()?
            && saved.text_hash == old::hash(b"kg:od1:request-text\0", wanted.body.as_bytes()),
        "G3 actual stored queue/EventId/body association"
    );
    Ok(())
}
pub(super) fn progress(
    before: &[Option<Value>; 3],
    after: &[Option<Value>; 3],
    maps: &[Mapping],
    directed: &Directed,
) -> Result<()> {
    ensure!(
        before.each_ref().map(Option::is_some) == after.each_ref().map(Option::is_some),
        "G3 progress live subset remains exact"
    );
    for m in maps.iter().filter(|m| m.context == directed.context) {
        let a = row(
            before[m.owner]
                .as_ref()
                .context("G3 live sender/receiver before")?,
            &m.context,
        )?;
        let b = row(
            after[m.owner]
                .as_ref()
                .context("G3 live sender/receiver after")?,
            &m.context,
        )?;
        let mut next: [u64; 2] = serde_json::from_value(field(a, "next")?.clone())?;
        next[directed.role as usize] = next[directed.role as usize]
            .checked_add(1)
            .context("G3 ordinal overflow")?;
        exact("G3 directed next", &json!(next), field(b, "next")?)?;
        ensure!(
            number(b, "native_generation")? > number(a, "native_generation")?,
            "G3 legitimate Native advance"
        );
        for key in [
            "context",
            "role",
            "d",
            "e",
            "sid",
            "q",
            "cuts",
            "controls",
            "current_consent",
            "current_consent2",
            "mode",
            "policy_generation",
        ] {
            exact(
                &format!("owner{}/{key}", m.owner),
                field(a, key)?,
                field(b, key)?,
            )?;
        }
    }
    for owner in 0..3 {
        let Some(prior) = before[owner].as_ref() else {
            continue;
        };
        let a = field(prior, "stats")?;
        let b = field(
            after[owner].as_ref().context("G3 live stats owner")?,
            "stats",
        )?;
        let participant = maps
            .iter()
            .any(|m| m.owner == owner && m.context == directed.context);
        for (name, delta) in [
            ("committed_encrypt", u64::from(owner == directed.author)),
            ("materialized", u64::from(owner == directed.author)),
            (
                "committed_decrypt",
                u64::from(participant && owner != directed.author),
            ),
            (
                "readable",
                u64::from(participant && owner != directed.author),
            ),
        ] {
            ensure!(
                number(a, name)?.checked_add(delta) == Some(number(b, name)?),
                "G3 exact directed work owner{owner}/{name}"
            );
        }
        for name in [
            "account_constructors",
            "outbound_constructors",
            "inbound_constructors",
            "aborted_packet_calls",
            "ambiguous_packet_calls",
        ] {
            ensure!(
                number(b, name)? == 0,
                "G3 no constructor/abort owner{owner}/{name}"
            );
        }
    }
    Ok(())
}
