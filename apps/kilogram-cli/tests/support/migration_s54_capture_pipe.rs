//! Windows overlapped pipe servers; no reader task, blocking adapter or spool.
use super::*;
use std::{
    io::ErrorKind,
    sync::atomic::{AtomicU64, Ordering},
};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

struct StatusChild<'a> {
    child: Child,
    cleanup_until: Instant,
    facts: &'a mut Facts,
    logs: &'a mut Logs,
    settled: bool,
}
impl StatusChild<'_> {
    fn observe(&mut self) -> Result<bool> {
        match self.child.try_wait() {
            Ok(Some(exit)) => {
                self.facts.exit_observed = true;
                self.facts.code = exit.code();
                // try_wait cached the confirmed exit: this wait cannot wait on
                // a running process and explicitly completes handle ownership.
                match self.child.wait() {
                    Ok(waited) => {
                        self.facts.waited = true;
                        self.facts.code = waited.code();
                        Ok(true)
                    }
                    Err(e) => {
                        self.facts.io_error = e.raw_os_error();
                        anyhow::bail!("capture confirmed-exit wait");
                    }
                }
            }
            Ok(None) => Ok(false),
            Err(e) => {
                self.facts.io_error = e.raw_os_error();
                anyhow::bail!("capture exit observation");
            }
        }
    }
    fn terminate(&mut self) {
        if self.facts.waited {
            return;
        }
        self.facts.kill_requested = true;
        if let Err(e) = self.child.kill() {
            self.facts.kill_error = e.raw_os_error();
        }
        while Instant::now() < self.cleanup_until {
            if self.observe().is_ok_and(|done| done) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.facts.unresolved();
    }
}
impl Drop for StatusChild<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.cleanup_until = self
                .cleanup_until
                .min(Instant::now() + Duration::from_secs(2));
            self.terminate();
            self.facts.classification = "capture_unwind_or_constructor_failure";
            poison();
            // Only safe bounded facts, even if the ordinary return writer is
            // bypassed by unwinding. The ancestor still owns job containment.
            let _ = self.logs.facts(self.facts);
        }
    }
}
fn server() -> Result<(NamedPipeServer, File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let name = format!(r"\\.\pipe\kilogram-s54-capture-{}-{n}", std::process::id());
    let server = ServerOptions::new()
        .access_inbound(true)
        .access_outbound(false)
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .max_instances(1)
        .create(&name)
        .map_err(|_| anyhow::anyhow!("capture pipe create"))?;
    // The unique, just-created local instance is available; do not call
    // WaitNamedPipe or retry an occupied instance with an unbounded wait.
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&name)
        .map_err(|_| anyhow::anyhow!("capture local pipe writer open"))?;
    Ok((server, writer))
}
fn read(
    pipe: &NamedPipeServer,
    buffer: &mut [u8],
    bytes: &mut usize,
    eof: &mut bool,
    mut retain: Option<&mut Vec<u8>>,
    ceiling: Option<usize>,
) -> Result<()> {
    if *eof {
        return Ok(());
    }
    match pipe.try_read(buffer) {
        Ok(0) => *eof = true,
        Ok(n) => {
            *bytes = bytes.saturating_add(n);
            ensure!(
                ceiling.is_none_or(|cap| *bytes <= cap),
                "capture stream_cap"
            );
            if let Some(retain) = retain.as_mut() {
                retain.extend_from_slice(&buffer[..n]);
            }
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock => (),
        Err(e) if e.kind() == ErrorKind::BrokenPipe || e.raw_os_error() == Some(109) => *eof = true,
        Err(_) => anyhow::bail!("capture pipe_read"),
    }
    Ok(())
}
async fn collect(
    command: &mut Command,
    bound: &s53_deadline::CaptureBound,
    facts: &mut Facts,
    logs: &mut Logs,
    ceiling: usize,
) -> Result<Zeroizing<Vec<u8>>> {
    ensure!(
        Instant::now() < bound.work,
        "capture expired before pipe setup"
    );
    let (out, writer_out) = server()?;
    let (err, writer_err) = server()?;
    let connect = async {
        out.connect().await?;
        err.connect().await?;
        Ok::<_, std::io::Error>(())
    };
    tokio::time::timeout_at(bound.work.into(), connect)
        .await
        .map_err(|_| anyhow::anyhow!("capture pipe_connect_deadline"))?
        .map_err(|_| anyhow::anyhow!("capture pipe_connect"))?;
    let mut stdout = Zeroizing::new(Vec::with_capacity(ceiling));
    let mut chunk = Zeroizing::new([0u8; 8192]);
    ensure!(Instant::now() < bound.work, "capture expired before spawn");
    use std::os::windows::process::CommandExt;
    // No CREATE_BREAKAWAY_FROM_JOB; containment is inherited from the admitted
    // ancestor. Override flags here, rather than trusting a mutable caller.
    command
        .creation_flags(0x0800_4000)
        .stdin(Stdio::null())
        .stdout(Stdio::from(writer_out))
        .stderr(Stdio::from(writer_err));
    let spawned = command.spawn();
    // Command owns parent copies too. Closing these is necessary for real EOF.
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let child = spawned.map_err(|e| {
        facts.io_error = e.raw_os_error();
        facts.classification = "spawn_failure";
        anyhow::anyhow!("capture spawn_failure")
    })?;
    facts.pid = Some(child.id());
    let mut owned = StatusChild {
        child,
        cleanup_until: bound.whole,
        facts,
        logs,
        settled: false,
    };
    let result = async {
        loop {
            if Instant::now() >= bound.work {
                owned.facts.classification = "deadline";
                anyhow::bail!("capture deadline");
            }
            let f = &mut owned.facts;
            if read(
                &out,
                &mut *chunk,
                &mut f.stdout_bytes,
                &mut f.stdout_eof,
                Some(&mut stdout),
                Some(ceiling),
            )
            .is_err()
            {
                f.classification = if f.stdout_bytes > ceiling {
                    "stdout_cap"
                } else {
                    "stdout_read"
                };
                anyhow::bail!("capture stdout read or cap");
            }
            if read(
                &err,
                &mut *chunk,
                &mut f.stderr_bytes,
                &mut f.stderr_eof,
                None,
                Some(ceiling),
            )
            .is_err()
            {
                f.classification = if f.stderr_bytes > ceiling {
                    "stderr_cap"
                } else {
                    "stderr_read"
                };
                anyhow::bail!("capture stderr read or cap");
            }
            if owned.observe()? && owned.facts.stdout_eof && owned.facts.stderr_eof {
                break;
            }
            // Yield to this runtime's I/O driver on every fair bounded pass,
            // including continuous output. No busy wait or spawned reader.
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        if owned.facts.code != Some(0) {
            owned.facts.classification = "nonzero_exit";
            anyhow::bail!("capture nonzero_exit");
        }
        Ok(())
    }
    .await;
    if result.is_err() {
        owned.cleanup_until = bound.whole.min(Instant::now() + Duration::from_secs(2));
        owned.terminate();
        // Discard only; never retain over-cap bytes while collecting real EOF.
        while Instant::now() < owned.cleanup_until
            && !(owned.facts.stdout_eof && owned.facts.stderr_eof)
        {
            let f = &mut owned.facts;
            if read(
                &out,
                &mut *chunk,
                &mut f.stdout_bytes,
                &mut f.stdout_eof,
                None,
                None,
            )
            .is_err()
                || read(
                    &err,
                    &mut *chunk,
                    &mut f.stderr_bytes,
                    &mut f.stderr_eof,
                    None,
                    None,
                )
                .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
    owned.facts.unresolved();
    owned.settled = true;
    result?;
    Ok(stdout)
}
pub(super) fn capture(
    command: &mut Command,
    bound: &s53_deadline::CaptureBound,
    facts: &mut Facts,
    logs: &mut Logs,
    ceiling: usize,
) -> Result<Zeroizing<Vec<u8>>> {
    let mut run = || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(|_| anyhow::anyhow!("capture io runtime"))?;
        runtime.block_on(collect(command, bound, facts, logs, ceiling))
    };
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(run)
        }
        Ok(_) => anyhow::bail!("capture requires blocking context or multithread runtime"),
        Err(_) => run(),
    }
}
