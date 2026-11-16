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
    state.acknowledge(Slot::Cancel);
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
    state.acknowledge(Slot::Active);
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
