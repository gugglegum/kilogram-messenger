//! Terminal account setup owns one Setup, one local worker and one writer.
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

mod service;
#[cfg(test)]
mod tests;
mod wire;
use service::{Service, unopened};
use wire::{Command, Payload, Request};

const POLL: Duration = Duration::from_millis(10);
const WRITE_LIMIT: Duration = Duration::from_secs(3);
const CLOSE_LIMIT: Duration = Duration::from_secs(30);
const REPLY_LIMIT: usize = 4;

type Shared = Arc<Mutex<State>>;

#[derive(Clone)]
struct Tag {
    generation: u64,
    id: Option<String>,
}

struct Entry {
    tag: Tag,
    command: Command,
}

struct Reply {
    tag: Option<Tag>,
    payload: Payload,
}

struct Running {
    tag: Tag,
    cancel: Option<Arc<AtomicBool>>,
    settling: bool,
}

struct Closing {
    until: Instant,
    quit: Option<Tag>,
    drain: bool,
    failed: bool,
}

struct State {
    ids: HashMap<String, u64>,
    generation: u64,
    pending: Option<Entry>,
    running: Option<Running>,
    replies: VecDeque<Reply>,
    snapshot: Value,
    shown: bool,
    allow_recovery: bool,
    closing: Option<Closing>,
}

impl State {
    fn new(allow_recovery: bool) -> Self {
        Self {
            ids: HashMap::new(),
            generation: 0,
            pending: None,
            running: None,
            replies: VecDeque::from([Reply {
                tag: None,
                payload: Payload::Ready,
            }]),
            snapshot: unopened(),
            shown: false,
            allow_recovery,
            closing: None,
        }
    }

    fn next_generation(&mut self) -> Option<u64> {
        let next = self.generation.checked_add(1);
        if let Some(next) = next {
            self.generation = next;
        } else {
            self.close(None, false, true);
        }
        next
    }

    fn push(&mut self, tag: Option<Tag>, payload: Payload) {
        if self.closing.as_ref().is_some_and(|close| !close.drain) {
            return;
        }
        if self.replies.len() == REPLY_LIMIT {
            self.close(None, false, true);
        } else {
            self.replies.push_back(Reply { tag, payload });
        }
    }

    fn admit(&mut self, request: Result<Request, &'static str>) -> bool {
        if self.closing.is_some() {
            return false;
        }
        let Request { id, command } = match request {
            Ok(request) => request,
            Err(code) => {
                self.push(None, Payload::FrameError(code));
                return self.closing.is_none();
            }
        };
        if id.as_ref().is_some_and(|id| self.ids.contains_key(id)) {
            self.push(None, Payload::FrameError("duplicate_id"));
            return self.closing.is_none();
        }
        let Some(generation) = self.next_generation() else {
            return false;
        };
        if let Some(id) = &id {
            self.ids.insert(id.clone(), generation);
        }
        let tag = Tag { generation, id };
        let payload = match command {
            Err(code) => Payload::Error(code),
            Ok(Command::Status) => Payload::Public(self.snapshot.clone()),
            Ok(Command::Help) => Payload::Help,
            Ok(Command::Quit) => {
                self.close(Some(tag), true, false);
                return false;
            }
            Ok(Command::Cancel(target)) => match self.cancel(target.as_deref()) {
                Ok(()) => Payload::Public(json!({"requested":true})),
                Err(code) => Payload::Error(code),
            },
            Ok(Command::Show) if !self.allow_recovery => Payload::Error("recovery_output_disabled"),
            Ok(_) if self.running.is_some() || self.pending.is_some() => Payload::Error("busy"),
            Ok(command) => {
                self.pending = Some(Entry { tag, command });
                return true;
            }
        };
        self.push(Some(tag), payload);
        self.closing.is_none()
    }

    fn cancel(&mut self, target: Option<&str>) -> Result<(), &'static str> {
        let target = match target {
            Some(id) => *self.ids.get(id).ok_or("not_active")?,
            None => self
                .running
                .as_ref()
                .map(|run| run.tag.generation)
                .or_else(|| self.pending.as_ref().map(|entry| entry.tag.generation))
                .ok_or("not_active")?,
        };
        if let Some(run) = &self.running
            && run.tag.generation == target
        {
            let signal = run.cancel.as_ref().ok_or("not_active")?;
            if run.settling {
                return Err("too_late");
            }
            signal.store(true, Ordering::Release);
            return Ok(());
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|entry| entry.tag.generation == target && entry.command.cancellable())
        {
            let entry = self.pending.take().unwrap();
            self.push(Some(entry.tag), Payload::Error("cancelled"));
            return Ok(());
        }
        Err("not_active")
    }

    fn settle(&mut self, generation: u64) -> bool {
        let Some(run) = &mut self.running else {
            return false;
        };
        if run.tag.generation != generation {
            return false;
        }
        run.settling = true;
        run.cancel
            .as_ref()
            .is_some_and(|cancel| !cancel.load(Ordering::Acquire))
    }

    fn complete(&mut self, tag: Tag, result: Result<Payload, &'static str>, snapshot: Value) {
        self.shown &= snapshot["state"] == "recovery_available";
        self.snapshot = snapshot;
        let cleanup_unconfirmed = matches!(result, Err("cleanup_unconfirmed"));
        let payload = match result {
            Ok(Payload::Secret(_)) if self.closing.is_some() => Payload::Error("cancelled"),
            Ok(payload) => payload,
            Err(code) => Payload::Error(code),
        };
        self.push(Some(tag), payload);
        if cleanup_unconfirmed {
            self.close(None, true, true);
        }
    }

    fn close(&mut self, quit: Option<Tag>, drain: bool, failed: bool) {
        if let Some(close) = &mut self.closing {
            close.failed |= failed;
            close.drain &= drain;
        } else {
            self.closing = Some(Closing {
                until: Instant::now() + CLOSE_LIMIT,
                quit,
                drain,
                failed,
            });
        }
        if let Some(run) = &self.running
            && !run.settling
            && let Some(signal) = &run.cancel
        {
            signal.store(true, Ordering::Release);
        }
        let pending = self.pending.take();
        if self.closing.as_ref().is_some_and(|close| close.drain) {
            for reply in &mut self.replies {
                if matches!(reply.payload, Payload::Secret(_)) {
                    reply.payload = Payload::Error("cancelled");
                }
            }
            if let Some(entry) = pending {
                self.push(Some(entry.tag), Payload::Error("cancelled"));
            }
        } else {
            self.replies.clear();
        }
    }

    #[cfg(any(windows, test))]
    fn interrupt(&mut self) {
        if self.running.is_some() {
            let _ = self.cancel(None);
        } else if let Some(entry) = self.pending.take() {
            self.push(Some(entry.tag), Payload::Error("cancelled"));
        } else {
            self.close(None, false, false);
        }
    }

    fn dispatch(&mut self, jobs: &mpsc::SyncSender<Job>) -> io::Result<()> {
        if self.closing.is_some() || self.running.is_some() {
            return Ok(());
        }
        let Some(entry) = self.pending.take() else {
            return Ok(());
        };
        let create = entry.command == Command::Create;
        let cancellable = entry.command.cancellable();
        if entry.command == Command::ConsentAction(kilogram_first_dialog::consent::Action::Import)
            && let Some(consent) = self.snapshot.get_mut("consent")
            && consent["local_installation"] == "confirmed_current_session"
        {
            consent["local_installation"] = json!("previously_accepted");
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let tag = entry.tag.clone();
        jobs.try_send(Job::Run {
            entry,
            shown: self.shown,
            cancel: cancel.clone(),
        })
        .map_err(io::Error::other)?;
        self.running = Some(Running {
            tag,
            cancel: cancellable.then_some(cancel),
            settling: false,
        });
        if create && self.snapshot["state"] == "prepared" {
            self.snapshot["state"] = json!("creating");
        }
        Ok(())
    }

    fn publishing(&mut self, tag: Option<&Tag>) {
        if let Some(Tag {
            generation,
            id: Some(id),
        }) = tag
            && self.ids.get(id) == Some(generation)
        {
            self.ids.remove(id);
        }
    }

    fn writer_ack(&mut self, receipt: Receipt, flight: &Flight) -> bool {
        if receipt.generation != flight.generation {
            return false;
        }
        if !receipt.ok || receipt.completed > flight.until {
            self.close(None, false, true);
        } else if flight.show && receipt.written && self.closing.is_none() {
            self.shown = true;
        }
        true
    }
}

enum Job {
    Run {
        entry: Entry,
        shown: bool,
        cancel: Arc<AtomicBool>,
    },
    Close,
}

struct WriteJob {
    generation: u64,
    reply: Reply,
}

#[derive(Clone, Copy)]
struct Receipt {
    generation: u64,
    completed: Instant,
    ok: bool,
    written: bool,
}

struct Flight {
    generation: u64,
    until: Instant,
    show: bool,
}

fn write(shared: &Shared, stdout: &mut impl Write, job: WriteJob, jsonl: bool) -> Receipt {
    let WriteJob {
        generation,
        reply: Reply { tag, payload },
    } = job;
    let secret = matches!(payload, Payload::Secret(_));
    let id = tag.as_ref().and_then(|tag| tag.id.as_deref());
    let mut state = shared.lock().unwrap();
    if state.closing.as_ref().is_some_and(|close| !close.drain) {
        return Receipt {
            generation,
            completed: Instant::now(),
            ok: true,
            written: false,
        };
    }
    let cancelled = secret && state.closing.is_some();
    let payload = if cancelled {
        Payload::Error("cancelled")
    } else {
        payload
    };
    let bytes = wire::encode(jsonl, id, payload);
    if bytes.is_ok() {
        state.publishing(tag.as_ref());
    }
    drop(state);
    // No lock is held during the potentially blocking write.
    let written = bytes.and_then(|bytes| stdout.write_all(&bytes).and_then(|()| stdout.flush()));
    Receipt {
        generation,
        completed: Instant::now(),
        ok: written.is_ok(),
        written: written.is_ok() && !cancelled,
    }
}

fn input(shared: Shared, jsonl: bool) {
    let mut stdin = io::stdin().lock();
    loop {
        let request = wire::read(&mut stdin, jsonl);
        let mut state = shared.lock().unwrap();
        match request {
            Ok(Some(request)) => {
                if !state.admit(request) {
                    return;
                }
            }
            Ok(None) => {
                state.close(None, false, false);
                return;
            }
            Err(_) => {
                state.close(None, false, true);
                return;
            }
        }
    }
}

fn coordinate(
    shared: &Shared,
    jobs: &mpsc::SyncSender<Job>,
    worker: thread::JoinHandle<()>,
    output: &mpsc::SyncSender<WriteJob>,
    receipts: &mpsc::Receiver<Receipt>,
) -> i32 {
    let mut flight: Option<Flight> = None;
    let mut close_sent = false;
    loop {
        let mut state = shared.lock().unwrap();
        if let Some(current) = &flight {
            match receipts.try_recv() {
                Ok(receipt) => {
                    if state.writer_ack(receipt, current) {
                        flight = None;
                    }
                }
                Err(TryRecvError::Empty) if Instant::now() < current.until => {}
                _ => {
                    state.close(None, false, true);
                    flight = None;
                }
            }
        }
        let retired = worker.is_finished();
        if retired && !close_sent {
            state.close(None, false, true);
        }
        if state.closing.is_some() && state.running.is_none() && !close_sent && !retired {
            close_sent = jobs.try_send(Job::Close).is_ok();
        }
        if let Some(close) = &mut state.closing {
            if Instant::now() >= close.until {
                return 2;
            }
            if retired {
                let failed = close.failed;
                if !close.drain {
                    drop(state);
                    return if worker.join().is_ok() && !failed {
                        0
                    } else {
                        2
                    };
                }
                if flight.is_none() && state.replies.is_empty() {
                    if let Some(tag) = state.closing.as_mut().unwrap().quit.take() {
                        let payload = if failed {
                            Payload::Error("cleanup_unconfirmed")
                        } else {
                            Payload::Public(json!({"closed":true}))
                        };
                        state.push(Some(tag), payload);
                    } else {
                        drop(state);
                        return if worker.join().is_ok() && !failed {
                            0
                        } else {
                            2
                        };
                    }
                }
            }
        }
        if flight.is_none() {
            if let Some(reply) = state.replies.pop_front() {
                let Some(generation) = state.next_generation() else {
                    continue;
                };
                let show = matches!(reply.payload, Payload::Secret(_));
                if output.try_send(WriteJob { generation, reply }).is_ok() {
                    flight = Some(Flight {
                        generation,
                        until: Instant::now() + WRITE_LIMIT,
                        show,
                    });
                } else {
                    state.close(None, false, true);
                }
            } else if state.dispatch(jobs).is_err() {
                state.close(None, false, true);
            }
        }
        drop(state);
        thread::sleep(POLL);
    }
}

#[cfg(windows)]
fn run_windows(home: PathBuf, jsonl: bool, allow_recovery: bool) -> io::Result<i32> {
    use std::os::windows::io::AsHandle;

    let executable = std::env::current_exe()?;
    let bootstrap = executable.with_file_name("kilogram-bootstrap.exe");
    let console_output = io::stdout().is_terminal();
    let allow = allow_recovery || (!jsonl && io::stdin().is_terminal() && console_output);
    let shared = Arc::new(Mutex::new(State::new(allow)));
    let signal = if !jsonl && io::stdin().is_terminal() {
        let mut receiver = tokio::signal::windows::ctrl_c()?;
        let state = shared.clone();
        Some(tokio::runtime::Handle::current().spawn(async move {
            while receiver.recv().await.is_some() {
                state.lock().unwrap().interrupt();
            }
        }))
    } else {
        None
    };
    // A dedicated unbuffered handle avoids copying a partial secret frame into
    // the standard library's line buffer when writing to a closed pipe.
    let mut stdout = std::fs::File::from(io::stdout().as_handle().try_clone_to_owned()?);
    let (output, writes) = mpsc::sync_channel::<WriteJob>(1);
    let (acks, receipts) = mpsc::sync_channel(1);
    let state = shared.clone();
    let writer = thread::Builder::new()
        .name("setup-stdout".into())
        .spawn(move || {
            for job in writes {
                let receipt = if console_output && !matches!(job.reply.payload, Payload::Secret(_))
                {
                    // Stdout converts Unicode public text for a Windows console.
                    write(&state, &mut io::stdout().lock(), job, jsonl)
                } else {
                    write(&state, &mut stdout, job, jsonl)
                };
                if acks.send(receipt).is_err() {
                    break;
                }
            }
        })?;
    let (jobs, queued) = mpsc::sync_channel(1);
    let state = shared.clone();
    let worker = thread::Builder::new()
        .name("setup-operation".into())
        .spawn(move || {
            service::execute(
                Service::with_cli(home, bootstrap, executable),
                queued,
                state,
            )
        })?;
    let state = shared.clone();
    thread::Builder::new()
        .name("setup-stdin".into())
        .spawn(move || input(state, jsonl))?;
    let code = coordinate(&shared, &jobs, worker, &output, &receipts);
    drop(output);
    if writer.is_finished() {
        let _ = writer.join();
    }
    if let Some(signal) = signal {
        signal.abort();
    }
    Ok(code)
}

pub fn run(home: PathBuf, jsonl: bool, allow_recovery: bool) -> i32 {
    #[cfg(windows)]
    {
        run_windows(home, jsonl, allow_recovery).unwrap_or(2)
    }
    #[cfg(not(windows))]
    {
        let _ = (home, jsonl, allow_recovery);
        eprintln!("Error: setup_requires_windows.");
        2
    }
}
