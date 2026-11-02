//! Shared validation gates are exercised directly with synthetic metadata.
use super::*;
pub(super) fn mapping(
    owner: usize,
    contexts: &[(old::Id, u8)],
    original: [old::Id; 2],
    ready: &Value,
) -> Result<[u8; 2]> {
    ensure!(
        owner < 2 && contexts.len() == 2 && original[0] != original[1],
        "original bounded owner/contexts"
    );
    let mut slots = [0; 2];
    for c in 0..2 {
        slots[c] = oracle::ready_slot(ready, &state::hex(&original[c]))?;
        ensure!(
            contexts.get(slots[c] as usize) == Some(&(original[c], owner as u8)),
            "original Ready physical slot/context/role association"
        );
    }
    ensure!(slots[0] != slots[1], "distinct actual slots");
    Ok(slots)
}
pub(super) fn counts(
    c: usize,
    r: &db::RuntimeCounts,
    n: &store::L1SourceCounts,
    meta: &store::L1Metadata,
    repair: &db::RuntimeRepairState,
    mode: &store::ExistingState7,
) -> Result<()> {
    ensure!(c < 2, "logical terminal context");
    let terminal = limit(c);
    ensure!(
        r.sources == terminal * 2 + 2
            && r.receipts == [terminal; 2]
            && r.queued == 0
            && r.pending == 0
            && r.debt == 0
            && n.lanes == [terminal + 1; 2]
            && n.projections == terminal * 2
            && n.requests == terminal,
        "authenticated exact source/lane/projection/own-request/receipt cardinalities and zero debt/queue/pending"
    );
    ensure!(
        meta.next == [terminal + 1; 2]
            && meta.tails.iter().all(Option::is_some)
            && meta.request_index_ready
            && repair.pending == 0
            && repair.head.is_none()
            && repair.outstanding == 0
            && repair.grant.as_ref().is_none_or(|g| g.3 == [0; 32])
            && !mode.control_debt,
        "terminal metadata and no repair/control obligation"
    );
    Ok(())
}
pub(super) struct Scope<'a> {
    pub c: old::Id,
    pub d: old::Id,
    pub e: old::Id,
    pub sid: &'a str,
}
pub(super) fn active(
    c: usize,
    role: usize,
    origin: &Scope<'_>,
    mode: &store::ExistingState7,
    retained: &Value,
) -> Result<()> {
    let q = mode.q.context("actual Q")?;
    ensure!(
        c < 2
            && role < 2
            && mode.mode == store::ExistingMode::Active
            && mode.cuts == [Some(if c == 0 { 25 } else { 33 }); 2]
            && retained["context"] == state::hex(&origin.c)
            && retained["role"] == role
            && retained["d"] == state::hex(&origin.d)
            && retained["e"] == state::hex(&origin.e)
            && retained["sid"] == origin.sid
            && retained["mode"] == "Active"
            && retained["q"] == state::hex(&q)
            && retained["cuts"] == json!(mode.cuts)
            && mode.current_consent.is_some()
            && mode.current_consent == mode.selected_consent
            && retained["current_consent2"]
                == json!(mode.current_consent.map(|id| state::hex(&id))),
        "both Active original selected scope/current consent/cuts"
    );
    for (key, id) in [
        ("p", mode.propose),
        ("a", mode.accept),
        ("k", mode.select),
        ("committed", mode.committed),
    ] {
        ensure!(
            id.is_some() && retained["controls"][key] == json!(id.map(|v| state::hex(&v))),
            "actual original current signed control association"
        );
    }
    Ok(())
}
pub(super) fn own_request(
    owner: usize,
    row: &records::Record,
    actual: Option<&store::L1Request>,
) -> Result<()> {
    if usize::from(row.role) == owner {
        let r = actual.context("actual sender request row")?;
        ensure!(
            r.event == Some(row.event) && r.text_hash == row.body_hash,
            "actual own request/event/body association"
        );
    } else {
        ensure!(
            actual.is_none(),
            "received Text has no sender-only request row"
        );
    }
    Ok(())
}
pub(super) fn projection(
    raw: &db::ProjectionValue,
    migration: old::Id,
    source: old::Id,
) -> Result<[u8; 16]> {
    let value = old::decode(&raw.binding, 2304)?;
    let f = value.fields(5)?;
    ensure!(
        f[0].uint()? == 7 && f[1].fixed::<32>()? == migration && f[2].fixed::<32>()? == source,
        "actual projection original migration/source binding"
    );
    Ok(f[4].fixed()?)
}
pub(super) fn held(row: &records::Record, original: &Value) -> Result<()> {
    ensure!(
        row.role == 1
            && row.ordinal == 640
            && original["request"] == state::hex(&row.request)
            && original["id"] == state::hex(&row.event)
            && original["source_blake3"] == state::hex(&row.source_hash)
            && original["ciphertext_blake3"] == state::hex(&row.ciphertext_hash),
        "retained original B640 source/request/raw/cipher, now verified receipt present"
    );
    Ok(())
}
pub(super) fn history_row(row: &Value, original: &Value) -> Result<old::Id> {
    for field in ["format", "id", "role", "ordinal", "request", "text"] {
        ensure!(
            row[field] == original[field],
            "actual retained history field {field}"
        );
    }
    ensure!(
        row["delivered"] == true,
        "actual verified history receipt association"
    );
    state::parse_id(row["id"].as_str().context("history EventID")?)
}
