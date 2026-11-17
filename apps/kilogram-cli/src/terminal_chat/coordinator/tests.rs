use super::*;

fn command(id: &str, command: Command) -> Input {
    Input::Command {
        id: Some(id.into()),
        command: Ok(command),
    }
}

fn activate(state: &mut State, now: Instant) -> Arc<OperationControl> {
    let entry = state.queued.take().unwrap();
    let control = Arc::new(entry.control(now));
    state.running = Some(Active {
        tag: entry.tag,
        control: control.clone(),
        result: None,
        finalised: false,
        retired: false,
        ack: false,
        opens_connection: None,
        status: false,
    });
    control
}

#[test]
fn t11c_complete_cancel_and_reuse_are_generation_bound() {
    let now = Instant::now();
    let mut state = State::new();
    state.admit(command("reuse", Command::Status), now);
    let old = activate(&mut state, now);
    state.admit(command("reuse", Command::Cancel(Some("reuse".into()))), now);
    assert!(matches!(
        state.next_output(),
        Some((Slot::Diagnostic, Payload::Frame("duplicate_id")))
    ));
    state.publishing(Slot::Diagnostic, None);
    state.admit(
        command("cancel", Command::Cancel(Some("reuse".into()))),
        now,
    );
    let Some((Slot::Cancel, Payload::Reply(ack))) = state.next_output() else {
        panic!("missing independent cancel");
    };
    assert_eq!(ack.target.as_deref(), Some("reuse"));
    assert_eq!(ack.value.unwrap().0["state"], "requested");
    state.publishing(Slot::Cancel, Some(&ack.tag));
    state.admit(
        command("cancel", Command::Cancel(Some("reuse".into()))),
        now,
    );
    state.acknowledge(Slot::Cancel, Some(ack.tag.generation));
    let Some((Slot::Cancel, Payload::Reply(second_ack))) = state.next_output() else {
        panic!("next cancel was lost before the prior flush");
    };
    assert_eq!(second_ack.value.unwrap().0["state"], "already_requested");
    state.publishing(Slot::Cancel, Some(&second_ack.tag));
    assert!(state.closing.is_none());
    assert_eq!(state.cancel(Some("reuse"), now), Ok("already_requested"));
    state.tick(now + CANCEL_LIMIT);
    assert_eq!(state.cancel(Some("reuse"), now), Err("target_completed"));
    state.interrupt(now);
    assert!(state.closing.is_none(), "retiring work is not idle");
    let Some((Slot::Active, Payload::Reply(result))) = state.next_output() else {
        panic!("missing original result");
    };
    assert_eq!(result.value.err(), Some("cancelled"));
    state.release(&result.tag);
    assert_eq!(state.cancel(Some("reuse"), now), Err("target_not_active"));
    state.admit(command("reuse", Command::Status), now);
    assert_eq!(state.cancel(Some("reuse"), now), Err("target_queued"));
    state.release(&result.tag);
    assert_eq!(state.cancel(Some("reuse"), now), Err("target_queued"));
    assert!(state.acknowledge(Slot::Active, Some(result.tag.generation)));
    state.tick(now + CANCEL_LIMIT);
    assert!(state.running.is_some());
    state.running.as_mut().unwrap().retired = true;
    state.tick(now + CANCEL_LIMIT);
    assert!(state.running.is_none());
    let fresh = activate(&mut state, now);
    assert!(!Arc::ptr_eq(&old, &fresh));
    assert!(fresh.cancel_time().is_none());
    state
        .running
        .as_mut()
        .unwrap()
        .finish(Ok((json!({"complete":true}), None)), false);
    assert_eq!(state.cancel(Some("reuse"), now), Err("target_completed"));
    assert!(!state.acknowledge(Slot::Active, Some(result.tag.generation)));
    assert!(!state.running.as_ref().unwrap().ack);
    assert!(
        state
            .running
            .as_ref()
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .value
            .is_ok()
    );
}

#[test]
fn t11c_control_and_close_do_not_wait_on_queue_or_writer() {
    let now = Instant::now();
    let mut state = State::new();
    state.admit(command("active", Command::Status), now);
    let active = activate(&mut state, now);
    state.admit(command("queued", Command::Status), now);
    assert!(state.admit(
        command("cancel", Command::Cancel(Some("active".into()))),
        now
    ));
    assert!(active.cancel_time().is_some());
    assert!(!state.admit(command("quit", Command::Quit), now));
    assert!(state.queued.is_none());
    assert_eq!(
        state.rejected.as_ref().unwrap().value.as_ref().err(),
        Some(&"session_closing")
    );
    assert_eq!(
        state.rejected.as_ref().unwrap().tag.id.as_deref(),
        Some("queued")
    );
    // The first close arms an absolute bound, independent of an unacknowledged write.
    state.tick(now + CLOSE_LIMIT - WRITE_LIMIT);
    let closing = state.closing.as_ref().unwrap();
    assert_eq!(closing.clean, Some(false));
    assert_eq!(closing.until, now + CLOSE_LIMIT);
    assert_eq!(state.end_close(now + CLOSE_LIMIT, true), Some(3));
}

const TEST_LIMIT: Duration = Duration::from_secs(10);

fn monitor_binding() -> monitor::Binding {
    monitor::Binding::from(&kilogram_first_dialog::chat::ChatStatus {
        mode: kilogram_first_dialog::chat::Mode::Observer,
        contexts: ["a".repeat(64), "b".repeat(64)],
        runtime_instance: "c".repeat(64),
        live_admitted: false,
        lan_discovery: None,
        own_roles: [0, 1],
    })
}

#[test]
fn t11e2b_monitor_job_preemption_and_stale_retirement() {
    let now = Instant::now();
    let mut state = State::new();
    state.ready = true;
    let epoch = state.next_generation(now).unwrap();
    state.connection = Some(epoch);
    state.monitor.connect(epoch, monitor_binding(), now);
    let (output, jobs) = mpsc::sync_channel(1);
    state.schedule_monitor(now, &output);
    let Job::Monitor(old, control, request) = jobs.try_recv().unwrap() else {
        panic!("expected monitor job")
    };
    assert!(state.admit(command("user", Command::Status), now));
    assert!(control.stopped() && !state.can_dispatch(None, false));
    state.monitor_retired(
        old,
        request.slot,
        request.reserved(),
        Err(monitor::Fault::Unavailable),
        now,
    );
    assert!(state.can_dispatch(None, false));
    assert_eq!(state.monitor.status()["contexts"][0]["state"], "anchoring");
    let pending = state.queued.take().unwrap();
    state.release(&pending.tag);
    state.monitor.disconnect("disconnected");
    let next = state.next_generation(now).unwrap();
    state.connection = Some(next);
    state.monitor.connect(next, monitor_binding(), now);
    state.schedule_monitor(now, &output);
    assert!(matches!(jobs.try_recv(), Err(TryRecvError::Empty)));
    let later = now + Duration::from_millis(250);
    state.schedule_monitor(later, &output);
    let Job::Monitor(new, _, request) = jobs.try_recv().unwrap() else {
        panic!("expected current job")
    };
    state.monitor_retired(
        old,
        request.slot,
        0,
        Err(monitor::Fault::Gap("stale")),
        later,
    );
    assert!(
        state
            .background
            .as_ref()
            .is_some_and(|work| work.scope == new)
    );
    assert_eq!(state.monitor.status()["contexts"][0]["state"], "anchoring");
    state.monitor_retired(new, request.slot, 0, Err(monitor::Fault::NoHistory), later);
    assert!(state.background.is_none());
    assert_eq!(
        state.monitor.status()["contexts"][0]["state"],
        "waiting_history"
    );
}

#[test]
fn t11e2b_old_gap_ack_cannot_consume_current_signal() {
    let now = Instant::now();
    let mut state = State::new();
    state.ready = true;
    state.connection = Some(1);
    state.monitor.connect(1, monitor_binding(), now);
    state
        .monitor
        .complete(1, 2, 0, 0, Err(monitor::Fault::Gap("invalid_cursor")), now);
    let (Slot::Notification(stale), _) = state.next_output().unwrap() else {
        panic!("missing gap")
    };
    state.monitor.disconnect("disconnected");
    state.connection = Some(3);
    state.monitor.connect(3, monitor_binding(), now);
    assert!(state.retire_notification(stale, None));
    state.monitor.written(stale, now);
    let (Slot::Notification(current), _) = state.next_output().unwrap() else {
        panic!("missing new epoch gap")
    };
    assert_eq!(current.connection, 3);
    assert!(!state.retire_notification(stale, None));
    assert!(state.notification_current(current));
    assert!(state.retire_notification(current, None));
    state.monitor.written(current, now);
    assert!(state.next_output().is_none());
}

fn ready_state() -> Shared {
    let mut state = State::new();
    state.ready = true;
    state.connection = state.next_generation(Instant::now());
    Arc::new(Mutex::new(state))
}

fn stage_notification(state: &Shared) -> (FrameTag, WriteJob, Option<Flight>) {
    let mut state = state.lock().unwrap();
    let tag = FrameTag {
        connection: state.connection.unwrap(),
        batch: state.next_generation(Instant::now()).unwrap(),
        index: 0,
        slot: u8::MAX,
    };
    assert!(state.notification.is_none());
    state.notification = Some(Notification::Queued(tag, b"test notification\n".to_vec()));
    let (slot, payload) = state.next_output().unwrap();
    let (output, writes) = mpsc::sync_channel(1);
    let flight = send_output(&output, true, slot, payload).unwrap();
    (tag, writes.recv_timeout(TEST_LIMIT).unwrap(), Some(flight))
}

struct HeldWriter<'a> {
    state: &'a Shared,
    entered: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
}

impl Write for HeldWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert!(
            self.state.try_lock().is_ok(),
            "writer holds the coordinator mutex"
        );
        self.entered.send(()).unwrap();
        self.release.recv_timeout(TEST_LIMIT).unwrap();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        assert!(
            self.state.try_lock().is_ok(),
            "flush holds the coordinator mutex"
        );
        Ok(())
    }
}

#[test]
fn t11e2a_paused_notification_permits_only_foreground_reads() {
    let state = ready_state();
    let (frame, job, mut flight) = stage_notification(&state);
    let (entered, writing) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    thread::scope(|scope| {
        let writer = &state;
        scope.spawn(move || {
            assert!(write_frame(
                writer,
                &mut HeldWriter {
                    state: writer,
                    entered,
                    release: gate
                },
                job
            ));
        });
        writing.recv_timeout(TEST_LIMIT).unwrap();
        let now = Instant::now();
        {
            let mut state = state.lock().unwrap();
            let occupied = Some(Slot::Notification(frame));
            for text in [
                "/send hello",
                "/retry 00000000000000000000000000000001",
                "/locator export",
                "/use 2",
            ] {
                let command = super::super::parse_tty(text).unwrap();
                assert!(state.admit(self::command("mutation", command), now));
                assert!(!state.can_dispatch(occupied, false), "{text}");
                let entry = state.queued.take().unwrap();
                state.release(&entry.tag);
            }
            assert!(state.admit(command("read", Command::Status), now));
            assert!(state.can_dispatch(occupied, false));
            let control = activate(&mut state, now);
            assert!(state.admit(command("cancel", Command::Cancel(Some("read".into()))), now));
            assert!(control.stopped());
            state.tick(now + CANCEL_LIMIT);
            assert!(!state.running.as_ref().unwrap().ack);
            assert!(matches!(state.next_output(), Some((Slot::Cancel, _))));
        }
        release.send(()).unwrap();
    });
    service_output(&state, &mut flight, &mut None, Instant::now());
    let mut state = state.lock().unwrap();
    assert!(!state.running.as_ref().unwrap().ack);
    assert!(state.notification.is_none());
    assert!(!state.output_failed);
    let send = super::super::parse_tty("/send still gated").unwrap();
    let now = Instant::now();
    state.admit(command("send", send), now);
    assert!(!state.can_dispatch(None, false));
    let read_generation = state.running.as_ref().unwrap().tag.generation;
    assert!(state.acknowledge(Slot::Active, Some(read_generation)));
    state.tick(now);
    assert!(!state.can_dispatch(None, false), "ACK retired the reader");
    state.running.as_mut().unwrap().retired = true;
    state.tick(now);
    assert!(state.can_dispatch(None, false));
}

#[test]
fn t11e2a_deferred_and_stale_frames_keep_exact_ownership() {
    let state = ready_state();
    let (tag, job, mut flight) = stage_notification(&state);
    let now = Instant::now();
    {
        let mut state = state.lock().unwrap();
        state.admit(command("read", Command::Status), now);
        state.retire_notification(FrameTag { index: 1, ..tag }, None);
        assert!(
            matches!(state.notification, Some(Notification::Writing(current, 18)) if current == tag)
        );
    }
    let mut bytes = Vec::new();
    assert!(write_frame(&state, &mut bytes, job));
    service_output(&state, &mut flight, &mut None, now);
    assert!(bytes.is_empty());
    let (stale, mut flight) = {
        let mut state = state.lock().unwrap();
        assert!(
            matches!(&state.notification, Some(Notification::Queued(current, bytes)) if *current == tag && bytes == b"test notification\n")
        );
        let entry = state.queued.take().unwrap();
        state.release(&entry.tag);
        let (slot, payload) = state.next_output().unwrap();
        let (output, writes) = mpsc::sync_channel(1);
        let flight = send_output(&output, true, slot, payload).unwrap();
        state.connection = state.next_generation(now);
        (writes.recv_timeout(TEST_LIMIT).unwrap(), Some(flight))
    };
    assert!(write_frame(&state, &mut bytes, stale));
    service_output(&state, &mut flight, &mut None, now);
    assert!(bytes.is_empty());
    let state = state.lock().unwrap();
    assert!(state.notification.is_none());
    assert!(!state.output_failed);
    assert!(state.closing.is_none());
}

#[test]
fn t11e2a_write_timeout_retains_flight_until_writer_retires() {
    let state = ready_state();
    let (tag, job, mut flight) = stage_notification(&state);
    let when = flight.as_ref().unwrap().until;
    service_output(&state, &mut flight, &mut None, when);
    assert!(flight.as_ref().unwrap().timed_out);
    assert!(
        matches!(state.lock().unwrap().notification, Some(Notification::Writing(current, _)) if current == tag)
    );
    let mut bytes = Vec::new();
    assert!(write_frame(&state, &mut bytes, job));
    service_output(&state, &mut flight, &mut None, when);
    let state = state.lock().unwrap();
    assert!(flight.is_none() && bytes.is_empty() && state.notification.is_none());
    assert!(state.output_failed && state.closing.is_some());
}

#[test]
fn t11e2a_foreground_receipt_must_have_completed_within_the_write_deadline() {
    for late in [false, true] {
        let shared = ready_state();
        let now = Instant::now();
        let (slot, payload) = {
            let mut state = shared.lock().unwrap();
            state.admit(command("read", Command::Status), now);
            activate(&mut state, now);
            let active = state.running.as_mut().unwrap();
            active.finish(Ok((json!({"connected":true}), None)), false);
            state.next_output().unwrap()
        };
        let (output, writes) = mpsc::sync_channel(1);
        let frame = send_output(&output, true, slot, payload).unwrap();
        let WriteJob { bytes, ack, .. } = writes.recv_timeout(TEST_LIMIT).unwrap();
        drop(bytes);
        let after = frame.until + Duration::from_millis(1);
        let completed = if late { after } else { frame.until };
        assert!(
            ack.try_send(WriteReceipt {
                outcome: WriteOutcome::Written,
                completed
            })
            .is_ok()
        );
        service_output(&shared, &mut Some(frame), &mut None, after);
        let state = shared.lock().unwrap();
        assert_eq!(state.running.as_ref().unwrap().ack, !late);
        assert_eq!(state.output_failed, late);
    }
}

fn receive_notice(state: &Shared) -> (WriteJob, Option<Flight>) {
    let deadline = Instant::now() + TEST_LIMIT;
    let (slot, payload) = loop {
        if let Some(output) = state.lock().unwrap().next_output() {
            break output;
        }
        assert!(Instant::now() < deadline, "notice producer did not run");
        thread::sleep(POLL);
    };
    assert!(matches!(slot, Slot::Notice(_)));
    let (output, writes) = mpsc::sync_channel(1);
    let flight = send_output(&output, false, slot, payload).unwrap();
    (writes.recv_timeout(TEST_LIMIT).unwrap(), Some(flight))
}

#[test]
fn t11e2a_cancel_and_close_interrupt_tty_notice_gate() {
    for (already_writing, close) in [(false, false), (true, false), (true, true)] {
        let state = ready_state();
        let now = Instant::now();
        let (control, generation) = {
            let mut state = state.lock().unwrap();
            state.admit(
                command("sending", super::super::parse_tty("/send gated").unwrap()),
                now,
            );
            let control = activate(&mut state, now);
            (control, state.running.as_ref().unwrap().tag.generation)
        };
        thread::scope(|scope| {
            let waiting = &state;
            let (allow, allowed) = mpsc::sync_channel(1);
            scope.spawn(move || {
                let key = RequestKey {
                    context: "fixture".into(),
                    request_id: "id".into(),
                };
                allow
                    .send(announce_notice(waiting, generation, &control, &key))
                    .unwrap();
            });
            let (job, mut flight) = receive_notice(&state);
            if already_writing {
                let (entered, writing) = mpsc::sync_channel(1);
                let (release, gate) = mpsc::sync_channel(1);
                let writer = &state;
                let written = scope.spawn(move || {
                    write_frame(
                        writer,
                        &mut HeldWriter {
                            state: writer,
                            entered,
                            release: gate,
                        },
                        job,
                    )
                });
                writing.recv_timeout(TEST_LIMIT).unwrap();
                if close {
                    state
                        .lock()
                        .unwrap()
                        .close(Instant::now(), None, true, false);
                } else {
                    assert_eq!(
                        state
                            .lock()
                            .unwrap()
                            .cancel(Some("sending"), Instant::now()),
                        Ok("requested")
                    );
                }
                assert!(
                    !allowed.recv_timeout(TEST_LIMIT).unwrap(),
                    "interrupted notice admitted the send"
                );
                release.send(()).unwrap();
                assert!(written.join().unwrap());
            } else {
                assert_eq!(
                    state
                        .lock()
                        .unwrap()
                        .cancel(Some("sending"), Instant::now()),
                    Ok("requested")
                );
                assert!(
                    !allowed.recv_timeout(TEST_LIMIT).unwrap(),
                    "cancel admitted the send"
                );
                let mut bytes = Vec::new();
                assert!(write_frame(&state, &mut bytes, job));
                assert!(bytes.is_empty(), "cancelled notice was published");
            }
            service_output(&state, &mut flight, &mut None, Instant::now());
            assert!(!state.lock().unwrap().output_failed);
        });
    }
}

#[test]
fn t11e2a_background_preemption_waits_for_retirement_and_close_has_a_bound() {
    for (timeout, eof) in [(false, false), (true, false), (false, true)] {
        let state = ready_state();
        let now = Instant::now();
        let control = Arc::new(OperationControl::new(
            OperationKind::Read,
            now + Duration::from_secs(5),
            None,
        ));
        let tag = {
            let mut state = state.lock().unwrap();
            let tag = CommandScope {
                generation: state.next_generation(now).unwrap(),
                connection: state.connection,
            };
            state.background = Some(Background {
                scope: tag,
                control: control.clone(),
                preempted: false,
            });
            tag
        };
        let (worker, jobs) = mpsc::sync_channel(1);
        let (entered, running) = mpsc::sync_channel(1);
        let (release, gate) = mpsc::sync_channel(1);
        thread::scope(|scope| {
            let worker_state = state.clone();
            scope.spawn(move || {
                let empty = Frontend {
                    service: Chat::new(PathBuf::new(), PathBuf::new()),
                    status: None,
                    selected: 0,
                    continuation: [None, None],
                };
                execute_jobs(empty, true, worker_state, jobs, None);
            });
            let task = Job::Background(
                tag,
                Box::new(move |control| {
                    entered.send(()).unwrap();
                    gate.recv_timeout(TEST_LIMIT).unwrap();
                    assert!(control.stopped());
                }),
            );
            assert!(worker.try_send(task).is_ok());
            running.recv_timeout(TEST_LIMIT).unwrap();
            {
                let mut state = state.lock().unwrap();
                if eof {
                    state.close(now, None, true, false);
                    state.tick(now + CANCEL_LIMIT);
                    let close = state.closing.as_ref().unwrap();
                    assert!(close.exit && !close.stream_error);
                    assert!(
                        worker
                            .try_send(Job::Close(close.until - WRITE_LIMIT))
                            .is_ok()
                    );
                } else if timeout {
                    let expiry = now + Duration::from_secs(5);
                    state.tick(expiry);
                    assert!(control.stopped() && state.closing.is_none());
                    state.tick(expiry + CANCEL_LIMIT);
                    let close = state.closing.as_ref().unwrap();
                    assert!(close.exit && close.stream_error);
                    assert!(
                        worker
                            .try_send(Job::Close(close.until - WRITE_LIMIT))
                            .is_ok()
                    );
                } else {
                    state.admit(command("foreground", Command::Status), now);
                    assert!(control.stopped());
                }
                assert!(!state.can_dispatch(None, false));
                assert!(
                    state
                        .background
                        .as_ref()
                        .is_some_and(|work| work.scope == tag)
                );
            }
            release.send(()).unwrap();
            drop(worker);
        });
        let mut state = state.lock().unwrap();
        assert!(state.background.is_none());
        if timeout || eof {
            assert_eq!(state.closing.as_ref().unwrap().clean, Some(true));
            assert_eq!(state.end_close(now, false), Some(if eof { 0 } else { 2 }));
        } else {
            assert!(state.can_dispatch(None, false));
        }
    }
}

#[test]
fn t11e2a_generation_exhaustion_closes_without_reusing_identity() {
    let now = Instant::now();
    let mut state = State::new();
    state.generation = u64::MAX;
    assert!(!state.admit(command("new", Command::Status), now));
    assert_eq!(state.generation, u64::MAX);
    assert!(state.ids.is_empty() && state.queued.is_none());
    assert!(
        state
            .closing
            .as_ref()
            .is_some_and(|close| close.exit && close.stream_error)
    );
}

#[cfg(windows)]
#[test]
fn t11e2a_history_proof_real_observer() -> anyhow::Result<()> {
    use kilogram_first_dialog::chat::Mode;

    let Some(home) = std::env::var_os("KILOGRAM_T11E2A_PROOF_SETUP") else {
        return Ok(());
    };
    let instance = std::env::var("KILOGRAM_T11E2A_PROOF_INSTANCE")?;
    let mut frontend = Frontend {
        service: Chat::new(PathBuf::from(home), std::env::current_exe()?),
        status: None,
        selected: 1,
        continuation: [None, None],
    };
    let status = frontend.service.connect(Mode::Observer, Some(&instance))?;
    let context = status.contexts[1].clone();
    frontend.status = Some(status);
    let shared = ready_state();
    for more in [false, true] {
        let now = Instant::now();
        let (scope, control, stop) = {
            let mut state = shared.lock().unwrap();
            assert!(state.admit(command("history", Command::Status), now));
            let control = activate(&mut state, now);
            let scope = CommandScope {
                generation: state.running.as_ref().unwrap().tag.generation,
                connection: state.connection,
            };
            (scope, control, state.stop.clone())
        };
        let previous = frontend.continuation[1]
            .as_ref()
            .map(|page| page.cursor.clone());
        let result = frontend
            .execute(
                Command::History {
                    context: None,
                    limit: 1,
                    cursor: None,
                    more,
                },
                scope,
                &control,
                &stop,
                |_| unreachable!(),
            )
            .map_err(anyhow::Error::msg)?;
        anyhow::ensure!(result.0["finished"] == more);
        let proof = result.1.as_ref().unwrap();
        assert!(proof.scope == scope && proof.context == context && proof.from == previous);
        assert_eq!(proof.next_token.is_none(), more);
        let cursor = proof.cursor.clone();
        let (slot, payload) = {
            let mut state = shared.lock().unwrap();
            let active = state.running.as_mut().unwrap();
            active.finish(Ok(result), false);
            active.retired = true;
            state.next_output().unwrap()
        };
        let (output, writes) = mpsc::sync_channel(1);
        let mut flight = Some(send_output(&output, true, slot, payload)?);
        let mut advance = None;
        service_output(&shared, &mut flight, &mut advance, now);
        assert!(advance.is_none());
        assert!(write_frame(
            &shared,
            &mut Vec::new(),
            writes.recv_timeout(TEST_LIMIT)?
        ));
        service_output(&shared, &mut flight, &mut advance, now);
        let confirmed = advance.unwrap();
        assert!(confirmed.cursor == cursor);
        confirmed.apply(&mut frontend, scope.connection);
        shared.lock().unwrap().tick(now);
        if more {
            assert!(frontend.continuation[1].is_none());
            let end = frontend
                .service
                .history_position_controlled(1, Some(cursor), &control)?;
            assert_eq!(end.next, end.frontiers);
        }
    }
    frontend.service.disconnect()?;
    println!(
        "T11E2A_HISTORY_PROOF_PASS continuation_and_finished=true ack_only=true real_observer=true"
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn t11c_windows_console_child() -> anyhow::Result<()> {
    let Some(marker) = std::env::var_os("KILOGRAM_T11C_CONSOLE_MARKER").map(PathBuf::from) else {
        return Ok(());
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let _runtime = runtime.enter();
    let shared = Arc::new(Mutex::new(State::new()));
    let jsonl = std::env::var_os("KILOGRAM_T11C_CONTROL_SETUP").is_some();
    let signal = if jsonl {
        None
    } else {
        Some(listen_interrupt(false, &shared)?.expect("private console stdin"))
    };
    let setup = std::env::var_os("KILOGRAM_T11C_CONTROL_SETUP")
        .map(PathBuf::from)
        .unwrap_or_else(|| marker.parent().unwrap().to_owned());
    let executable = match std::env::var_os("KILOGRAM_T11C_CONTROL_EXE") {
        Some(path) => PathBuf::from(path),
        None => std::env::current_exe()?,
    };
    let frontend = Frontend {
        service: Chat::new(setup, executable),
        status: None,
        selected: 0,
        continuation: [None, None],
    };
    let result = run_threads(frontend, jsonl, &shared, Some((marker, jsonl)));
    if let Some(signal) = signal {
        signal.abort();
    }
    assert_eq!(result, 0);
    println!("T11C_CONSOLE_HANDLED");
    Ok(())
}
