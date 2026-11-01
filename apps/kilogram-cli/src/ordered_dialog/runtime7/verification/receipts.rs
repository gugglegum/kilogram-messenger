//! Metadata of an already-built verified ACK; no storage, signing or packet work.
use super::*;
use kilogram_protocol::ordered_dialog_v2 as v2;

pub(super) struct ReceiptFact {
    pub encoding: u8,
    pub source_format: u8,
    pub lane: u8,
    pub ordinal: u64,
    pub event: Id,
    pub digest: Id,
}
pub(super) struct AckContents {
    pub lane: u8,
    pub receipts: Vec<ReceiptFact>,
    pub digest: String,
}
impl AckContents {
    pub(super) fn parse(kind: v2::FrameKind, body: &wire::Value) -> Result<Option<Self>> {
        if kind != v2::FrameKind::AckBatch {
            return Ok(None);
        }
        let f = body.fields(8)?;
        let context = f[0].fixed::<32>()?;
        let d = f[1].fixed::<32>()?;
        let e = f[2].fixed::<32>()?;
        let sid = f[3].text()?;
        ensure!(sid.len() == 43 && sid.is_ascii(), "S54 actual ACK SID");
        let lane: u8 = f[4].uint()?.try_into()?;
        ensure!(lane < 2, "S54 actual ACK lane");
        let items = v2::tagged_items(&f[6], 1024)?;
        ensure!(items.len() <= 16, "S54 ACK16 bound");
        if items.is_empty() {
            return Ok(None);
        }
        let mut receipts = Vec::with_capacity(items.len());
        let mut positions = BTreeSet::new();
        let mut events = BTreeSet::new();
        let mut q = None;
        for item in items {
            let (source_format, actual_lane, n, event) = match item.format {
                1 => {
                    let parsed = v2::ParsedReceipt1::parse(&item.raw)?;
                    let value = wire::decode(parsed.raw(), 1024)?;
                    let r = value.fields(2)?[0].fields(8)?;
                    ensure!(
                        r[1].fixed::<32>()? == d && r[2].fixed::<32>()? == e,
                        "S54 Receipt1 actual ACK D/E"
                    );
                    (1, r[3].uint()?, r[4].uint()?, r[5].fixed::<32>()?)
                }
                2 => {
                    let parsed = v2::ParsedReceipt2::parse(&item.raw)?;
                    let r = parsed.fields();
                    ensure!(
                        r.c == context
                            && r.d == d
                            && r.e == e
                            && r.sid == sid
                            && q.is_none_or(|old| old == r.q),
                        "S54 Receipt2 actual ACK scope/Q"
                    );
                    q = Some(r.q);
                    (r.format, u64::from(r.lane), r.n, r.event)
                }
                _ => anyhow::bail!("S54 unsupported receipt encoding"),
            };
            ensure!(
                actual_lane == u64::from(lane)
                    && (1..=4096).contains(&n)
                    && positions.insert((lane, n))
                    && events.insert(event),
                "S54 actual receipt lane/ordinal/unique source"
            );
            receipts.push(ReceiptFact {
                encoding: item.format,
                source_format,
                lane,
                ordinal: n,
                event,
                digest: *blake3::hash(&item.raw).as_bytes(),
            });
        }
        Ok(Some(Self {
            lane,
            receipts,
            digest: blake3::hash(&wire::encode(&f[6], 16 * 1024)?)
                .to_hex()
                .to_string(),
        }))
    }
    pub(super) fn matches(&self, from: Option<u64>, role: u8, query_reply: bool) -> bool {
        from.is_none_or(|n| {
            role == 1
                && self.lane == 0
                && !query_reply
                && self.receipts.iter().any(|r| r.ordinal >= n)
        })
    }
    pub(super) fn tuples(&self) -> Vec<(u8, u8, u8, u64, String, String)> {
        self.receipts
            .iter()
            .map(|r| {
                (
                    r.encoding,
                    r.source_format,
                    r.lane,
                    r.ordinal,
                    state::hex(&r.event),
                    state::hex(&r.digest),
                )
            })
            .collect()
    }
    pub(super) fn facts(
        &self,
        context: Id,
        binding: Id,
        generation: u64,
        request: [u8; 16],
        query_reply: bool,
        from: Option<u64>,
    ) -> Json {
        json!({"context":state::hex(&context),"binding":state::hex(&binding),"lease_generation":generation,"request":state::hex(&request),"query_reply":query_reply,"receipt_count":self.receipts.len(),"saved_receipts_digest":self.digest,"lane":self.lane,"receipts":self.tuples(),"hold_ack_from":from})
    }
}
