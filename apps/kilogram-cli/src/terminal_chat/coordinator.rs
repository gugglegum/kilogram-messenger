//! Bounded terminal admission, cancellation and output publication.
use super::{
    Command, CommandScope, Frontend, HistoryAdvance, Input, Prepared, RequestKey, encode_reply,
    hex, prepare_result_with_monitor, read_commands,
};
use kilogram_first_dialog::{
    chat::{Chat, SessionStop},
    operation::{OperationControl, OperationKind},
    send_recovery::RequestOperation,
};
use serde_json::json;
#[cfg(windows)]
use std::io::IsTerminal;
mod monitor;
#[cfg(test)]
mod tests;
use std::{
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(10);
const WRITE_LIMIT: Duration = Duration::from_secs(3);
const CANCEL_LIMIT: Duration = Duration::from_secs(2);
const CLOSE_LIMIT: Duration = Duration::from_secs(30);

type Shared = Arc<Mutex<State>>;

#[derive(Clone)]
struct Tag {
    generation: u64,
    id: Option<String>,
}

struct Entry {
    tag: Tag,
    command: super::CommandResult<Command>,
}

impl Entry {
    fn read_only_during_notification(&self) -> bool {
        matches!(
            self.command,
            Ok(Command::Status
                | Command::History { .. }
                | Command::Message {
                    operation: RequestOperation::Status,
                    ..
                })
        )
    }

    fn identity(&self) -> Option<RequestKey> {
        match &self.command {
            Ok(Command::Message {
                context: Some(context),
                request: Some(request),
                ..
            }) => Some(RequestKey {
                context: context.clone(),
                request_id: hex(request),
            }),
            _ => None,
        }
    }

    fn control(&self, now: Instant) -> OperationControl {
        let (kind, seconds) = match &self.command {
            Ok(Command::Connect { .. }) => (OperationKind::Read, 45),
            Ok(Command::Locator(_)) => (OperationKind::Locator, 30),
            Ok(Command::Message {
                operation: RequestOperation::Send(_),
                ..
            }) => (OperationKind::Send, 30),
            Ok(Command::Message {
                operation: RequestOperation::Retry,
                ..
            }) => (OperationKind::Retry, 30),
            _ => (OperationKind::Read, 10),
        };
        OperationControl::new(kind, now + Duration::from_secs(seconds), self.identity())
    }
}

struct Active {
    tag: Tag,
    control: Arc<OperationControl>,
    result: Option<Reply>,
    finalised: bool,
    retired: bool,
    ack: bool,
    opens_connection: Option<monitor::Binding>,
    status: bool,
}

struct Reply {
    tag: Tag,
    request: Option<RequestKey>,
    target: Option<String>,
    monitor: Option<serde_json::Value>,
    value: Prepared<HistoryAdvance>,
}

struct Closing {
    until: Instant,
    exit: bool,
    stream_error: bool,
    submitted: bool,
    clean: Option<bool>,
    reply: Option<Tag>,
    acknowledged: bool,
    generation: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FrameTag {
    connection: u64,
    batch: u64,
    index: u8,
    slot: u8,
}

// One handoff retains ownership while the sole writer holds the bytes.
enum Notification {
    Queued(FrameTag, Vec<u8>),
    Writing(FrameTag, usize),
}

struct Notice {
    generation: u64,
    text: String,
    ack: mpsc::SyncSender<bool>,
}

struct Background {
    scope: CommandScope,
    control: Arc<OperationControl>,
    preempted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Ready,
    Active,
    Cancel,
    Diagnostic,
    Queued,
    Close,
    Notice(u64),
    Notification(FrameTag),
}

enum Payload {
    Reply(Reply),
    Frame(&'static str),
    Text(Vec<u8>),
    Notice(Notice),
}

struct State {
    ids: HashMap<String, u64>,
    generation: u64,
    connection: Option<u64>,
    background: Option<Background>,
    monitor: monitor::Monitor,
    jsonl: bool,
    running: Option<Active>,
    queued: Option<Entry>,
    rejected: Option<Reply>,
    cancel_reply: Option<Reply>,
    diagnostic: Option<Payload>,
    cancel_slot: bool,
    diagnostic_slot: bool,
    notice: Option<Notice>,
    notification: Option<Notification>,
    closing: Option<Closing>,
    cleanup_unconfirmed: bool,
    output_failed: bool,
    ready: bool,
    stop: Arc<SessionStop>,
}

impl State {
    fn new() -> Self {
        Self {
            ids: HashMap::new(),
            generation: 0,
            connection: None,
            background: None,
            monitor: monitor::Monitor::default(),
            jsonl: false,
            running: None,
            queued: None,
            rejected: None,
            cancel_reply: None,
            diagnostic: None,
            cancel_slot: false,
            diagnostic_slot: false,
            notice: None,
            notification: None,
            closing: None,
            cleanup_unconfirmed: false,
            output_failed: false,
            ready: false,
            stop: Arc::new(SessionStop::default()),
        }
    }

    fn next_generation(&mut self, now: Instant) -> Option<u64> {
        let Some(generation) = self.generation.checked_add(1) else {
            self.close(now, None, true, true);
            return None;
        };
        self.generation = generation;
        Some(generation)
    }

    fn notice_live(&self, generation: u64) -> bool {
        !self.output_failed
            && self.closing.is_none()
            && self.running.as_ref().is_some_and(|active| {
                active.tag.generation == generation
                    && !active.finalised
                    && !active.control.stopped()
            })
    }

    fn discard_notice(&mut self, generation: u64) {
        if self
            .notice
            .as_ref()
            .is_some_and(|notice| notice.generation == generation)
        {
            self.notice.take();
        }
    }

    fn notification_allowed(&self) -> bool {
        !self.output_failed
            && self.closing.is_none()
            && self.running.is_none()
            && self.queued.is_none()
            && self.cancel_reply.is_none()
            && self.rejected.is_none()
            && self.diagnostic.is_none()
            && self.notice.is_none()
    }

    fn can_dispatch(&self, writing: Option<Slot>, output_selected: bool) -> bool {
        self.ready
            && self.closing.is_none()
            && !self.cleanup_unconfirmed
            && self.running.is_none()
            && self.background.is_none()
            && !output_selected
            && self.queued.as_ref().is_some_and(|entry| {
                writing.is_none()
                    || matches!(writing, Some(Slot::Notification(_)))
                        && entry.read_only_during_notification()
            })
    }

    fn release(&mut self, tag: &Tag) {
        if let Some(id) = &tag.id
            && self.ids.get(id) == Some(&tag.generation)
        {
            self.ids.remove(id);
        }
    }

    fn publishing(&mut self, slot: Slot, tag: Option<&Tag>) {
        if let Some(tag) = tag {
            self.release(tag);
        }
        // The previous reply now belongs exclusively to the writer. A
        // sequential caller can submit the next control frame immediately
        // after seeing it, before the coordinator observes the write ACK.
        match slot {
            Slot::Cancel => self.cancel_slot = false,
            Slot::Diagnostic => self.diagnostic_slot = false,
            _ => {}
        }
    }

    fn diagnostic(&mut self, result: Payload, now: Instant) {
        if self.diagnostic_slot {
            self.close(now, None, true, true);
        } else {
            self.diagnostic_slot = true;
            self.diagnostic = Some(result);
        }
    }

    fn admit(&mut self, input: Input, now: Instant) -> bool {
        if self.closing.as_ref().is_some_and(|c| c.exit) {
            return false;
        }
        let (id, command) = match input {
            Input::Command { id, command } => (id, command),
            Input::FrameError(code) => {
                self.diagnostic(Payload::Frame(code), now);
                return true;
            }
            Input::ReadError => {
                self.close(now, None, true, true);
                return false;
            }
        };
        if id.as_ref().is_some_and(|id| self.ids.contains_key(id)) {
            self.diagnostic(Payload::Frame("duplicate_id"), now);
            return true;
        }

        let Some(generation) = self.next_generation(now) else {
            return false;
        };
        let tag = Tag { generation, id };
        if let Some(id) = &tag.id {
            self.ids.insert(id.clone(), tag.generation);
        }
        let entry = Entry { tag, command };
        match &entry.command {
            Ok(Command::Quit | Command::Disconnect) if self.closing.is_none() => {
                let exit = entry.command == Ok(Command::Quit);
                self.close(now, Some(entry.tag), exit, false);
                return !exit;
            }
            Ok(Command::Cancel(target)) => {
                if self.cancel_slot {
                    self.release(&entry.tag);
                    self.close(now, None, true, true);
                    return false;
                }
                let value = self
                    .cancel(target.as_deref(), now)
                    .map(|state| (json!({"state":state}), None));
                self.cancel_slot = true;
                self.cancel_reply = Some(Reply {
                    tag: entry.tag,
                    request: None,
                    target: target.clone(),
                    monitor: None,
                    value,
                });
            }
            _ => {
                let code = if self.closing.is_some() {
                    Some("session_closing")
                } else if self.cleanup_unconfirmed {
                    Some("cleanup_unconfirmed")
                } else if self.queued.is_some() {
                    Some("busy")
                } else {
                    None
                };
                if let Some(code) = code {
                    if self.diagnostic_slot {
                        self.release(&entry.tag);
                        self.close(now, None, true, true);
                        return false;
                    }
                    self.diagnostic(Payload::Reply(reject(entry, code)), now);
                } else {
                    self.queued = Some(entry);
                    if let Some(background) = &mut self.background
                        && background.control.cancel(now)
                    {
                        background.preempted = now < background.control.deadline();
                    }
                }
            }
        }
        true
    }

    fn cancel(&mut self, target: Option<&str>, now: Instant) -> super::CommandResult<&'static str> {
        let generation = match target {
            Some(id) => self.ids.get(id).copied().ok_or("target_not_active")?,
            None => self
                .running
                .as_ref()
                .map(|run| run.tag.generation)
                .ok_or("target_not_active")?,
        };
        if self
            .queued
            .as_ref()
            .is_some_and(|entry| entry.tag.generation == generation)
        {
            return Err("target_queued");
        }
        let active = self
            .running
            .as_ref()
            .filter(|run| run.tag.generation == generation)
            .ok_or("target_not_active")?;
        if active.finalised {
            return Err("target_completed");
        }
        let requested = active.control.cancel(now);
        self.discard_notice(generation);
        Ok(if requested {
            "requested"
        } else {
            "already_requested"
        })
    }

    #[cfg(any(windows, test))]
    fn interrupt(&mut self, now: Instant) {
        if self.running.is_some() {
            let _ = self.cancel(None, now);
        } else if self.closing.is_none() {
            self.close(now, None, true, false);
        }
    }

    fn close(&mut self, now: Instant, reply: Option<Tag>, exit: bool, stream_error: bool) {
        if let Some(closing) = &mut self.closing {
            closing.exit |= exit;
            closing.stream_error |= stream_error;
            return;
        }
        if let Some(active) = &self.running {
            active.control.cancel(now);
        }
        if let Some(background) = &self.background {
            background.control.cancel(now);
        }
        self.connection = None;
        self.monitor.disconnect(if stream_error {
            "output_unconfirmed"
        } else {
            "disconnected"
        });
        self.notice.take();
        if matches!(self.notification, Some(Notification::Queued(..))) {
            self.notification.take();
        }
        self.stop.stop();
        self.rejected = self
            .queued
            .take()
            .map(|entry| reject(entry, "session_closing"));
        let generation = reply.as_ref().map(|tag| tag.generation);
        self.closing = Some(Closing {
            until: now + CLOSE_LIMIT,
            exit,
            stream_error,
            submitted: false,
            clean: None,
            acknowledged: reply.is_none(),
            reply,
            generation,
        });
    }

    fn failed_output(&mut self, now: Instant) {
        self.output_failed = true;
        self.notice.take();
        self.close(now, None, true, true);
    }

    fn tick(&mut self, now: Instant) {
        if let Some(background) = &self.background {
            if now >= background.control.deadline()
                || background.scope.connection != self.connection
            {
                background.control.cancel(now);
            }
            if background
                .control
                .cancel_time()
                .is_some_and(|at| now >= at + CANCEL_LIMIT)
                && self.closing.is_none()
            {
                self.monitor.disconnect("worker_unretired");
                self.close(now, None, true, true);
            }
        }
        if let Some(active) = &mut self.running {
            if !active.finalised
                && (now >= active.control.deadline()
                    || active
                        .control
                        .cancel_time()
                        .is_some_and(|at| now >= at + CANCEL_LIMIT))
            {
                active.control.cancel(now);
                active.finish(Err("cancelled"), true);
            }
            if active.ack && active.retired {
                if let Some(binding) = active.opens_connection.take()
                    && self.closing.is_none()
                {
                    self.connection = Some(active.tag.generation);
                    self.monitor.connect(active.tag.generation, binding, now);
                }
                self.running = None;
            }
        }
        if let Some(notice) = &self.notice
            && !self.notice_live(notice.generation)
        {
            self.notice.take();
        }
        if let Some(closing) = &mut self.closing
            && closing.clean.is_none()
            && now >= closing.until - WRITE_LIMIT
        {
            closing.clean = Some(false);
        }
    }

    fn next_output(&mut self) -> Option<(Slot, Payload)> {
        if self.output_failed {
            return None;
        }
        if let Some(reply) = self.cancel_reply.take() {
            return Some((Slot::Cancel, Payload::Reply(reply)));
        }
        if let Some(notice) = self.notice.take() {
            return Some((Slot::Notice(notice.generation), Payload::Notice(notice)));
        }
        if let Some(active) = &mut self.running
            && let Some(mut reply) = active.result.take()
        {
            if active.status {
                reply.monitor = Some(self.monitor.status());
            }
            return Some((Slot::Active, Payload::Reply(reply)));
        }
        if let Some(reply) = self.rejected.take() {
            return Some((Slot::Queued, Payload::Reply(reply)));
        }
        if let Some(payload) = self.diagnostic.take() {
            return Some((Slot::Diagnostic, payload));
        }
        let Some(closing) = self.closing.as_mut() else {
            return self.next_notification();
        };
        let clean = closing.clean?;
        let tag = closing.reply.take()?;
        let value = if clean {
            Ok((json!({"connected":false}), None))
        } else {
            Err("cleanup_unconfirmed")
        };
        Some((
            Slot::Close,
            Payload::Reply(Reply {
                tag,
                request: None,
                target: None,
                monitor: None,
                value,
            }),
        ))
    }

    fn next_notification(&mut self) -> Option<(Slot, Payload)> {
        if !self.notification_allowed() {
            return None;
        }
        if self.notification.is_none() {
            let (tag, bytes) = self.monitor.frame(self.jsonl)?;
            self.notification = Some(Notification::Queued(tag, bytes));
        }
        if !matches!(self.notification, Some(Notification::Queued(..))) {
            return None;
        }
        let Some(Notification::Queued(tag, bytes)) = self.notification.take() else {
            unreachable!();
        };
        if !self.notification_current(tag) {
            return None;
        }
        self.notification = Some(Notification::Writing(tag, bytes.len()));
        Some((Slot::Notification(tag), Payload::Text(bytes)))
    }

    fn notification_current(&self, tag: FrameTag) -> bool {
        if self.connection != Some(tag.connection) {
            return false;
        }
        #[cfg(test)]
        if tag.slot == u8::MAX {
            return true;
        }
        self.monitor.owns(tag)
    }

    fn notification_credit_used(&self) -> (usize, usize) {
        match &self.notification {
            Some(Notification::Queued(tag, bytes)) if tag.index != monitor::GAP_INDEX => {
                (1, bytes.len())
            }
            Some(Notification::Writing(tag, size)) if tag.index != monitor::GAP_INDEX => (1, *size),
            _ => (0, 0),
        }
    }

    fn retire_notification(&mut self, tag: FrameTag, deferred: Option<Vec<u8>>) -> bool {
        let Some(Notification::Writing(current, size)) = self.notification else {
            return false;
        };
        if current != tag {
            return false;
        }
        self.notification = deferred
            .filter(|bytes| {
                bytes.len() == size
                    && self.notification_current(tag)
                    && self.closing.is_none()
                    && !self.output_failed
            })
            .map(|bytes| Notification::Queued(tag, bytes));
        true
    }

    fn acknowledge(&mut self, slot: Slot, generation: Option<u64>) -> bool {
        match slot {
            Slot::Ready => self.ready = true,
            Slot::Active => {
                if let Some(active) = &mut self.running
                    && generation == Some(active.tag.generation)
                    && active.finalised
                    && !active.ack
                {
                    active.ack = true;
                    return true;
                }
            }
            Slot::Close => {
                if let Some(closing) = &mut self.closing
                    && generation.is_some()
                    && generation == closing.generation
                {
                    closing.acknowledged = true;
                }
            }
            Slot::Notification(_) => {}
            Slot::Cancel | Slot::Diagnostic | Slot::Queued | Slot::Notice(_) => {}
        }
        false
    }

    fn schedule_monitor(&mut self, now: Instant, worker: &mpsc::SyncSender<Job>) {
        if !self.ready
            || self.cleanup_unconfirmed
            || self.background.is_some()
            || !self.notification_allowed()
        {
            return;
        }
        let writing = self.notification_credit_used();
        let Some(request) = self.monitor.request(now, writing) else {
            return;
        };
        let Some(generation) = self.next_generation(now) else {
            return;
        };
        let scope = CommandScope {
            generation,
            connection: self.connection,
        };
        let control = Arc::new(OperationControl::new(
            OperationKind::Read,
            now + Duration::from_secs(5),
            None,
        ));
        if worker
            .try_send(Job::Monitor(scope, control.clone(), request))
            .is_ok()
        {
            self.background = Some(Background {
                scope,
                control,
                preempted: false,
            });
        } else {
            self.close(now, None, true, true);
        }
    }

    fn monitor_retired(
        &mut self,
        scope: CommandScope,
        slot: usize,
        reserved: usize,
        reply: Result<monitor::Response, monitor::Fault>,
        now: Instant,
    ) {
        if self
            .background
            .as_ref()
            .is_none_or(|work| work.scope != scope)
        {
            return;
        }
        let work = self.background.take().unwrap();
        let Some(epoch) = scope
            .connection
            .filter(|epoch| Some(*epoch) == self.connection)
        else {
            return;
        };
        if work.preempted || self.closing.is_some() {
            return;
        }
        let reply = if work.control.stopped() {
            Err(monitor::Fault::Unavailable)
        } else {
            reply
        };
        self.monitor
            .complete(epoch, scope.generation, slot, reserved, reply, now);
    }

    fn end_close(&mut self, now: Instant, writing: bool) -> Option<i32> {
        let closing = self.closing.as_ref()?;
        let clean = closing.clean?;
        let drained = self.running.is_none()
            && self.background.is_none()
            && self.rejected.is_none()
            && !self.cancel_slot
            && !self.diagnostic_slot
            && self.notice.is_none();
        let expired = now >= closing.until;
        if !(self.output_failed || expired || (drained && !writing && closing.acknowledged)) {
            return None;
        }
        let flagged = self.cleanup_unconfirmed || !clean;
        let code = if flagged {
            3
        } else if closing.stream_error || expired {
            2
        } else {
            0
        };
        if closing.exit || self.output_failed || expired {
            return Some(code);
        }
        self.cleanup_unconfirmed = flagged;
        self.closing = None;
        if !flagged {
            self.stop = Arc::new(SessionStop::default());
        }
        None
    }
}

impl Active {
    fn finish(&mut self, value: Prepared<HistoryAdvance>, fallback: bool) {
        let snapshot = self.control.finish(fallback);
        let value = if let Some(report) = snapshot.locator {
            Ok((json!(report), None))
        } else {
            snapshot
                .cancellation
                .map_or(value, |result| result.map(|report| (json!(report), None)))
        };
        self.result = Some(Reply {
            tag: self.tag.clone(),
            request: snapshot.identity,
            target: None,
            monitor: None,
            value,
        });
        self.finalised = true;
    }
}

fn reject(entry: Entry, code: &'static str) -> Reply {
    let request = entry.identity();
    Reply {
        tag: entry.tag,
        request,
        target: None,
        monitor: None,
        value: Err(code),
    }
}

enum Job {
    Run(
        Entry,
        CommandScope,
        Arc<OperationControl>,
        Arc<SessionStop>,
        Option<HistoryAdvance>,
        [Option<u64>; 2],
    ),
    Close(Instant),
    Monitor(CommandScope, Arc<OperationControl>, monitor::Request),
    #[cfg(test)]
    Background(CommandScope, Box<dyn FnOnce(&OperationControl) + Send>),
}

fn announce_notice(
    shared: &Shared,
    generation: u64,
    control: &OperationControl,
    key: &RequestKey,
) -> bool {
    let (ack, done) = mpsc::sync_channel(1);
    {
        let mut state = shared.lock().unwrap();
        if !state.notice_live(generation) || state.notice.is_some() {
            return false;
        }
        state.notice = Some(Notice {
            generation,
            text: format!("Request {}\n", key.request_id),
            ack,
        });
    }
    loop {
        if control.stopped() {
            shared.lock().unwrap().discard_notice(generation);
            return false;
        }
        match done.recv_timeout(POLL) {
            Ok(written) => return written && !control.stopped(),
            Err(mpsc::RecvTimeoutError::Disconnected) => return false,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn execute_jobs(
    mut frontend: Frontend,
    jsonl: bool,
    shared: Shared,
    jobs: mpsc::Receiver<Job>,
    #[cfg(test)] mut test_barrier: Option<(PathBuf, bool)>,
) {
    for job in jobs {
        match job {
            Job::Run(entry, scope, control, stop, advance, gaps) => {
                #[cfg(test)]
                if matches!(entry.command, Ok(Command::Status))
                    && let Some((marker, hold_after_cancel)) = test_barrier.take()
                {
                    std::fs::write(&marker, b"executing\n").unwrap();
                    let until = Instant::now() + Duration::from_secs(20);
                    while if hold_after_cancel {
                        !marker.with_extension("release").is_file() && Instant::now() < until
                    } else {
                        !control.stopped()
                    } {
                        thread::sleep(POLL);
                    }
                }
                if let Some(advance) = advance {
                    advance.apply(&mut frontend, scope.connection);
                }
                let opening = frontend.status.is_none()
                    && matches!(entry.command, Ok(Command::Connect { .. }));
                let selected = frontend.selected;
                let mut value = entry.command.and_then(|command| {
                    if control.stopped() {
                        return Err("cancelled");
                    }
                    frontend.execute(command, scope, &control, &stop, |key| {
                        jsonl || announce_notice(&shared, scope.generation, &control, key)
                    })
                });
                if let Ok((_, Some(proof))) = &mut value {
                    proof.gap = gaps[proof.slot];
                }
                let generation = entry.tag.generation;
                let rollback;
                {
                    let mut state = shared.lock().unwrap();
                    if matches!(value, Err("session_changed")) {
                        state.monitor.disconnect("scope_changed");
                    }
                    let Some(active) = state
                        .running
                        .as_mut()
                        .filter(|active| active.tag.generation == generation)
                    else {
                        return;
                    };
                    let stopped = control.stopped();
                    rollback = opening && (value.is_err() || stopped);
                    if stopped {
                        frontend.selected = selected;
                    }
                    if !active.finalised {
                        if stopped {
                            control.cancel(Instant::now());
                        }
                        active.finish(value, stopped);
                        if opening && !rollback {
                            active.opens_connection =
                                frontend.status.as_ref().map(monitor::Binding::from);
                        }
                    }
                    active.retired = !rollback;
                }
                if rollback {
                    frontend.status = None;
                    frontend.continuation = [None, None];
                    let end = shared.lock().unwrap().closing.as_ref().map_or_else(
                        || Instant::now() + CLOSE_LIMIT - WRITE_LIMIT,
                        |closing| closing.until - WRITE_LIMIT,
                    );
                    let clean = frontend.service.disconnect_until(end).is_ok();
                    let mut state = shared.lock().unwrap();
                    state.cleanup_unconfirmed |= !clean;
                    if let Some(active) = state.running.as_mut() {
                        active.retired = true;
                    }
                }
            }
            Job::Close(until) => {
                frontend.status = None;
                frontend.continuation = [None, None];
                let clean = frontend.service.disconnect_until(until).is_ok();
                let mut state = shared.lock().unwrap();
                if let Some(closing) = &mut state.closing {
                    if closing.until - WRITE_LIMIT == until {
                        closing.clean.get_or_insert(clean);
                    }
                }
            }
            Job::Monitor(scope, control, request) => {
                let slot = request.slot;
                let reserved = request.reserved();
                let result = if control.stopped() {
                    Err(monitor::Fault::Unavailable)
                } else {
                    request.execute(&mut frontend, &control, jsonl)
                };
                shared.lock().unwrap().monitor_retired(
                    scope,
                    slot,
                    reserved,
                    result,
                    Instant::now(),
                );
            }
            #[cfg(test)]
            Job::Background(scope, run) => {
                let control = shared
                    .lock()
                    .unwrap()
                    .background
                    .as_ref()
                    .filter(|work| work.scope == scope)
                    .map(|work| work.control.clone());
                if let Some(control) = control {
                    run(&control);
                    let mut state = shared.lock().unwrap();
                    if state
                        .background
                        .as_ref()
                        .is_some_and(|work| work.scope == scope)
                    {
                        state.background.take();
                    }
                }
            }
        }
    }
}

struct WriteJob {
    bytes: Vec<u8>,
    slot: Slot,
    tag: Option<Tag>,
    ack: mpsc::SyncSender<WriteReceipt>,
}

#[derive(Debug)]
enum WriteOutcome {
    Written,
    Skipped,
    Deferred(Vec<u8>),
    Failed,
}

struct WriteReceipt {
    outcome: WriteOutcome,
    completed: Instant,
}

struct Flight {
    slot: Slot,
    generation: Option<u64>,
    done: mpsc::Receiver<WriteReceipt>,
    until: Instant,
    timed_out: bool,
    advance: Option<HistoryAdvance>,
    notice: Option<mpsc::SyncSender<bool>>,
}

fn write_frame(shared: &Shared, stdout: &mut impl Write, job: WriteJob) -> bool {
    enum Admission {
        Start,
        Skip,
        Defer,
    }
    let admission = {
        let mut state = shared.lock().unwrap();
        let admission = match job.slot {
            _ if state.output_failed => Admission::Skip,
            Slot::Notice(generation) if !state.notice_live(generation) => Admission::Skip,
            Slot::Notification(tag) => {
                if !state.notification_current(tag)
                    || state.closing.is_some()
                    || !matches!(state.notification, Some(Notification::Writing(current, _)) if current == tag)
                {
                    Admission::Skip
                } else if !state.notification_allowed() {
                    Admission::Defer
                } else {
                    Admission::Start
                }
            }
            _ => Admission::Start,
        };
        if matches!(admission, Admission::Start) {
            state.publishing(job.slot, job.tag.as_ref());
        }
        admission
    };
    let WriteJob { bytes, ack, .. } = job;
    let outcome = match admission {
        Admission::Skip => {
            drop(bytes);
            WriteOutcome::Skipped
        }
        Admission::Defer => WriteOutcome::Deferred(bytes),
        Admission::Start => {
            let written = stdout
                .write_all(&bytes)
                .and_then(|()| stdout.flush())
                .is_ok();
            drop(bytes);
            if written {
                WriteOutcome::Written
            } else {
                WriteOutcome::Failed
            }
        }
    };
    let healthy = !matches!(outcome, WriteOutcome::Failed);
    let _ = ack.try_send(WriteReceipt {
        outcome,
        completed: Instant::now(),
    });
    healthy
}

fn send_output(
    output: &mpsc::SyncSender<WriteJob>,
    jsonl: bool,
    slot: Slot,
    payload: Payload,
) -> io::Result<Flight> {
    let mut advance = None;
    let mut notice = None;
    let mut tag = None;
    let bytes = match payload {
        Payload::Reply(reply) => {
            let id = reply.tag.id.as_deref();
            let request = reply.request.as_ref();
            let bytes = if let Some(target) = &reply.target {
                let value = reply.value.map(|(value, _)| value);
                encode_reply(jsonl, id, request, Some(target), &value, None)?
            } else {
                let (bytes, next) = prepare_result_with_monitor(
                    jsonl,
                    id,
                    request,
                    reply.value,
                    reply.monitor.as_ref(),
                )?;
                advance = next;
                bytes
            };
            tag = Some(reply.tag);
            bytes
        }
        Payload::Frame(code) if jsonl => {
            format!("{}\n", json!({"v":1,"type":"frame_error","code":code})).into_bytes()
        }
        Payload::Frame(code) => format!("Error: {code}.\n").into_bytes(),
        Payload::Text(bytes) => bytes,
        Payload::Notice(frame) => {
            notice = Some(frame.ack);
            frame.text.into_bytes()
        }
    };
    let generation = tag.as_ref().map(|tag| tag.generation);
    let (ack, done) = mpsc::sync_channel(1);
    let until = Instant::now() + WRITE_LIMIT;
    output
        .try_send(WriteJob {
            bytes,
            slot,
            tag,
            ack,
        })
        .map_err(io::Error::other)?;
    Ok(Flight {
        slot,
        generation,
        done,
        until,
        timed_out: false,
        advance,
        notice,
    })
}

fn service_output(
    shared: &Shared,
    flight: &mut Option<Flight>,
    advance: &mut Option<HistoryAdvance>,
    now: Instant,
) {
    let Some(item) = flight.as_mut() else {
        return;
    };
    let outcome = match item.done.try_recv() {
        Ok(receipt) => {
            if !item.timed_out
                && matches!(receipt.outcome, WriteOutcome::Written)
                && receipt.completed > item.until
            {
                item.timed_out = true;
                shared.lock().unwrap().failed_output(now);
            }
            receipt.outcome
        }
        Err(TryRecvError::Empty) if now < item.until => return,
        Err(TryRecvError::Empty) => {
            if !item.timed_out {
                item.timed_out = true;
                shared.lock().unwrap().failed_output(now);
            }
            return;
        }
        Err(TryRecvError::Disconnected) => WriteOutcome::Failed,
    };
    let item = flight.take().unwrap();
    let mut state = shared.lock().unwrap();
    if let Some(notice) = item.notice {
        if let Slot::Notice(generation) = item.slot {
            let success = matches!(outcome, WriteOutcome::Written)
                && !item.timed_out
                && state.notice_live(generation);
            let _ = notice.try_send(success);
        }
    }
    if item.timed_out || !matches!(outcome, WriteOutcome::Written) {
        let failed = matches!(outcome, WriteOutcome::Failed);
        if let Slot::Notification(tag) = item.slot {
            let returned = match outcome {
                WriteOutcome::Deferred(bytes) if !item.timed_out => Some(bytes),
                _ => None,
            };
            state.retire_notification(tag, returned);
        }
        if failed && !item.timed_out {
            state.failed_output(now);
        }
        return;
    }
    if let Slot::Notification(tag) = item.slot {
        if state.retire_notification(tag, None) {
            state.monitor.written(tag, now);
        }
        return;
    }
    if state.acknowledge(item.slot, item.generation)
        && let Some(proof) = item.advance
        && Some(proof.scope.generation) == item.generation
        && proof.scope.connection.is_some()
        && proof.scope.connection == state.connection
    {
        state.monitor.recover(&proof, now);
        *advance = Some(proof);
    }
}

#[cfg(windows)]
fn listen_interrupt(
    jsonl: bool,
    shared: &Shared,
) -> io::Result<Option<tokio::task::JoinHandle<()>>> {
    if jsonl || !io::stdin().is_terminal() {
        return Ok(None);
    }
    let mut receiver = tokio::signal::windows::ctrl_c()?;
    let shared = shared.clone();
    Ok(Some(tokio::runtime::Handle::current().spawn(async move {
        while receiver.recv().await.is_some() {
            shared.lock().unwrap().interrupt(Instant::now());
        }
    })))
}

pub fn run(setup: PathBuf, jsonl: bool) -> i32 {
    let Ok(executable) = std::env::current_exe() else {
        return 2;
    };
    let shared = Arc::new(Mutex::new(State::new()));
    #[cfg(windows)]
    let signal = match listen_interrupt(jsonl, &shared) {
        Ok(signal) => signal,
        Err(_) => return 2,
    };
    let result = run_threads(
        Frontend {
            service: Chat::new(setup, executable),
            status: None,
            selected: 0,
            continuation: [None, None],
        },
        jsonl,
        &shared,
        #[cfg(test)]
        None,
    );
    #[cfg(windows)]
    if let Some(signal) = signal {
        signal.abort();
    }
    result
}

fn run_threads(
    frontend: Frontend,
    jsonl: bool,
    shared: &Shared,
    #[cfg(test)] test_barrier: Option<(PathBuf, bool)>,
) -> i32 {
    shared.lock().unwrap().jsonl = jsonl;
    let (output, writes) = mpsc::sync_channel::<WriteJob>(1);
    let state = shared.clone();
    if thread::Builder::new()
        .name("chat-stdout".into())
        .spawn(move || {
            let mut stdout = io::stdout().lock();
            for job in writes {
                if !write_frame(&state, &mut stdout, job) {
                    break;
                }
            }
        })
        .is_err()
    {
        return 2;
    }

    let (worker, jobs) = mpsc::sync_channel(1);
    let state = shared.clone();
    if thread::Builder::new()
        .name("chat-operation".into())
        .spawn(move || {
            execute_jobs(
                frontend,
                jsonl,
                state,
                jobs,
                #[cfg(test)]
                test_barrier,
            )
        })
        .is_err()
    {
        return 2;
    }

    let state = shared.clone();
    if thread::Builder::new()
        .name("chat-stdin".into())
        .spawn(move || {
            if read_commands(io::stdin().lock(), jsonl, |input| {
                state.lock().unwrap().admit(input, Instant::now())
            }) {
                state
                    .lock()
                    .unwrap()
                    .close(Instant::now(), None, true, false);
            }
        })
        .is_err()
    {
        return 2;
    }

    let ready = if jsonl {
        b"{\"v\":1,\"type\":\"ready\"}\n".as_slice()
    } else {
        b"First-dialog terminal. Use /help.\n"
    };
    let Ok(ready) = send_output(&output, jsonl, Slot::Ready, Payload::Text(ready.to_vec())) else {
        return 2;
    };
    let mut flight = Some(ready);
    let mut advance = None;
    loop {
        let now = Instant::now();
        service_output(shared, &mut flight, &mut advance, now);
        let pending_output;
        {
            let mut state = shared.lock().unwrap();
            state.tick(now);
            if let Some(code) = state.end_close(now, flight.is_some()) {
                return code;
            }
            if let Some(closing) = &mut state.closing {
                if !closing.submitted && closing.clean.is_none() {
                    closing.submitted = worker
                        .try_send(Job::Close(closing.until - WRITE_LIMIT))
                        .is_ok();
                    advance = None;
                }
            }
            pending_output = if flight.is_none() {
                state.next_output()
            } else {
                None
            };
            if state.can_dispatch(
                flight.as_ref().map(|item| item.slot),
                pending_output.is_some(),
            ) && let Some(entry) = state.queued.take()
            {
                let control = Arc::new(entry.control(now));
                let tag = entry.tag.clone();
                let status = matches!(entry.command, Ok(Command::Status));
                let scope = CommandScope {
                    generation: tag.generation,
                    connection: state.connection,
                };
                let job = Job::Run(
                    entry,
                    scope,
                    control.clone(),
                    state.stop.clone(),
                    advance.take(),
                    state.monitor.gaps(),
                );
                if worker.try_send(job).is_ok() {
                    state.running = Some(Active {
                        tag,
                        control,
                        result: None,
                        finalised: false,
                        retired: false,
                        ack: false,
                        opens_connection: None,
                        status,
                    });
                } else {
                    state.close(now, None, true, true);
                }
            }
            state.schedule_monitor(now, &worker);
        }
        if let Some((slot, payload)) = pending_output {
            match send_output(&output, jsonl, slot, payload) {
                Ok(sent) => flight = Some(sent),
                Err(_) => shared.lock().unwrap().failed_output(now),
            }
        }
        thread::sleep(POLL);
    }
}
