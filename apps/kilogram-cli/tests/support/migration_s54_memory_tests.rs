use super::*;
fn ledger() -> Result<Ledger> {
    Ledger::new([7; 32], SPAN)
}
fn profile(owner: u8, pid: u32, current: bool) -> Profile {
    Profile {
        owner,
        pid,
        current,
        profile: [owner + 1; 32],
        image: [3; 32],
    }
}
fn binding(p: Profile) -> Binding {
    let mut nonce = [0; 32];
    nonce[..4].copy_from_slice(&p.pid.to_le_bytes());
    Binding {
        owner: p.owner,
        profile: p.profile,
        source: [p.owner + 10; 32],
        nonce: p.current.then_some(nonce),
    }
}
fn reading(pid: u32) -> Reading {
    Reading {
        pid,
        ticks: 10000 + u64::from(pid),
        bytes: 1024,
        evidence: Evidence::Injected,
    }
}
fn measure(l: &mut Ledger, t: Token, r: Reason, start: u64, end: u64) -> Result<()> {
    let a = l.begin_at(t, r, Phase::Stage(0), start)?;
    l.complete_at(a, end, Ok(reading(t.pid)))
}
fn start(l: &mut Ledger, p: Profile, at: u64) -> Result<Token> {
    let t = l.register_at(at, p)?;
    l.bind(t, binding(p))?;
    measure(l, t, Reason::Start, at, at)?;
    Ok(t)
}
fn stop(l: &mut Ledger, t: Token, at: u64, kind: Stop) -> Result<()> {
    measure(l, t, Reason::BeforeStop, at, at)?;
    l.request_stop_at(t, kind, at)?;
    l.close_at(t, at, true, Some(if kind == Stop::Normal { 0 } else { 1 }))
}
fn status(p: Profile, n: u64, c: u64) -> Value {
    json!({"version":3,"profile_mode":"ordered-dialog-indexed","storage_stage":"Ready",
        "source_id":kilogram_cli::ordered_dialog::state::hex(&binding(p).source),"stats":{"max_native_bytes":n,"max_commit_app_bytes":c}})
}
#[test]
fn s54_memory_cadence_boundaries_gaps_and_no_queries_while_closing() -> Result<()> {
    let mut early = ledger()?;
    let t = start(&mut early, profile(0, 10, false), 0)?;
    assert!(
        early
            .begin_at(t, Reason::Progress, Phase::Setup, 4999)
            .is_err()
    );
    assert!(!early.diagnostic().coverage_valid);
    let mut l = ledger()?;
    let t = start(&mut l, profile(0, 10, false), 0)?;
    measure(&mut l, t, Reason::Progress, 5000, 12000)?;
    measure(&mut l, t, Reason::Progress, 12000, 12000)?;
    // The late attempt schedules the next one at17000, not a catch-up at15000.
    assert!(
        l.begin_at(t, Reason::Progress, Phase::Setup, 15000)
            .is_err()
    );
    let mut normal = ledger()?;
    let t = start(&mut normal, profile(0, 20, false), 0)?;
    measure(&mut normal, t, Reason::Progress, 20000, 21000)?;
    measure(&mut normal, t, Reason::BeforeStop, 21000, 21001)?;
    normal.request_stop_at(t, Stop::Normal, 21001)?;
    normal.close_at(t, 30000, true, Some(0))?;
    let summary = normal.finish()?;
    assert!(
        summary.coverage_valid
            && summary.profiles[0]["closing_tail_ms"] == 9000
            && summary.profiles[0]["missed_periodic_slots"] == 3
            && summary.profiles[0]["max_conservative_query_gap_ms"] == 21000
    );
    let before = normal.rows().len();
    assert!(
        normal
            .begin_at(t, Reason::Progress, Phase::Closing, 30000)
            .is_err()
    );
    assert!(normal.rows().len() == before);
    // Closing suppresses progress even when the process has not yet waited.
    let mut closing = ledger()?;
    let t = start(&mut closing, profile(0, 21, false), 0)?;
    measure(&mut closing, t, Reason::BeforeStop, 1, 1)?;
    closing.request_stop_at(t, Stop::Normal, 1)?;
    assert!(
        closing
            .begin_at(t, Reason::Progress, Phase::Closing, 5000)
            .is_err()
    );
    closing.close_at(t, 5001, true, Some(0))?;
    assert!(closing.finish().is_err());
    Ok(())
}
#[test]
fn s54_memory_identity_foreign_overlap_creation_ticks_and_late_setup_bind() -> Result<()> {
    let mut first = ledger()?;
    let t = start(&mut first, profile(0, 10, false), 0)?;
    let mut other = ledger()?;
    assert!(other.begin_at(t, Reason::Start, Phase::Setup, 0).is_err());
    assert!(first.register_at(0, profile(0, 11, false)).is_err());
    let mut l = ledger()?;
    let p = profile(0, 10, false);
    let t = l.register_at(0, p)?;
    measure(&mut l, t, Reason::Start, 0, 0)?;
    stop(&mut l, t, 1, Stop::Kill)?;
    assert!(!l.diagnostic().coverage_valid);
    l.bind(t, binding(p))?;
    assert!(l.finish()?.coverage_valid); // Original setup can bind AFTER wait.
    let mut l = ledger()?;
    let t = start(&mut l, p, 0)?;
    stop(&mut l, t, 1, Stop::Kill)?;
    let restarted = l.register_at(2, p)?;
    l.bind(restarted, binding(p))?;
    let a = l.begin_at(restarted, Reason::Start, Phase::Setup, 2)?;
    assert!(l.complete_at(a, 2, Ok(reading(p.pid))).is_err()); // Same PID+ticks is not a new process.
    let mut l = ledger()?;
    let t = start(&mut l, p, 0)?;
    let a = l.begin_at(t, Reason::Progress, Phase::Setup, 5000)?;
    let mut changed = reading(p.pid);
    changed.ticks += 1;
    assert!(l.complete_at(a, 5000, Ok(changed)).is_err());
    let mut l = ledger()?;
    let p = profile(0, 10, true);
    let t = l.register_at(0, p)?;
    let mut foreign = binding(p);
    foreign.owner = 1;
    assert!(l.bind(t, foreign).is_err());
    let mut duplicate_bind = ledger()?;
    let t = start(&mut duplicate_bind, p, 0)?;
    assert!(duplicate_bind.bind(t, binding(p)).is_err());
    let mut overlap = ledger()?;
    start(&mut overlap, profile(0, 10, false), 0)?;
    let mut q = profile(1, 11, false);
    q.profile = profile(0, 10, false).profile;
    assert!(overlap.register_at(0, q).is_err());
    let mut l = ledger()?;
    let a = start(&mut l, profile(0, 10, true), 0)?;
    // Reject the same current incarnation nonce even after the old process closes.
    l.candidate_at(a, 0, &status(profile(0, 10, true), 1, 1))?;
    l.injected_checkpoint(a)?;
    stop(&mut l, a, 0, Stop::Kill)?;
    let p2 = profile(0, 11, true);
    let b = l.register_at(1, p2)?;
    let mut duplicate = binding(p2);
    duplicate.nonce = binding(profile(0, 10, true)).nonce;
    assert!(l.bind(b, duplicate).is_err());
    Ok(())
}
#[test]
fn s54_memory_count_span_many_short_restarts_and_exact_full_bound() -> Result<()> {
    assert!(Ledger::new([7; 32], SPAN + 1).is_err());
    let mut short = ledger()?;
    for n in 0..36u32 {
        let at = u64::from(n) * 5000;
        let t = start(&mut short, profile(0, 100 + n, false), at)?;
        assert!(!short.progress_due_at(t, at + 4999)?);
        stop(&mut short, t, at + 4999, Stop::Kill)?;
    }
    assert!(short.finish()?.attempts == 72);
    let mut l = ledger()?;
    // Thirty-four sub5s lifetimes do NOT obtain an immediate progress sample.
    for n in 0..34 {
        let t = start(&mut l, profile(0, 100 + n, false), 0)?;
        stop(&mut l, t, 0, Stop::Kill)?;
    }
    let a = start(&mut l, profile(0, 200, false), 0)?;
    let b = start(&mut l, profile(1, 201, false), 0)?;
    for slot in 1..=480 {
        let at = slot * 5000;
        assert!(l.progress_due_at(a, at)?);
        measure(&mut l, a, Reason::Progress, at, at)?;
        measure(&mut l, b, Reason::Progress, at, at)?;
    }
    stop(&mut l, a, SPAN, Stop::Kill)?;
    stop(&mut l, b, SPAN, Stop::Kill)?;
    let full = l.finish()?;
    assert!(full.attempts == 1032 && full.lifetimes == 36 && full.coverage_valid);
    let mut overflow = ledger()?;
    for n in 0..36 {
        let t = start(&mut overflow, profile(0, 100 + n, false), 0)?;
        stop(&mut overflow, t, 0, Stop::Kill)?;
    }
    assert!(overflow.register_at(0, profile(0, 300, false)).is_err());
    let mut span = ledger()?;
    let t = start(&mut span, profile(0, 10, false), 0)?;
    assert!(
        span.begin_at(t, Reason::Progress, Phase::Setup, SPAN + 1)
            .is_err()
    );
    Ok(())
}
#[test]
fn s54_memory_failure_is_not_zero_missing_coverage_and_cleanup_stays_recordable() -> Result<()> {
    let mut l = ledger()?;
    let p = profile(0, 10, false);
    let t = l.register_at(0, p)?;
    l.bind(t, binding(p))?;
    let a = l.begin_at(t, Reason::Start, Phase::Setup, 0)?;
    assert!(
        l.complete_at(a, 1, Err(anyhow::anyhow!("untrusted WIRE_SENTINEL")))
            .is_err()
    );
    assert!(l.rows()[0].reading.is_none() && l.rows()[0].failed);
    assert!(l.request_stop_at(t, Stop::Kill, 1).is_err());
    l.close_at(t, 2, true, Some(1))?;
    assert!(
        l.finish().is_err()
            && l.diagnostic().profiles[0]["sampled_max_private_bytes"].is_null()
            && l.diagnostic().profiles[0]["waited"] == true
    );
    for bytes in [0, PRIVATE + 1] {
        let mut l = ledger()?;
        let p = profile(0, 10, false);
        let t = l.register_at(0, p)?;
        let a = l.begin_at(t, Reason::Start, Phase::Setup, 0)?;
        let mut r = reading(10);
        r.bytes = bytes;
        assert!(l.complete_at(a, 0, Ok(r)).is_err() && l.rows()[0].failed);
    }
    let mut missing = ledger()?;
    let t = start(&mut missing, profile(0, 10, false), 0)?;
    assert!(missing.request_stop_at(t, Stop::Kill, 1).is_err());
    missing.close_at(t, 2, true, Some(1))?;
    assert!(missing.finish().is_err());
    let mut unexpected = ledger()?;
    let t = start(&mut unexpected, profile(0, 10, false), 0)?;
    assert!(unexpected.close_at(t, 1, true, Some(0)).is_err());
    assert!(unexpected.finish().is_err());
    Ok(())
}
#[test]
fn s54_memory_stats_each_candidate_validated_before_pending_aggregation() -> Result<()> {
    let p = profile(0, 10, true);
    for corrupt in [
        json!(null),
        json!("1"),
        json!(true),
        json!(-1),
        json!(128 * 1024 + 1),
    ] {
        let mut l = ledger()?;
        let t = start(&mut l, p, 0)?;
        l.candidate_at(t, 1, &status(p, 100, 200))?;
        let mut bad = status(p, 101, 201);
        bad["stats"]["max_native_bytes"] = corrupt;
        assert!(l.candidate_at(t, 2, &bad).is_err());
        assert!(l.candidate_at(t, 3, &status(p, 102, 202)).is_err()); // Never hide earlier invalid data.
    }
    let mut regression = ledger()?;
    let t = start(&mut regression, p, 0)?;
    regression.candidate_at(t, 1, &status(p, 100, 200))?;
    assert!(regression.candidate_at(t, 2, &status(p, 99, 201)).is_err());
    assert!(regression.candidate_at(t, 3, &status(p, 200, 300)).is_err());
    for mut bad in [
        status(p, 101, 199),
        status(p, 101, 192 * 1024 * 1024 + 1),
        status(p, 101, 201),
    ] {
        let mut invalid = ledger()?;
        let t = start(&mut invalid, p, 0)?;
        invalid.candidate_at(t, 1, &status(p, 100, 200))?;
        if bad["stats"]["max_commit_app_bytes"] == 201 {
            bad["source_id"] = json!("foreign");
        }
        assert!(invalid.candidate_at(t, 2, &bad).is_err());
        assert!(invalid.candidate_at(t, 3, &status(p, 102, 202)).is_err());
    }
    let mut l = ledger()?;
    let t = start(&mut l, p, 0)?;
    l.candidate_at(t, 1, &status(p, 100, 200))?;
    l.candidate_at(t, 2, &status(p, 101, 201))?;
    l.injected_checkpoint(t)?;
    stop(&mut l, t, 3, Stop::Normal)?;
    let observed = l.finish()?;
    assert!(
        observed.profiles[0]["observed_stats"]["first_ms"] == 1
            && observed.profiles[0]["observed_stats"]["last_ms"] == 2
            && observed.profiles[0]["stats_end_gap_ms"] == 1
            && observed.interpretation.contains("not terminal")
    );
    let mut pending = ledger()?;
    let t = start(&mut pending, p, 0)?;
    pending.candidate_at(t, 1, &status(p, 1, 1))?;
    measure(&mut pending, t, Reason::BeforeStop, 2, 2)?;
    assert!(pending.request_stop_at(t, Stop::Normal, 2).is_err());
    let mut absent = ledger()?;
    let t = start(&mut absent, p, 0)?;
    stop(&mut absent, t, 1, Stop::Normal)?;
    assert!(absent.finish().is_err());
    Ok(())
}
#[test]
fn s54_memory_wire_schema_cap_and_safe_errors() -> Result<()> {
    let good = br#"{"pid":10,"ticks":20,"bytes":30}"#;
    assert!(query::parse(good, 10)?["bytes"] == 30);
    for raw in [
        br#"{"pid":10,"ticks":20,"bytes":"WIRE_SENTINEL"}"#.as_slice(),
        br#"{"pid":11,"ticks":20,"bytes":30}"#,
        br#"{"pid":10,"ticks":0,"bytes":30}"#,
        br#"{"pid":10,"ticks":20,"bytes":30,"extra":"WIRE_SENTINEL"}"#,
        b"{",
        &[255],
    ] {
        let e = query::parse(raw, 10)
            .err()
            .context("invalid memory wire rejected")?;
        assert!(!format!("{e:#}").contains("WIRE_SENTINEL"));
    }
    assert!(query::parse(&vec![b' '; 4097], 10).is_err());
    assert!(
        query::Shell::validated(Path::new("C:/Windows/System32/cmd.exe"), 1, &"0".repeat(64))
            .is_err()
    );
    Ok(())
}

#[test]
fn s54_memory_r1_late_stop_settles_missing_progress_and_exact_due_boundary() -> Result<()> {
    // An exact boundary serves its instant; a late boundary serves none of the
    // earlier due instants. No progress query occurred in these lifetimes.
    for (at, expected) in [(5000, 0), (20000, 3), (20001, 4)] {
        let mut l = ledger()?;
        let t = start(&mut l, profile(0, 10, false), 0)?;
        stop(&mut l, t, at, Stop::Kill)?;
        let s = l.finish()?;
        assert!(s.profiles[0]["missed_periodic_slots"] == expected && s.attempts == 2);
    }
    Ok(())
}
#[test]
fn s54_memory_r1_progress_then_late_stop_does_not_double_count() -> Result<()> {
    for (progress, end, expected) in [(5000, 20001, 3), (10001, 30002, 5)] {
        let mut l = ledger()?;
        let t = start(&mut l, profile(0, 10, false), 0)?;
        measure(&mut l, t, Reason::Progress, progress, progress)?;
        stop(&mut l, t, end, Stop::Kill)?;
        let s = l.finish()?;
        assert!(s.profiles[0]["missed_periodic_slots"] == expected && s.attempts == 3);
    }
    Ok(())
}
#[test]
fn s54_memory_r1_normal_closing_tail_is_separate_from_missed_slots() -> Result<()> {
    let mut l = ledger()?;
    let t = start(&mut l, profile(0, 10, false), 0)?;
    measure(&mut l, t, Reason::BeforeStop, 20001, 20001)?;
    l.request_stop_at(t, Stop::Normal, 20001)?;
    assert!(!l.progress_due_at(t, 25000)?);
    l.close_at(t, 30000, true, Some(0))?;
    let s = l.finish()?;
    assert!(
        s.profiles[0]["missed_periodic_slots"] == 4
            && s.profiles[0]["closing_tail_ms"] == 9999
            && s.attempts == 2
    );
    // Missing boundaries still fail, while actual cleanup remains recordable.
    let mut failed = ledger()?;
    let t = start(&mut failed, profile(0, 10, false), 0)?;
    assert!(failed.request_stop_at(t, Stop::Kill, 20001).is_err());
    failed.close_at(t, 60000, true, Some(1))?;
    assert!(
        failed.diagnostic().profiles[0]["missed_periodic_slots"] == 4
            && failed.diagnostic().profiles[0]["waited"] == true
            && failed.finish().is_err()
    );
    Ok(())
}
#[test]
fn s54_memory_r2_finalized_rows_summary_and_cleanup_are_immutable() -> Result<()> {
    let mut l = ledger()?;
    let p = profile(0, 10, false);
    let t = l.register_at(0, p)?;
    l.bind(t, binding(p))?;
    let saved = l.begin_at(t, Reason::Start, Phase::Setup, 0)?;
    l.complete_at(saved, 0, Ok(reading(p.pid)))?;
    stop(&mut l, t, 1, Stop::Kill)?;
    assert!(l.finish()?.coverage_valid);
    let snapshot = |l: &Ledger| {
        Ok::<_, anyhow::Error>((
            serde_json::to_vec(l.rows())?,
            serde_json::to_vec(&l.diagnostic())?,
            serde_json::to_vec(&l.lives)?,
            l.last_ms,
            l.failed,
            l.sealed,
        ))
    };
    let before = snapshot(&l)?;
    assert!(l.complete_at(saved, 2, Ok(reading(p.pid))).is_err());
    assert!(snapshot(&l)? == before);
    assert!(l.request_stop_at(t, Stop::Normal, 2).is_err());
    assert!(snapshot(&l)? == before);
    assert!(l.close_at(t, 3, true, Some(0)).is_err());
    assert!(snapshot(&l)? == before);
    assert!(l.register_at(3, profile(1, 11, false)).is_err());
    assert!(l.bind(t, binding(p)).is_err());
    assert!(l.progress_due_at(t, 5000).is_err());
    assert!(
        l.begin_at(t, Reason::Progress, Phase::Closing, 5000)
            .is_err()
    );
    assert!(l.candidate_at(t, 5000, &status(p, 1, 1)).is_err());
    assert!(l.injected_checkpoint(t).is_err());
    assert!(l.finish().is_err());
    assert!(snapshot(&l)? == before && l.diagnostic().coverage_valid);
    let mut failed = ledger()?;
    let t = start(&mut failed, p, 0)?;
    assert!(failed.finish().is_err()); // A failed finalization must not seal cleanup.
    assert!(failed.request_stop_at(t, Stop::Kill, 20001).is_err());
    failed.close_at(t, 20002, true, Some(1))?;
    assert!(
        failed.diagnostic().profiles[0]["waited"] == true
            && !failed.sealed
            && !failed.diagnostic().coverage_valid
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn s54_memory_actual_three_queries_same_benign_child_and_bounded_cleanup() -> Result<()> {
    use sha2::{Digest, Sha256};
    let base = workspace().join(".tmp/s54-p11a8a-memory-tests");
    std::fs::create_dir_all(&base)?;
    let dir = base.join(format!("resident-{}", std::process::id()));
    std::fs::create_dir(&dir)?;
    let _managed = s53_deadline::Guard::enter(&dir, 40)?;
    let until = s53_deadline::capture_deadline()?;
    let _capture = s54_capture::Guard::enter(until)?;
    let shell = query::Shell::validated(
        Path::new("C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"),
        495616,
        "60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f",
    )?;
    let resident_exe = Path::new(
        "C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe",
    );
    let image = Sha256::digest(std::fs::read(resident_exe)?);
    ensure!(
        format!("{image:x}") == "362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139",
        "pinned benign resident image"
    );
    let fixture =
        workspace().join("apps/kilogram-cli/tests/support/migration_s54_memory_fixture.ps1");
    ensure!(
        std::fs::read(&fixture)? == include_bytes!("migration_s54_memory_fixture.ps1"),
        "immutable benign fixture"
    );
    struct Resident {
        child: Child,
        until: Instant,
    }
    impl Resident {
        fn stop(&mut self) -> Result<i32> {
            self.child.kill()?;
            let until = self.until.min(Instant::now() + Duration::from_secs(2));
            loop {
                if self.child.try_wait()?.is_some() {
                    return self.child.wait()?.code().context("resident kill code");
                }
                ensure!(
                    Instant::now() < until,
                    "resident cleanup pending under outer Job"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    impl Drop for Resident {
        fn drop(&mut self) {
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.stop();
            }
        }
    }
    let mut l = Ledger::new([8; 32], 40000)?;
    let ready = dir.join("ready");
    let child = command(resident_exe)
        .args(["-NoProfile", "-NonInteractive", "-NoLogo", "-File"])
        .arg(fixture)
        .arg("-ReadyFile")
        .arg(&ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut resident = Resident { child, until };
    let pid = resident.child.id();
    let t = l.register_at(
        l.now_ms(),
        Profile {
            owner: 0,
            profile: [1; 32],
            image: image.into(),
            current: false,
            pid,
        },
    )?;
    l.bind(
        t,
        Binding {
            owner: 0,
            profile: [1; 32],
            source: [10; 32],
            nonce: None,
        },
    )?; // Labelled benign metadata, not original-source proof.
    l.sample(t, Reason::Start, Phase::Setup, &mut resident.child, &shell)?;
    let ready_until = until.min(Instant::now() + Duration::from_secs(5));
    while !ready.exists() {
        ensure!(
            resident.child.try_wait()?.is_none() && Instant::now() < ready_until,
            "benign explicit ready marker"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let progress_at = l.lives[0].born_ms + 5000;
    while l.now_ms() < progress_at {
        s53_deadline::check()?;
        std::thread::sleep(Duration::from_millis(10));
    }
    l.sample(
        t,
        Reason::Progress,
        Phase::Stage(0),
        &mut resident.child,
        &shell,
    )?;
    l.sample(
        t,
        Reason::BeforeStop,
        Phase::Closing,
        &mut resident.child,
        &shell,
    )?;
    l.request_stop_at(t, Stop::Kill, l.now_ms())?;
    let code = resident.stop()?;
    l.close_at(t, l.now_ms(), true, Some(code))?;
    let summary = l.finish()?;
    ensure!(
        summary.coverage_valid && l.rows().len() == 3,
        "three real query coverage"
    );
    let first = l.rows()[0].reading.context("first actual reading")?;
    ensure!(
        l.rows().iter().all(|row| row
            .reading
            .is_some_and(|r| r.pid == pid && r.ticks == first.ticks && r.bytes > 0)
            && row
                .end_ms
                .is_some_and(|end| end >= row.start_ms && end - row.start_ms <= 2000)),
        "same real child/ticks and query ceiling"
    );
    ensure!(
        l.rows()[1].start_ms >= l.lives[0].born_ms + 5000,
        "actual first progress spacing"
    );
    ensure!(
        l.sample(
            t,
            Reason::Progress,
            Phase::Cleanup,
            &mut resident.child,
            &shell
        )
        .is_err(),
        "no query after finalized/waited child"
    );
    let mut facts = Vec::new();
    for n in 0..3 {
        let row: Value = serde_json::from_slice(&std::fs::read(
            dir.join(format!("command-{n:04}.stderr.log")),
        )?)?;
        ensure!(
            row["capture"]["classification"] == "success"
                && row["capture"]["waited"] == true
                && row["capture"]["stdout_eof"] == true
                && row["capture"]["stderr_eof"] == true
                && row["capture"]["cleanup_pending"] == false
                && row["capture"]["stdout_bytes"]
                    .as_u64()
                    .is_some_and(|n| n <= 4096),
            "actual bounded query exits/EOF"
        );
        facts.push(row);
    }
    ensure!(
        !dir.join("command-0003.stdout.log").exists(),
        "closed rejection starts zero extra query"
    );
    std::fs::write(
        dir.join("result.json"),
        serde_json::to_vec_pretty(
            &json!({"classification":"A8a benign resident/query evidence only; not S54 profile coverage",
        "resident_pid":pid,"resident_waited":true,"resident_code":code,"sampled_core":summary,"rows":l.rows(),"queries":facts,
        "expected_finalized_rejection_without_spawn":true,"memory_shell_path":"C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe",
        "memory_shell_bytes":495616,"memory_shell_sha256":"60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f",
        "memory_command_sha256":format!("{:x}",Sha256::digest(s54_capture::memory_command(pid).as_bytes()))}),
        )?,
    )?;
    println!(
        "synthetic_memory resident_pid={pid} evidence={}",
        dir.display()
    );
    Ok(())
}
