use super::*;
fn hash_pin(p: &Path) -> Result<Pin> {
    let bytes = std::fs::read(p)?;
    Ok(Pin {
        path: p.canonicalize()?,
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}
fn shell() -> Result<query::Shell> {
    query::Shell::validated(
        Path::new("C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"),
        495616,
        "60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f",
    )
}
fn scratch() -> Result<PathBuf> {
    let base = workspace().join(".tmp/s54-p11a8b-fixtures");
    std::fs::create_dir_all(&base)?;
    Ok(tempfile::Builder::new()
        .prefix("scope-")
        .tempdir_in(base)?
        .keep())
}
fn required(dir: &Path, attempt: String) -> Result<RequiredInput> {
    let inventory = dir.join("inventory.json");
    std::fs::write(
        &inventory,
        b"{\"classification\":\"synthetic input; not whole runner admission\"}",
    )?;
    let image = hash_pin(Path::new(
        "C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe",
    ))?;
    ensure!(
        image.sha256 == "362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139",
        "pinned benign fixture image"
    );
    Ok(RequiredInput {
        run: [8; 32],
        attempt,
        parent: dir.into(),
        inventory: hash_pin(&inventory)?,
        images: [image.clone(), image],
        shell: shell()?,
    })
}
fn aliases(s: &mut State) -> Result<()> {
    for role in 0..2u8 {
        let root = s.input.required.parent.join(format!("state-{role}"));
        std::fs::create_dir_all(&root)?;
        for current in [false, true] {
            let profile = root.join(if current { "current.json" } else { "old.json" });
            std::fs::write(&profile, b"{\"fixture_config\":1}")?;
            let ipc = root.join(if current { "current.ipc" } else { "old.ipc" });
            s.alias(role, &root, &profile, &ipc, current)?;
        }
    }
    Ok(())
}
fn model() -> Result<State> {
    let dir = scratch()?;
    let r = required(&dir, "injected".into())?;
    let origin = Instant::now();
    let clock = s53_deadline::ObserverContext {
        origin,
        whole: origin + Duration::from_secs(60),
        run: dir,
    };
    let ledger = Ledger::with_origin(r.run, &clock)?;
    let mut s = State {
        ledger,
        input: Input {
            required: r,
            clock,
            synthetic: true,
        },
        roots: [None, None],
        sources: [None, None],
        aliases: vec![],
        raw: vec![],
        phase: Phase::Setup,
        finished: false,
        output: None,
        queries: 0,
    };
    aliases(&mut s)?;
    Ok(s)
}
fn raw(s: &mut State, owner: u8, pid: u32, current: bool) -> Result<usize> {
    let a = s
        .aliases
        .iter()
        .find(|a| a.owner == owner && a.current == current)
        .context("fixture alias")?
        .clone();
    s.seen(
        Tag {
            pid,
            handle: pid as usize,
        },
        Prepared {
            alias: a,
            config: "synthetic config digest".into(),
            image: [3; 32],
        },
    )
}
fn register(s: &mut State, i: usize) -> Result<Token> {
    let t = s.identity_register(i)?;
    let at = s.ledger.now_ms();
    let a = s.ledger.begin_at(t, Reason::Start, Phase::Setup, at)?;
    s.ledger.complete_at(
        a,
        at,
        Ok(Reading {
            pid: t.pid,
            ticks: u64::from(t.pid),
            bytes: 1024,
            evidence: Evidence::Injected,
        }),
    )?;
    Ok(t)
}
fn close_model(s: &mut State, i: usize, aborted: bool) -> Result<()> {
    let r = &s.raw[i];
    let t = r.token.context("fixture token")?;
    let at = s.ledger.now_ms();
    if !aborted {
        let a = s
            .ledger
            .begin_at(t, Reason::BeforeStop, Phase::Closing, at)?;
        s.ledger.complete_at(
            a,
            at,
            Ok(Reading {
                pid: t.pid,
                ticks: u64::from(t.pid),
                bytes: 1024,
                evidence: Evidence::Injected,
            }),
        )?;
        s.ledger.request_stop_at(t, Stop::Kill, at)?;
        s.raw[i].stop = Some(Stop::Kill);
    }
    s.terminal_tag(s.raw[i].tag, true, Some(1), aborted)
}
fn value(source: Id, n: u64) -> Value {
    json!({"version":3,"profile_mode":"ordered-dialog-indexed","storage_stage":"Ready",
    "source_id":kilogram_cli::ordered_dialog::state::hex(&source),"stats":{"max_native_bytes":n,"max_commit_app_bytes":n}})
}
/// Injected ledger observations only. Production binding functions still run
/// through the actual registry; each poisoned negative uses a separate model.
pub(crate) struct DeferredModel;
impl DeferredModel {
    pub(crate) fn enter() -> Result<Self> {
        ensure!(!active(), "separate deferred model");
        let state = model()?;
        SLOT.with_borrow_mut(|s| s.state = Some(state));
        Ok(Self)
    }
    pub(crate) fn legacy(&self, owner: u8, pid: u32, closed: bool) -> Result<usize> {
        eval(false, |s| {
            let i = raw(s, owner, pid, false)?;
            register(s, i)?;
            if closed {
                close_model(s, i, false)?;
            }
            Ok(i)
        })?
        .context("model active")
    }
    pub(crate) fn current(&self, owner: u8, pid: u32, source: Id, nonce: Id) -> Result<usize> {
        eval(false, |s| {
            let i = raw(s, owner, pid, true)?;
            register(s, i)?;
            s.bind_one(i, source, Some(nonce))?;
            Ok(i)
        })?
        .context("model active")
    }
    pub(crate) fn close(&self, i: usize) -> Result<()> {
        eval(false, |s| close_model(s, i, false))?.context("model active")
    }
    pub(crate) fn stats(&self, i: usize, source: Id) -> Result<()> {
        eval(false, |s| {
            let a = s.raw[i].alias.clone();
            let t = s
                .ticket(&a.profile, &a.ipc, true)?
                .context("current ticket")?;
            s.candidate(t, &value(source, 10))?;
            s.ledger
                .injected_checkpoint(s.raw[i].token.context("current token")?)
        })?
        .context("model active")
    }
    pub(crate) fn source(&self, owner: usize) -> Result<Option<Id>> {
        eval(true, |s| Ok(s.sources[owner]))?.context("model active")
    }
    pub(crate) fn bound(&self, owner: u8) -> Result<usize> {
        eval(true, |s| {
            Ok(s.raw
                .iter()
                .filter(|r| r.alias.owner == owner && r.binding.is_some())
                .count())
        })?
        .context("model active")
    }
    pub(crate) fn failed(&self) -> Result<bool> {
        eval(true, |s| Ok(s.ledger.failed))?.context("model active")
    }
}
impl Drop for DeferredModel {
    fn drop(&mut self) {
        SLOT.with_borrow_mut(|s| {
            s.state = None;
            s.busy = false;
            s.poison = false;
        });
    }
}

/// Existing benign resident, actual Child/query adapter; application start and
/// Stats values remain injected, with source IDs supplied by verified storage.
#[cfg(windows)]
pub(crate) fn actual_deferred_adapter(
    dir: &Path,
    roots: [&Path; 2],
    public: [&Path; 2],
    sources: [Id; 2],
    mut bind: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let _deadline = s53_deadline::Guard::enter(dir, 100)?;
    let until = s53_deadline::capture_deadline()?;
    let _capture = s54_capture::Guard::enter(until)?;
    let _input = InputGuard::install_kind(required(dir, s54_capture::admitted_attempt()?)?, true)?;
    let observer = Guard::enter(dir, until)?;
    let mut profiles = Vec::new();
    let mut ipcs = Vec::new();
    for owner in 0..2 {
        let mut p = Vec::new();
        let mut q = Vec::new();
        for current in [false, true] {
            let stem = if current { "ao-current" } else { "ao-legacy" };
            let profile = public[owner].join(format!("{stem}.json"));
            std::fs::write(&profile, b"{\"adapter_fixture\":true}")?;
            let ipc = public[owner].join(format!("{stem}.ipc"));
            super::owner(owner as u8, roots[owner], &profile, &ipc, current)?;
            p.push(profile);
            q.push(ipc);
        }
        profiles.push(p);
        ipcs.push(q);
    }
    let exe = Path::new(
        "C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe",
    );
    let fixture =
        workspace().join("apps/kilogram-cli/tests/support/migration_s54_memory_fixture.ps1");
    ensure!(
        std::fs::read(&fixture)? == include_bytes!("migration_s54_memory_fixture.ps1"),
        "pinned resident fixture"
    );
    let spawn = |profile: &Path, ready: &Path| -> Result<OwnedChild> {
        let mut cmd = command(exe);
        cmd.args(["-NoProfile", "-NonInteractive", "-NoLogo", "-File"])
            .arg(&fixture)
            .arg("-ReadyFile")
            .arg(ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        OwnedChild::spawn_memory_fixture(&mut cmd, profile)
    };
    let mut a = spawn(&profiles[0][0], &public[0].join("ao-old-ready"))?;
    let mut b = spawn(&profiles[1][0], &public[1].join("ao-old-ready"))?;
    ensure!(
        eval(true, |s| Ok(
            s.sources == [None, None] && s.raw.iter().all(|r| r.binding.is_none())
        ))?
        .context("state")?,
        "actual legacy residents unbound"
    );
    a.kill_wait()?;
    ensure!(a.0.try_wait()?.is_some(), "actual old A wait");
    bind(0)?;
    ensure!(
        eval(true, |s| Ok(s.sources == [Some(sources[0]), None]
            && s.raw[0].binding.is_some()
            && s.raw[1].binding.is_none()))?
        .context("state")?,
        "A backfilled B remains unbound"
    );
    let mut waits = 1;
    for nonce in [[61; 32], [62; 32]] {
        b.alive()?;
        let ready = public[0].join(format!("ao-current-{}-ready", nonce[0]));
        let mut current = spawn(&profiles[0][1], &ready)?;
        bind_current(&current.0, sources[0], nonce)?;
        around_status(&profiles[0][1], &ipcs[0][1], true, || {
            Ok(value(sources[0], 10))
        })?;
        checkpoint(&mut current.0)?;
        b.alive()?;
        current.kill_wait()?;
        ensure!(current.0.try_wait()?.is_some(), "actual current wait");
        waits += 1;
    }
    b.kill_wait()?;
    ensure!(b.0.try_wait()?.is_some(), "actual old B wait");
    waits += 1;
    bind(1)?;
    let facts = finish_coverage()?;
    ensure!(
        facts["original_lifetime_coverage_complete"] == true
            && facts["coverage"]["coverage_valid"] == true
            && facts["actual_resident_spawns"] == 4
            && facts["actual_terminal_outcomes"] == 4,
        "actual deferred adapter coverage"
    );
    drop(observer);
    println!(
        "ao_actual_adapter=passed;resident_spawns=4;actual_waits={waits};migration_sources=real;decoder_Stats=injected;owner7_network=false;new_nonce=true"
    );
    Ok(())
}
#[test]
fn s54_memory_scope_optout_and_required_input_absence_do_no_observation() -> Result<()> {
    ensure!(!active(), "pure test scope absent");
    let v = around_status(Path::new("unused"), Path::new("unused"), true, || {
        Ok(json!({"legacy":true}))
    })?;
    assert!(v["legacy"] == true && diagnostic().is_err() && finish_coverage().is_err());
    let cmd = command(Path::new("must-not-launch"));
    assert!(prepare(&cmd)?.is_none());
    assert!(Guard::enter(Path::new("unused"), Instant::now() + Duration::from_secs(1)).is_err());
    Ok(())
}
#[test]
fn s54_memory_scope_independent_spawn_coverage_and_late_setup_binding() -> Result<()> {
    let mut s = model()?;
    raw(&mut s, 0, 10, false)?;
    assert!(s.raw.len() == 1 && s.ledger.lives.is_empty() && s.finish().is_err());
    let mut s = model()?;
    let i = raw(&mut s, 0, 10, false)?;
    register(&mut s, i)?;
    close_model(&mut s, i, false)?;
    assert!(s.raw[0].waited && s.raw[0].binding.is_none());
    s.sources[0] = Some([10; 32]);
    s.bind_one(i, [10; 32], None)?;
    let j = raw(&mut s, 1, 11, false)?;
    register(&mut s, j)?;
    s.sources[1] = Some([11; 32]);
    s.bind_one(j, [11; 32], None)?;
    close_model(&mut s, j, false)?;
    assert!(s.finish()?["original_lifetime_coverage_complete"] == true);
    let mut omitted = model()?;
    let i = raw(&mut omitted, 0, 10, false)?;
    assert!(
        omitted
            .terminal_tag(omitted.raw[i].tag, true, Some(1), true)
            .is_err()
    );
    assert!(omitted.raw[0].waited && omitted.raw[0].token.is_none() && omitted.ledger.failed);
    let mut overlap = model()?;
    raw(&mut overlap, 0, 10, false)?;
    assert!(raw(&mut overlap, 0, 11, false).is_err());
    assert!(overlap.raw.len() == 2 && overlap.ledger.lives.is_empty());
    assert!(
        overlap
            .terminal_tag(overlap.raw[1].tag, true, Some(1), true)
            .is_err()
    );
    assert!(overlap.raw[1].waited && overlap.ledger.failed);
    Ok(())
}
#[test]
fn s54_memory_scope_restart_tickets_config_identity_and_each_candidate() -> Result<()> {
    let mut s = model()?;
    s.sources = [Some([10; 32]), Some([11; 32])];
    let i = raw(&mut s, 0, 10, true)?;
    register(&mut s, i)?;
    s.bind_one(i, [10; 32], Some([1; 32]))?;
    let a = s.raw[i].alias.clone();
    let old = s.ticket(&a.profile, &a.ipc, true)?.context("ticket")?;
    s.candidate(
        s.ticket(&a.profile, &a.ipc, true)?.context("ticket")?,
        &value([10; 32], 10),
    )?;
    s.ledger
        .injected_checkpoint(s.raw[i].token.context("token")?)?;
    close_model(&mut s, i, false)?;
    std::fs::write(&a.profile, b"{\"fixture_config\":2}")?;
    let p = s.prepare(&a.profile, &s.input.required.images[1].path.clone())?;
    assert!(p.config != "synthetic config digest");
    let j = s.seen(
        Tag {
            pid: 11,
            handle: 11,
        },
        p,
    )?;
    register(&mut s, j)?;
    s.bind_one(j, [10; 32], Some([2; 32]))?;
    assert!(s.ledger.lives[0].profile.profile == s.ledger.lives[1].profile.profile);
    assert!(s.candidate(old, &value([10; 32], 20)).is_err());
    let mut invalid = model()?;
    let i = raw(&mut invalid, 0, 10, true)?;
    register(&mut invalid, i)?;
    invalid.bind_one(i, [10; 32], Some([1; 32]))?;
    let a = invalid.raw[i].alias.clone();
    invalid.candidate(
        invalid
            .ticket(&a.profile, &a.ipc, true)?
            .context("ticket")?,
        &value([10; 32], 10),
    )?;
    assert!(
        invalid
            .candidate(
                invalid
                    .ticket(&a.profile, &a.ipc, true)?
                    .context("ticket")?,
                &value([10; 32], 9)
            )
            .is_err()
    );
    // A regressing candidate poisons admission before another ticket exists.
    assert!(invalid.ticket(&a.profile, &a.ipc, true).is_err());
    Ok(())
}
#[test]
fn s54_memory_scope_bounded_lease_no_status_borrow_and_error_cleanup() -> Result<()> {
    let mut s = model()?;
    let i = raw(&mut s, 0, 10, true)?;
    register(&mut s, i)?;
    s.bind_one(i, [10; 32], Some([1; 32]))?;
    let a = s.raw[i].alias.clone();
    SLOT.with_borrow_mut(|slot| slot.state = Some(s));
    let guard = Guard(PhantomData);
    around_status(&a.profile, &a.ipc, true, || {
        assert!(diagnostic().is_ok());
        Ok(value([10; 32], 1))
    })?;
    assert!(diagnostic()?["query_attempts"] == 0);
    assert!(eval(false, |_| diagnostic()).is_err());
    assert!(diagnostic()?["coverage"]["coverage_valid"] == false);
    drop(guard);
    let mut cleanup = model()?;
    let i = raw(&mut cleanup, 0, 10, false)?;
    register(&mut cleanup, i)?;
    cleanup.fail();
    let before = cleanup.queries;
    assert!(close_model(&mut cleanup, i, true).is_err());
    assert!(
        cleanup.queries == before
            && cleanup.raw[i].waited
            && cleanup.raw[i].aborted
            && cleanup.finish().is_err()
    );
    Ok(())
}
#[test]
fn s54_memory_scope_two_owner_cadence_normal_closing_and_duplicate_guards() -> Result<()> {
    let mut s = model()?;
    for owner in 0..2 {
        let i = raw(&mut s, owner, 10 + u32::from(owner), false)?;
        let t = register(&mut s, i)?;
        assert!(!s.ledger.progress_due_at(t, s.ledger.now_ms())?);
    }
    let at = s.ledger.last_ms + 6000;
    for i in 0..2 {
        let t = s.raw[i].token.context("token")?;
        assert!(s.ledger.progress_due_at(t, at)?);
        let a = s
            .ledger
            .begin_at(t, Reason::Progress, Phase::Stage(0), at)?;
        s.ledger.complete_at(
            a,
            at,
            Ok(Reading {
                pid: t.pid,
                ticks: u64::from(t.pid),
                bytes: 1024,
                evidence: Evidence::Injected,
            }),
        )?;
    }
    s.ledger.origin = Instant::now() - Duration::from_millis(at + 10);
    let i = 0;
    let t = s.raw[i].token.context("token")?;
    let stop = s.ledger.now_ms();
    let a = s
        .ledger
        .begin_at(t, Reason::BeforeStop, Phase::Closing, stop)?;
    s.ledger.complete_at(
        a,
        stop,
        Ok(Reading {
            pid: t.pid,
            ticks: u64::from(t.pid),
            bytes: 1024,
            evidence: Evidence::Injected,
        }),
    )?;
    s.ledger.request_stop_at(t, Stop::Normal, stop)?;
    s.raw[i].stop = Some(Stop::Normal);
    assert!(!s.ledger.progress_due_at(t, stop + 5000)?);
    s.ledger.origin = Instant::now() - Duration::from_millis(stop + 5010);
    s.terminal_tag(s.raw[i].tag, true, Some(0), false)?;
    assert!(s.terminal_tag(s.raw[i].tag, true, Some(0), false).is_err());
    let mut duplicate = model()?;
    let i = raw(&mut duplicate, 0, 10, false)?;
    register(&mut duplicate, i)?;
    assert!(duplicate.identity_register(i).is_err());
    Ok(())
}
#[cfg(windows)]
#[test]
fn s54_memory_scope_actual_shared_restart_status_checkpoint_and_failed_prekill() -> Result<()> {
    let dir = scratch()?;
    let _deadline = s53_deadline::Guard::enter(&dir, 40)?;
    let until = s53_deadline::capture_deadline()?;
    let _capture = s54_capture::Guard::enter(until)?;
    assert!(Guard::enter(&dir, until).is_err()); // No success marker supplies required input.
    let _input = InputGuard::install_kind(required(&dir, s54_capture::admitted_attempt()?)?, true)?;
    let observer = Guard::enter(&dir, until)?;
    eval(false, aliases)?;
    for role in 0..2 {
        bind_legacy(role, [10 + role; 32])?;
    }
    let profile = dir.join("state-0/current.json");
    let ipc = dir.join("state-0/current.ipc");
    let exe = Path::new(
        "C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe",
    );
    let fixture =
        workspace().join("apps/kilogram-cli/tests/support/migration_s54_memory_fixture.ps1");
    ensure!(
        std::fs::read(&fixture)? == include_bytes!("migration_s54_memory_fixture.ps1"),
        "pinned resident fixture"
    );
    let make = |ready: &Path| {
        let mut cmd = command(exe);
        cmd.args(["-NoProfile", "-NonInteractive", "-NoLogo", "-File"])
            .arg(&fixture)
            .arg("-ReadyFile")
            .arg(ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd
    };
    let mut first = OwnedChild::spawn_memory_fixture(&mut make(&dir.join("ready-1")), &profile)?;
    bind_current(&first.0, [10; 32], [1; 32])?;
    let count = diagnostic()?["query_attempts"].clone();
    for n in 1..=10 {
        around_status(&profile, &ipc, true, || {
            assert!(diagnostic().is_ok());
            Ok(value([10; 32], n))
        })?;
        checkpoint(&mut first.0)?;
    }
    assert!(diagnostic()?["query_attempts"] == count); // ten Stats accepts, zero extra RAM queries.
    let born = eval(true, |s| Ok(s.ledger.lives[0].born_ms))?.context("birth")?;
    while eval(true, |s| Ok(s.ledger.now_ms()))?.context("clock")? < born + 5000 {
        s53_deadline::check()?;
        std::thread::sleep(Duration::from_millis(10));
    }
    first.alive()?;
    first.kill_wait()?;
    assert!(first.0.try_wait()?.is_some());
    std::fs::write(&profile, b"{\"fixture_config\":2}")?;
    let mut second = OwnedChild::spawn_memory_fixture(&mut make(&dir.join("ready-2")), &profile)?;
    bind_current(&second.0, [10; 32], [2; 32])?;
    let bad = around_status(&profile, &ipc, true, || Ok(value([10; 32], 128 * 1024 + 1)));
    assert!(bad.is_err());
    assert!(second.kill_wait().is_err()); // failed observation did NOT cancel actual kill/wait.
    assert!(second.0.try_wait()?.is_some());
    let facts = diagnostic()?;
    assert!(
        facts["actual_resident_spawns"] == 2
            && facts["actual_terminal_outcomes"] == 2
            && facts["query_attempts"] == 4
            && facts["coverage"]["coverage_valid"] == false
    );
    let raw = facts["actual_lifetimes"].as_array().context("raw")?;
    assert!(
        raw[0]["config"] != raw[1]["config"]
            && facts["coverage"]["profiles"][0]["profile"]["profile"]
                == facts["coverage"]["profiles"][1]["profile"]["profile"]
    );
    assert!(finish_coverage().is_err());
    std::fs::write(
        dir.join("adapter-result.json"),
        serde_json::to_vec_pretty(
            &json!({"classification":"synthetic shared lifecycle wiring; application source/nonce/Stats injected; not Controller/protocol/whole",
        "facts":facts,"first_pid":first.0.id(),"second_pid":second.0.id(),"first_waited":true,"second_waited":true,"failed_prekill_still_waited":true,
        "no_per_status_ram_query":true}),
        )?,
    )?;
    drop(first);
    drop(second);
    drop(observer);
    println!("synthetic_memory_scope evidence={}", dir.display());
    Ok(())
}
