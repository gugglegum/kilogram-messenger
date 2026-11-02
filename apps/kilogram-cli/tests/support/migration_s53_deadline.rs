//! An opt-in absolute budget for the new managed process controllers. Legacy
//! callers retain their old behavior when this exact-test scope is absent.
use super::*;
use std::{cell::RefCell, process::Output};
struct Scope {
    origin: Instant,
    deadline: Instant,
    containment: Option<Instant>,
    local: Option<Instant>,
    run: PathBuf,
    next: u64,
    measurements: Option<std::collections::BTreeMap<&'static str, Measurement>>,
}
thread_local! {static SCOPE:RefCell<Option<Scope>>=const {RefCell::new(None)};}
/// F-only inclusive duration classes. Nested classes must not be added twice
/// during projection. No arguments, private output or guessed protocol timings.
#[derive(Default, serde::Serialize)]
struct Measurement {
    count: u64,
    total_us: u128,
    maximum_us: u128,
    first_start_us: u128,
    last_end_us: u128,
}
pub(super) struct MeasurementSpan {
    label: &'static str,
    start: Option<Instant>,
}
pub(super) fn measure(label: &'static str) -> MeasurementSpan {
    let enabled = SCOPE.with_borrow(|s| s.as_ref().is_some_and(|s| s.measurements.is_some()));
    MeasurementSpan {
        label,
        start: enabled.then(Instant::now),
    }
}
impl Drop for MeasurementSpan {
    fn drop(&mut self) {
        let Some(start) = self.start else {
            return;
        };
        let end = Instant::now();
        SCOPE.with_borrow_mut(|s| {
            if let Some(s) = s
                && let Some(rows) = &mut s.measurements
            {
                let m = rows.entry(self.label).or_default();
                if m.count == 0 {
                    m.first_start_us = start.saturating_duration_since(s.origin).as_micros();
                }
                m.count += 1;
                m.total_us += end.duration_since(start).as_micros();
                m.maximum_us = m.maximum_us.max(end.duration_since(start).as_micros());
                m.last_end_us = end.saturating_duration_since(s.origin).as_micros();
            }
        });
    }
}
pub(super) fn feasibility_timings() -> Result<Value> {
    check()?;
    SCOPE.with_borrow(|s| {
        let rows=s.as_ref().and_then(|s|s.measurements.as_ref()).context("F timing scope required")?;
        ensure!(rows.len() <= 32,"bounded fixed F timing classes");
        Ok(json!({"classification":"inclusive actual F durations; nested classes overlap; not a P prediction", "classes":rows}))
    })
}
pub(super) struct Guard;
impl Guard {
    pub(super) fn enter(run: &Path, seconds: u64) -> Result<Self> {
        SCOPE.with_borrow_mut(|scope| {
            ensure!(scope.is_none(), "one absolute managed controller scope");
            let origin = Instant::now();
            *scope = Some(Scope {
                origin,
                deadline: origin + Duration::from_secs(seconds),
                containment: None,
                local: None,
                run: run.into(),
                next: 0,
                measurements: None,
            });
            Ok(Self)
        })
    }
    /// S54 alone: a conservative original origin, never reply/startup time.
    /// Controller and observer stop at2350; capture cleanup cannot exceed2400.
    pub(super) fn enter_original(run: &Path, origin: Instant) -> Result<Self> {
        Self::enter_original_limits(run, origin, 2350, 2400)
    }
    pub(super) fn enter_original_feasibility(run: &Path, origin: Instant) -> Result<Self> {
        Self::enter_original_limits(run, origin, 450, 510)
    }
    fn enter_original_limits(
        run: &Path,
        origin: Instant,
        controller: u64,
        outer: u64,
    ) -> Result<Self> {
        let now = Instant::now();
        let deadline = origin
            .checked_add(Duration::from_secs(controller))
            .context("original controller overflow")?;
        let whole = origin
            .checked_add(Duration::from_secs(outer))
            .context("original outer overflow")?;
        ensure!(
            origin <= now && now < deadline,
            "expired/future original controller origin"
        );
        SCOPE.with_borrow_mut(|scope| {
            ensure!(scope.is_none(), "one absolute managed controller scope");
            *scope = Some(Scope {
                origin,
                deadline,
                containment: Some(whole),
                local: None,
                run: run.into(),
                next: 0,
                measurements: (controller == 450 && outer == 510).then(Default::default),
            });
            Ok(Self)
        })
    }
}
/// A local phase can only shorten the current bound. Dropping it restores the
/// previous phase, never the absolute managed deadline or command sequence.
pub(super) struct Phase(Option<Instant>);
impl Phase {
    pub(super) fn until(deadline: Instant) -> Result<Self> {
        SCOPE.with_borrow_mut(|scope| {
            let s = scope.as_mut().context("phase requires managed scope")?;
            let previous = s.local;
            let effective = deadline.min(s.deadline).min(previous.unwrap_or(deadline));
            ensure!(Instant::now() < effective, "expired S5.3 phase deadline");
            s.local = Some(effective);
            Ok(Self(previous))
        })
    }
}
impl Drop for Phase {
    fn drop(&mut self) {
        SCOPE.with_borrow_mut(|scope| {
            if let Some(s) = scope {
                s.local = self.0;
            }
        });
    }
}
fn effective(s: &Scope) -> Instant {
    s.deadline.min(s.local.unwrap_or(s.deadline))
}
impl Drop for Guard {
    fn drop(&mut self) {
        SCOPE.with_borrow_mut(|scope| *scope = None);
    }
}
pub(super) fn check() -> Result<()> {
    SCOPE.with_borrow(|scope| {
        ensure!(
            scope.as_ref().is_none_or(|s| Instant::now() < effective(s)),
            "absolute S5.3 process controller deadline"
        );
        Ok(())
    })
}
/// Reserve a status slot without changing the legacy output path or deadline.
pub(super) struct CaptureBound {
    pub work: Instant,
    pub whole: Instant,
    pub out: PathBuf,
    pub err: PathBuf,
    pub sequence: u64,
}
pub(super) fn capture_bound() -> Result<CaptureBound> {
    check()?;
    SCOPE.with_borrow_mut(|scope| {
        let s = scope.as_mut().context("status requires managed deadline")?;
        let sequence = s.next;
        s.next += 1;
        Ok(CaptureBound {
            work: effective(s),
            whole: s.containment.unwrap_or(s.deadline),
            out: s.run.join(format!("command-{sequence:04}.stdout.log")),
            err: s.run.join(format!("command-{sequence:04}.stderr.log")),
            sequence,
        })
    })
}
pub(super) fn original_containment() -> Option<Instant> {
    SCOPE.with_borrow(|scope| scope.as_ref().and_then(|s| s.containment))
}
pub(super) fn capture_deadline() -> Result<Instant> {
    check()?;
    SCOPE.with_borrow(|scope| {
        Ok(effective(
            scope.as_ref().context("status requires managed deadline")?,
        ))
    })
}
#[derive(Clone)]
pub(super) struct ObserverContext {
    pub origin: Instant,
    pub whole: Instant,
    pub run: PathBuf,
}
pub(super) fn observer_context() -> Result<ObserverContext> {
    check()?;
    SCOPE.with_borrow(|s| {
        let s = s.as_ref().context("observer requires original deadline")?;
        Ok(ObserverContext {
            origin: s.origin,
            whole: s.deadline,
            run: s.run.clone(),
        })
    })
}
pub(super) fn output(command: &mut Command) -> Result<Output> {
    let class = match command.get_args().nth(1).and_then(|a| a.to_str()) {
        Some("history" | "history3") => "history_cli",
        Some("send" | "send3") => "send_cli",
        _ => "cli",
    };
    let _measurement = measure(class);
    check()?;
    let bound = SCOPE.with_borrow_mut(|scope| {
        scope.as_mut().map(|s| {
            let n = s.next;
            s.next += 1;
            (
                effective(s),
                s.run.join(format!("command-{n:04}.stdout.log")),
                s.run.join(format!("command-{n:04}.stderr.log")),
            )
        })
    });
    let Some((deadline, out, err)) = bound else {
        return Ok(command.output()?);
    };
    // File-backed output cannot deadlock on a full Windows pipe while the
    // parent polls its own child and deadline. No helper shell is spawned.
    command
        .stdout(Stdio::from(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&out)?,
        ))
        .stderr(Stdio::from(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&err)?,
        ));
    let mut child = OwnedChild(command.spawn()?);
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        ensure!(Instant::now() < deadline, "absolute CLI deadline");
        std::thread::sleep(Duration::from_millis(15));
    };
    ensure!(
        std::fs::metadata(&out)?.len() <= 1024 * 1024
            && std::fs::metadata(&err)?.len() <= 1024 * 1024,
        "bounded CLI logs"
    );
    let output = Output {
        status,
        stdout: std::fs::read(out)?,
        stderr: std::fs::read(err)?,
    };
    // A child observed as exited on the final polling tick must not turn an
    // expired local phase into success. Legacy callers retain their old path.
    if SCOPE.with_borrow(|s| s.as_ref().is_some_and(|s| s.local.is_some())) {
        check()?;
    }
    Ok(output)
}

#[cfg(test)]
fn test_scope<T>(f: impl FnOnce(&mut Scope) -> T) -> Result<T> {
    SCOPE.with_borrow_mut(|s| Ok(f(s.as_mut().context("pure test managed scope")?)))
}
#[test]
fn b_start_deadline_nested_minimum_restores_without_reset() -> Result<()> {
    let _managed = Guard::enter(Path::new("unused-pure-deadline-test"), 60)?;
    let absolute = test_scope(|s| s.deadline)?;
    test_scope(|s| s.next = 17)?;
    let local = Instant::now() + Duration::from_secs(20);
    {
        let _outer = Phase::until(local)?;
        {
            let _longer = Phase::until(absolute + Duration::from_secs(30))?;
            assert_eq!(test_scope(|s| effective(s))?, local);
            let short = Instant::now() + Duration::from_secs(5);
            {
                let _inner = Phase::until(short)?;
                assert_eq!(test_scope(|s| effective(s))?, short);
            }
            assert_eq!(test_scope(|s| effective(s))?, local);
        }
    }
    test_scope(|s| {
        assert_eq!(effective(s), absolute);
        assert_eq!(s.next, 17);
    })?;
    let _cannot_extend = Phase::until(absolute + Duration::from_secs(30))?;
    assert_eq!(test_scope(|s| effective(s))?, absolute);
    Ok(())
}

#[test]
fn b_start_deadline_expired_bound_cannot_spawn_or_revive() -> Result<()> {
    assert!(Phase::until(Instant::now() + Duration::from_secs(1)).is_err());
    let _managed = Guard::enter(Path::new("unused-pure-deadline-test"), 60)?;
    assert!(Phase::until(Instant::now()).is_err());
    check()?;
    let outer = Phase::until(Instant::now() + Duration::from_secs(1))?;
    test_scope(|s| s.deadline = Instant::now())?;
    assert!(check().is_err());
    assert!(output(&mut Command::new("must-not-spawn-b-start-test")).is_err());
    assert_eq!(test_scope(|s| s.next)?, 0);
    drop(outer);
    assert!(check().is_err());
    assert!(Phase::until(Instant::now() + Duration::from_secs(60)).is_err());
    Ok(())
}
#[test]
fn s54_original_scope_charges_startup_and_preserves_observer_and_containment_origins() -> Result<()>
{
    let origin = Instant::now() - Duration::from_secs(12);
    let _guard = Guard::enter_original(Path::new("unused-original-clock-public-test"), origin)?;
    let context = observer_context()?;
    ensure!(
        context.origin == origin && context.whole == origin + Duration::from_secs(2350),
        "same original observer/controller bound"
    );
    let capture = capture_bound()?;
    ensure!(
        capture.work == context.whole && capture.whole == origin + Duration::from_secs(2400),
        "capture cleanup stays inside original outer bound"
    );
    ensure!(
        original_containment() == Some(capture.whole),
        "explicit S54-only containment"
    );
    Ok(())
}
#[test]
fn s54_original_scope_expiry_does_not_create_a_new_budget() -> Result<()> {
    let now = Instant::now();
    ensure!(
        Guard::enter_original(Path::new("unused"), now + Duration::from_secs(1)).is_err(),
        "no future origin"
    );
    ensure!(
        Guard::enter_original(Path::new("unused"), now - Duration::from_secs(2351)).is_err(),
        "late startup cannot revive controller"
    );
    let _legacy = Guard::enter(Path::new("unused"), 60)?;
    ensure!(
        original_containment().is_none(),
        "legacy constructor behavior remains separate"
    );
    Ok(())
}

#[test]
fn s54_f_original_clock_preserves_elapsed_start_and_fixed_containment() -> Result<()> {
    let origin = Instant::now() - Duration::from_secs(7);
    let _guard = Guard::enter_original_feasibility(Path::new("unused-pure-F-clock"), origin)?;
    let context = observer_context()?;
    let bound = capture_bound()?;
    ensure!(
        context.origin == origin
            && context.whole == origin + Duration::from_secs(450)
            && bound.whole == origin + Duration::from_secs(510),
        "F450/510 from original launch, no reset"
    );
    Ok(())
}

#[test]
fn s54_f_timing_is_actual_bounded_and_does_not_enable_the_legacy_scope() -> Result<()> {
    {
        let _guard = Guard::enter(Path::new("unused-pure-duration"), 60)?;
        {
            let _span = measure("pure");
        }
        ensure!(
            feasibility_timings().is_err(),
            "legacy scope has no F observations"
        );
    }
    let _guard =
        Guard::enter_original_feasibility(Path::new("unused-pure-F-duration"), Instant::now())?;
    {
        let _span = measure("pure");
    }
    let actual = feasibility_timings()?;
    ensure!(
        actual["classes"]["pure"]["count"] == 1
            && actual["classes"]["pure"]["last_end_us"]
                .as_u64()
                .context("actual timestamp")?
                >= actual["classes"]["pure"]["first_start_us"]
                    .as_u64()
                    .context("actual timestamp")?,
        "actual measured class span"
    );
    Ok(())
}
