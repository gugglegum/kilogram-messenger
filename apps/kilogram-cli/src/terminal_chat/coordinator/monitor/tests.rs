use super::*;

fn binding() -> Binding {
    Binding {
        contexts: ["a".repeat(64), "b".repeat(64)],
        roles: [1, 0],
        instance: "c".repeat(64),
    }
}

#[test]
fn t11e2b_monitor_deadlines_fairness_and_errors() {
    let start = Instant::now();
    let mut watcher = Monitor::default();
    watcher.connect(1, binding(), start);
    assert_eq!(watcher.request(start, (0, 0)).unwrap().slot, 0);
    assert!(
        watcher
            .request(start + STEP - Duration::from_millis(1), (0, 0))
            .is_none()
    );
    assert_eq!(watcher.request(start + STEP, (0, 0)).unwrap().slot, 1);
    watcher.complete(1, 2, 0, 0, Err(Fault::Unavailable), start);
    watcher.complete(1, 3, 1, 0, Err(Fault::NoHistory), start);
    assert_eq!(watcher.status()["contexts"][0]["retry"], 1);
    assert_eq!(watcher.status()["contexts"][1]["state"], "waiting_history");
    assert!(
        watcher
            .request(start + Duration::from_millis(999), (0, 0))
            .is_none()
    );
    let mut time = start + IDLE;
    for (i, delay) in [2, 4, 0].into_iter().enumerate() {
        // Let the second context run when its deadline permits, without losing the first one.
        let chosen = watcher.request(time, (0, 0)).unwrap();
        if chosen.slot == 1 {
            watcher.complete(1, 10, 1, 0, Err(Fault::NoHistory), time);
            time += STEP;
            assert_eq!(watcher.request(time, (0, 0)).unwrap().slot, 0);
        }
        watcher.complete(1, 20 + i as u64, 0, 0, Err(Fault::Unavailable), time);
        if delay > 0 {
            assert_eq!(
                watcher.contexts.as_ref().unwrap()[0].due,
                time + Duration::from_secs(delay)
            );
            time += Duration::from_secs(delay);
        }
    }
    assert_eq!(
        watcher.status()["contexts"][0]["code"],
        "runtime_unavailable"
    );
    assert_eq!(watcher.status()["contexts"][1]["state"], "waiting_history");
    watcher.disconnect("disconnected");
    watcher.connect(31, binding(), time);
    assert_eq!(watcher.status()["contexts"][0]["state"], "resync_required");
    time += STEP;
    assert_eq!(watcher.request(time, (0, 0)).unwrap().slot, 1);
    watcher.complete(31, 32, 1, 0, Err(Fault::Unsupported), time);
    assert_eq!(watcher.status()["contexts"][1]["state"], "unsupported");
    assert!(
        watcher
            .request(time + Duration::from_secs(10), (0, 0))
            .is_none()
    );
}

#[test]
fn t11e2b_frame_caps_and_safe_text_roundtrip() -> anyhow::Result<()> {
    assert_eq!(page_limit(0, 0), 8);
    assert_eq!(page_limit(62, 0), 0);
    assert_eq!(page_limit(0, DATA_BYTES - ROW_CAP + 1), 0);
    assert_eq!(page_limit(61, DATA_BYTES - ROW_CAP), 1);
    let binding = binding();
    let request = Request {
        slot: 1,
        read: Read::Anchor,
        epoch: u64::MAX,
        context: binding.contexts[1].clone(),
        instance: binding.instance.clone(),
        role: 0,
    };
    let heaviest = "\0".repeat(4096);
    assert_eq!(
        incoming(true, &request, &[255; 32], &heaviest)?.len(),
        24898
    );
    assert_eq!(
        incoming(false, &request, &[255; 32], &"\x1b".repeat(4096))?.len(),
        24596
    );
    let text = "Б \" \\ 🌓 \u{80}\u{061c}\u{200e}\u{200f}\u{2028}\u{202e}\u{2066}\u{2069}\r\n\t";
    for jsonl in [false, true] {
        let frame = incoming(jsonl, &request, &[1; 32], text)?;
        assert_eq!(frame.iter().filter(|&&b| b == b'\n').count(), 1);
        let visible = std::str::from_utf8(&frame[..frame.len() - 1])?;
        assert!(!visible.chars().any(escaped));
        if jsonl {
            let data: Value = serde_json::from_slice(&frame)?;
            assert_eq!(data["text"], text);
            assert_eq!(data["event"], "incoming");
            assert_eq!(data["event_id"], "01".repeat(32));
        }
    }
    assert!(gap_notice(true, u64::MAX, &binding, 1, "runtime_unavailable").len() <= GAP_CAP);
    Ok(())
}

#[cfg(windows)]
#[test]
fn t11e2b_seed_live_tail() -> anyhow::Result<()> {
    use kilogram_runtime_ipc::{
        RuntimeIpcDescriptor,
        ordered2::{Client2, Delivery2, id},
    };
    use std::{env, path::PathBuf};

    let Some(path) = env::var_os("KILOGRAM_T11E2B_SEED_DESCRIPTOR") else {
        return Ok(());
    };
    let descriptor = RuntimeIpcDescriptor::load(&PathBuf::from(path))?;
    let identity = (descriptor.account_id(), descriptor.device_id());
    let contexts = [
        id(&env::var("KILOGRAM_T11E2B_SEED_CONTEXT0")?)?,
        id(&env::var("KILOGRAM_T11E2B_SEED_CONTEXT1")?)?,
    ];
    let client = Client2::new(descriptor, identity.0, identity.1, contexts)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let count: u8 = env::var("KILOGRAM_T11E2B_SEED_COUNT")?.parse()?;
    anyhow::ensure!((1..=160).contains(&count));
    let events = runtime.block_on(async {
        let base = client.history_anchor(0, contexts[0]).await?;
        let mut events = Vec::new();
        for first in (0..count).step_by(4) {
            let mut batch = Vec::with_capacity(4);
            for number in first..(first + 4).min(count) {
                let mut request = [0xe2; 16];
                request[0] = number;
                let text = format!("tail-{number:03}");
                client.send_text(0, contexts[0], request, &text).await?;
                batch.push((number, request));
            }
            let deadline = Instant::now() + Duration::from_secs(20);
            for (number, request) in batch {
                loop {
                    if let Some(status) = client.request_status(0, contexts[0], request).await?
                        && status.delivery == Delivery2::Delivered
                    {
                        events.push(hex(&status
                            .receipt
                            .event
                            .ok_or_else(|| anyhow::anyhow!("missing delivered event"))?));
                        break;
                    }
                    anyhow::ensure!(
                        Instant::now() < deadline,
                        "missing durable receipt {number}"
                    );
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            }
        }
        let tail = client.history_follow(0, contexts[0], &base.cursor).await?;
        anyhow::ensure!(
            tail.frontiers
                .into_iter()
                .zip(tail.next)
                .map(|(end, next)| end - next)
                .sum::<u64>()
                >= u64::from(count)
        );
        let now = Instant::now();
        let mut monitor = Monitor::default();
        monitor.connect(1, binding(), now);
        let [first, second] = monitor.contexts.as_mut().unwrap();
        first.completed = Some(Position::from(base));
        first.range = Some(Position::from(tail));
        second.due = now + IDLE;
        assert!(monitor.request(now, (62, DATA_BYTES)).is_none());
        assert_eq!(monitor.status()["contexts"][0]["state"], "watching");
        let request = monitor.request(now, (61, DATA_BYTES - ROW_CAP)).unwrap();
        assert_eq!((request.slot, request.reserved()), (0, 1));
        println!("T11E2B_FULL_CREDIT_DEFER_PASS");
        let first = &monitor.contexts.as_ref().unwrap()[0];
        let completed = first.completed.as_ref().unwrap().clone();
        let range = first.range.as_ref().unwrap().clone();
        monitor.disconnect("disconnected");
        monitor.connect(2, binding(), now);
        let gap = monitor.gaps()[0];
        assert!(gap.is_some());
        let mut proof = HistoryAdvance {
            scope: super::super::CommandScope {
                generation: 3,
                connection: Some(2),
            },
            gap: None,
            slot: 0,
            context: binding().contexts[0].clone(),
            from: None,
            view: completed.view,
            frontiers: completed.frontiers,
            cursor: completed.cursor.clone(),
            next_token: None,
        };
        // Neither a pre-gap completion nor a tail cursor without an ACKed new prefix clears the gap.
        monitor.recover(&proof, now);
        assert_eq!(monitor.gaps()[0], gap);
        proof.gap = gap;
        proof.from = Some(range.cursor.clone());
        monitor.recover(&proof, now);
        assert_eq!(monitor.gaps()[0], gap);
        let first = &monitor.contexts.as_ref().unwrap()[0];
        assert!(first.completed.as_ref().unwrap().cursor == completed.cursor);
        assert!(first.range.as_ref().unwrap().cursor == range.cursor);
        println!("T11E2B_RESYNC_REJECT_STALE_PROOF_PASS");
        Ok::<_, anyhow::Error>(events)
    })?;
    println!("T11E2B_SEEDED {}", serde_json::to_string(&events)?);
    Ok(())
}
