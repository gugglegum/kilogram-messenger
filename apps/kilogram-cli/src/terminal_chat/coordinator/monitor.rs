//! Bounded terminal observation over the existing authenticated history reader.
use super::{FrameTag, Frontend, HistoryAdvance, OperationControl, hex, json};
use kilogram_first_dialog::chat::ChatStatus;
use kilogram_runtime_ipc::ordered2::{HistoryCursor2, HistoryPosition2, MessageError2};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, Write},
    time::{Duration, Instant},
};

const STEP: Duration = Duration::from_millis(250);
const IDLE: Duration = Duration::from_secs(1);
const WAIT_HISTORY: Duration = Duration::from_secs(2);
const ROW_CAP: usize = 25 * 1024;
const GAP_CAP: usize = 512;
const DATA_BYTES: usize = 256 * 1024 - 2 * GAP_CAP;
const DATA_SLOTS: usize = 62;
pub(super) const GAP_INDEX: u8 = u8::MAX;

#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq)]
pub(super) struct Binding {
    contexts: [String; 2],
    roles: [u8; 2],
    instance: String,
}

impl From<&ChatStatus> for Binding {
    fn from(status: &ChatStatus) -> Self {
        Self {
            contexts: status.contexts.clone(),
            roles: status.own_roles,
            instance: status.runtime_instance.clone(),
        }
    }
}

#[derive(Clone)]
struct Position {
    view: [u8; 32],
    frontiers: [u64; 2],
    next: [u64; 2],
    cursor: HistoryCursor2,
}

impl From<HistoryPosition2> for Position {
    fn from(position: HistoryPosition2) -> Self {
        Self {
            view: position.view,
            frontiers: position.frontiers,
            next: position.next,
            cursor: position.cursor,
        }
    }
}

impl Position {
    fn finished(&self) -> bool {
        self.next == self.frontiers
    }
}

struct Batch {
    id: u64,
    index: u8,
    frames: VecDeque<Vec<u8>>,
    position: Position,
}

struct Recovery {
    view: [u8; 32],
    frontiers: [u64; 2],
    cursor: HistoryCursor2,
}

struct Gap {
    id: u64,
    code: &'static str,
    signalled: bool,
    recovery: Option<Recovery>,
}

struct Context {
    id: String,
    due: Instant,
    failures: u8,
    waiting: bool,
    completed: Option<Position>,
    range: Option<Position>,
    batch: Option<Batch>,
    gap: Option<Gap>,
}

impl Context {
    fn new(id: String, now: Instant) -> Self {
        Self {
            id,
            due: now,
            failures: 0,
            waiting: false,
            completed: None,
            range: None,
            batch: None,
            gap: None,
        }
    }

    fn gap(&mut self, id: u64, code: &'static str) {
        self.batch = None;
        self.gap.get_or_insert(Gap {
            id,
            code,
            signalled: false,
            recovery: None,
        });
    }

    fn progress(&mut self, position: Position, now: Instant) {
        if position.finished() {
            self.completed = Some(position);
            self.range = None;
            self.due = now + IDLE;
        } else {
            self.range = Some(position);
            self.due = now;
        }
    }
}

enum Read {
    Anchor,
    Follow(Position),
    Page(Position, usize),
}

pub(super) struct Request {
    pub(super) slot: usize,
    read: Read,
    context: String,
    instance: String,
    role: u8,
    epoch: u64,
}

impl Request {
    pub(super) fn reserved(&self) -> usize {
        match self.read {
            Read::Page(_, limit) => limit,
            _ => 0,
        }
    }

    pub(super) fn execute(
        self,
        frontend: &mut Frontend,
        control: &std::sync::Arc<OperationControl>,
        jsonl: bool,
    ) -> Result<Response, Fault> {
        let service = &mut frontend.service;
        let slot = self.slot as u8;
        match &self.read {
            Read::Anchor | Read::Follow(_) => {
                let cursor = match &self.read {
                    Read::Follow(previous) => Some(previous.cursor.clone()),
                    _ => None,
                };
                let reply = service
                    .history_position_controlled(slot, cursor, control)
                    .map_err(Fault::from)?;
                if hex(&reply.context) != self.context {
                    return Err(Fault::Gap("invalid_scope"));
                }
                let reply = Position::from(reply);
                match &self.read {
                    Read::Anchor if !reply.finished() => return Err(Fault::Gap("invalid_cursor")),
                    Read::Follow(previous)
                        if reply.view != previous.view
                            || reply.next != previous.frontiers
                            || reply
                                .next
                                .into_iter()
                                .zip(reply.frontiers)
                                .any(|(from, to)| from > to) =>
                    {
                        return Err(Fault::Gap("invalid_cursor"));
                    }
                    _ => {}
                }
                Ok(Response::position(reply))
            }
            Read::Page(previous, limit) => {
                let page = service
                    .history_controlled(slot, Some(previous.cursor.clone()), *limit, Some(control))
                    .map_err(|err| {
                        err.downcast_ref::<MessageError2>()
                            .copied()
                            .map_or(Fault::Unavailable, Fault::from)
                    })?;
                if page.view != previous.view
                    || page.frontiers != previous.frontiers
                    || page.items.len() > *limit
                    || page.items.is_empty()
                {
                    return Err(Fault::Gap("invalid_cursor"));
                }
                let mut next = previous.next;
                let mut ids = Vec::with_capacity(page.items.len());
                let mut frames = VecDeque::new();
                for row in page.items {
                    let role = usize::from(row.role);
                    if role >= 2 || next[role] != row.ordinal || row.ordinal >= page.frontiers[role]
                    {
                        return Err(Fault::Gap("invalid_cursor"));
                    }
                    next[role] = row
                        .ordinal
                        .checked_add(1)
                        .ok_or(Fault::Gap("invalid_cursor"))?;
                    if row.role != self.role && !ids.contains(&row.event) {
                        ids.push(row.event);
                        frames.push_back(
                            incoming(jsonl, &self, &row.event, &row.text)
                                .map_err(|_| Fault::Gap("output_bound"))?,
                        );
                    }
                }
                let position = Position {
                    view: page.view,
                    frontiers: page.frontiers,
                    next,
                    cursor: page.cursor,
                };
                if page.finished != position.finished() {
                    return Err(Fault::Gap("invalid_cursor"));
                }
                Ok(Response { position, frames })
            }
        }
    }
}

pub(super) struct Response {
    position: Position,
    frames: VecDeque<Vec<u8>>,
}

impl Response {
    fn position(position: Position) -> Self {
        Self {
            position,
            frames: VecDeque::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fault {
    Unavailable,
    NoHistory,
    Unsupported,
    Gap(&'static str),
}

impl From<MessageError2> for Fault {
    fn from(error: MessageError2) -> Self {
        match error {
            MessageError2::Unavailable => Self::Unavailable,
            MessageError2::HistoryUnavailable => Self::NoHistory,
            MessageError2::HistoryPositionUnsupported => Self::Unsupported,
            _ => Self::Gap("invalid_cursor"),
        }
    }
}

#[derive(Default)]
pub(super) struct Monitor {
    epoch: Option<u64>,
    binding: Option<Binding>,
    contexts: Option<[Context; 2]>,
    next_read: Option<Instant>,
    read_turn: usize,
    write_turn: usize,
    unsupported: bool,
}

impl Monitor {
    pub(super) fn connect(&mut self, epoch: u64, binding: Binding, now: Instant) {
        let mut old = self
            .contexts
            .take()
            .map(|items| items.map(Some))
            .unwrap_or([None, None]);
        self.contexts = Some(std::array::from_fn(|slot| {
            let id = &binding.contexts[slot];
            let previous = old
                .iter_mut()
                .find(|ctx| ctx.as_ref().is_some_and(|ctx| &ctx.id == id))
                .and_then(Option::take);
            if let Some(mut previous) = previous
                && previous.gap.is_some()
            {
                // A new attachment still needs explicit history for this gap.
                previous.due = now;
                previous.failures = 0;
                previous.waiting = false;
                previous
            } else {
                Context::new(id.clone(), now)
            }
        }));
        self.epoch = Some(epoch);
        self.binding = Some(binding);
        self.unsupported = false;
        self.read_turn = 0;
        self.write_turn = 0;
    }

    pub(super) fn disconnect(&mut self, code: &'static str) {
        let Some(epoch) = self.epoch.take() else {
            return;
        };
        self.binding = None;
        self.unsupported = false;
        for ctx in self.contexts.iter_mut().flatten() {
            if ctx.completed.is_some() {
                ctx.gap(epoch, code);
            }
            ctx.batch = None;
            if let Some(gap) = &mut ctx.gap {
                gap.signalled = false;
                gap.recovery = None;
            }
        }
    }

    pub(super) fn status(&self) -> Value {
        let contexts: Vec<_> = self
            .contexts
            .iter()
            .flatten()
            .enumerate()
            .filter_map(|(slot, ctx)| {
                let (state, reason, retry) = match &ctx.gap {
                    Some(gap) => ("resync_required", Some(gap.code), None),
                    None if self.epoch.is_none() => return None,
                    None if self.unsupported => {
                        ("unsupported", Some("history_follow_unsupported"), None)
                    }
                    None if ctx.failures > 0 => ("retrying", None, Some(ctx.failures)),
                    None if ctx.waiting => ("waiting_history", None, None),
                    None if ctx.completed.is_some() => ("watching", None, None),
                    None => ("anchoring", None, None),
                };
                let mut row = json!({"context":ctx.id, "slot":slot, "state":state});
                if let Some(code) = reason {
                    row["code"] = json!(code);
                }
                if let Some(retry) = retry {
                    row["retry"] = json!(retry);
                }
                Some(row)
            })
            .collect();
        json!({"epoch":self.epoch, "contexts":contexts})
    }

    pub(super) fn gaps(&self) -> [Option<u64>; 2] {
        self.contexts.as_ref().map_or([None, None], |contexts| {
            std::array::from_fn(|slot| contexts[slot].gap.as_ref().map(|gap| gap.id))
        })
    }

    pub(super) fn request(&mut self, now: Instant, writing: (usize, usize)) -> Option<Request> {
        let epoch = self.epoch?;
        if self.unsupported || self.next_read.is_some_and(|time| now < time) {
            return None;
        }
        let binding = self.binding.as_ref()?;
        let contexts = self.contexts.as_ref()?;
        let (slots, bytes) = contexts.iter().filter_map(|ctx| ctx.batch.as_ref()).fold(
            writing,
            |(slots, bytes), batch| {
                (
                    slots + batch.frames.len(),
                    bytes + batch.frames.iter().map(Vec::len).sum::<usize>(),
                )
            },
        );
        let credit = page_limit(slots, bytes);
        for slot in [self.read_turn, 1 - self.read_turn] {
            let ctx = &contexts[slot];
            if ctx.gap.is_some() || ctx.batch.is_some() || now < ctx.due {
                continue;
            }
            let read = if let Some(range) = &ctx.range {
                if credit == 0 {
                    continue;
                }
                Read::Page(range.clone(), credit)
            } else if let Some(position) = &ctx.completed {
                Read::Follow(position.clone())
            } else {
                Read::Anchor
            };
            self.next_read = Some(now + STEP);
            self.read_turn = 1 - slot;
            return Some(Request {
                slot,
                read,
                context: ctx.id.clone(),
                instance: binding.instance.clone(),
                role: binding.roles[slot],
                epoch,
            });
        }
        None
    }

    pub(super) fn complete(
        &mut self,
        epoch: u64,
        id: u64,
        slot: usize,
        reserved: usize,
        response: Result<Response, Fault>,
        now: Instant,
    ) {
        if self.epoch != Some(epoch) || self.unsupported {
            return;
        }
        let Some(contexts) = &mut self.contexts else {
            return;
        };
        if response.as_ref().err() == Some(&Fault::Unsupported) {
            self.unsupported = true;
            for ctx in contexts {
                ctx.batch = None;
            }
            return;
        }
        let ctx = &mut contexts[slot];
        if ctx.gap.is_some() {
            return;
        }
        match response {
            Ok(response) => {
                ctx.waiting = false;
                ctx.failures = 0;
                if response.frames.len() > reserved
                    || response.frames.iter().any(|frame| frame.len() > ROW_CAP)
                {
                    ctx.gap(id, "output_bound");
                } else if response.frames.is_empty() {
                    ctx.progress(response.position, now);
                } else {
                    ctx.batch = Some(Batch {
                        id,
                        index: 0,
                        frames: response.frames,
                        position: response.position,
                    });
                }
            }
            Err(Fault::NoHistory) if ctx.completed.is_none() => {
                ctx.failures = 0;
                ctx.waiting = true;
                ctx.due = now + WAIT_HISTORY;
            }
            Err(Fault::Unavailable) if ctx.failures < 3 => {
                ctx.due = now + Duration::from_secs(1 << ctx.failures);
                ctx.failures += 1;
            }
            Err(error) => ctx.gap(
                id,
                match error {
                    Fault::Unavailable => "runtime_unavailable",
                    Fault::NoHistory => "history_unavailable",
                    Fault::Gap(code) => code,
                    Fault::Unsupported => unreachable!(),
                },
            ),
        }
    }

    pub(super) fn frame(&mut self, jsonl: bool) -> Option<(FrameTag, Vec<u8>)> {
        let connection = self.epoch?;
        let binding = self.binding.as_ref()?;
        let contexts = self.contexts.as_mut()?;
        for slot in [self.write_turn, 1 - self.write_turn] {
            let ctx = &mut contexts[slot];
            let item = if let Some(gap) = &ctx.gap {
                if gap.signalled {
                    continue;
                }
                (
                    gap.id,
                    GAP_INDEX,
                    gap_notice(jsonl, connection, binding, slot, gap.code),
                )
            } else if let Some(batch) = &mut ctx.batch {
                let Some(bytes) = batch.frames.pop_front() else {
                    continue;
                };
                (batch.id, batch.index, bytes)
            } else {
                continue;
            };
            self.write_turn = 1 - slot;
            return Some((
                FrameTag {
                    connection,
                    batch: item.0,
                    index: item.1,
                    slot: slot as u8,
                },
                item.2,
            ));
        }
        None
    }

    pub(super) fn owns(&self, tag: FrameTag) -> bool {
        if self.epoch != Some(tag.connection) {
            return false;
        }
        let Some(ctx) = self
            .contexts
            .as_ref()
            .and_then(|contexts| contexts.get(usize::from(tag.slot)))
        else {
            return false;
        };
        if tag.index == GAP_INDEX {
            ctx.gap
                .as_ref()
                .is_some_and(|gap| gap.id == tag.batch && !gap.signalled)
        } else {
            ctx.batch
                .as_ref()
                .is_some_and(|batch| batch.id == tag.batch && batch.index == tag.index)
        }
    }

    pub(super) fn written(&mut self, tag: FrameTag, now: Instant) {
        if !self.owns(tag) {
            return;
        }
        let ctx = &mut self.contexts.as_mut().unwrap()[usize::from(tag.slot)];
        if tag.index == GAP_INDEX {
            ctx.gap.as_mut().unwrap().signalled = true;
        } else if ctx.batch.as_ref().unwrap().frames.is_empty() {
            let batch = ctx.batch.take().unwrap();
            ctx.progress(batch.position, now);
        } else {
            // An admitted page contains at most eight frames.
            ctx.batch.as_mut().unwrap().index += 1;
        }
    }

    pub(super) fn recover(&mut self, proof: &HistoryAdvance, now: Instant) {
        if self.epoch.is_none() || self.epoch != proof.scope.connection {
            return;
        }
        let Some(ctx) = self
            .contexts
            .as_mut()
            .and_then(|contexts| contexts.get_mut(proof.slot))
        else {
            return;
        };
        let Some(gap) = ctx.gap.as_mut() else { return };
        if ctx.id != proof.context || Some(gap.id) != proof.gap {
            return;
        }
        if let Some(from) = &proof.from
            && gap.recovery.as_ref().is_none_or(|previous| {
                previous.cursor != *from
                    || previous.view != proof.view
                    || previous.frontiers != proof.frontiers
            })
        {
            return;
        }
        if proof.next_token.is_none() {
            ctx.gap = None;
            ctx.failures = 0;
            ctx.waiting = false;
            ctx.progress(
                Position {
                    view: proof.view,
                    frontiers: proof.frontiers,
                    next: proof.frontiers,
                    cursor: proof.cursor.clone(),
                },
                now,
            );
        } else {
            gap.recovery = Some(Recovery {
                view: proof.view,
                frontiers: proof.frontiers,
                cursor: proof.cursor.clone(),
            });
        }
    }
}

fn page_limit(slots: usize, bytes: usize) -> usize {
    8.min(DATA_SLOTS.saturating_sub(slots))
        .min(DATA_BYTES.saturating_sub(bytes) / ROW_CAP)
}

struct Frame(Vec<u8>);

impl Write for Frame {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > ROW_CAP - self.0.len() {
            return Err(io::Error::other("notification exceeds output cap"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn escaped(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn incoming(jsonl: bool, request: &Request, event: &[u8; 32], text: &str) -> io::Result<Vec<u8>> {
    let mut out = Frame(Vec::new());
    if jsonl {
        write!(
            out,
            r#"{{"v":1,"type":"event","event":"incoming","epoch":{},"runtime_instance":"{}","context":"{}","event_id":"{}","text":""#,
            request.epoch,
            request.instance,
            request.context,
            hex(event)
        )?;
    } else {
        write!(out, "Incoming [chat {}]: ", request.slot + 1)?;
    }
    for c in text.chars() {
        match c {
            '"' | '\\' if jsonl => write!(out, "\\{c}")?,
            c if escaped(c) && jsonl => write!(out, "\\u{:04x}", u32::from(c))?,
            c if escaped(c) => write!(out, "{}", c.escape_default())?,
            c => write!(out, "{c}")?,
        }
    }
    out.write_all(if jsonl { b"\"}\n" } else { b"\n" })?;
    Ok(out.0)
}

fn gap_notice(
    jsonl: bool,
    epoch: u64,
    binding: &Binding,
    slot: usize,
    code: &'static str,
) -> Vec<u8> {
    if jsonl {
        format!(
            "{}\n",
            json!({
                "v":1, "type":"event", "event":"resync_required", "epoch":epoch,
                "runtime_instance":binding.instance, "context":binding.contexts[slot], "code":code
            })
        )
        .into_bytes()
    } else {
        format!(
            "Chat {}: resync_required ({code}). Use /history.\n",
            slot + 1
        )
        .into_bytes()
    }
}
