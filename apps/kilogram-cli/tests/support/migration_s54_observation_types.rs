//! One bounded validated pause record; facts contain public source metadata only.
use super::*;
pub(super) fn number(f: &Value, k: &str) -> Result<u64> {
    f[k].as_u64().context("actual observation integer")
}
pub(super) fn id(f: &Value, k: &str) -> Result<old::Id> {
    let text = f[k].as_str().context("actual observation ID")?;
    let value = state::parse_id(text)?;
    ensure!(state::hex(&value) == text, "canonical observation ID");
    Ok(value)
}
pub(super) fn fields(f: &Value, n: usize) -> Result<()> {
    ensure!(
        f.as_object().is_some_and(|o| o.len() == n),
        "exact bounded fact schema"
    );
    Ok(())
}
fn exact(f: &Value, names: &[&str]) -> Result<()> {
    fields(f, names.len())?;
    ensure!(
        names.iter().all(|key| f.get(*key).is_some()),
        "missing or unknown fact field"
    );
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Stamp {
    pub incarnation: oracle::Incarnation,
    pub sequence: u64,
    pub line: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct PageObservation {
    pub stamp: Stamp,
    pub context: u8,
    pub binding: old::Id,
    pub lease_generation: u64,
    pub local_page: u64,
    pub start: u64,
    pub end: u64,
    pub new_sources: u64,
    pub head: bool,
    pub operation: old::Id,
    pub generation: u64,
    raw: Value,
}
impl PageObservation {
    pub(super) fn parse(f: &Value, stamp: Stamp, context: u8) -> Result<Self> {
        exact(
            f,
            &[
                "context",
                "binding",
                "lease_generation",
                "local_page",
                "head",
                "grant",
                "start",
                "end",
                "new_sources",
                "generation",
                "operation",
            ],
        )?;
        let start = number(f, "start")?;
        let end = number(f, "end")?;
        let new_sources = number(f, "new_sources")?;
        let head = f["head"].as_bool().context("explicit page head")?;
        let generation = number(f, "generation")?;
        let lease_generation = number(f, "lease_generation")?;
        ensure!(
            start > 0
                && end > start
                && end <= 4097
                && end - start <= 16
                && new_sources <= end - start
                && (!head || end - start == 1)
                && generation > 0
                && lease_generation > 0
                && number(f, "local_page")? <= 256,
            "actual bounded page"
        );
        id(f, "context")?;
        if !f["grant"].is_null() {
            id(f, "grant")?;
        }
        Ok(Self {
            stamp,
            context,
            binding: id(f, "binding")?,
            lease_generation,
            local_page: number(f, "local_page")?,
            start,
            end,
            new_sources,
            head,
            operation: id(f, "operation")?,
            generation,
            raw: f.clone(),
        })
    }
    pub fn facts(&self) -> &Value {
        &self.raw
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct ReceiptObservation {
    pub encoding: u8,
    pub source_format: u8,
    pub lane: u8,
    pub ordinal: u64,
    pub event: old::Id,
    pub digest: old::Id,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct AckObservation {
    pub stamp: Stamp,
    pub context: u8,
    pub binding: old::Id,
    pub lease_generation: u64,
    pub request: [u8; 16],
    pub query_reply: bool,
    pub lane: u8,
    pub from: Option<u64>,
    pub receipts: Vec<ReceiptObservation>,
    pub digest: old::Id,
    raw: Value,
}
impl AckObservation {
    pub(super) fn parse(f: &Value, stamp: Stamp, context: u8, role: u8) -> Result<Self> {
        exact(
            f,
            &[
                "context",
                "binding",
                "lease_generation",
                "request",
                "query_reply",
                "receipt_count",
                "saved_receipts_digest",
                "lane",
                "receipts",
                "hold_ack_from",
            ],
        )?;
        id(f, "context")?;
        let lane: u8 = number(f, "lane")?.try_into()?;
        let query_reply = f["query_reply"]
            .as_bool()
            .context("actual ACK push/query classification")?;
        let from = if f["hold_ack_from"].is_null() {
            None
        } else {
            Some(number(f, "hold_ack_from")?)
        };
        let rows = f["receipts"].as_array().context("actual receipt tuples")?;
        ensure!(
            role < 2
                && lane == 1 - role
                && !rows.is_empty()
                && rows.len() <= 16
                && number(f, "receipt_count")? == rows.len() as u64
                && number(f, "lease_generation")? > 0,
            "actual ACK count/lane/current lease"
        );
        let mut receipts = Vec::new();
        let mut positions = BTreeSet::new();
        let mut events = BTreeSet::new();
        for row in rows {
            let tuple = row.as_array().context("compact receipt tuple")?;
            ensure!(tuple.len() == 6, "exact receipt tuple");
            let encoding: u8 = tuple[0].as_u64().context("receipt encoding")?.try_into()?;
            let source_format: u8 = tuple[1].as_u64().context("source format")?.try_into()?;
            let actual_lane: u8 = tuple[2].as_u64().context("receipt lane")?.try_into()?;
            let ordinal = tuple[3].as_u64().context("receipt ordinal")?;
            let read_id = |i: usize| -> Result<old::Id> {
                let s = tuple[i].as_str().context("receipt ID")?;
                let id = state::parse_id(s)?;
                ensure!(state::hex(&id) == s, "canonical tuple ID");
                Ok(id)
            };
            let event = read_id(4)?;
            let digest = read_id(5)?;
            ensure!(
                matches!(encoding, 1 | 2)
                    && matches!(source_format, 1 | 2)
                    && (encoding == 2 || source_format == 1)
                    && actual_lane == lane
                    && (1..=4096).contains(&ordinal)
                    && positions.insert((lane, ordinal))
                    && events.insert(event),
                "supported unique actual receipt association"
            );
            receipts.push(ReceiptObservation {
                encoding,
                source_format,
                lane,
                ordinal,
                event,
                digest,
            });
        }
        ensure!(
            from.is_none_or(|n| role == 1
                && lane == 0
                && !query_reply
                && (1..=4096).contains(&n)
                && receipts.iter().any(|r| r.ordinal >= n)),
            "targeted actual B push/source receipt"
        );
        Ok(Self {
            stamp,
            context,
            binding: id(f, "binding")?,
            lease_generation: number(f, "lease_generation")?,
            request: source::parse_request_id(
                f["request"].as_str().context("actual ACK request")?,
            )?,
            query_reply,
            lane,
            from,
            receipts,
            digest: id(f, "saved_receipts_digest")?,
            raw: f.clone(),
        })
    }
    pub fn facts(&self) -> &Value {
        &self.raw
    }
}
#[derive(Clone, Copy)]
pub(super) struct AckArm {
    pub stamp: Stamp,
    pub context: u8,
    pub binding: old::Id,
    pub generation: u64,
    pub from: Option<u64>,
}
