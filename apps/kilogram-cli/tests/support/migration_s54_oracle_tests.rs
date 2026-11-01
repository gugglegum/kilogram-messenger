use super::*;
fn key(owner: u8, pid: u32, nonce: u8) -> Incarnation {
    Incarnation {
        owner: [owner; 32],
        pid,
        nonce: [nonce; 32],
    }
}
fn count(e: u64, d: u64) -> Counts {
    Counts {
        encrypt: e,
        decrypt: d,
        attempts: e + d,
        ..Counts::default()
    }
}

#[test]
fn s54_p5_requests_retry_and_cardinality_are_one_continuous_domain() -> Result<()> {
    let mut a = BTreeSet::new();
    let mut b = BTreeSet::new();
    a.extend(1..=24);
    b.extend(1..=25);
    let early = Request::new(0, 1, 25)?;
    for round in 0..16 {
        let (aa, bb) = expected_turn(round)?;
        a.extend(aa);
        for n in bb {
            let request = Request::new(0, 1, n)?;
            if n == 25 {
                ensure!(
                    request.numeric() == early.numeric() && request.body() == early.body(),
                    "B25 exact retry"
                );
            }
            b.insert(n);
        }
    }
    ensure!(
        a.len() == 56 && b.len() == 536,
        "turn accounting includes the retry once"
    );
    a.extend(57..=640);
    b.extend(537..=640);
    let mut ids = BTreeSet::new();
    for context in 0..2 {
        for role in 0..2 {
            for n in 1..=if context == 0 { 640 } else { 32 } {
                ensure!(
                    ids.insert(Request::new(context, role, n)?.numeric()),
                    "request alias across C/role"
                );
            }
        }
    }
    ensure!(
        a.len() + b.len() + 64 == ids.len() && (ids.len() + 4) * 2 == 2696,
        "actual expected request set"
    );
    ensure!(
        Request::new(0, 0, 641).is_err()
            && Request::new(1, 0, 33).is_err()
            && expected_turn(16).is_err(),
        "invalid request/range"
    );
    Ok(())
}
#[test]
fn s54_p5_phase_order_rejects_stitching_and_replayed_prefix() -> Result<()> {
    let mut p = Phases { next: 0 };
    ensure!(p.accept(Phase::Handoff).is_err(), "cannot skip origin");
    for phase in ORDER {
        p.accept(phase)?;
        ensure!(p.accept(phase).is_err(), "cannot replay phase");
    }
    ensure!(p.complete(), "all phases");
    Ok(())
}
#[test]
fn s54_p5_original_ready_and_slot_mapping_reject_extra_setup_text() -> Result<()> {
    let mut row = json!({"phase":3,"session_id":"actual-public-SID","send_next":1,"receive_next":1,"unacked":0,"pending":0});
    ensure!(old_zero_text_ready(&row)?, "genuine zero-Text readiness");
    row["send_next"] = 2.into();
    ensure!(!old_zero_text_ready(&row)?, "warm Text forbidden");
    row["session_id"] = Value::Null;
    ensure!(
        old_zero_text_ready(&row).is_err(),
        "incomplete session cannot be fabricated"
    );
    let ready = json!({"contexts":[{"context":"C2"},{"context":"C1"}]});
    ensure!(
        ready_slot(&ready, "C1")? == 1,
        "lookup actual position not assumed slot0"
    );
    ensure!(
        ready_slot(
            &json!({"contexts":[{"context":"C1"},{"context":"C1"}]}),
            "C1"
        )
        .is_err(),
        "duplicate mapping"
    );
    Ok(())
}
#[test]
fn s54_p5_history_freezes_old_suffix_across_append_and_reorder() -> Result<()> {
    let old = (0..39).map(|n| format!("event{n}")).collect::<Vec<_>>();
    let mut grown = old.clone();
    grown.reverse();
    grown.extend((39..1280).map(|n| format!("event{n}")));
    let suffix = frozen_suffix(&old, 16, &grown)?;
    ensure!(
        suffix == old[16..] && suffix.len() == 23,
        "frozen causal snapshot order"
    );
    grown.pop();
    grown.retain(|id| id != "event20");
    ensure!(frozen_suffix(&old, 16, &grown).is_err(), "old row lost");
    ensure!(
        frozen_suffix(&old, 40, &old).is_err(),
        "cursor outside snapshot"
    );
    Ok(())
}
#[test]
fn s54_p5_counter_samples_reject_overlap_regression_and_pid_confusion() -> Result<()> {
    let mut ledger = Operations::default();
    let a = key(1, 11, 2);
    ledger.start(a)?;
    ledger.commit(a, 1, 20, [10; 32], count(2, 0))?;
    ensure!(
        ledger.commit(a, 1, 21, [11; 32], count(3, 0)).is_err(),
        "overlapping cumulative sample"
    );
    ensure!(
        ledger.commit(a, 2, 21, [11; 32], count(1, 0)).is_err(),
        "counter regression"
    );
    ensure!(
        ledger
            .commit(key(1, 12, 2), 2, 21, [11; 32], count(3, 0))
            .is_err(),
        "PID/incarnation mismatch"
    );
    ensure!(
        ledger.start(key(1, 12, 3)).is_err(),
        "one owner incarnation alive"
    );
    ensure!(
        ledger.complete().is_err(),
        "missing final outcome stays unresolved"
    );
    ledger.finish(a, count(2, 0))?;
    let b = key(1, 11, 3);
    ledger.start(b)?;
    ensure!(
        ledger.commit(b, 1, 21, [10; 32], count(1, 0)).is_err(),
        "replayed operation after PID reuse"
    );
    ledger.commit(b, 1, 21, [11; 32], count(1, 0))?;
    ledger.finish(b, count(1, 0))?;
    ensure!(
        ledger.complete()? == (3, 0),
        "incarnation reset does not lose or double-count work"
    );
    Ok(())
}
#[test]
fn s54_p5_stopped_source_reconciliation_requires_actual_resolved_effects() -> Result<()> {
    let mut ledger = Operations::default();
    let a = key(1, 1, 9);
    ledger.start(a)?;
    ledger.commit(a, 1, 5, [5; 32], count(1, 0))?;
    ensure!(
        ledger
            .reconcile_final(a, 6, [6; 32], (100, 50), (100, 51), count(2, 0))
            .is_err(),
        "wrong direction/source effect"
    );
    let ambiguous = Counts {
        ambiguous: 1,
        ..count(1, 1)
    };
    ensure!(
        ledger
            .reconcile_final(a, 6, [6; 32], (100, 50), (100, 51), ambiguous)
            .is_err(),
        "unresolved ambiguity"
    );
    ledger.reconcile_final(a, 6, [6; 32], (100, 50), (100, 51), count(1, 1))?;
    ensure!(
        ledger.complete()? == (1, 1),
        "actual orphan durable commit reconciled once"
    );
    Ok(())
}
#[test]
fn s54_p5_page2_then_reopened_page3_is_global_page5() -> Result<()> {
    let a = key(1, 10, 1);
    let b = key(1, 11, 2);
    let mut p = Pages::default();
    p.accepted(a, 0, [1; 32], true, 1)?;
    p.accepted(a, 0, [2; 32], false, 0)?;
    ensure!(p.total == 0, "head and replay/empty aren't normal progress");
    ensure!(p.local_target(a, 2)? == 2, "first stop");
    p.accepted(a, 1, [3; 32], false, 16)?;
    p.accepted(a, 2, [4; 32], false, 16)?;
    ensure!(p.local_target(b, 5)? == 3, "new PID resets local only");
    ensure!(
        p.accepted(b, 1, [4; 32], false, 16).is_err(),
        "replay cannot count across restart"
    );
    for n in 1..=3 {
        p.accepted(b, n, [10 + n as u8; 32], false, 16)?;
    }
    ensure!(
        p.total == 5 && p.local_target(b, 5).is_err(),
        "exact cumulative fifth, not fresh five"
    );
    Ok(())
}
#[test]
fn s54_p5_old104_subtotal_is_not_chronological106() -> Result<()> {
    let old_setup = 8 + 32 + 31;
    let old_interop = old_setup + 16 + 8;
    let early = old_interop + 1 + 9 + 1;
    ensure!(
        old_setup == 71 && old_interop == 95 && old_interop + 9 == 104 && early == 106,
        "B25 effects separate"
    );
    let final_total = (old_interop + 9) + 2 * (2 * (640 - 24)) + 2 * (32 + 32);
    ensure!(
        final_total == 2696 && final_total != early + 2592,
        "no B25 double-count"
    );
    let mut ledger = Operations::default();
    let mixed = (25..=640).count() as u64;
    let c2 = (1..=32).count() as u64;
    let compat_encrypt = (17..=24).count() as u64;
    let late_decrypt = (16..=24).count() as u64;
    for (owner, e, d) in [
        (1, compat_encrypt + mixed + c2, late_decrypt + mixed + c2),
        (2, mixed + c2, mixed + c2),
    ] {
        let process = key(owner, owner as u32, owner);
        ledger.start(process)?;
        let mut seen = count(0, 0);
        let mut sequence = 0u64;
        for (encrypt, amount) in [(true, e), (false, d)] {
            let mut left = amount;
            while left > 0 {
                let batch = left.min(4);
                if encrypt {
                    seen.encrypt += batch;
                } else {
                    seen.decrypt += batch;
                }
                seen.attempts += batch;
                sequence += 1;
                let mut operation = [0; 32];
                operation[..8].copy_from_slice(&sequence.to_be_bytes());
                ledger.commit(process, sequence, sequence, operation, seen)?;
                left -= batch;
            }
        }
        ledger.finish(process, seen)?;
    }
    let actual = ledger.complete()?;
    ensure!(
        actual.0 + actual.1 == 2609,
        "all current process operations include old interop"
    );
    ensure!(
        whole_packet_total(104, compat_encrypt + late_decrypt, &ledger)? == 2696,
        "subtract source-proved overlap17"
    );
    ensure!(
        whole_packet_total(104, 105, &ledger).is_err(),
        "cannot hide operations by an invented overlap"
    );
    Ok(())
}
#[test]
fn s54_p5_history_rejects_changed_body_format_parent_and_cross_context() -> Result<()> {
    let request = Request::new(0, 0, 24)?;
    let row = TextRow {
        id: [1; 32],
        request,
        format: 1,
        text: request.body(),
        parents: vec![[2; 32]],
    };
    let old = vec![row.clone()];
    retained_rows(&old, &old, 0)?;
    let mut changed = row.clone();
    changed.text.push('!');
    ensure!(text_history(0, &[changed]).is_err(), "changed text");
    let mut changed = row.clone();
    changed.format = 2;
    ensure!(text_history(0, &[changed]).is_err(), "old ordinal relabel");
    let mut changed = row.clone();
    changed.parents = vec![[3; 32]];
    ensure!(
        retained_rows(&old, &[changed], 0).is_err(),
        "changed causal parents"
    );
    let mut changed = row.clone();
    changed.request.context = 1;
    ensure!(text_history(0, &[changed]).is_err(), "cross-C row");
    ensure!(
        text_history(0, &[row.clone(), row]).is_err(),
        "duplicate row/request"
    );
    Ok(())
}
#[test]
fn s54_p5_counters_decode_actual_fields_and_reject_constructor_or_missing_outcome() -> Result<()> {
    let mut row = json!({"encrypt":1,"decrypt":0,"attempts":1,"aborted":0,"ambiguous":0,"accounts":0,"outbound":0,"inbound":0});
    ensure!(
        Counts::read(&row)? == count(1, 0),
        "actual public counter schema"
    );
    row["attempts"] = json!(-1);
    ensure!(Counts::read(&row).is_err(), "not an observed u64");
    row["attempts"] = 1.into();
    row["unknown"] = 0.into();
    ensure!(Counts::read(&row).is_err(), "schema drift");
    let mut ledger = Operations::default();
    let a = key(1, 1, 8);
    ledger.start(a)?;
    let recreated = Counts {
        accounts: 1,
        ..count(1, 0)
    };
    ensure!(
        ledger.commit(a, 1, 1, [1; 32], recreated).is_err(),
        "new current constructor"
    );
    ledger.commit(a, 1, 1, [1; 32], count(1, 0))?;
    let unresolved = Counts {
        attempts: 2,
        ..count(1, 0)
    };
    ensure!(
        ledger.finish(a, unresolved).is_err(),
        "attempt with no actual outcome"
    );
    ledger.finish(
        a,
        Counts {
            attempts: 2,
            aborted: 1,
            ..count(1, 0)
        },
    )?;
    Ok(())
}
