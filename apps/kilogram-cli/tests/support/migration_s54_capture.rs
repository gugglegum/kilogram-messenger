//! Explicit S54-only status capture. Original JSON never goes through cli's
//! raw Output/error path. Outside this scope the legacy behavior is unchanged.
use super::*;
use std::{cell::RefCell, fs::File, io::Write, marker::PhantomData, rc::Rc};
use zeroize::Zeroizing;
#[path = "migration_s54_capture_admission.rs"]
mod admission;
#[cfg(windows)]
#[path = "migration_s54_capture_pipe.rs"]
mod pipe;
#[cfg(test)]
#[path = "migration_s54_capture_tests.rs"]
mod tests;

const CAP: usize = 1024 * 1024;
struct Scope {
    until: Instant,
    whole: Instant,
    admission: admission::Admission,
    failed: bool,
}
thread_local! {static CAPTURE: RefCell<Option<Scope>> = const { RefCell::new(None) };}
pub(super) struct Guard(PhantomData<Rc<()>>);
impl Guard {
    pub(super) fn enter(until: Instant) -> Result<Self> {
        ensure!(!active(), "capture scope cannot reenter");
        let until = until.min(s53_deadline::capture_deadline()?);
        let (admission, whole) = admission::Admission::load(until)?;
        Self::admit(until, whole, admission)
    }
    fn admit(until: Instant, whole: Instant, admission: admission::Admission) -> Result<Self> {
        CAPTURE.with_borrow_mut(|scope| {
            ensure!(
                scope.is_none() && Instant::now() < until,
                "capture scope admission"
            );
            *scope = Some(Scope {
                until,
                whole,
                admission,
                failed: false,
            });
            Ok(Self(PhantomData))
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        CAPTURE.with_borrow_mut(|scope| *scope = None);
    }
}
fn active() -> bool {
    CAPTURE.with_borrow(|scope| scope.is_some())
}
fn poison() {
    CAPTURE.with_borrow_mut(|scope| {
        if let Some(scope) = scope {
            scope.failed = true;
        }
    });
}
fn healthy() -> Result<()> {
    CAPTURE.with_borrow(|scope| {
        ensure!(
            scope.as_ref().is_some_and(|s| !s.failed),
            "capture scope failed or absent"
        );
        Ok(())
    })
}
fn is_status(args: &[String]) -> bool {
    args.first().is_some_and(|s| s == "ordered")
        && args.get(1).is_some_and(|s| s == "status" || s == "status3")
}
pub(super) fn intercept(exe: &Path, args: &[String]) -> Option<Result<Value>> {
    if !active() || !is_status(args) {
        return None;
    }
    Some((|| {
        ensure!(
            args.len() == 4 && args[2] == "--ipc-file",
            "capture status argument schema"
        );
        let mut cmd = command(exe);
        cmd.args(args);
        run(cmd)
    })())
}

#[derive(Default, serde::Serialize)]
struct Facts {
    classification: &'static str,
    pid: Option<u32>,
    code: Option<i32>,
    exit_observed: bool,
    waited: bool,
    stdout_bytes: usize,
    stderr_bytes: usize,
    stdout_eof: bool,
    stderr_eof: bool,
    kill_requested: bool,
    kill_error: Option<i32>,
    cleanup_pending: bool,
    io_error: Option<i32>,
    parse_line: Option<usize>,
    parse_column: Option<usize>,
}
impl Facts {
    fn unresolved(&mut self) {
        self.cleanup_pending = !self.waited;
        if self.cleanup_pending {
            poison();
        }
    }
}
struct Logs {
    out: File,
    err: File,
    sequence: u64,
    attempt: String,
    owner: u32,
}
impl Logs {
    fn new(bound: &s53_deadline::CaptureBound) -> Result<Self> {
        let open = |path: &Path| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
        };
        let out = open(&bound.out).map_err(|_| anyhow::anyhow!("capture stdout create_new"))?;
        let err = open(&bound.err).map_err(|_| anyhow::anyhow!("capture metadata create_new"))?;
        let (attempt, owner) = CAPTURE.with_borrow(|s| {
            let s = s.as_ref().context("capture logs scope")?;
            Ok::<_, anyhow::Error>((s.admission.attempt.clone(), s.admission.owner_pid))
        })?;
        Ok(Self {
            out,
            err,
            sequence: bound.sequence,
            attempt,
            owner,
        })
    }
    fn facts(&mut self, facts: &Facts) -> Result<()> {
        let raw = serde_json::to_vec(&json!({"sequence":self.sequence,"attempt":self.attempt,
            "containment_owner_pid":self.owner,"harness_pid":std::process::id(),"capture":facts}))
        .map_err(|_| anyhow::anyhow!("capture metadata serialization"))?;
        self.err
            .write_all(&raw)
            .map_err(|_| anyhow::anyhow!("capture metadata write"))?;
        self.err
            .write_all(b"\n")
            .map_err(|_| anyhow::anyhow!("capture metadata write"))
    }
}
fn transform(
    raw: &[u8],
    facts: &mut Facts,
    sanitize: impl FnOnce(&mut Value) -> Result<()>,
) -> Result<(Value, Vec<u8>)> {
    if std::str::from_utf8(raw).is_err() {
        facts.classification = "invalid_utf8";
        anyhow::bail!("capture invalid_utf8");
    }
    let original: Value = serde_json::from_slice(raw).map_err(|e| {
        facts.classification = "invalid_json";
        facts.parse_line = Some(e.line());
        facts.parse_column = Some(e.column());
        anyhow::anyhow!("capture invalid_json")
    })?;
    let mut public = original.clone();
    sanitize(&mut public).map_err(|_| {
        facts.classification = "sanitizer_failure";
        anyhow::anyhow!("capture sanitizer_failure")
    })?;
    let durable = serde_json::to_vec(&public)
        .map_err(|_| anyhow::anyhow!("capture sanitized serialization"))?;
    ensure!(durable.len() <= CAP, "capture sanitized output cap");
    Ok((original, durable))
}
fn run(mut command: Command) -> Result<Value> {
    run_kind(&mut command, Kind::Status)
}
enum Kind {
    Status,
    Memory(u32, Instant),
}
/// Closed internal memory query, not an arbitrary command/script interface.
pub(super) fn memory_command(pid: u32) -> String {
    format!(
        "$ErrorActionPreference='Stop';$p=[Diagnostics.Process]::GetProcessById({pid});try{{$h=$p.Handle;$t=$p.StartTime.ToUniversalTime().Ticks;$p.Refresh();$b=$p.PrivateMemorySize64;[Console]::Write(('{{\"pid\":'+$p.Id.ToString([Globalization.CultureInfo]::InvariantCulture)+',\"ticks\":'+$t.ToString([Globalization.CultureInfo]::InvariantCulture)+',\"bytes\":'+$b.ToString([Globalization.CultureInfo]::InvariantCulture)+'}}'))}}finally{{$p.Dispose()}}"
    )
}
pub(super) fn memory_query(shell: &Path, pid: u32, until: Instant) -> Result<Value> {
    ensure!(pid > 0, "memory owned PID");
    let script = memory_command(pid);
    let mut cmd = command(shell);
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    run_kind(&mut cmd, Kind::Memory(pid, until))
}
fn run_kind(command: &mut Command, kind: Kind) -> Result<Value> {
    healthy()?;
    let mut bound = s53_deadline::capture_bound()?;
    CAPTURE.with_borrow(|s| {
        let s = s.as_ref().context("capture bound scope")?;
        bound.work = bound.work.min(s.until);
        bound.whole = bound.whole.min(s.whole);
        Ok::<_, anyhow::Error>(())
    })?;
    // Reserve cleanup inside the already established whole bound, never extend it.
    bound.work = bound.work.min(
        bound
            .whole
            .checked_sub(Duration::from_secs(2))
            .context("capture cleanup reserve")?,
    );
    let ceiling = match kind {
        Kind::Status => CAP,
        Kind::Memory(_, until) => {
            bound.work = bound.work.min(until);
            4096
        }
    };
    let mut logs = Logs::new(&bound)?;
    let mut facts = Facts {
        classification: "capture_setup",
        ..Default::default()
    };
    #[cfg(windows)]
    let captured = pipe::capture(command, &bound, &mut facts, &mut logs, ceiling);
    #[cfg(not(windows))]
    let captured: Result<Zeroizing<Vec<u8>>> = Err(anyhow::anyhow!("capture Windows-only"));
    let result = captured.and_then(|raw| {
        ensure!(
            Instant::now() < bound.work,
            "capture deadline before transform"
        );
        let (original, durable) = match kind {
            Kind::Status => transform(&raw, &mut facts, live_l1::status_fingerprints)?,
            Kind::Memory(pid, _) => {
                let value = s54_memory::parse_memory(&raw, pid).map_err(|_| {
                    facts.classification = "memory_schema";
                    anyhow::anyhow!("memory query schema")
                })?;
                let safe = serde_json::to_vec(&value)?;
                (value, safe)
            }
        };
        ensure!(
            Instant::now() < bound.work,
            "capture deadline before durable write"
        );
        logs.out.write_all(&durable).map_err(|_| {
            facts.classification = "sanitized_write_failure";
            anyhow::anyhow!("capture sanitized_write_failure")
        })?;
        facts.classification = "success";
        Ok(original)
    });
    if result.is_err() {
        poison();
    }
    if logs.facts(&facts).is_err() {
        poison();
        anyhow::bail!("capture metadata_write_failure");
    }
    result.map_err(|_| {
        anyhow::anyhow!(
            "capture {} cleanup_pending={}",
            facts.classification,
            facts.cleanup_pending
        )
    })
}
