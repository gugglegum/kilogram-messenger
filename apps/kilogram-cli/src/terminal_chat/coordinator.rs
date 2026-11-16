//! Bounded terminal admission, cancellation and output publication.
use super::{
    Command, Frontend, HistoryAdvance, Input, Prepared, RequestKey, encode_reply, hex,
    prepare_result, read_commands,
};
use kilogram_first_dialog::{
    chat::{Chat, SessionStop},
    operation::{OperationControl, OperationKind},
    send_recovery::RequestOperation,
};
use serde_json::json;
#[cfg(windows)]
use std::io::IsTerminal;
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
}

struct Reply {
    tag: Tag,
    request: Option<RequestKey>,
    target: Option<String>,
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
}

#[derive(Clone, Copy)]
enum Slot {
    Ready,
    Active,
    Cancel,
    Diagnostic,
    Queued,
    Close,
    Notice,
}

enum Payload {
    Reply(Reply),
    Frame(&'static str),
    Text(Vec<u8>),
    Notice(String, mpsc::SyncSender<bool>),
}

struct State {
    ids: HashMap<String, u64>,
    generation: u64,
    running: Option<Active>,
    queued: Option<Entry>,
    rejected: Option<Reply>,
    cancel_reply: Option<Reply>,
    diagnostic: Option<Payload>,
    cancel_slot: bool,
    diagnostic_slot: bool,
    notice: Option<(String, mpsc::SyncSender<bool>)>,
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
            running: None,
            queued: None,
            rejected: None,
            cancel_reply: None,
            diagnostic: None,
            cancel_slot: false,
            diagnostic_slot: false,
            notice: None,
            closing: None,
            cleanup_unconfirmed: false,
            output_failed: false,
            ready: false,
            stop: Arc::new(SessionStop::default()),
        }
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

        self.generation += 1;
        let tag = Tag {
            generation: self.generation,
            id,
        };
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
                }
            }
        }
        true
    }

    fn cancel(&self, target: Option<&str>, now: Instant) -> super::CommandResult<&'static str> {
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
        Ok(if active.control.cancel(now) {
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
        self.stop.stop();
        self.rejected = self
            .queued
            .take()
            .map(|entry| reject(entry, "session_closing"));
        self.closing = Some(Closing {
            until: now + CLOSE_LIMIT,
            exit,
            stream_error,
            submitted: false,
            clean: None,
            acknowledged: reply.is_none(),
            reply,
        });
    }

    fn failed_output(&mut self, now: Instant) {
        self.output_failed = true;
        self.notice.take();
        self.close(now, None, true, true);
    }

    fn tick(&mut self, now: Instant) {
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
                self.running = None;
            }
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
        if let Some((text, ack)) = self.notice.take() {
            return Some((Slot::Notice, Payload::Notice(text, ack)));
        }
        if let Some(reply) = self
            .running
            .as_mut()
            .and_then(|active| active.result.take())
        {
            return Some((Slot::Active, Payload::Reply(reply)));
        }
        if let Some(reply) = self.rejected.take() {
            return Some((Slot::Queued, Payload::Reply(reply)));
        }
        if let Some(payload) = self.diagnostic.take() {
            return Some((Slot::Diagnostic, payload));
        }
        let closing = self.closing.as_mut()?;
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
                value,
            }),
        ))
    }

    fn acknowledge(&mut self, slot: Slot) {
        match slot {
            Slot::Ready => self.ready = true,
            Slot::Active => {
                if let Some(active) = &mut self.running {
                    active.ack = true;
                }
            }
            Slot::Close => {
                if let Some(closing) = &mut self.closing {
                    closing.acknowledged = true;
                }
            }
            Slot::Cancel | Slot::Diagnostic | Slot::Queued | Slot::Notice => {}
        }
    }

    fn end_close(&mut self, now: Instant, writing: bool) -> Option<i32> {
        let closing = self.closing.as_ref()?;
        let clean = closing.clean?;
        let drained = self.running.is_none()
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
        value: Err(code),
    }
}

enum Job {
    Run(
        Entry,
        Arc<OperationControl>,
        Arc<SessionStop>,
        Option<HistoryAdvance>,
    ),
    Close(Instant),
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
            Job::Run(entry, control, stop, advance) => {
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
                if let Some(HistoryAdvance { slot, next }) = advance {
                    frontend.continuation[slot] = next;
                }
                let opening = frontend.status.is_none()
                    && matches!(entry.command, Ok(Command::Connect { .. }));
                let selected = frontend.selected;
                let value = entry.command.and_then(|command| {
                    if control.stopped() {
                        return Err("cancelled");
                    }
                    frontend.execute(command, &control, &stop, |key| {
                        if jsonl {
                            return true;
                        }
                        let (ack, done) = mpsc::sync_channel(1);
                        {
                            let mut state = shared.lock().unwrap();
                            if state.output_failed || control.stopped() {
                                return false;
                            }
                            state.notice = Some((format!("Request {}\n", key.request_id), ack));
                        }
                        done.recv().unwrap_or(false)
                    })
                });
                let generation = entry.tag.generation;
                let rollback;
                {
                    let mut state = shared.lock().unwrap();
                    let Some(active) = state
                        .running
                        .as_mut()
                        .filter(|active| active.tag.generation == generation)
                    else {
                        return;
                    };
                    rollback = opening && (value.is_err() || control.stopped());
                    if control.stopped() {
                        frontend.selected = selected;
                    }
                    if !active.finalised {
                        let stopped = control.stopped();
                        if stopped {
                            control.cancel(Instant::now());
                        }
                        active.finish(value, stopped);
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
        }
    }
}

struct WriteJob {
    bytes: Vec<u8>,
    slot: Slot,
    tag: Option<Tag>,
    ack: mpsc::SyncSender<io::Result<()>>,
}

struct Flight {
    slot: Slot,
    done: mpsc::Receiver<io::Result<()>>,
    until: Instant,
    advance: Option<HistoryAdvance>,
    notice: Option<mpsc::SyncSender<bool>>,
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
                encode_reply(jsonl, id, request, Some(target), &value)?
            } else {
                let (bytes, next) = prepare_result(jsonl, id, request, reply.value)?;
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
        Payload::Notice(text, ack) => {
            notice = Some(ack);
            text.into_bytes()
        }
    };
    let (ack, done) = mpsc::sync_channel(1);
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
        done,
        until: Instant::now() + WRITE_LIMIT,
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
    let Some(item) = flight.as_ref() else {
        return;
    };
    let success = match item.done.try_recv() {
        Ok(result) => result.is_ok(),
        Err(TryRecvError::Empty) if now < item.until => return,
        Err(_) => false,
    };
    let item = flight.take().unwrap();
    let mut state = shared.lock().unwrap();
    if let Some(notice) = item.notice {
        let _ = notice.try_send(success);
    }
    if success {
        if item.advance.is_some() {
            *advance = item.advance;
        }
        state.acknowledge(item.slot);
    } else {
        state.failed_output(now);
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
    let (output, writes) = mpsc::sync_channel::<WriteJob>(1);
    let state = shared.clone();
    if thread::Builder::new()
        .name("chat-stdout".into())
        .spawn(move || {
            let mut stdout = io::stdout().lock();
            for job in writes {
                state.lock().unwrap().publishing(job.slot, job.tag.as_ref());
                let result = stdout.write_all(&job.bytes).and_then(|()| stdout.flush());
                let failed = result.is_err();
                let _ = job.ack.try_send(result);
                if failed {
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
            if state.ready
                && state.closing.is_none()
                && !state.cleanup_unconfirmed
                && state.running.is_none()
                && flight.is_none()
                && pending_output.is_none()
                && let Some(entry) = state.queued.take()
            {
                let control = Arc::new(entry.control(now));
                let tag = entry.tag.clone();
                let job = Job::Run(entry, control.clone(), state.stop.clone(), advance.take());
                if worker.try_send(job).is_ok() {
                    state.running = Some(Active {
                        tag,
                        control,
                        result: None,
                        finalised: false,
                        retired: false,
                        ack: false,
                    });
                } else {
                    state.close(now, None, true, true);
                }
            }
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
