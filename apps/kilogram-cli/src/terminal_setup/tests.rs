use super::*;
use std::io::Cursor;
use zeroize::Zeroizing;

fn request(id: &str, command: Command) -> Result<Request, &'static str> {
    Ok(Request {
        id: Some(id.into()),
        command: Ok(command),
    })
}

fn show_job(shared: &Shared) -> WriteJob {
    let mut state = shared.lock().unwrap();
    state.replies.clear();
    assert!(state.admit(request("same", Command::Show)));
    let show = state.pending.take().unwrap();
    WriteJob {
        generation: 3,
        reply: Reply {
            tag: Some(show.tag),
            payload: Payload::Secret(Zeroizing::new("test words".into())),
        },
    }
}

#[test]
fn t12a_immediate_id_reuse_and_show_ack_wait_for_the_exact_flight() {
    struct Client<'a>(&'a Shared, bool);
    impl Write for Client<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            // The client sees bytes while the writer has not yet sent its receipt.
            if !self.1 {
                self.1 = true;
                let mut state = self.0.lock().unwrap();
                assert!(state.admit(request("same", Command::Acknowledge)));
                assert_eq!(
                    state.pending.as_ref().unwrap().command,
                    Command::Acknowledge
                );
                assert!(!state.shown);
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let shared = Arc::new(Mutex::new(State::new(true)));
    let job = show_job(&shared);
    let old_tag = job.reply.tag.clone().unwrap();
    let receipt = write(&shared, &mut Client(&shared, false), job, true);
    let mut state = shared.lock().unwrap();
    let generation = state.pending.as_ref().unwrap().tag.generation;
    state.publishing(Some(&old_tag));
    assert_eq!(state.ids.get("same"), Some(&generation));
    let flight = Flight {
        generation: 3,
        until: Instant::now() + WRITE_LIMIT,
        show: true,
    };
    let stale = Receipt {
        generation: 2,
        ..receipt
    };
    assert!(!state.writer_ack(stale, &flight));
    assert!(!state.shown);
    assert_eq!(state.ids.get("same"), Some(&generation));
    assert!(state.writer_ack(receipt, &flight));
    let (jobs, queued) = mpsc::sync_channel(1);
    state.dispatch(&jobs).unwrap();
    let Job::Run { shown, entry, .. } = queued.try_recv().unwrap() else {
        panic!("expected acknowledgement job")
    };
    assert!(shown);
    assert_eq!(entry.tag.id.as_deref(), Some("same"));
}

#[test]
fn t12a_late_or_partial_show_never_enables_acknowledgement() {
    struct Broken(bool);
    impl Write for Broken {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            if self.0 {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "test"))
            } else {
                self.0 = true;
                Ok(1)
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            unreachable!()
        }
    }

    for late in [false, true] {
        let shared = Arc::new(Mutex::new(State::new(true)));
        let job = show_job(&shared);
        let receipt = if late {
            write(&shared, &mut io::sink(), job, true)
        } else {
            write(&shared, &mut Broken(false), job, true)
        };
        let mut state = shared.lock().unwrap();
        state.admit(request("ack", Command::Acknowledge));
        let flight = Flight {
            generation: 3,
            until: if late {
                receipt.completed - Duration::from_millis(1)
            } else {
                Instant::now() + WRITE_LIMIT
            },
            show: true,
        };
        assert!(state.writer_ack(receipt, &flight));
        assert!(!state.shown);
        assert!(state.pending.is_none());
        assert!(state.closing.as_ref().unwrap().failed);
    }
}

#[test]
fn t12a_cancel_is_bound_on_admission_and_status_is_a_snapshot() {
    let mut state = State::new(true);
    state.replies.clear();
    state.snapshot["state"] = json!("prepared");
    assert!(state.admit(request("create", Command::Create)));
    let (jobs, queued) = mpsc::sync_channel(1);
    state.dispatch(&jobs).unwrap();
    let Job::Run { entry, cancel, .. } = queued.try_recv().unwrap() else {
        panic!("missing job")
    };
    assert!(state.admit(request("status", Command::Status)));
    assert!(
        matches!(&state.replies[0].payload, Payload::Public(value) if value["state"] == "creating")
    );
    assert!(state.admit(request("create", Command::Create)));
    assert!(matches!(
        &state.replies[1].payload,
        Payload::FrameError("duplicate_id")
    ));
    assert!(state.admit(request("cancel", Command::Cancel(Some("create".into())))));
    assert!(cancel.load(Ordering::Acquire));
    assert!(!state.settle_creation(entry.tag.generation));
    assert_eq!(state.cancel(Some("create")), Err("too_late"));
    state.running = None;
    state.publishing(Some(&entry.tag));
    assert!(state.admit(request("create", Command::Create)));
    assert!(state.pending.is_some());
    state.interrupt();
    assert!(state.pending.is_none());
    assert!(state.closing.is_none());
    state.interrupt();
    assert!(state.closing.is_some());
}

#[test]
fn t12a_frames_and_explicit_recovery_flags() {
    let mut input = Cursor::new(
        b"{\"v\":1,\"id\":\"y\",\"op\":\"recovery_ack\",\"args\":{\"saved\":false}}\r\n",
    );
    let request = wire::read(&mut input, true).unwrap().unwrap().ok().unwrap();
    assert_eq!(request.command, Err("invalid_args"));
    let mut input = Cursor::new(b"\x03\n/recovery saved\n");
    assert_eq!(
        wire::read(&mut input, false)
            .unwrap()
            .unwrap()
            .ok()
            .unwrap()
            .command,
        Err("invalid_command")
    );
    assert_eq!(
        wire::read(&mut input, false)
            .unwrap()
            .unwrap()
            .ok()
            .unwrap()
            .command,
        Ok(Command::Acknowledge)
    );
    let mut state = State::new(false);
    state.admit(self::request("reveal", Command::Show));
    assert!(matches!(
        &state.replies[1].payload,
        Payload::Error("recovery_output_disabled")
    ));
}
