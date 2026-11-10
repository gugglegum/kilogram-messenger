//! B uses scripted IPC replies and the actual CLI; C injects scheduling only.
//! Neither is an Owner7/storage/native/F feasibility proof.
use super::*;
use kilogram_identity::{AccountId, DeviceIdentity};
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse, RuntimeIpcServer};
use std::{cell::Cell, sync::mpsc, thread::JoinHandle};
use tokio::sync::oneshot;

#[derive(Clone)]
enum Reply {
    Busy,
    Error,
    Json(Vec<u8>),
}
struct Step {
    limit: u64,
    cursor: Option<Vec<u8>>,
    reply: Reply,
}
#[derive(Default)]
struct Facts {
    requests: usize,
    busy: usize,
    pages: usize,
    errors: usize,
}
struct Peer {
    stop: Option<oneshot::Sender<()>>,
    join: Option<JoinHandle<Result<Facts>>>,
    ipc: PathBuf,
}
impl Peer {
    fn start(ipc: PathBuf, steps: Vec<Step>, repeat_busy: bool, until: Instant) -> Result<Self> {
        let (ready, started) = mpsc::sync_channel(1);
        let (stop, stopped) = oneshot::channel();
        let path = ipc.clone();
        let join = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let result = rt.block_on(async move {
                let identity = DeviceIdentity::generate()?;
                let server = RuntimeIpcServer::start(path, AccountId::from_bytes([73; 32]), &identity).await;
                let (server, mut work) = match server {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return Ok(Facts::default());
                    }
                };
                let _ = ready.send(Ok(()));
                let mut stopped = stopped;
                let mut facts = Facts::default();
                let result = async {
                    loop {
                        let item = tokio::select! {
                            _ = &mut stopped => break,
                            _ = tokio::time::sleep_until(until.into()) => anyhow::bail!("scripted peer deadline"),
                            item = work.recv() => item.context("scripted peer closed")?,
                        };
                        let (command, response) = item.into_parts();
                        let RuntimeIpcCommand::OrderedDialog { request } = command else {
                            anyhow::bail!("scripted peer command");
                        };
                        let value = old::decode(&request, 32 * 1024)?;
                        let fields = value.fields(5)?;
                        ensure!(fields[0].uint()? == 3 && fields[1].uint()? == 4 && fields[2].uint()? == 1, "scripted IPC version/tag/slot");
                        let step = steps.get(facts.requests).or_else(|| repeat_busy.then(|| steps.last()).flatten()).context("unexpected scripted request")?;
                        ensure!(facts.requests < 128 && fields[4].uint()? == step.limit, "scripted IPC count/limit");
                        let cursor = fields[3].option()?.map(|v| v.bytes().map(Vec::from)).transpose()?;
                        ensure!(cursor == step.cursor, "scripted IPC cursor unchanged");
                        facts.requests += 1;
                        let reply = match &step.reply {
                            Reply::Busy => {
                                facts.busy += 1;
                                RuntimeIpcResponse::Error { message: "SchedulingWindowBusy".into() }
                            }
                            Reply::Error => {
                                facts.errors += 1;
                                RuntimeIpcResponse::Error { message: "HistoryUnavailable".into() }
                            }
                            Reply::Json(page) => {
                                facts.pages += 1;
                                RuntimeIpcResponse::OrderedDialog { response: old::encode(&old::Value::A(vec![old::Value::U(3), old::Value::U(4), old::Value::B(page.clone())]), 48 * 1024)? }
                            }
                        };
                        ensure!(response.send(reply).is_ok(), "scripted IPC response receiver");
                    }
                    Ok(())
                }.await;
                let shutdown = server.shutdown().await;
                drop(work);
                result?;
                shutdown?;
                Ok(facts)
            });
            // Dropping the owned current-thread runtime cancels all accept/connection tasks.
            drop(rt);
            result
        });
        let mut peer = Self {
            stop: Some(stop),
            join: Some(join),
            ipc,
        };
        match started.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(peer),
            Ok(Err(error)) => {
                peer.close()?;
                Err(error)
            }
            Err(_) => {
                peer.close()?;
                anyhow::bail!("scripted peer startup")
            }
        }
    }
    fn close(&mut self) -> Result<Facts> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let facts = self
            .join
            .take()
            .context("scripted peer join ownership")?
            .join()
            .map_err(|_| anyhow::anyhow!("scripted peer panic"))??;
        ensure!(!self.ipc.exists(), "scripted peer descriptor removed");
        Ok(facts)
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        if self.join.is_some() {
            let _ = self.close();
        }
    }
}
fn run_dir() -> Result<PathBuf> {
    Ok(tempfile::Builder::new()
        .prefix("history-reader-")
        .tempdir_in(workspace().join(".tmp/s54-block4aw1-history-reader-01"))?
        .keep())
}
fn diagnostic_cli() -> PathBuf {
    workspace().join("target/s54-diagnostics-source-checks/verification/kilogram-cli.exe")
}
fn synthetic_cursor() -> Result<db::HistoryCursor> {
    let mut bytes = [0; 275];
    bytes[..8].copy_from_slice(b"KGH2\x01\x02\0\0");
    Ok(db::HistoryCursor::from_bytes(&bytes)?)
}

#[test]
fn s54_f_history_b_actual_cli_first_and_suffix_busy() -> Result<()> {
    let dir = run_dir()?;
    let _scope = s53_deadline::Guard::enter(dir.as_path(), 30)?;
    let _phase = s53_deadline::Phase::until(Instant::now() + Duration::from_secs(15))?;
    let until = s53_deadline::capture_deadline()?;
    let cursor = synthetic_cursor()?; // Transport-valid only; no authenticated history claim.
    let text = cursor.to_base64();
    let page = |n, finished| {
        serde_json::to_vec(
            &json!({"view":"scripted","F":[7,35],"cursor":text,"finished":finished,"items":vec![json!({"scripted":true});n]}),
        )
    };
    let mut peer = Peer::start(
        dir.as_path().join("peer.ipc"),
        vec![
            Step {
                limit: 16,
                cursor: None,
                reply: Reply::Busy,
            },
            Step {
                limit: 16,
                cursor: None,
                reply: Reply::Json(page(16, false)?),
            },
            Step {
                limit: 32,
                cursor: Some(cursor.as_bytes().to_vec()),
                reply: Reply::Busy,
            },
            Step {
                limit: 32,
                cursor: Some(cursor.as_bytes().to_vec()),
                reply: Reply::Json(page(24, true)?),
            },
        ],
        false,
        until + Duration::from_secs(2),
    )?;
    let mut exits = Vec::new();
    let mut progress = 0;
    let mut reader = Reader {
        until,
        attempt: |limit, cursor: Option<&str>| {
            let output = cli_page(&diagnostic_cli(), &peer.ipc, 1, limit, cursor)?;
            exits.push(output.status.code());
            Ok(output)
        },
        progress: || {
            progress += 1;
            s53_deadline::check()
        },
        now: Instant::now,
        yield_for: std::thread::sleep,
    };
    let first = reader.page(16, None)?;
    let suffix = reader.page(32, Some(&text))?;
    reader.check()?;
    ensure!(
        first["items"].as_array().is_some_and(|a| a.len() == 16)
            && suffix["items"].as_array().is_some_and(|a| a.len() == 24)
            && first["view"] == suffix["view"]
            && first["F"] == suffix["F"]
            && suffix["finished"] == true,
        "scripted page continuity"
    );
    ensure!(
        exits == [Some(1), Some(0), Some(1), Some(0)] && progress >= 12,
        "actual CLI exits and progress"
    );
    let facts = peer.close()?;
    ensure!(
        facts.requests == 4 && facts.busy == 2 && facts.pages == 2,
        "scripted peer counts"
    );
    println!(
        "s54_history_B scripted=true actual_CLI=true first=16 suffix=24 exits=1,0,1,0 requests=4 peer_joined=true descriptor_removed=true storage_proof=false"
    );
    Ok(())
}

#[test]
fn s54_f_history_b_actual_cli_terminal_errors() -> Result<()> {
    for reply in [Reply::Error, Reply::Json(b"malformed".to_vec())] {
        let command_failure = matches!(reply, Reply::Error);
        let dir = run_dir()?;
        let _scope = s53_deadline::Guard::enter(dir.as_path(), 15)?;
        let until = s53_deadline::capture_deadline()?;
        let mut peer = Peer::start(
            dir.as_path().join("peer.ipc"),
            vec![Step {
                limit: 16,
                cursor: None,
                reply,
            }],
            false,
            until,
        )?;
        let mut exits = Vec::new();
        let mut reader = Reader {
            until,
            attempt: |limit, cursor: Option<&str>| {
                let output = cli_page(&diagnostic_cli(), &peer.ipc, 1, limit, cursor)?;
                exits.push(output.status.code());
                Ok(output)
            },
            progress: s53_deadline::check,
            now: Instant::now,
            yield_for: std::thread::sleep,
        };
        let failure = reader
            .page(16, None)
            .err()
            .context("actual CLI terminal result")?;
        ensure!(
            failure.to_string()
                == if command_failure {
                    "F history command_failed"
                } else {
                    "F history invalid_page"
                },
            "closed terminal category"
        );
        ensure!(
            exits == [Some(if command_failure { 1 } else { 0 })],
            "actual terminal CLI exit"
        );
        let facts = peer.close()?;
        ensure!(
            facts.requests == 1 && facts.errors == usize::from(command_failure),
            "terminal response must not retry"
        );
    }
    println!(
        "s54_history_B scripted=true actual_CLI=true terminal_cases=2 no_retry=true peer_joined=true storage_proof=false"
    );
    Ok(())
}

#[test]
fn s54_f_history_b_actual_cli_persistent_busy_bound() -> Result<()> {
    let dir = run_dir()?;
    let _scope = s53_deadline::Guard::enter(dir.as_path(), 15)?;
    let peer_until = s53_deadline::capture_deadline()?;
    let mut peer = Peer::start(
        dir.as_path().join("peer.ipc"),
        vec![Step {
            limit: 16,
            cursor: None,
            reply: Reply::Busy,
        }],
        true,
        peer_until,
    )?;
    let _phase = s53_deadline::Phase::until(Instant::now() + Duration::from_secs(2))?;
    let until = s53_deadline::capture_deadline()?;
    let mut reader = Reader {
        until,
        attempt: |limit, cursor: Option<&str>| {
            cli_page(&diagnostic_cli(), &peer.ipc, 1, limit, cursor)
        },
        progress: s53_deadline::check,
        now: Instant::now,
        yield_for: std::thread::sleep,
    };
    ensure!(
        reader.page(16, None).is_err() && Instant::now() >= until,
        "persistent busy cannot extend operation"
    );
    let facts = peer.close()?;
    ensure!(
        facts.requests > 0 && facts.requests == facts.busy && facts.pages == 0,
        "actual busy only"
    );
    println!(
        "s54_history_B scripted=true actual_CLI=true persistent_busy_requests={} peer_joined=true storage_proof=false",
        facts.requests
    );
    Ok(())
}

fn output(success: bool, stdout: &[u8], stderr: &[u8]) -> Output {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    #[cfg(windows)]
    let status = std::process::ExitStatus::from_raw(u32::from(!success));
    #[cfg(unix)]
    let status = std::process::ExitStatus::from_raw(i32::from(!success) << 8);
    Output {
        status,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}
#[test]
fn s54_f_history_c_exact_unsuccessful_empty_stdout_busy_only() {
    assert!(matches!(classify(output(false, b"", BUSY)), Attempt::Busy));
    for stderr in [
        b"Error: SchedulingWindowBusy\r\n".as_slice(),
        b"SchedulingWindowBusy\n",
        b"Error: SchedulingWindowBusy\n\n",
        b"Error: HistoryUnavailable\n",
        b"\xff",
    ] {
        assert!(matches!(
            classify(output(false, b"", stderr)),
            Attempt::Terminal("command_failed")
        ));
    }
    assert!(matches!(
        classify(output(false, b"{}", BUSY)),
        Attempt::Terminal("command_failed")
    ));
    assert!(matches!(
        classify(output(true, b"", BUSY)),
        Attempt::Terminal("invalid_page")
    ));
    assert!(matches!(
        classify(output(true, &vec![b' '; PAGE_CAP + 1], b"")),
        Attempt::Terminal("page_cap")
    ));
}
#[test]
fn s54_f_history_c_shared_deadline_late_success_and_yield_cap() -> Result<()> {
    let base = Instant::now();
    let elapsed = Cell::new(Duration::ZERO);
    let calls = Cell::new(0);
    let mut reader = Reader {
        until: base + Duration::from_millis(15),
        attempt: |_, _: Option<&str>| {
            calls.set(calls.get() + 1);
            elapsed.set(elapsed.get() + Duration::from_millis(8));
            Ok(output(true, b"{}", b""))
        },
        progress: || Ok(()),
        now: || base + elapsed.get(),
        yield_for: |_| {},
    };
    reader.page(16, None)?;
    ensure!(
        reader.page(32, Some("same injected cursor")).is_err() && calls.get() == 2,
        "same deadline rejects late suffix success"
    );
    elapsed.set(Duration::ZERO);
    calls.set(0);
    let mut reader = Reader {
        until: base + Duration::from_millis(30),
        attempt: |_, _: Option<&str>| {
            calls.set(calls.get() + 1);
            Ok(output(false, b"", BUSY))
        },
        progress: || Ok(()),
        now: || base + elapsed.get(),
        yield_for: |duration| {
            assert!(duration <= Duration::from_millis(25));
            elapsed.set(elapsed.get() + duration)
        },
    };
    ensure!(
        reader.page(16, None).is_err()
            && elapsed.get() == Duration::from_millis(30)
            && calls.get() == 2,
        "busy yields capped by remaining original time"
    );
    Ok(())
}
#[test]
fn s54_f_history_c_progress_failure_and_nested_phase() -> Result<()> {
    let checks = Cell::new(0);
    let attempts = Cell::new(0);
    let yields = Cell::new(0);
    let mut reader = Reader {
        until: Instant::now() + Duration::from_secs(1),
        attempt: |_, _: Option<&str>| {
            attempts.set(attempts.get() + 1);
            Ok(output(false, b"", BUSY))
        },
        progress: || {
            checks.set(checks.get() + 1);
            ensure!(checks.get() < 2, "injected liveness failure");
            Ok(())
        },
        now: Instant::now,
        yield_for: |_| yields.set(yields.get() + 1),
    };
    ensure!(
        reader.page(16, None).is_err() && attempts.get() == 1 && yields.get() == 0,
        "progress error terminates without retry"
    );
    let dir = run_dir()?;
    let _scope = s53_deadline::Guard::enter(dir.as_path(), 60)?;
    let original = s53_deadline::capture_deadline()?;
    {
        let _outer = s53_deadline::Phase::until(Instant::now() + Duration::from_secs(1))?;
        let short = s53_deadline::capture_deadline()?;
        {
            let _inner = s53_deadline::Phase::until(Instant::now() + Duration::from_secs(30))?;
            ensure!(
                s53_deadline::capture_deadline()? == short,
                "inner cannot extend outer phase"
            );
        }
        ensure!(
            s53_deadline::capture_deadline()? == short,
            "restore shorter phase"
        );
    }
    ensure!(
        s53_deadline::capture_deadline()? == original,
        "restore original absolute bound"
    );
    Ok(())
}

#[test]
fn s54_f_history_c_collector_preserves_continuity_and_bounds() -> Result<()> {
    let dir = run_dir()?;
    let _scope = s53_deadline::Guard::enter(dir.as_path(), 30)?;
    let page = |view: &str, finished: bool, cursor: &str, count: usize| json!({"view":view,"F":[7,35],"finished":finished,"cursor":cursor,"items":vec![json!({});count]});
    let mut n = 0;
    let rows = early::history_with(|cursor| {
        n += 1;
        ensure!(
            cursor == if n == 1 { None } else { Some("next") },
            "collector supplied cursor"
        );
        Ok(page("same", n == 2, "next", 1))
    })?;
    ensure!(rows.len() == 2 && n == 2, "collector terminal rows");
    for (view, count) in [("changed", 1), ("same", 0), ("same", 1)] {
        let mut n = 0;
        ensure!(
            early::history_with(|_| {
                n += 1;
                Ok(page(
                    if n == 1 { "same" } else { view },
                    false,
                    "repeat",
                    count,
                ))
            })
            .is_err(),
            "collector rejects identity or empty/repeated progress"
        );
    }
    let mut n = 0;
    ensure!(
        early::history_with(|_| {
            n += 1;
            Ok(page("same", false, &n.to_string(), 1))
        })
        .is_err()
            && n == 43,
        "collector page bound"
    );
    ensure!(
        early::history_with(|_| Ok(page("same", true, "", 1345))).is_err(),
        "collector row bound"
    );
    ensure!(
        early::history_with(|_| Ok(json!({"padding":"x".repeat(PAGE_CAP+1)}))).is_err(),
        "collector byte bound"
    );
    Ok(())
}
