//! Actual CLI children. The controller never creates native state/events/projections.
use anyhow::{Context, Result, ensure};
use kilogram_identity::AccountRootState;
use kilogram_protocol::ordered_dialog_v2::{self as wire, Value, a, b, opt, u};
use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct OwnedChild(Child);
impl std::ops::Deref for OwnedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl std::ops::DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[path = "runtime2_process/bootstrap.rs"]
mod bootstrap;
#[path = "runtime2_process/diagnostic.rs"]
mod diagnostic;
#[path = "runtime2_process/faults.rs"]
mod faults;
#[path = "runtime2_process/lifecycle.rs"]
mod lifecycle;
#[path = "runtime2_process/positive.rs"]
mod positive;
#[path = "runtime2_process/schedule_control.rs"]
mod schedule_control;
#[path = "runtime2_process/versions.rs"]
mod versions;
fn message(slot: u8, role: u8, n: u64) -> ([u8; 16], String) {
    let mut id = [0; 16];
    id[0] = slot;
    id[1] = role;
    id[8..].copy_from_slice(&n.to_be_bytes());
    (id, format!("process:{slot}:{role}:{n}:Привет"))
}
fn hex(raw: &[u8]) -> String {
    raw.iter().map(|n| format!("{n:02x}")).collect()
}
#[cfg(windows)]
fn private_bytes(child: &Child) -> Result<u64> {
    use std::os::windows::process::CommandExt;
    let output = Command::new("powershell.exe")
        .creation_flags(0x0800_4000)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "[Console]::Write((Get-Process -Id {}).PrivateMemorySize64)",
                child.id()
            ),
        ])
        .output()?;
    ensure!(output.status.success(), "PrivateMemorySize64 query");
    Ok(String::from_utf8(output.stdout)?.trim().parse()?)
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn exe() -> PathBuf {
    repo().join("target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe")
}
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
fn command() -> Command {
    let mut cmd = Command::new(exe());
    cmd.current_dir(repo());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_4000);
    }
    cmd
}
fn cli(args: Vec<String>) -> Result<String> {
    let output = command().args(&args).output()?;
    ensure!(
        output.status.success(),
        "CLI {args:?} failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}
struct Node {
    state: PathBuf,
    profile: PathBuf,
    ipc: PathBuf,
    locators: PathBuf,
    credential: PathBuf,
    root: PathBuf,
    schedule: PathBuf,
    pins: serde_json::Value,
    role: u8,
    child: Option<OwnedChild>,
    starts: u32,
    memory_clock: std::sync::Mutex<Instant>,
}
impl Node {
    fn new(base: &Path, index: usize, contexts: &[String; 2]) -> Result<Self> {
        let root = base.join(format!("root{index}"));
        AccountRootState::create(&root)?;
        let state = base.join(format!("state{index}"));
        let credential = base.join(format!("credential{index}.bin"));
        let output = cli(vec![
            "device-enroll".into(),
            "--profile-mode".into(),
            "ordered-dialog-v2".into(),
            "--state-dir".into(),
            text(&state),
            "--account-dir".into(),
            text(&root),
            "--ordered-context".into(),
            contexts[0].clone(),
            "--ordered-context".into(),
            contexts[1].clone(),
            "--certificate-file".into(),
            text(&credential),
        ])?;
        let pins = serde_json::from_str(output.lines().last().context("enrollment result")?)?;
        let schedule = base.join(format!("schedule{index}.json"));
        std::fs::write(&schedule, b"{}")?;
        Ok(Self {
            state,
            profile: base.join(format!("profile{index}.json")),
            ipc: base.join(format!("ipc{index}.json")),
            locators: base.join(format!("locators{index}")),
            credential,
            root,
            schedule,
            pins,
            role: index as u8,
            child: None,
            starts: 0,
            memory_clock: std::sync::Mutex::new(Instant::now() - Duration::from_secs(5)),
        })
    }
    fn install(&self, artifact: &Path) -> Result<()> {
        cli(vec![
            "ordered".into(),
            "context2-import".into(),
            "--state-dir".into(),
            text(&self.state),
            "--expect-account".into(),
            self.pins["account"].as_str().context("account")?.into(),
            "--expect-claim".into(),
            self.pins["claim"].as_str().context("claim")?.into(),
            "--invitation-file".into(),
            text(artifact),
        ])?;
        Ok(())
    }
    fn create_profile(&self, contexts: &[String; 2]) -> Result<()> {
        cli(vec![
            "runtime-profile-create".into(),
            "--profile-mode".into(),
            "ordered-dialog-v2".into(),
            "--state-dir".into(),
            text(&self.state),
            "--profile-file".into(),
            text(&self.profile),
            "--expect-account".into(),
            self.pins["account"].as_str().context("account")?.into(),
            "--expect-claim".into(),
            self.pins["claim"].as_str().context("claim")?.into(),
            "--expect-device".into(),
            self.pins["device"].as_str().context("device")?.into(),
            "--ordered-context".into(),
            contexts[0].clone(),
            "--ordered-context".into(),
            contexts[1].clone(),
            "--locator-directory".into(),
            text(&self.locators),
            "--ipc-file".into(),
            text(&self.ipc),
            "--route-policy".into(),
            "direct-only".into(),
            "--ordered-bind".into(),
            "127.0.0.1:0".into(),
        ])?;
        Ok(())
    }
    fn launch(&mut self) -> Result<PathBuf> {
        ensure!(self.child.is_none(), "already running");
        self.starts += 1;
        if self.ipc.exists() {
            std::fs::remove_file(&self.ipc)?;
        }
        let out = self
            .profile
            .with_extension(format!("{}.stdout.log", self.starts));
        let err = self
            .profile
            .with_extension(format!("{}.stderr.log", self.starts));
        let child = command()
            .args(["runtime-from-profile", "--profile-file"])
            .arg(&self.profile)
            .env("KILOGRAM_TEST_OD2_SCHEDULE", &self.schedule)
            .stdout(Stdio::from(std::fs::File::create(&out)?))
            .stderr(Stdio::from(std::fs::File::create(&err)?))
            .spawn()?;
        println!(
            "process_start pid={} profile={} stderr={}",
            child.id(),
            self.profile.display(),
            err.display()
        );
        self.child = Some(OwnedChild(child));
        Ok(err)
    }
    async fn start(&mut self) -> Result<()> {
        let err = self.launch()?;
        self.wait_ready(&err).await
    }
    async fn wait_ready(&mut self, err: &Path) -> Result<()> {
        let start = Instant::now();
        let mut last_error = String::new();
        loop {
            if let Some(status) = self.child.as_mut().context("child")?.try_wait()? {
                anyhow::bail!("runtime exited {status}: {}", std::fs::read_to_string(err)?);
            }
            if self.ipc.exists() {
                match self.status().await {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        let message = format!("{e:#}");
                        if message != last_error {
                            println!("startup_ipc_error={message}");
                            last_error = message;
                        }
                    }
                }
            }
            ensure!(
                start.elapsed() < Duration::from_secs(30),
                "runtime start deadline: IPC={last_error}; {}",
                std::fs::read_to_string(err)?
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    async fn stop(&mut self, kill: bool) -> Result<()> {
        let mut child = self.child.take().context("child stopped")?;
        let pid = child.id();
        if kill {
            ensure!(
                child.try_wait()?.is_none(),
                "real kill target already exited pid={pid}"
            );
            child.kill()?;
        } else {
            ensure!(
                matches!(
                    kilogram_runtime_ipc::call(&self.ipc, RuntimeIpcCommand::Shutdown).await?,
                    RuntimeIpcResponse::ShutdownAccepted
                ),
                "shutdown reply"
            );
        }
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                println!("process_exit pid={pid} kill={kill} status={status}");
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(15) {
                child.kill()?;
                child.wait()?;
                anyhow::bail!("graceful stop deadline pid={pid}");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    async fn call(&self, body: Value) -> Result<Value> {
        let request = wire::encode(&body, 32 * 1024)?;
        match kilogram_runtime_ipc::call(&self.ipc, RuntimeIpcCommand::OrderedDialog { request })
            .await?
        {
            RuntimeIpcResponse::OrderedDialog { response } => {
                let v = wire::decode(&response, 48 * 1024)?;
                let f = v.fields(3)?;
                ensure!(f[0].uint()? == 2, "IPC2 version");
                Ok(f[2].clone())
            }
            RuntimeIpcResponse::Error { message } => anyhow::bail!("{message}"),
            _ => anyhow::bail!("IPC2 response"),
        }
    }
    async fn status(&self) -> Result<serde_json::Value> {
        #[cfg(windows)]
        if let Some(child) = &self.child {
            let mut clock = self
                .memory_clock
                .lock()
                .map_err(|_| anyhow::anyhow!("memory sampler"))?;
            if clock.elapsed() >= Duration::from_secs(5) {
                let bytes = private_bytes(child)?;
                ensure!(
                    bytes <= 384 * 1024 * 1024,
                    "runtime PrivateBytes bound {bytes}"
                );
                println!("private_bytes pid={} bytes={bytes}", child.id());
                *clock = Instant::now();
            }
        }
        let status: serde_json::Value =
            serde_json::from_slice(self.call(a([u(2), u(1)])).await?.bytes()?)?;
        ensure!(
            status["resources"]["connections"]
                .as_u64()
                .is_some_and(|n| n <= 2)
                && status["resources"]["peak_mailbox"]
                    .as_u64()
                    .is_some_and(|n| n <= 16)
                && status["stats"]["max_native_bytes"]
                    .as_u64()
                    .is_some_and(|n| n <= 128 * 1024)
                && status["stats"]["max_commit_app_bytes"]
                    .as_u64()
                    .is_some_and(|n| n <= 192 * 1024 * 1024),
            "measured runtime resource bound: {status}"
        );
        Ok(status)
    }
    async fn import(&self, slot: u8, peer: &Node, contexts: &[String; 2]) -> Result<()> {
        let raw = std::fs::read(
            peer.locators
                .join(format!("{}.locator", contexts[usize::from(slot)])),
        )?;
        self.call(a([u(2), u(5), u(u64::from(slot)), b(raw)]))
            .await?;
        Ok(())
    }
    async fn send(&self, slot: u8, sequence: u64) -> Result<()> {
        let (request, text) = message(slot, self.role, sequence);
        self.call(a([
            u(2),
            u(2),
            u(u64::from(slot)),
            b(request),
            b(text.as_bytes()),
        ]))
        .await?;
        Ok(())
    }
    fn schedule(&self, value: serde_json::Value) -> Result<()> {
        let report = schedule_control::write(&self.schedule, &value)?;
        if report.attempts > 1 {
            println!(
                "controller_schedule_persist_completed attempts={} micros={}",
                report.attempts,
                report.elapsed.as_micros()
            );
        }
        Ok(())
    }
    fn route(&self, policy: &str, relay: Option<&str>, bind: Option<&str>) -> Result<()> {
        let mut p: serde_json::Value = serde_json::from_slice(&std::fs::read(&self.profile)?)?;
        p["route"] = serde_json::json!({"policy":policy,"relay":relay,"bind":bind});
        std::fs::write(&self.profile, serde_json::to_vec_pretty(&p)?)?;
        Ok(())
    }
}
impl Drop for Node {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
struct Pair {
    nodes: [Node; 2],
    contexts: [String; 2],
    base: PathBuf,
}
impl Pair {
    fn prepared(label: &str) -> Result<Self> {
        ensure!(
            exe().is_file(),
            "build/copy production CLI to fixed path first"
        );
        let parent = repo().join(".tmp/a3-s4/process");
        std::fs::create_dir_all(&parent)?;
        let base = tempfile::Builder::new()
            .prefix(label)
            .tempdir_in(parent)?
            .keep();
        println!(
            "fixture={} cli_sha256_external_manifest_required=true",
            base.display()
        );
        let contexts = [
            wire::hash(b"S4 C1", text(&base).as_bytes()),
            wire::hash(b"S4 C2", text(&base).as_bytes()),
        ]
        .map(|id| id.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let mut nodes = [
            Node::new(&base, 0, &contexts)?,
            Node::new(&base, 1, &contexts)?,
        ];
        if nodes[0].pins["device"].as_str() > nodes[1].pins["device"].as_str() {
            nodes.swap(0, 1);
        }
        nodes[0].role = 0;
        nodes[1].role = 1;
        for (slot, c) in contexts.iter().enumerate() {
            let artifact = base.join(format!("context{slot}.bin"));
            cli(vec![
                "ordered".into(),
                "context2-create".into(),
                "--context".into(),
                c.clone(),
                "--credentials-file".into(),
                text(&nodes[0].credential),
                text(&nodes[1].credential),
                "--account-dir".into(),
                text(&nodes[0].root),
                text(&nodes[1].root),
                "--output-file".into(),
                text(&artifact),
            ])?;
            for node in &nodes {
                node.install(&artifact)?;
            }
        }
        for node in &nodes {
            node.create_profile(&contexts)?;
        }
        Ok(Self {
            nodes,
            contexts,
            base,
        })
    }
    async fn new(label: &str) -> Result<Self> {
        let mut pair = Self::prepared(label)?;
        pair.nodes[0].start().await?;
        pair.nodes[1].start().await?;
        pair.import().await?;
        pair.wait_active().await?;
        Ok(pair)
    }
    async fn import(&self) -> Result<()> {
        for c in 0..2 {
            self.nodes[0]
                .import(c, &self.nodes[1], &self.contexts)
                .await?;
            self.nodes[1]
                .import(c, &self.nodes[0], &self.contexts)
                .await?;
        }
        Ok(())
    }
    async fn wait_active(&self) -> Result<()> {
        let start = Instant::now();
        let mut previous = String::new();
        loop {
            let mut ready = true;
            let mut states = Vec::new();
            for node in &self.nodes {
                let status = node.status().await?;
                for c in status["contexts"].as_array().context("contexts")? {
                    ready &= c["active"].as_bool() == Some(true);
                }
                states.push(status);
            }
            let phases = states
                .iter()
                .map(|s| {
                    s["contexts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|c| (c["phase"].clone(), c["active"].clone()))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let phases = format!("{phases:?}");
            if previous != phases {
                println!(
                    "active_progress seconds={:.3} phases={phases}",
                    start.elapsed().as_secs_f64()
                );
                previous = phases;
            }
            if ready {
                return Ok(());
            }
            ensure!(
                start.elapsed() < Duration::from_secs(45),
                "Active2 deadline; fixture {}; states={states:?}",
                self.base.display()
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    async fn wait_counts(&self, slot: usize, sources: u64) -> Result<()> {
        let start = Instant::now();
        loop {
            let mut done = true;
            for node in &self.nodes {
                let s = node.status().await?;
                let c = &s["contexts"][slot];
                done &= c["counts"]["sources"].as_u64() == Some(sources)
                    && c["counts"]["debt"].as_u64() == Some(0)
                    && c["counts"]["queued"].as_u64() == Some(0);
            }
            if done {
                return Ok(());
            }
            ensure!(
                start.elapsed() < Duration::from_secs(60),
                "counts deadline slot={slot} sources={sources}; fixture {}",
                self.base.display()
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    async fn history(&self, node: usize, slot: u8, expected: usize) -> Result<()> {
        let mut cursor = None;
        let mut count = 0;
        let mut ids = std::collections::BTreeSet::new();
        loop {
            let raw = self.nodes[node]
                .call(a([
                    u(2),
                    u(4),
                    u(u64::from(slot)),
                    opt(cursor.as_ref().map(|c: &Vec<u8>| b(c))),
                    u(3),
                ]))
                .await?;
            let p: serde_json::Value = serde_json::from_slice(raw.bytes()?)?;
            for row in p["items"].as_array().context("items")? {
                ensure!(
                    row["text"]
                        .as_str()
                        .is_some_and(|t| t.starts_with("process:") && t.ends_with("Привет")),
                    "readable text"
                );
                ensure!(
                    ids.insert(row["id"].as_str().context("ID")?.to_owned()),
                    "duplicate history ID"
                );
                count += 1;
            }
            if p["finished"].as_bool() == Some(true) {
                break;
            }
            cursor = Some(
                kilogram_state::indexed_dialog::HistoryCursor::from_base64(
                    p["cursor"].as_str().context("cursor")?,
                )?
                .as_bytes()
                .to_vec(),
            );
        }
        ensure!(count == expected, "history count {count} != {expected}");
        Ok(())
    }
    async fn state(
        &self,
        node: usize,
        limit: Duration,
        predicate: impl Fn(&serde_json::Value) -> bool,
        label: &str,
    ) -> Result<serde_json::Value> {
        let start = Instant::now();
        let mut report = Instant::now();
        loop {
            let state = self.nodes[node].status().await?;
            if predicate(&state) {
                return Ok(state);
            }
            ensure!(
                start.elapsed() < limit,
                "{label} deadline fixture={} state={state}",
                self.base.display()
            );
            if report.elapsed() > Duration::from_secs(10) {
                println!(
                    "wait={label} seconds={:.3} node={node} contexts={}",
                    start.elapsed().as_secs_f64(),
                    state["contexts"]
                );
                report = Instant::now();
            }
            tokio::time::sleep(Duration::from_millis(75)).await;
        }
    }
    async fn converged(&self, slot: usize, total: u64) -> Result<()> {
        for node in 0..2 {
            self.state(
                node,
                Duration::from_secs(240),
                |s| {
                    let c = &s["contexts"][slot];
                    c["counts"]["sources"] == total
                        && c["counts"]["queued"] == 0
                        && c["counts"]["debt"] == 0
                        && c["counts"]["pending"] == 0
                },
                "convergence",
            )
            .await?;
        }
        Ok(())
    }
    async fn restarted(&mut self, kill: bool) -> Result<()> {
        self.nodes[0].stop(kill).await?;
        self.nodes[1].stop(kill).await?;
        self.nodes[0].start().await?;
        self.nodes[1].start().await?;
        self.import().await?;
        self.wait_active().await
    }
}
#[tokio::test]
async fn a3_s4_process_burst128_bounded_rounds() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(240), async {
        let mut pair = Pair::new("burst-").await?;
        let start = Instant::now();
        for n in 1..=128 {
            pair.nodes[0].send(0, n).await?;
        }
        pair.converged(0, 130).await?;
        for node in 0..2 {
            pair.history(node, 0, 128).await?;
        }
        for node in &mut pair.nodes {
            node.stop(false).await?;
        }
        println!(
            "S4_BURST_PASS one_way=128 exact_readable=true seconds={:.3}",
            start.elapsed().as_secs_f64()
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("bounded burst deadline")?
}
#[tokio::test]
async fn a3_s4_process_smoke_fresh_offline_restart_readable() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(240),async{
        let mut pair=Pair::new("smoke-").await?;
        for slot in 0..2{for sequence in 1..=4{pair.nodes[0].send(slot,sequence).await?;pair.nodes[1].send(slot,sequence).await?;}pair.wait_counts(usize::from(slot),10).await?;}
        let sid=pair.nodes[0].status().await?["contexts"][0]["sid"].clone();
        pair.nodes[1].stop(true).await?;
        for sequence in 5..=12{pair.nodes[0].send(0,sequence).await?;}
        let start=Instant::now();loop{let s=pair.nodes[0].status().await?;if s["contexts"][0]["counts"]["sources"]==18{break;}ensure!(start.elapsed()<Duration::from_secs(30),"offline materialize deadline");tokio::time::sleep(Duration::from_millis(50)).await;}
        pair.nodes[0].stop(true).await?;pair.nodes[0].start().await?;
        ensure!(pair.nodes[0].status().await?["contexts"][0]["sid"]==sid,"SID changed on offline restart");
        pair.nodes[1].start().await?;pair.import().await?;pair.wait_active().await?;pair.wait_counts(0,18).await?;
        for node in 0..2{pair.history(node,0,16).await?;pair.history(node,1,8).await?;println!("final_node={node} {}",pair.nodes[node].status().await?);}
        pair.nodes[0].stop(false).await?;pair.nodes[1].stop(false).await?;
        println!("S4_SMOKE_PASS real_processes=2 contexts=2 text=24 genesis=4 offline_text=8 restarts=both history=readable");Ok::<_,anyhow::Error>(())
    }).await.context("process smoke absolute deadline")?
}
#[tokio::test]
async fn a3_s4_component_ipc_history_escaped_byte_budget_no_cursor_skip() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(120),async{
        let mut pair=Pair::new("history-byte-budget-").await?;
        let text="\0".repeat(wire::MAX_TEXT);let mut expected=std::collections::BTreeSet::new();
        for n in 1..=6u64{let request=message(0,0,n).0;expected.insert(hex(&request));pair.nodes[0].call(a([u(2),u(2),u(0),b(request),b(text.as_bytes())])).await?;}
        pair.converged(0,8).await?;
        for node in &pair.nodes{let mut cursor=None;let mut actual=std::collections::BTreeSet::new();let mut pages=0;loop{
            let reply=node.call(a([u(2),u(4),u(0),opt(cursor.as_ref().map(|c:&Vec<u8>|b(c))),u(32)])).await?;
            ensure!(reply.bytes()?.len()<=48*1024,"escaped JSON exceeded authenticated IPC cap");
            let page:serde_json::Value=serde_json::from_slice(reply.bytes()?)?;let rows=page["items"].as_array().context("escaped rows")?;
            ensure!(rows.len()==1,"max escaped Text should split page by bytes before row limit");
            for row in rows{ensure!(row["text"]==text&&actual.insert(row["request"].as_str().context("request")?.to_owned()),"byte-budget cursor skipped/duplicated/changed Text");}
            pages+=1;if page["finished"]==true{break;}cursor=Some(kilogram_state::indexed_dialog::HistoryCursor::from_base64(page["cursor"].as_str().context("cursor")?)?.as_bytes().to_vec());
        }ensure!(pages==6&&actual==expected,"byte-budget cursor completeness");}
        for node in &mut pair.nodes{node.stop(false).await?;}
        println!("S4_HISTORY_BYTE_BUDGET_PASS real_text=6 bytes_per_text=4096 escaped_JSON_page_cap=49152 pages_per_peer=6 exact_cursor_no_skip=true");Ok::<_,anyhow::Error>(())
    }).await.context("history byte budget deadline")?
}
