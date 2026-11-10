//! F-only bounded reads. Busy is admission failure, never a history result.
use super::*;
use std::process::Output;

#[cfg(test)]
#[path = "migration_s54_history_reader_tests.rs"]
mod tests;

const BUSY: &[u8] = b"Error: SchedulingWindowBusy\n";
const PAGE_CAP: usize = 48 * 1024;
enum Attempt {
    Page(Value),
    Busy,
    Terminal(&'static str),
}
fn classify(output: Output) -> Attempt {
    if !output.status.success() {
        return if output.stdout.is_empty() && output.stderr == BUSY {
            Attempt::Busy
        } else {
            Attempt::Terminal("command_failed")
        };
    }
    if output.stdout.len() > PAGE_CAP {
        return Attempt::Terminal("page_cap");
    }
    match serde_json::from_slice(&output.stdout) {
        Ok(page) => Attempt::Page(page),
        Err(_) => Attempt::Terminal("invalid_page"),
    }
}

fn cli_page(
    exe: &Path,
    ipc: &Path,
    slot: u8,
    limit: usize,
    cursor: Option<&str>,
) -> Result<Output> {
    ensure!(slot < 2 && (1..=32).contains(&limit), "F history request");
    let mut cmd = command(exe);
    cmd.args(["ordered", "history3", "--ipc-file"])
        .arg(ipc)
        .args(["--slot", &slot.to_string(), "--limit", &limit.to_string()]);
    if let Some(cursor) = cursor {
        cmd.args(["--cursor", cursor]);
    }
    s53_deadline::output(&mut cmd).map_err(|_| anyhow::anyhow!("F history command failure"))
}

struct Reader<A, P, N, Y> {
    until: Instant,
    attempt: A,
    progress: P,
    now: N,
    yield_for: Y,
}
impl<A, P, N, Y> Reader<A, P, N, Y>
where
    A: FnMut(usize, Option<&str>) -> Result<Output>,
    P: FnMut() -> Result<()>,
    N: FnMut() -> Instant,
    Y: FnMut(Duration),
{
    fn check(&mut self) -> Result<()> {
        (self.progress)()?;
        ensure!((self.now)() < self.until, "F history deadline");
        Ok(())
    }
    fn page(&mut self, limit: usize, cursor: Option<&str>) -> Result<Value> {
        // The same immutable slot/limit/cursor is used on every retry.
        loop {
            self.check()?;
            let output = (self.attempt)(limit, cursor);
            self.check()?;
            match classify(output?) {
                Attempt::Page(page) => {
                    self.check()?;
                    return Ok(page);
                }
                Attempt::Terminal(reason) => anyhow::bail!("F history {reason}"),
                Attempt::Busy => {
                    self.check()?;
                    let remaining = self.until.saturating_duration_since((self.now)());
                    (self.yield_for)(remaining.min(Duration::from_millis(25)));
                    self.check()?;
                }
            }
        }
    }
}

pub(super) fn operation<T>(
    controller: &mut control::Controller,
    owner: usize,
    context: usize,
    work: impl FnOnce(&mut dyn FnMut(usize, Option<&str>) -> Result<Value>) -> Result<T>,
) -> Result<T> {
    let node = controller.view(owner, context)?;
    let _phase = s53_deadline::Phase::until(
        controller
            .until
            .min(Instant::now() + Duration::from_secs(30)),
    )?;
    let until = s53_deadline::capture_deadline()?;
    let mut reader = Reader {
        until,
        attempt: |limit, cursor: Option<&str>| {
            cli_page(&node.exe(), &node.ipc, node.slot, limit, cursor)
        },
        progress: || controller.check(),
        now: Instant::now,
        yield_for: std::thread::sleep,
    };
    let result = {
        let mut read = |limit, cursor: Option<&str>| reader.page(limit, cursor);
        work(&mut read)
    };
    reader.check()?;
    result
}

pub(super) fn history(
    controller: &mut control::Controller,
    owner: usize,
    context: usize,
) -> Result<Vec<Value>> {
    operation(controller, owner, context, |read| {
        early::history_with(|cursor| read(32, cursor))
    })
}
