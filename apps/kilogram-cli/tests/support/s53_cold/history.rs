//! Complete authenticated pagination from the same already-open repository.
use super::*;
use kilogram_state::indexed_dialog::HistoryCursor;
pub fn read(
    repo: &mut store::L1Repository,
    expected: &BTreeMap<String, Value>,
    frontiers: [u64; 2],
    until: Instant,
    digest: &mut readers::Digest,
) -> Result<usize> {
    let mut cursor = None;
    let mut seen_cursors = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut bytes = 0usize;
    let mut view = None;
    for page_no in 1..=32 {
        readers::check(until)?;
        let raw = repo.history(0, cursor.as_ref(), 32)?;
        bytes = bytes
            .checked_add(raw.len())
            .context("history byte overflow")?;
        ensure!(
            bytes <= 4 * 1024 * 1024 && raw.len() <= 48 * 1024,
            "history encoded byte cap"
        );
        let page: Value = serde_json::from_slice(&raw)?;
        ensure!(
            page["version"] == 3 && page["F"] == json!(frontiers),
            "history original frontiers/version"
        );
        let now = page["view"].as_str().context("history view")?;
        ensure!(
            now.len() == 64 && now.bytes().all(|b| b.is_ascii_hexdigit()),
            "history view encoding"
        );
        if let Some(old) = &view {
            ensure!(old == now, "history view drift")
        } else {
            view = Some(now.to_owned())
        }
        let items = page["items"].as_array().context("history items")?;
        ensure!(
            !items.is_empty() && items.len() <= 32,
            "history progress/page cap"
        );
        for row in items {
            let id = row["id"].as_str().context("history source ID")?;
            ensure!(seen.insert(id.to_owned()), "history duplicate source");
            ensure!(
                expected.get(id) == Some(row),
                "history exact source/body/receipt correspondence"
            );
        }
        let encoded = page["cursor"].as_str().context("authenticated cursor")?;
        ensure!(
            seen_cursors.insert(encoded.to_owned()),
            "history repeated cursor"
        );
        digest.bytes(&raw);
        if page["finished"] == true {
            ensure!(seen.len() == expected.len(), "history truncation");
            return Ok(page_no);
        }
        ensure!(
            page["finished"] == false && seen.len() < expected.len(),
            "history terminal/progress"
        );
        cursor = Some(HistoryCursor::from_base64(encoded)?);
    }
    anyhow::bail!("history page cap")
}
