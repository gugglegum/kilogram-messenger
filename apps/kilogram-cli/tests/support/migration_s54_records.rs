//! Dormant one-record terminal facts. No repository, graph, Native or phase driver.
use super::*;
use kilogram_protocol::ordered_dialog_v2 as v2;
use zeroize::Zeroizing;
#[cfg(test)]
#[path = "migration_s54_records_tests.rs"]
mod tests;

pub(super) struct Expected<'a> {
    pub contexts: &'a [old::Id; 2],
    pub request: oracle::Request,
    pub event: old::Id,
    pub d: old::Id,
    pub e: old::Id,
    pub sid: &'a str,
    pub selection: old::Id,
}
pub(super) struct Projection<'a> {
    pub event: old::Id,
    pub request: [u8; 16],
    pub text: &'a str,
}
pub(super) struct Credentials<'a> {
    pub author: &'a v2::RootVerifiedCredential,
    pub receiver: &'a v2::RootVerifiedCredential,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Record {
    pub format: u8,
    pub event: old::Id,
    pub context: old::Id,
    pub d: old::Id,
    pub e: old::Id,
    pub sid: String,
    pub selection: old::Id,
    pub role: u8,
    pub ordinal: u64,
    pub request: [u8; 16],
    pub previous: Option<old::Id>,
    pub parents: Vec<old::Id>,
    pub sequence: u64,
    pub author_auth: old::Id,
    pub source_hash: old::Id,
    pub ciphertext_hash: old::Id,
    pub body_hash: old::Id,
    pub receipt_format: u8,
    pub receipt_hash: old::Id,
    pub receiver_auth: old::Id,
}
impl Record {
    pub fn same(&self, actual: &Self) -> Result<()> {
        ensure!(
            self == actual,
            "exact actual signed record/chain/author-sequence/raw/cipher/projection/receipt facts"
        );
        Ok(())
    }
}
fn digest(raw: &[u8]) -> old::Id {
    *blake3::hash(raw).as_bytes()
}
pub(super) fn verify(
    expected: &Expected<'_>,
    projection: &Projection<'_>,
    selected: &v2::StaticSelection2,
    credentials: Credentials<'_>,
    raw: &[u8],
    receipt: &[u8],
) -> Result<Record> {
    ensure!(
        raw.len() <= v2::MAX_EVENT
            && receipt.len() <= v2::MAX_RECEIPT
            && projection.text.len() <= v2::MAX_TEXT,
        "bounded actual one-record inputs"
    );
    let request = oracle::Request::new(
        expected.request.context,
        expected.request.role,
        expected.request.number,
    )?;
    let context = expected.contexts[request.context as usize];
    let cuts = if request.context == 0 {
        [25, 25]
    } else {
        [33, 33]
    };
    ensure!(
        expected.contexts[0] != expected.contexts[1]
            && selected.c() == context
            && selected.d() == expected.d
            && selected.e() == expected.e
            && selected.sid() == Some(expected.sid)
            && selected.proof().q() == expected.selection
            && selected.proof().proof().cuts() == cuts,
        "original context/D/E/SID/selected Q and C1/C2 cuts"
    );
    let format = old::decode(raw, v2::MAX_EVENT)?.fields(5)?[0].uint()?;
    let required = if request.context == 0 && request.number >= 25 {
        2
    } else {
        1
    };
    ensure!(
        format == required,
        "actual source encoding matches original selected Text boundary"
    );
    let verified = v2::TaggedItem {
        format: format.try_into()?,
        raw: raw.to_vec(),
    }
    .verify_event(selected, credentials.author)?;
    let (event, c, d, e, sid, role, n, kind, id, previous, parents, sequence, author_auth) =
        match &verified {
            v2::MixedEvent::Legacy(v) => {
                let event = v.event();
                let core = &event.core;
                (
                    event.id()?,
                    core.c,
                    core.d,
                    core.e,
                    core.sid.as_str(),
                    u8::try_from(core.role)?,
                    core.n,
                    core.kind,
                    core.request,
                    core.previous,
                    core.parents.clone(),
                    core.sequence,
                    core.author_auth,
                )
            }
            v2::MixedEvent::Current(v) => {
                let event = v.event();
                let core = event.core();
                (
                    event.id(),
                    core.c,
                    core.d,
                    core.e,
                    core.sid.as_str(),
                    core.role,
                    core.n,
                    u64::from(core.kind),
                    core.request,
                    core.previous,
                    core.parents.clone(),
                    core.sequence,
                    core.author_auth,
                )
            }
        };
    let request_id = source::parse_request_id(&source::request_id(request))?;
    ensure!(
        c == context
            && d == expected.d
            && e == expected.e
            && sid == expected.sid
            && role == request.role
            && n == u64::from(request.number)
            && kind == 3
            && event == expected.event
            && id == request_id
            && previous.is_some()
            && parents.len() <= 2,
        "verified actual Text scope/role/ordinal/EventID/request; genesis is separate"
    );
    let body = Zeroizing::new(request.body());
    ensure!(
        projection.event == event
            && projection.request == request_id
            && projection.text == body.as_str(),
        "actual projection/request agrees with original signed source association and body"
    );
    // Receipt encoding comes from its own structure; never from Event encoding.
    let value = old::decode(receipt, v2::MAX_RECEIPT)?;
    let fields = value.fields(2)?[0].list()?;
    let encoding = fields.first().context("actual receipt version")?.uint()?;
    ensure!(
        matches!(encoding, 1 | 2),
        "supported explicit receipt encoding, no fallback"
    );
    v2::TaggedItem {
        format: encoding.try_into()?,
        raw: receipt.to_vec(),
    }
    .verify_receipt(&verified, selected, credentials.receiver)?;
    let value = old::decode(raw, v2::MAX_EVENT)?;
    let ciphertext = value.fields(5)?[3].bytes()?;
    Ok(Record {
        format: format.try_into()?,
        event,
        context: c,
        d,
        e,
        sid: sid.to_owned(),
        selection: selected.proof().q(),
        role,
        ordinal: n,
        request: id,
        previous,
        parents,
        sequence,
        author_auth,
        source_hash: digest(raw),
        ciphertext_hash: digest(ciphertext),
        body_hash: old::hash(b"kg:od1:request-text\0", projection.text.as_bytes()),
        receipt_format: encoding.try_into()?,
        receipt_hash: digest(receipt),
        receiver_auth: credentials.receiver.reference(),
    })
}
