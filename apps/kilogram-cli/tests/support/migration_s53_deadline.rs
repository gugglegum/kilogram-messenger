//! An opt-in absolute budget for the new managed process controllers. Legacy
//! callers retain their old behavior when this exact-test scope is absent.
use super::*;
use std::{cell::RefCell, process::Output};
struct Scope {
    deadline: Instant,
    run: PathBuf,
    next: u64,
}
thread_local! {static SCOPE:RefCell<Option<Scope>>=const {RefCell::new(None)};}
pub(super) struct Guard;
impl Guard {
    pub(super) fn enter(run: &Path, seconds: u64) -> Result<Self> {
        SCOPE.with_borrow_mut(|scope| {
            ensure!(scope.is_none(), "one absolute managed controller scope");
            *scope = Some(Scope {
                deadline: Instant::now() + Duration::from_secs(seconds),
                run: run.into(),
                next: 0,
            });
            Ok(Self)
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        SCOPE.with_borrow_mut(|scope| *scope = None);
    }
}
pub(super) fn check() -> Result<()> {
    SCOPE.with_borrow(|scope| {
        ensure!(
            scope.as_ref().is_none_or(|s| Instant::now() < s.deadline),
            "absolute S5.3 process controller deadline"
        );
        Ok(())
    })
}
pub(super) fn output(command: &mut Command) -> Result<Output> {
    check()?;
    let bound = SCOPE.with_borrow_mut(|scope| {
        scope.as_mut().map(|s| {
            let n = s.next;
            s.next += 1;
            (
                s.deadline,
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
    Ok(Output {
        status,
        stdout: std::fs::read(out)?,
        stderr: std::fs::read(err)?,
    })
}
