use super::*;
fn schedule() -> Json {
    json!({"context":state::hex(&[1;32]),"lane":0,"phase":"pressure","hold_source64":true,"max_pause_ms":1000})
}
fn rows(after: Option<u64>) -> Vec<(u64, Id)> {
    let start = after.unwrap_or(0) + 1;
    (start..start + 32).map(|n| (n, [n as u8; 32])).collect()
}
fn window() -> Arc<window::RefWindow2> {
    window::RefWindow2::new(&[0, 1])
}

#[test]
fn s54_controls_are_closed_bounded_and_debug_only() -> Result<()> {
    let raw = serde_json::to_vec(&schedule())?;
    ensure!(
        Schedule::parse(&raw, true)?.applies([1; 32], 0),
        "exact applicability"
    );
    ensure!(
        !Schedule::parse(&raw, true)?.applies([2; 32], 0)
            && !Schedule::parse(&raw, true)?.applies([1; 32], 1),
        "wrong C/lane"
    );
    ensure!(
        Schedule::parse(&raw, false).is_err() && Schedule::parse(&vec![b' '; 16385], true).is_err(),
        "release/cap"
    );
    for (key, value) in [
        ("unexpected", json!(1)),
        ("lane", json!(2)),
        ("phase", json!("unknown")),
        ("max_pause_ms", json!(120001)),
        ("hold_ordinals", json!([640, 640])),
    ] {
        let mut s = schedule();
        s[key] = value;
        ensure!(
            Schedule::parse(&serde_json::to_vec(&s)?, true).is_err(),
            "malformed field {key}"
        );
    }
    let mut s = schedule();
    s["hold_ordinals"] = json!([640]);
    ensure!(
        Schedule::parse(&serde_json::to_vec(&s)?, true)?.hold_ordinals == [640],
        "mixed ordinal beyond unchanged OD1 limit"
    );
    s["hold_ack"] = true.into();
    ensure!(
        Schedule::parse(&serde_json::to_vec(&s)?, true).is_err(),
        "wrong ACK phase"
    );
    Ok(())
}
#[test]
fn s54_control_location_revalidates_updates_and_rejects_inside_state() -> Result<()> {
    let base = tempfile::tempdir()?;
    let root = base.path().join("state");
    std::fs::create_dir(&root)?;
    let inside = root.join("schedule.json");
    std::fs::write(&inside, serde_json::to_vec(&schedule())?)?;
    ensure!(
        Location::new(&inside, &root, true).is_err(),
        "inside-state schedule"
    );
    let file = base.path().join("schedule.json");
    std::fs::write(&file, serde_json::to_vec(&schedule())?)?;
    let pinned = Location::new(&file, &root, true)?;
    let mut invalid = schedule();
    invalid["new_field"] = 1.into();
    std::fs::write(&file, serde_json::to_vec(&invalid)?)?;
    ensure!(
        pinned.load().is_err(),
        "dynamic update still parses strictly"
    );
    ensure!(
        Location::new(&file, &root, false).is_err(),
        "release opener refuses before reading"
    );
    Ok(())
}
#[cfg(windows)]
#[test]
fn s54_control_parent_cannot_be_retargeted_while_pinned() -> Result<()> {
    let base = tempfile::tempdir()?;
    let root = base.path().join("state");
    std::fs::create_dir(&root)?;
    let dir = base.path().join("controls");
    std::fs::create_dir(&dir)?;
    let path = dir.join("schedule.json");
    std::fs::write(&path, serde_json::to_vec(&schedule())?)?;
    let pin = Location::new(&path, &root, true)?;
    ensure!(
        std::fs::rename(&dir, root.join("escaped")).is_err(),
        "held parent refuses DELETE/retarget"
    );
    drop(pin);
    std::fs::rename(&dir, root.join("escaped"))?;
    Ok(())
}
#[test]
fn s54_real64_share_is_owned_before_either_read_and_other_share_is_independent() -> Result<()> {
    let w = window();
    let mut reads = Vec::new();
    let held = select64(
        &w,
        0,
        |after| {
            ensure!(w.snapshot().1[0] == 64, "reserve before first metadata");
            reads.push(after);
            Ok(rows(after))
        },
        |_, _| Ok(()),
    )?;
    ensure!(
        held.ids.len() == 64 && reads == [None, Some(32)],
        "two bounded monotonic pages"
    );
    let other = w.reserve(1, 64)?;
    ensure!(
        w.reserve(0, 1).is_err() && w.snapshot().1 == [64, 64],
        "no borrowing"
    );
    drop(held);
    ensure!(w.snapshot().1 == [0, 64], "C1 RAII release");
    drop(other);
    ensure!(w.snapshot().1 == [0, 0], "all released");
    Ok(())
}
#[test]
fn s54_busy64_never_calls_reader_or_spins_in_owner() -> Result<()> {
    let w = window();
    let occupied = w.reserve(0, 1)?;
    let mut reads = 0;
    ensure!(
        select64(
            &w,
            0,
            |after| {
                reads += 1;
                Ok(rows(after))
            },
            |_, _| Ok(())
        )
        .is_err()
            && reads == 0,
        "fail before reads"
    );
    drop(occupied);
    ensure!(w.snapshot().1 == [0, 0], "no retained partial selection");
    Ok(())
}
#[test]
fn s54_partial_duplicate_wrong_context_and_read_errors_release_same_permit() -> Result<()> {
    let w = window();
    ensure!(
        select64(
            &w,
            0,
            |after| {
                let mut p = rows(after);
                p.pop();
                Ok(p)
            },
            |_, _| Ok(())
        )
        .is_err(),
        "fewer actual refs"
    );
    ensure!(
        select64(
            &w,
            0,
            |after| {
                let mut p = rows(after);
                p[1].1 = p[0].1;
                Ok(p)
            },
            |_, _| Ok(())
        )
        .is_err(),
        "duplicate actual IDs"
    );
    ensure!(
        select64(
            &w,
            0,
            |after| Ok(rows(after)),
            |_, _| anyhow::bail!("immutable source belongs to another C")
        )
        .is_err(),
        "source/context verifier refusal"
    );
    ensure!(
        select64(&w, 0, |_| anyhow::bail!("read failure"), |_, _| Ok(())).is_err(),
        "reader failure"
    );
    ensure!(w.snapshot().1 == [0, 0], "all paths drop permit");
    Ok(())
}
#[test]
fn s54_cancelled_owner_reply_drops_selection_without_leaking_quota() -> Result<()> {
    let w = window();
    let held = select64(&w, 0, |after| Ok(rows(after)), |_, _| Ok(()))?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    drop(rx);
    drop(tx.send(held));
    ensure!(
        w.snapshot().1 == [0, 0],
        "cancelled Handle7 reply owns and drops result"
    );
    Ok(())
}
#[tokio::test]
async fn s54_async_retention_cancellation_drops_real_references() -> Result<()> {
    let w = window();
    let held = select64(&w, 0, |after| Ok(rows(after)), |_, _| Ok(()))?;
    let task = tokio::spawn(async move {
        let _held = held;
        std::future::pending::<()>().await;
    });
    tokio::task::yield_now().await;
    ensure!(w.snapshot().1[0] == 64, "actual retained selection");
    task.abort();
    ensure!(
        task.await.is_err() && w.snapshot().1[0] == 0,
        "worker cancellation releases selection"
    );
    Ok(())
}
#[test]
fn s54_page_observations_classify_head_replay_and_pause_only_after_progress() -> Result<()> {
    let base = tempfile::tempdir()?;
    let root = base.path().join("state");
    std::fs::create_dir(&root)?;
    let file = base.path().join("control.json");
    let s = json!({"context":state::hex(&[1;32]),"lane":1,"phase":"catch_up","pause_after_normal_page":2,"max_pause_ms":1000});
    std::fs::write(&file, serde_json::to_vec(&s)?)?;
    let mut o = Observer {
        location: Some(Location::new(&file, &root, true)?),
        incarnation: [2; 32],
        owner: [3; 32],
        sequence: 0,
        pages: [0; 2],
        last_page: [None; 2],
    };
    let page = |new_sources, head| Page {
        start: if head { 385 } else { 386 },
        end: if head { 386 } else { 402 },
        new_sources,
        head,
        grant: head.then_some([8; 32]),
        generation: 10,
        operation: [4; 32],
    };
    let head = o
        .page(0, [1; 32], 1, page(1, true))?
        .context("actual head notice")?;
    let replay = o
        .page(0, [1; 32], 1, page(0, false))?
        .context("actual replay notice")?;
    ensure!(
        head.event["kind"] == "head_page"
            && replay.event["kind"] == "replayed_page"
            && head.pause.is_none()
            && replay.pause.is_none(),
        "head/replay classified without normal progress"
    );
    let mut empty = page(0, false);
    empty.end = empty.start;
    ensure!(
        o.page(0, [1; 32], 1, empty).is_err(),
        "empty/rejected page cannot emit progress"
    );
    ensure!(o.pages == [0, 0], "head/replay excluded");
    let first = o
        .page(0, [1; 32], 1, page(16, false))?
        .context("actual first notice")?;
    ensure!(first.pause.is_none(), "first page not second");
    ensure!(
        o.page(0, [1; 32], 1, page(16, false)).is_err(),
        "duplicate operation cannot count twice"
    );
    let mut next = page(16, false);
    next.generation = 11;
    next.operation = [5; 32];
    let second = o.page(0, [1; 32], 1, next)?.context("second notice")?;
    let pause = second.pause.context("second page pause")?;
    ensure!(
        !file.with_extension("s54-page.json").exists(),
        "owner reply precedes published observation"
    );
    pause.publish()?;
    let marker: Json =
        serde_json::from_slice(&std::fs::read(file.with_extension("s54-page.json"))?)?;
    ensure!(
        marker["facts"]["local_page"] == 2 && marker["facts"]["new_sources"] == 16,
        "marker uses observed page"
    );
    Ok(())
}
#[test]
fn s54_ack_gate_covers_push_and_query_reply_but_never_empty_or_malformed_ack() -> Result<()> {
    use kilogram_protocol::ordered_dialog_v2::FrameKind as K;
    let body = |items| {
        wire::a([
            wire::b([1; 32]),
            wire::b([2; 32]),
            wire::b([3; 32]),
            wire::b(b"SID"),
            wire::u(0),
            wire::a([]),
            items,
            wire::a([]),
        ])
    };
    let one = wire::a([wire::a([wire::u(1), wire::b([9; 16])])]);
    ensure!(
        ack_count(K::AckBatch, &body(one))? == Some(1),
        "actual nonempty tagged ACK"
    );
    ensure!(
        ack_count(K::AckBatch, &body(wire::a([])))?.is_none(),
        "no fabricated ACK marker"
    );
    ensure!(
        ack_count(K::AckQuery, &wire::a([]))?.is_none(),
        "query is not transmission of saved receipts"
    );
    ensure!(
        ack_count(K::AckBatch, &wire::a([])).is_err(),
        "malformed ACK"
    );
    let too_many = wire::a((0..17).map(|_| wire::a([wire::u(1), wire::b([9; 16])])));
    ensure!(
        ack_count(K::AckBatch, &body(too_many)).is_err(),
        "wire16 stays unchanged"
    );
    let base = tempfile::tempdir()?;
    let root = base.path().join("state");
    std::fs::create_dir(&root)?;
    let file = base.path().join("control.json");
    let s = json!({"context":state::hex(&[1;32]),"lane":1,"phase":"catch_up","hold_ack":true,"max_pause_ms":1000});
    std::fs::write(&file, serde_json::to_vec(&s)?)?;
    let o = Observer {
        location: Some(Location::new(&file, &root, true)?),
        incarnation: [2; 32],
        owner: [3; 32],
        sequence: 0,
        pages: [0; 2],
        last_page: [None; 2],
    };
    for query_reply in [false, true] {
        let pause = o
            .pause(
                [1; 32],
                1,
                PauseKind::Ack,
                json!({"receipt_count":1,"query_reply":query_reply}),
            )?
            .context("push and reply share actual gate")?;
        pause.publish()?;
        let marker: Json =
            serde_json::from_slice(&std::fs::read(file.with_extension("s54-ack.json"))?)?;
        ensure!(
            marker["facts"]["query_reply"] == query_reply,
            "cannot hide ACK-query reply behind push hold"
        );
    }
    ensure!(
        o.pause([2; 32], 1, PauseKind::Ack, json!({}))?.is_none(),
        "unrelated C independent"
    );
    Ok(())
}
#[test]
fn s54_absent_control_has_no_file_wait_marker_or_page_side_effect() -> Result<()> {
    let mut o = Observer {
        location: None,
        incarnation: [0; 32],
        owner: [0; 32],
        sequence: 0,
        pages: [0; 2],
        last_page: [None; 2],
    };
    ensure!(
        !o.enabled() && o.schedule([1; 32], 0)?.is_none(),
        "default is disabled"
    );
    ensure!(
        o.page(
            0,
            [1; 32],
            0,
            Page {
                start: 1,
                end: 17,
                new_sources: 16,
                head: false,
                grant: None,
                generation: 2,
                operation: [1; 32]
            }
        )?
        .is_none(),
        "no default observer work"
    );
    ensure!(
        o.pages == [0, 0]
            && o.pause([1; 32], 0, PauseKind::Source64, json!({}))?
                .is_none(),
        "no marker or delay"
    );
    Ok(())
}
#[tokio::test]
async fn s54_pressure_wait_releases_actual_selection_on_control_release_and_connection_loss()
-> Result<()> {
    for connection_loss in [false, true] {
        let base = tempfile::tempdir()?;
        let root = base.path().join("state");
        std::fs::create_dir(&root)?;
        let file = base.path().join("control.json");
        let plan = schedule();
        std::fs::write(&file, serde_json::to_vec(&plan)?)?;
        let o = Observer {
            location: Some(Location::new(&file, &root, true)?),
            incarnation: [2; 32],
            owner: [3; 32],
            sequence: 0,
            pages: [0; 2],
            last_page: [None; 2],
        };
        let pause = o
            .pause([1; 32], 0, PauseKind::Source64, json!({"count":64}))?
            .context("actual pause")?;
        let w = window();
        let held = select64(&w, 0, |after| Ok(rows(after)), |_, _| Ok(()))?;
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let _held = held;
            pause
                .wait_until(async {
                    let _ = rx.await;
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !file.with_extension("s54-source64.json").exists() {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        ensure!(
            w.snapshot().1[0] == 64,
            "pause retains real quota outside callback"
        );
        if connection_loss {
            tx.send(())
                .map_err(|_| anyhow::anyhow!("lost close receiver"))?;
        } else {
            let mut released = plan;
            released["hold_source64"] = false.into();
            std::fs::write(&file, serde_json::to_vec(&released)?)?;
        }
        let result = task.await?;
        ensure!(
            result.is_err() == connection_loss && w.snapshot().1[0] == 0,
            "all wait outcomes release quota"
        );
    }
    Ok(())
}
