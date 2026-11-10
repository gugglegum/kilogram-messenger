//! A single trusted bootstrap child, not a general command supervisor.
use anyhow::{Result, bail, ensure};
use kilogram_bootstrap_contract::Ordered2BootstrapOutput;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
use zeroize::Zeroizing;

const CAP: usize = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(60);

async fn drain(mut pipe: impl AsyncRead + Unpin) -> Result<Zeroizing<Vec<u8>>> {
    // Allocate once: growth must not leave an old secret allocation behind.
    let mut bytes = Zeroizing::new(Vec::with_capacity(CAP));
    let mut chunk = Zeroizing::new([0u8; 4096]);
    loop {
        let n = pipe
            .read(&mut *chunk)
            .await
            .map_err(|_| anyhow::anyhow!("Helper pipe failed"))?;
        if n == 0 {
            return Ok(bytes);
        }
        ensure!(
            bytes.len() + n <= CAP,
            "Helper output exceeded its safety limit"
        );
        bytes.extend_from_slice(&chunk[..n]);
    }
}

async fn collect(
    mut command: Command,
    cancel: Arc<AtomicBool>,
    limit: Duration,
) -> Result<Zeroizing<Vec<u8>>> {
    use std::process::Stdio;
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Account creation cancelled"
    );
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_4000); // no console, below normal
    let mut child = command
        .spawn()
        .map_err(|_| anyhow::anyhow!("Cannot start the trusted account helper"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Missing helper pipe"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Missing helper pipe"))?;
    let result = {
        let finish = async {
            let (out, _err, status) = tokio::try_join!(drain(stdout), drain(stderr), async {
                child
                    .wait()
                    .await
                    .map_err(|_| anyhow::anyhow!("Cannot wait for account helper"))
            })?;
            ensure!(
                status.success(),
                "Account helper failed; its private output was omitted"
            );
            Ok(out)
        };
        tokio::pin!(finish);
        let watchdog = tokio::time::sleep(limit);
        tokio::pin!(watchdog);
        loop {
            tokio::select! {
                result = &mut finish => break result,
                _ = &mut watchdog => break Err(anyhow::anyhow!("Account helper timed out")),
                _ = tokio::time::sleep(Duration::from_millis(20)) => {
                    if cancel.load(Ordering::Acquire) { break Err(anyhow::anyhow!("Account creation cancelled")); }
                }
            }
        }
    }; // drop pipe futures before cleanup, including cancelled readers
    if result.is_err() {
        let _ = child.start_kill();
        match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
            Ok(Ok(_)) => (),
            _ => bail!("Account helper cleanup could not be verified"),
        }
    }
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Account creation cancelled"
    );
    result
}

pub(super) fn run(
    exe: &Path,
    workspace: &Path,
    contexts: &[String; 2],
    cancel: Arc<AtomicBool>,
) -> Result<Ordered2BootstrapOutput> {
    let mut command = Command::new(exe);
    command
        .arg("create-ordered2")
        .arg("--workspace-dir")
        .arg(workspace)
        .arg("--context")
        .args(contexts);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let bytes = runtime.block_on(collect(command, cancel.clone(), DEADLINE))?;
    let output = Ordered2BootstrapOutput::decode(&bytes)
        .map_err(|_| anyhow::anyhow!("Invalid private account-helper response"))?;
    ensure!(
        output.public().workspace_dir == workspace && output.public().contexts == *contexts,
        "Account helper returned different setup bindings"
    );
    ensure!(
        !cancel.load(Ordering::Acquire),
        "Account creation cancelled"
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn cs31_producer() -> Result<()> {
        let Ok(mode) = std::env::var("KILOGRAM_CS31_PRODUCER") else {
            return Ok(());
        };
        match mode.as_str() {
            "out" => {
                let _ = std::io::stdout().write_all(&[b'x'; CAP + 4096]);
            }
            "err" => {
                let _ = std::io::stderr().write_all(&[b'x'; CAP + 4096]);
            }
            "both" => {
                for _ in 0..12 {
                    std::io::stdout().write_all(&[b'x'; 2048])?;
                    std::io::stderr().write_all(&[b'y'; 2048])?;
                }
            }
            "stall" => std::thread::sleep(Duration::from_secs(30)),
            "fail" => {
                std::io::stderr().write_all(b"TEST_PRIVATE_MARKER")?;
                std::process::exit(23);
            }
            _ => bail!("unknown synthetic producer"),
        }
        Ok(())
    }
    #[test]
    fn cs31_helper_caps_drain_timeout_cancel_and_wait() -> Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        for mode in ["out", "err", "both", "stall", "cancel", "fail"] {
            let mut cmd = Command::new(std::env::current_exe()?); // fixed stable harness path
            cmd.args([
                "--exact",
                "fresh_onboarding::helper::tests::cs31_producer",
                "--nocapture",
            ])
            .env(
                "KILOGRAM_CS31_PRODUCER",
                if mode == "cancel" { "stall" } else { mode },
            );
            let cancel = Arc::new(AtomicBool::new(false));
            let signal = cancel.clone();
            let result = runtime.block_on(async {
                let trigger = async {
                    if mode == "cancel" {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        signal.store(true, Ordering::Release);
                    }
                };
                let (result, _) = tokio::join!(
                    collect(
                        cmd,
                        cancel,
                        if mode == "stall" {
                            Duration::from_millis(300)
                        } else {
                            Duration::from_secs(10)
                        }
                    ),
                    trigger
                );
                result
            });
            if mode == "both" {
                ensure!(result?.len() > 24 * 1024, "both pipes actually drained");
            } else {
                let error = result
                    .err()
                    .ok_or_else(|| anyhow::anyhow!("synthetic producer unexpectedly passed"))?
                    .to_string();
                let expected = match mode {
                    "out" | "err" => "exceeded",
                    "stall" => "timed out",
                    "cancel" => "cancelled",
                    "fail" => "private output was omitted",
                    _ => "unreachable",
                };
                ensure!(
                    error.contains(expected)
                        && !error.contains("TEST_PRIVATE_MARKER")
                        && !error.contains("xxxx")
                        && !error.contains("cleanup"),
                    "sanitized error and verified child wait"
                );
            }
        }
        Ok(())
    }
}
