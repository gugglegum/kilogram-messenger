use super::{Client2, RuntimeIpcResponse, decode_reply, wire};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use wire::{Id, Value, b, opt, u};
use zeroize::Zeroizing;

pub const MAX_HISTORY_ROWS: usize = 32;
pub const HISTORY_ANCHOR_OPCODE: u64 = 16;
pub const HISTORY_FOLLOW_OPCODE: u64 = 17;
/// Canonical CBOR maximum including [version, opcode, position].
pub const HISTORY_POSITION_REPLY_CAP: usize = 388;
const HISTORY_JSON_CAP: usize = 48 * 1024 - 512;
const CURSOR_BYTES: usize = 275;
const CURSOR_HEADER: &[u8; 8] = b"KGH2\x01\x02\0\0";

type Result<T> = std::result::Result<T, MessageError2>;

/// Safe to report to the desktop without attaching the underlying IPC error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageError2 {
    InvalidInput,
    InvalidReply,
    Unavailable,
    RequestIdConflict,
    HistoryUnavailable,
    HistoryCursorInvalid,
    HistoryPositionUnsupported,
}
impl std::fmt::Display for MessageError2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Invalid message request",
            Self::InvalidReply => "Invalid message response",
            Self::Unavailable => "Message operation could not be confirmed",
            Self::RequestIdConflict => "Message request ID conflict",
            Self::HistoryUnavailable => "History unavailable",
            Self::HistoryCursorInvalid => "Refresh history from the beginning",
            Self::HistoryPositionUnsupported => "Runtime does not support following history",
        })
    }
}
impl std::error::Error for MessageError2 {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SendReceipt2 {
    pub queue_sequence: u64,
    pub event: Option<Id>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Delivery2 {
    Queued,
    Materialized,
    Delivered,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RequestStatus2 {
    pub receipt: SendReceipt2,
    pub delivery: Delivery2,
}
#[derive(Clone, PartialEq, Eq)]
pub struct HistoryCursor2 {
    raw: [u8; CURSOR_BYTES],
    context: Id,
    view: Id,
    frontiers: [u64; 2],
}
impl HistoryCursor2 {
    // Compare public metadata in the authenticated IPC reply. Only storage verifies the MAC.
    fn matches_position(&self, next: [u64; 2]) -> bool {
        self.raw.starts_with(CURSOR_HEADER)
            && self.raw[40..72] == self.view
            && self.raw[72..104] == self.context
            && self.raw[211..243]
                .chunks_exact(8)
                .zip(self.frontiers.into_iter().chain(next))
                .all(|(encoded, ordinal)| encoded == ordinal.to_be_bytes())
    }
}

pub struct HistoryPosition2 {
    pub context: Id,
    pub view: Id,
    pub frontiers: [u64; 2],
    pub next: [u64; 2],
    pub cursor: HistoryCursor2,
}
pub struct HistoryRow2 {
    pub event: Id,
    pub role: u8,
    pub ordinal: u64,
    pub sequence: u64,
    pub request: [u8; 16],
    pub text: Zeroizing<String>,
}
pub struct HistoryPage2 {
    pub view: Id,
    pub frontiers: [u64; 2],
    pub finished: bool,
    pub cursor: HistoryCursor2,
    pub items: Vec<HistoryRow2>,
}

impl Client2 {
    fn message_slot(&self, slot: u8, context: Id) -> Result<Value> {
        if self.contexts.get(usize::from(slot)) != Some(&context) {
            return Err(MessageError2::InvalidInput);
        }
        Ok(u(u64::from(slot)))
    }

    async fn message_request(&self, op: u64, fields: Vec<Value>) -> Result<Value> {
        let response = self
            .response(op, fields)
            .await
            .map_err(|_| MessageError2::Unavailable)?;
        message_reply(op, response)
    }

    pub async fn send_text(
        &self,
        slot: u8,
        context: Id,
        request: [u8; 16],
        text: &str,
    ) -> Result<SendReceipt2> {
        if text.is_empty() || text.len() > wire::MAX_TEXT {
            return Err(MessageError2::InvalidInput);
        }
        let slot = self.message_slot(slot, context)?;
        let value = self
            .message_request(2, vec![slot, b(request), b(text.as_bytes())])
            .await?;
        let fields = value.fields(2).map_err(invalid)?;
        receipt(fields)
    }

    pub async fn request_status(
        &self,
        slot: u8,
        context: Id,
        request: [u8; 16],
    ) -> Result<Option<RequestStatus2>> {
        let slot = self.message_slot(slot, context)?;
        let value = self.message_request(3, vec![slot, b(request)]).await?;
        request_status(&value)
    }

    pub async fn history_page(
        &self,
        slot: u8,
        context: Id,
        cursor: Option<&HistoryCursor2>,
        limit: usize,
    ) -> Result<HistoryPage2> {
        if !(1..=MAX_HISTORY_ROWS).contains(&limit)
            || cursor.is_some_and(|cursor| cursor.context != context)
        {
            return Err(MessageError2::InvalidInput);
        }
        let slot = self.message_slot(slot, context)?;
        let value = self
            .message_request(
                4,
                vec![slot, opt(cursor.map(|c| b(c.raw))), u(limit as u64)],
            )
            .await?;
        history_page(value.bytes().map_err(invalid)?, context, cursor, limit)
    }

    pub async fn history_anchor(&self, slot: u8, context: Id) -> Result<HistoryPosition2> {
        let slot = self.message_slot(slot, context)?;
        let value = self
            .message_request(HISTORY_ANCHOR_OPCODE, vec![slot])
            .await?;
        history_position(&value, context, None)
    }

    pub async fn history_follow(
        &self,
        slot: u8,
        context: Id,
        completed: &HistoryCursor2,
    ) -> Result<HistoryPosition2> {
        let slot = self.message_slot(slot, context)?;
        if completed.context != context || !completed.matches_position(completed.frontiers) {
            return Err(MessageError2::HistoryCursorInvalid);
        }
        let value = self
            .message_request(HISTORY_FOLLOW_OPCODE, vec![slot, b(completed.raw)])
            .await?;
        history_position(&value, context, Some(completed))
    }
}

fn message_reply(op: u64, response: RuntimeIpcResponse) -> Result<Value> {
    if let RuntimeIpcResponse::Error { message } = response {
        let message = Zeroizing::new(message);
        return Err(match (op, message.as_str()) {
            (2, "RequestIdConflict") => MessageError2::RequestIdConflict,
            (3 | 4 | HISTORY_ANCHOR_OPCODE | HISTORY_FOLLOW_OPCODE, "HistoryUnavailable") => {
                MessageError2::HistoryUnavailable
            }
            (4 | HISTORY_FOLLOW_OPCODE, "InvalidCursor" | "SnapshotUnavailable") => {
                MessageError2::HistoryCursorInvalid
            }
            (HISTORY_ANCHOR_OPCODE | HISTORY_FOLLOW_OPCODE, "UnsupportedOperation") => {
                MessageError2::HistoryPositionUnsupported
            }
            _ => MessageError2::Unavailable,
        });
    }
    if matches!(op, HISTORY_ANCHOR_OPCODE | HISTORY_FOLLOW_OPCODE)
        && let RuntimeIpcResponse::OrderedDialog { response } = &response
        && response.len() > HISTORY_POSITION_REPLY_CAP
    {
        return Err(MessageError2::InvalidReply);
    }
    decode_reply(op, response).map_err(invalid)
}

fn history_position(
    value: &Value,
    context: Id,
    completed: Option<&HistoryCursor2>,
) -> Result<HistoryPosition2> {
    fn ordinals(value: &Value) -> Result<[u64; 2]> {
        let pair = value.fields(2).map_err(invalid)?;
        Ok([
            pair[0].uint().map_err(invalid)?,
            pair[1].uint().map_err(invalid)?,
        ])
    }
    let fields = value.fields(5).map_err(invalid)?;
    if fields[0].fixed::<32>().map_err(invalid)? != context {
        return Err(MessageError2::InvalidReply);
    }
    let view = fields[1].fixed().map_err(invalid)?;
    let frontiers = ordinals(&fields[2])?;
    let next = ordinals(&fields[3])?;
    let cursor = HistoryCursor2 {
        raw: fields[4].fixed().map_err(invalid)?,
        context,
        view,
        frontiers,
    };
    let expected_next = completed.map_or(frontiers, |previous| previous.frontiers);
    if next != expected_next
        || next.contains(&0)
        || next
            .into_iter()
            .zip(frontiers)
            .any(|(next, end)| next > end)
        || completed.is_some_and(|previous| previous.view != view)
        || !cursor.matches_position(next)
    {
        return Err(MessageError2::InvalidReply);
    }
    Ok(HistoryPosition2 {
        context,
        view,
        frontiers,
        next,
        cursor,
    })
}

fn invalid<T>(_: T) -> MessageError2 {
    MessageError2::InvalidReply
}

fn receipt(fields: &[Value]) -> Result<SendReceipt2> {
    Ok(SendReceipt2 {
        queue_sequence: fields[0].uint().map_err(invalid)?,
        event: fields[1]
            .option()
            .map_err(invalid)?
            .map(Value::fixed)
            .transpose()
            .map_err(invalid)?,
    })
}

fn request_status(value: &Value) -> Result<Option<RequestStatus2>> {
    let Some(value) = value.option().map_err(invalid)? else {
        return Ok(None);
    };
    let fields = value.fields(3).map_err(invalid)?;
    let receipt = receipt(fields)?;
    let delivery = match (fields[2].uint().map_err(invalid)?, receipt.event) {
        (0, None) => Delivery2::Queued,
        (1, Some(_)) => Delivery2::Materialized,
        (2, Some(_)) => Delivery2::Delivered,
        _ => return Err(MessageError2::InvalidReply),
    };
    Ok(Some(RequestStatus2 { receipt, delivery }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageJson {
    version: u8,
    view: String,
    #[serde(rename = "F")]
    frontiers: [u64; 2],
    finished: bool,
    cursor: String,
    items: Vec<RowJson>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RowJson {
    id: String,
    role: u8,
    ordinal: u64,
    sequence: u64,
    request: String,
    text: String,
}
impl Drop for RowJson {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.text);
    }
}

fn hex<const N: usize>(text: &str) -> Result<[u8; N]> {
    fn nibble(byte: u8) -> Result<u8> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(MessageError2::InvalidReply),
        }
    }
    if text.len() != N * 2 {
        return Err(MessageError2::InvalidReply);
    }
    let mut result = [0; N];
    for (out, [a, b]) in result.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *out = (nibble(*a)? << 4) | nibble(*b)?;
    }
    Ok(result)
}

fn history_page(
    json: &[u8],
    context: Id,
    previous: Option<&HistoryCursor2>,
    limit: usize,
) -> Result<HistoryPage2> {
    if json.len() > HISTORY_JSON_CAP {
        return Err(MessageError2::InvalidReply);
    }
    let page: PageJson = serde_json::from_slice(json).map_err(invalid)?;
    if page.version != 2
        || page.frontiers.contains(&0)
        || page.items.len() > limit
        || page.items.is_empty() && !page.finished
        || page.cursor.len() != 367
    {
        return Err(MessageError2::InvalidReply);
    }
    let view = hex(&page.view)?;
    let mut raw = [0; CURSOR_BYTES];
    if URL_SAFE_NO_PAD
        .decode_slice(&page.cursor, &mut raw)
        .map_err(invalid)?
        != CURSOR_BYTES
        || !raw.starts_with(CURSOR_HEADER)
        || URL_SAFE_NO_PAD.encode(raw) != page.cursor
    {
        return Err(MessageError2::InvalidReply);
    }
    if let Some(prev) = previous
        && (prev.context != context
            || prev.view != view
            || prev.frontiers != page.frontiers
            || prev.raw == raw && !page.items.is_empty())
    {
        return Err(MessageError2::InvalidReply);
    }

    let mut last_ordinal = [0; 2];
    let mut items: Vec<HistoryRow2> = Vec::with_capacity(page.items.len());
    for mut row in page.items {
        let role = usize::from(row.role);
        let event = hex(&row.id)?;
        if role >= 2
            || row.ordinal <= last_ordinal[role]
            || row.ordinal >= page.frontiers[role]
            || row.text.len() > wire::MAX_TEXT
            || items.iter().any(|item| item.event == event)
        {
            return Err(MessageError2::InvalidReply);
        }
        last_ordinal[role] = row.ordinal;
        items.push(HistoryRow2 {
            event,
            role: row.role,
            ordinal: row.ordinal,
            sequence: row.sequence,
            request: hex(&row.request)?,
            text: Zeroizing::new(std::mem::take(&mut row.text)),
        });
    }
    Ok(HistoryPage2 {
        view,
        frontiers: page.frontiers,
        finished: page.finished,
        cursor: HistoryCursor2 {
            raw,
            context,
            view,
            frontiers: page.frontiers,
        },
        items,
    })
}

#[cfg(test)]
mod tests;
