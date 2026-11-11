use super::*;
use crate::fresh_onboarding::send_recovery::Progress;
use kilogram_runtime_ipc::ordered2::{Delivery2, HistoryCursor2, HistoryPage2, MAX_HISTORY_ROWS};
use std::collections::BTreeMap;

const SENTINEL: &str = "CS33A wire synthetic sentinel";
const TEXT: &str = "  CS33A wire synthetic sentinel \u{e9} e\u{301} \0\n  ";

struct ObservedRuntime<'a> {
    real: SessionSend<'a>,
    store: &'a send_intent::Store,
    lose_reply: bool,
    submits: usize,
    lookups: usize,
    lost_id: Option<[u8; 16]>,
}
impl SendRuntime for ObservedRuntime<'_> {
    fn check_send(&mut self, slot: u8) -> IpcResult<()> {
        self.real.check_send(slot)
    }
    fn lookup(&mut self, slot: u8, request: [u8; 16]) -> IpcResult<Option<RequestStatus2>> {
        self.lookups += 1;
        self.real.lookup(slot, request)
    }
    fn send(&mut self, slot: u8, request: [u8; 16], text: &str) -> IpcResult<SendReceipt2> {
        let saved = self
            .store
            .load()
            .map_err(|_| MessageError2::Unavailable)?
            .ok_or(MessageError2::Unavailable)?;
        if saved.request != request || saved.slot != slot || saved.text != text {
            return Err(MessageError2::InvalidInput);
        }
        self.submits += 1;
        let result = self.real.send(slot, request, text)?;
        if self.lose_reply {
            self.lost_id = Some(request);
            return Err(MessageError2::Unavailable);
        }
        Ok(result)
    }
}

fn launch(setup: &mut Setup) -> Result<Session> {
    Session::start(
        setup,
        &cli(),
        Arc::new(AtomicBool::new(false)),
        false,
        &Cleanup::default(),
    )
}

fn stop(session: &mut Session) -> Result<()> {
    let outcome = session.finish(&Cleanup::default());
    ensure!(
        outcome.graceful && outcome.reaped && outcome.exit == Some(0) && outcome.eof,
        "Owned message runtime did not stop gracefully"
    );
    Ok(())
}

fn connect(sessions: &mut [Session; 2], nodes: &mut [Setup; 2], signal: &AtomicBool) -> Result<()> {
    let [a, b] = sessions;
    let from_a = a.export(&nodes[0], signal)?;
    let from_b = b.export(&nodes[1], signal)?;
    a.import(&mut nodes[0], &from_b, signal)?;
    b.import(&mut nodes[1], &from_a, signal)?;
    let until = Instant::now() + Duration::from_secs(40);
    loop {
        let av = a.poll(signal)?;
        let bv = b.poll(signal)?;
        if av.current(Instant::now(), consent::now()?)
            && bv.current(Instant::now(), consent::now()?)
        {
            return Ok(());
        }
        ensure!(
            Instant::now() < until,
            "Local message fixture did not reach both live slots"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_delivery(
    session: &mut Session,
    slot: u8,
    request: [u8; 16],
    signal: &AtomicBool,
) -> Result<RequestStatus2> {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let status = SessionSend {
            session,
            cancel: signal,
        }
        .lookup(slot, request)?;
        if let Some(status) = status
            && status.delivery == Delivery2::Delivered
        {
            return Ok(status);
        }
        ensure!(Instant::now() < until, "Missing durable message receipt");
        thread::sleep(Duration::from_millis(100));
    }
}

fn page(session: &Session, slot: u8, cursor: Option<&HistoryCursor2>) -> Result<HistoryPage2> {
    let client = session.client.as_ref().context("Owned message client")?;
    Ok(session.rt.block_on(client.history_page(
        slot,
        id(&session.doc.contexts[usize::from(slot)])?,
        cursor,
        MAX_HISTORY_ROWS,
    ))?)
}

fn collect(
    session: &Session,
    mut page: HistoryPage2,
    expected: &BTreeMap<[u8; 16], ([u8; 32], &str)>,
    author: u8,
) -> Result<usize> {
    let mut observed = std::collections::BTreeSet::new();
    for n in 1..=8 {
        for row in &page.items {
            let (event, text) = expected
                .get(&row.request)
                .context("Unexpected history request")?;
            ensure!(
                observed.insert(row.request)
                    && &row.event == event
                    && row.text.as_bytes() == text.as_bytes()
                    && row.role == author,
                "History duplicated or changed a durable event"
            );
        }
        if page.finished {
            ensure!(
                observed.len() == expected.len(),
                "History omitted a durable request"
            );
            return Ok(n);
        }
        page = self::page(session, 0, Some(&page.cursor))?;
    }
    anyhow::bail!("History fixture did not finish in its page budget")
}

fn no_plaintext(nodes: &[Setup; 2], sessions: &[Session; 2]) -> Result<()> {
    let contains = |raw: &[u8]| {
        raw.windows(SENTINEL.len())
            .any(|w| w == SENTINEL.as_bytes())
    };
    for node in nodes {
        ensure!(!contains(&model::read(
            &node.home.join("setup.json"),
            128 * 1024
        )?));
        let pending = node.home.join("pending-send");
        if pending.is_dir() {
            for entry in fs::read_dir(pending)? {
                ensure!(!contains(&model::read(&entry?.path(), 32 * 1024)?));
            }
        }
    }
    for session in sessions {
        for drain in &session.drains {
            let bytes = drain
                .buffer
                .lock()
                .map_err(|_| anyhow::anyhow!("Runtime drain poisoned"))?;
            let raw: Vec<_> = bytes.iter().copied().collect();
            ensure!(
                !contains(&raw),
                "Runtime output contained synthetic plaintext"
            );
        }
    }
    Ok(())
}

#[test]
fn cs33a_actual_reopen_and_framed_history() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/clean-start-cs3-3a/fixtures");
    let mut nodes = ready_pair_at(&root, "messages-", false)?;
    let signal = AtomicBool::new(false);
    let mut sessions = [launch(&mut nodes[0])?, launch(&mut nodes[1])?];
    connect(&mut sessions, &mut nodes, &signal)?;
    let role = sessions[0].role;
    let request = {
        let store = send_intent::Store::new(&nodes[0], &sessions[0].doc)?;
        let mut recovery = Recovery::new(send_intent::Store::new(&nodes[0], &sessions[0].doc));
        let mut runtime = ObservedRuntime {
            real: SessionSend {
                session: &mut sessions[0],
                cancel: &signal,
            },
            store: &store,
            lose_reply: true,
            submits: 0,
            lookups: 0,
            lost_id: None,
        };
        recovery.submit(&mut runtime, 0, Zeroizing::new(TEXT.into()));
        let request = runtime
            .lost_id
            .context("Actual reply was not intercepted")?;
        ensure!(
            runtime.submits == 1
                && runtime.lookups == 1
                && recovery.progress == Progress::Unconfirmed
        );
        ensure!(store.load()?.is_some_and(|x| x.request == request
            && x.phase == send_intent::Phase::Prepared
            && x.text == TEXT));
        request
    };
    let original = wait_delivery(&mut sessions[0], 0, request, &signal)?;
    no_plaintext(&nodes, &sessions)?;
    stop(&mut sessions[0])?;
    // Verify the store while no runtime owns it, then reopen the same Setup and runtime.
    let mut repo = repository(&nodes[0])?;
    let (saved, saved_text) = repo
        .request(0, request)?
        .context("Missing committed request")?;
    ensure!(
        saved.queue_sequence == original.receipt.queue_sequence
            && saved.event == original.receipt.event
            && saved_text == TEXT
    );
    ensure!(repo.request_delivery(0, request)? == Some(2));
    drop(repo);
    let [left, right] = nodes;
    let home = left.home.clone();
    drop(left);
    nodes = [Setup::open(&home)?, right];
    sessions[0] = launch(&mut nodes[0])?;
    {
        let store = send_intent::Store::new(&nodes[0], &sessions[0].doc)?;
        let mut recovery = Recovery::new(send_intent::Store::new(&nodes[0], &sessions[0].doc));
        let mut runtime = ObservedRuntime {
            real: SessionSend {
                session: &mut sessions[0],
                cancel: &signal,
            },
            store: &store,
            lose_reply: false,
            submits: 0,
            lookups: 0,
            lost_id: None,
        };
        recovery.retry(&mut runtime);
        ensure!(
            runtime.lookups == 1
                && runtime.submits == 0
                && recovery.progress == Progress::Delivered
                && store.load()?.is_none(),
            "Reopened committed intent was replayed or not cleared"
        );
        let retry = runtime.real.send(0, request, TEXT)?;
        ensure!(
            retry == original.receipt,
            "Exact runtime retry changed the accepted event"
        );
        ensure!(
            runtime.real.send(0, request, "different bytes").err()
                == Some(MessageError2::RequestIdConflict),
            "Different bytes were accepted under the same ID"
        );
    }
    connect(&mut sessions, &mut nodes, &signal)?;
    let max_a = "\0".repeat(4096);
    let max_b = "\u{1}".repeat(4096);
    let mut expected = BTreeMap::from([(
        request,
        (original.receipt.event.context("Delivered event")?, TEXT),
    )]);
    for (i, text) in [
        "short A",
        max_a.as_str(),
        " short B ",
        max_b.as_str(),
        "short C",
    ]
    .into_iter()
    .enumerate()
    {
        let request = [0x40 + i as u8; 16];
        SessionSend {
            session: &mut sessions[0],
            cancel: &signal,
        }
        .send(0, request, text)?;
        let status = wait_delivery(&mut sessions[0], 0, request, &signal)?;
        expected.insert(
            request,
            (status.receipt.event.context("Durable history event")?, text),
        );
    }
    let snapshots = [page(&sessions[0], 0, None)?, page(&sessions[1], 0, None)?];
    for (session, first) in sessions.iter().zip(&snapshots) {
        ensure!(first.items.len() > 1 && first.items.len() < MAX_HISTORY_ROWS && !first.finished);
        ensure!(
            first
                .items
                .iter()
                .filter(|row| row.text.len() == 4096)
                .count()
                == 1
        );
        let repeat = page(session, 0, None)?;
        ensure!(
            first.cursor == repeat.cursor
                && first
                    .items
                    .iter()
                    .map(|r| r.event)
                    .eq(repeat.items.iter().map(|r| r.event))
        );
        let other = page(session, 1, None)?;
        ensure!(
            other.items.is_empty() && other.finished,
            "History crossed slots"
        );
        ensure!(
            page(session, 1, Some(&first.cursor)).is_err(),
            "Foreign context cursor accepted"
        );
    }
    let appended = [0x60; 16];
    SessionSend {
        session: &mut sessions[0],
        cancel: &signal,
    }
    .send(0, appended, "snapshot append")?;
    let appended_status = wait_delivery(&mut sessions[0], 0, appended, &signal)?;
    for (session, snapshot) in sessions.iter().zip(snapshots) {
        ensure!(
            collect(session, snapshot, &expected, role)? == 2,
            "Actual byte split count"
        );
    }
    expected.insert(
        appended,
        (
            appended_status
                .receipt
                .event
                .context("Appended history event")?,
            "snapshot append",
        ),
    );
    for session in &sessions {
        ensure!(collect(session, page(session, 0, None)?, &expected, role)? == 2);
    }
    no_plaintext(&nodes, &sessions)?;

    // A Prepared-only record is an interrupted UI operation before dispatch.
    // Exercise actual Work actions without adding a product send button.
    let offline = [0x62; 16];
    let store = send_intent::Store::new(&nodes[0], &sessions[0].doc)?;
    store.prepare(1, offline, Zeroizing::new("offline same-id retry".into()))?;
    drop(store);
    stop(&mut sessions[0])?;
    stop(&mut sessions[1])?;
    let [left, _right] = nodes;
    let mut work = Work::start(left, cli(), false)?;
    let wait_notice = |work: &mut Work, text: &str| -> Result<()> {
        let until = Instant::now() + Duration::from_secs(40);
        loop {
            ensure!(work.poll().is_none(), "Owned worker ended during messaging");
            if work.view.text.contains(text) {
                return Ok(());
            }
            ensure!(
                Instant::now() < until,
                "Missing worker messaging observation"
            );
            thread::sleep(Duration::from_millis(50));
        }
    };
    wait_notice(
        &mut work,
        Progress::RetryAvailable.notice().context("Retry notice")?,
    )?;
    ensure!(
        !work.view.current(Instant::now(), consent::now()?),
        "Offline worker displayed current admission"
    );
    work.action(Action::RetrySend)?;
    let materialized = Progress::Materialized
        .notice()
        .context("Materialized notice")?;
    wait_notice(&mut work, materialized)?;
    work.action(Action::Send {
        slot: 1,
        text: Zeroizing::new("offline new send".into()),
    })?;
    wait_notice(&mut work, materialized)?;
    work.cancel();
    let until = Instant::now() + Duration::from_secs(25);
    let returned = loop {
        if let Some((returned, result)) = work.poll() {
            ensure!(result?.graceful, "Worker did not shut down cleanly");
            break returned;
        }
        ensure!(Instant::now() < until, "Worker exit deadline");
        thread::sleep(Duration::from_millis(50));
    };
    let mut repo = repository(&returned)?;
    let (_, text) = repo
        .request(1, offline)?
        .context("Worker lost explicit same-ID retry")?;
    ensure!(text == "offline same-id retry");
    let history = repo.history(1, None, 32)?;
    ensure!(
        history.items.len() == 2
            && history.items[0].request == offline
            && history.items[1].text == "offline new send"
            && repo.request_delivery(1, offline)? == Some(1),
        "Offline worker changed, duplicated or falsely delivered a message"
    );
    println!(
        "CS33A_ACTUAL_PASS commit_before_ignored_reply=true reopened_same_intent_no_op2=true exact_id_conflict=true mixed_framed_pages=true max_escaped_split=true snapshot_append=true offline_worker_actions=true"
    );
    Ok(())
}
