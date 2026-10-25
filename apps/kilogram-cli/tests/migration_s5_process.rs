//! Parent-owned real process evidence. Sources come from frozen d9b or a fresh
//! frozen d9b enrollment; this controller never writes Native, Head or Event rows.
use anyhow::{Context, Result, ensure};
use kilogram_cli::ordered_dialog::handshake::Credentials;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct OwnedChild(Child);
impl OwnedChild {
    fn alive(&mut self) -> Result<()> {
        ensure!(
            self.0.try_wait()?.is_none(),
            "child {} exited before parent kill",
            self.0.id()
        );
        Ok(())
    }
    fn kill_wait(&mut self) -> Result<()> {
        self.alive()?;
        self.0.kill()?;
        let exit = self.0.wait()?;
        ensure!(!exit.success(), "actual parent kill exit");
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn workspace() -> PathBuf {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();
    root
}
fn current() -> PathBuf {
    workspace().join("target/compatibility/long-lived-dialog-migration/kilogram-cli.exe")
}
fn frozen() -> PathBuf {
    workspace().join("target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe")
}
fn control_peer() -> PathBuf {
    workspace()
        .join("target/compatibility/long-lived-dialog-migration/kilogram-l1-control-peer.exe")
}
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
fn command(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    command.current_dir(workspace());
    for name in [
        "KILOGRAM_TEST_OD1_SCHEDULE",
        "KILOGRAM_TEST_OD1_CLOCK_FILE",
        "KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE",
        "KILOGRAM_TEST_MIGRATION_PAUSE_FILE",
    ] {
        command.env_remove(name);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_4000);
    }
    command
}
fn cli(exe: &Path, args: &[String]) -> Result<String> {
    let output = command(exe).args(args).output()?;
    ensure!(
        output.status.success(),
        "CLI {args:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}
fn rejected(exe: &Path, args: &[String]) -> Result<String> {
    let output = command(exe).args(args).output()?;
    ensure!(
        !output.status.success(),
        "unexpected writer admission {args:?}"
    );
    let error = String::from_utf8(output.stderr)?;
    println!("version_refusal exe={} reason={error}", exe.display());
    Ok(error)
}
fn json_cli(exe: &Path, args: &[String]) -> Result<Value> {
    Ok(serde_json::from_str(&cli(exe, args)?)?)
}
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "source symlink");
        if kind.is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}
fn run_dir(prefix: &str) -> Result<PathBuf> {
    let base = workspace().join(".tmp/migration-s5-storage");
    std::fs::create_dir_all(&base)?;
    let path = tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&base)?
        .keep();
    ensure!(
        path.canonicalize()?.starts_with(base.canonicalize()?),
        "owned fixture root"
    );
    Ok(path)
}
fn spawn(
    exe: &Path,
    args: &[String],
    base: &Path,
    name: &str,
    pause: Option<&Path>,
    schedule: Option<&Path>,
) -> Result<OwnedChild> {
    let mut cmd = command(exe);
    cmd.args(args)
        .stdout(Stdio::from(std::fs::File::create(
            base.join(format!("{name}.stdout.log")),
        )?))
        .stderr(Stdio::from(std::fs::File::create(
            base.join(format!("{name}.stderr.log")),
        )?));
    if let Some(path) = pause {
        cmd.env("KILOGRAM_TEST_MIGRATION_PAUSE_FILE", path);
    }
    if let Some(path) = schedule {
        cmd.env("KILOGRAM_TEST_OD1_SCHEDULE", path);
    }
    Ok(OwnedChild(cmd.spawn()?))
}
fn wait_file(child: &mut OwnedChild, path: &Path, seconds: u64) -> Result<()> {
    let start = Instant::now();
    while !path.is_file() {
        child.alive()?;
        ensure!(
            start.elapsed() < Duration::from_secs(seconds),
            "marker deadline {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    child.alive()
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
    ensure!(output.status.success(), "native child memory query");
    Ok(String::from_utf8(output.stdout)?.trim().parse()?)
}

struct Branch {
    base: PathBuf,
    state: PathBuf,
    account: String,
    device: String,
    config: PathBuf,
    context: String,
    claim: String,
}
impl Branch {
    fn from_seed(prefix: &str) -> Result<Self> {
        let seed = PathBuf::from(
            std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged new stopped d9b seed")?,
        );
        let base = run_dir(prefix)?;
        let state = base.join("state");
        copy_tree(&seed.join("one.state"), &state)?;
        let credentials = Credentials::parse(&std::fs::read(seed.join("one.credentials"))?)?;
        let result: Value = serde_json::from_slice(&std::fs::read(seed.join("result.json"))?)?;
        let claim = std::fs::read(state.join("runtime/od1/profile-claim"))?;
        let c = kilogram_protocol::ordered_dialog_v1::decode(&claim, 1024)?;
        let mut branch = Self {
            account: credentials.cert.account_id().to_string(),
            device: credentials.cert.device_id().to_string(),
            context: result["context"].as_str().context("source C")?.into(),
            claim: kilogram_cli::ordered_dialog::state::hex(&c.fields(5)?[2].fixed::<32>()?),
            config: base.join("migration.json"),
            base,
            state,
        };
        branch.write_config()?;
        Ok(branch)
    }
    fn write_config(&mut self) -> Result<()> {
        let cfg = kilogram_state::indexed_dialog::Config::default();
        std::fs::write(
            &self.config,
            serde_json::to_vec_pretty(
                &json!({"logical_quota":cfg.logical_quota,"physical_capacity":cfg.physical_quota,"output_profile":self.base.join("indexed.profile"),"control_ipc_file":self.base.join("indexed.ipc"),"locator_directory":self.base.join("indexed.locators")}),
            )?,
        )?;
        Ok(())
    }
    fn args(&self, verb: &str) -> Vec<String> {
        vec![
            "ordered".into(),
            verb.into(),
            "--state-dir".into(),
            text(&self.state),
            "--expect-account".into(),
            self.account.clone(),
            "--expect-device".into(),
            self.device.clone(),
        ]
    }
    fn upgrade(&self, check: bool) -> Vec<String> {
        let mut args = self.args("storage-upgrade");
        args.extend([
            "--config-file".into(),
            text(&self.config),
            if check {
                "--check".into()
            } else {
                "--resume".into()
            },
        ]);
        args
    }
}

#[test]
fn migration_s5_process_fence_copy_ready_publication_kills() -> Result<()> {
    for point in [
        "fence-staged",
        "fence-committed",
        "copy-committed",
        "ready-staged",
        "ready-committed",
        "profile-staged",
    ] {
        let branch = Branch::from_seed(&format!("kill-{point}-"))?;
        let preview = json_cli(&current(), &branch.upgrade(true))?;
        let source = preview["source_id"].clone();
        let pause = branch.base.join("pause");
        std::fs::write(&pause, point)?;
        let mut child = spawn(
            &current(),
            &branch.upgrade(false),
            &branch.base,
            "migration",
            Some(&pause),
            None,
        )?;
        wait_file(&mut child, &pause.with_extension("hit"), 25)?;
        ensure!(
            std::fs::read_to_string(pause.with_extension("hit"))? == point,
            "exact pause checkpoint"
        );
        child.alive()?;
        #[cfg(windows)]
        let memory = private_bytes(&child.0)?;
        #[cfg(not(windows))]
        let memory = 0u64;
        println!(
            "M05 parent_verified_alive=true point={point} pid={} private_bytes={memory} root={}",
            child.0.id(),
            branch.base.display()
        );
        if point == "copy-committed" {
            let mut s3 = vec![
                "probe".into(),
                text(&branch.state),
                branch.account.clone(),
                branch.claim.clone(),
            ];
            let cfg = kilogram_state::indexed_dialog::Config::default();
            s3.extend([
                cfg.logical_quota[0].to_string(),
                cfg.logical_quota[1].to_string(),
                cfg.physical_quota.to_string(),
            ]);
            let s3_exe=workspace().join("target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe");
            let s4_exe =
                workspace().join("target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe");
            let s4_args = vec![
                "ordered".into(),
                "context2-import".into(),
                "--state-dir".into(),
                text(&branch.state),
                "--expect-account".into(),
                branch.account.clone(),
                "--expect-claim".into(),
                branch.claim.clone(),
                "--invitation-file".into(),
                text(&branch.base.join("must-not-read")),
            ];
            for (exe, args) in [
                (
                    frozen(),
                    vec![
                        "ordered".into(),
                        "inspect".into(),
                        "--state-dir".into(),
                        text(&branch.state),
                    ],
                ),
                (s4_exe, s4_args),
                (s3_exe, s3),
            ] {
                let error = rejected(&exe, &args)?;
                ensure!(
                    error.contains("locked") || error.contains("AlreadyLocked"),
                    "live owner Busy fence, not an unrelated setup refusal"
                );
            }
            child.alive()?;
        }
        child.kill_wait()?;
        println!("M05 parent_killed_and_waited=true point={point}");
        let recovered = json_cli(&current(), &branch.upgrade(true))?;
        ensure!(
            recovered["source_id"] == source,
            "immutable source identity through kill"
        );
        if point == "fence-staged" {
            ensure!(
                recovered.get("storage_stage").is_none(),
                "uncommitted Fence remains V4"
            );
        } else {
            ensure!(recovered.get("migration_id").is_some(), "durable Root7");
        }
        if matches!(point, "ready-committed" | "profile-staged") {
            ensure!(
                recovered["storage_stage"] == "Ready"
                    && !branch.base.join("indexed.profile").exists(),
                "durable Ready before public profile"
            );
        }
        if point == "copy-committed" {
            ensure!(
                recovered["copied_entries"].as_u64().context("position")? > 0
                    && recovered["copied_entries"] != recovered["total_entries"],
                "actual middle copy"
            );
            let paused = json_cli(&current(), &branch.args("storage-upgrade-pause"))?;
            ensure!(paused["storage_stage"] == "Paused", "durable offline pause");
        }
        let ready = json_cli(&current(), &branch.upgrade(false))?;
        ensure!(
            ready["storage_stage"] == "Ready" && ready["zero_migration_native_calls"] == true,
            "storage Ready after exact resume"
        );
        let repeated = json_cli(&current(), &branch.upgrade(false))?;
        ensure!(
            repeated["generation"] == ready["generation"]
                && repeated["credits"]["repair_free"] == ready["credits"]["repair_free"],
            "Ready resume neither recopy nor funding"
        );
        let profile = branch.base.join("indexed.profile");
        let error = rejected(
            &current(),
            &[
                "runtime-from-profile".into(),
                "--profile-file".into(),
                text(&profile),
            ],
        )?;
        ensure!(
            error.contains("S5.2"),
            "storage Ready must not enter live old/fresh runtime"
        );
        let mut history = branch.args("storage-history");
        history.extend([
            "--context".into(),
            branch.context.clone(),
            "--offset".into(),
            "0".into(),
            "--limit".into(),
            "8".into(),
        ]);
        let page = json_cli(&current(), &history)?;
        ensure!(
            page["total"] == 4 && page["items"].as_array().context("history")?.len() == 4,
            "bounded old exact history"
        );
        let expected: Value = serde_json::from_slice(&std::fs::read(
            PathBuf::from(std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("seed")?)
                .join("one.history.json"),
        )?)?;
        let actual = page["items"].as_array().context("history array")?;
        let wanted = expected.as_array().context("old history array")?;
        ensure!(
            actual
                .iter()
                .map(|v| (&v["event_id"], &v["request_id"], &v["text"]))
                .eq(wanted
                    .iter()
                    .map(|v| (&v["event_id"], &v["request_id"], &v["text"]))),
            "exact d9b IDs/requests/UTF8 order"
        );
        std::fs::write(
            branch.base.join("parent-result.json"),
            serde_json::to_vec_pretty(
                &json!({"point":point,"source_id":source,"recovered":recovered,"ready":ready,"private_bytes_at_pause":memory,"parent_alive_before_kill":true,"parent_killed":true,"history":page}),
            )?,
        )?;
    }
    Ok(())
}

struct OldNode {
    state: PathBuf,
    root: PathBuf,
    credentials: PathBuf,
    account: String,
    device: String,
    profile: PathBuf,
    ipc: PathBuf,
    locators: PathBuf,
}
fn field(output: &str, name: &str) -> Result<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}=")))
        .map(ToOwned::to_owned)
        .with_context(|| format!("missing public CLI field {name}"))
}
impl OldNode {
    fn enrolled(base: &Path, index: usize) -> Result<Self> {
        let root = base.join(format!("root{index}"));
        let state = base.join(format!("state{index}"));
        let credentials = base.join(format!("credentials{index}"));
        let account = field(
            &cli(
                &frozen(),
                &["account-create".into(), "--account-dir".into(), text(&root)],
            )?,
            "account_id",
        )?;
        let device = field(
            &cli(
                &frozen(),
                &[
                    "device-enroll".into(),
                    "--profile-mode".into(),
                    "ordered-dialog-v1".into(),
                    "--account-dir".into(),
                    text(&root),
                    "--state-dir".into(),
                    text(&state),
                ],
            )?,
            "device_id",
        )?;
        cli(
            &frozen(),
            &[
                "ordered".into(),
                "credentials-export".into(),
                "--state-dir".into(),
                text(&state),
                "--output-file".into(),
                text(&credentials),
            ],
        )?;
        Ok(Self {
            state,
            root,
            credentials,
            account,
            device,
            profile: base.join(format!("profile{index}")),
            ipc: base.join(format!("ipc{index}")),
            locators: base.join(format!("locators{index}")),
        })
    }
    fn profile(&self, c: &str) -> Result<()> {
        cli(
            &frozen(),
            &[
                "runtime-profile-create".into(),
                "--profile-mode".into(),
                "ordered-dialog-v1".into(),
                "--profile-file".into(),
                text(&self.profile),
                "--state-dir".into(),
                text(&self.state),
                "--ipc-file".into(),
                text(&self.ipc),
                "--locator-directory".into(),
                text(&self.locators),
                "--ordered-context".into(),
                c.into(),
            ],
        )?;
        Ok(())
    }
    fn runtime(&self) -> Vec<String> {
        vec![
            "runtime-from-profile".into(),
            "--profile-file".into(),
            text(&self.profile),
        ]
    }
}
fn old_pair(base: &Path) -> Result<([OldNode; 2], String)> {
    let mut nodes = [OldNode::enrolled(base, 0)?, OldNode::enrolled(base, 1)?];
    nodes.sort_by(|a, b| a.device.cmp(&b.device));
    let [a, b] = &nodes;
    let invitation = base.join("invitation");
    let signed = base.join("countersigned");
    let context = field(
        &cli(
            &frozen(),
            &[
                "ordered".into(),
                "context-create".into(),
                "--state-dir".into(),
                text(&a.state),
                "--account-dir".into(),
                text(&a.root),
                "--peer-credentials-file".into(),
                text(&b.credentials),
                "--expect-account".into(),
                b.account.clone(),
                "--output-file".into(),
                text(&invitation),
            ],
        )?,
        "context",
    )?;
    cli(
        &frozen(),
        &[
            "ordered".into(),
            "context-import".into(),
            "--state-dir".into(),
            text(&b.state),
            "--account-dir".into(),
            text(&b.root),
            "--invitation-file".into(),
            text(&invitation),
            "--expect-account".into(),
            a.account.clone(),
            "--output-file".into(),
            text(&signed),
        ],
    )?;
    cli(
        &frozen(),
        &[
            "ordered".into(),
            "context-import".into(),
            "--state-dir".into(),
            text(&a.state),
            "--invitation-file".into(),
            text(&signed),
            "--expect-account".into(),
            b.account.clone(),
        ],
    )?;
    for node in &nodes {
        node.profile(&context)?;
    }
    Ok((nodes, context))
}
fn import_peer(node: &OldNode, peer: &OldNode, c: &str) -> Result<()> {
    cli(
        &frozen(),
        &[
            "ordered".into(),
            "locator-import".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--context".into(),
            c.into(),
            "--locator-file".into(),
            text(&peer.locators.join(format!("{c}.locator"))),
        ],
    )?;
    Ok(())
}

#[test]
fn migration_s5_process_real_d9b_partial_control_peer_origins() -> Result<()> {
    // Only the *current remote peer* receives debug controls. The origin is the
    // immutable d9b EXE, using its real authenticated ALPN1 state machine.
    for (point, victim_role, phase) in [
        ("after-offer", 1, 0),
        ("after-grant", 0, 1),
        ("before-init-commit", 1, 2),
        ("before-confirm-commit", 0, 3),
        ("after-receive-confirm", 1, 3),
    ] {
        let base = run_dir(&format!("partial-{point}-"))?;
        let (nodes, c) = old_pair(&base)?;
        let victim = &nodes[victim_role];
        let helper = &nodes[1 - victim_role];
        let schedule = base.join("control-peer.schedule.json");
        std::fs::write(
            &schedule,
            serde_json::to_vec(&json!({"pause_point":point}))?,
        )?;
        let mut origin = spawn(
            &frozen(),
            &victim.runtime(),
            &base,
            "d9b-origin",
            None,
            None,
        )?;
        let mut peer = spawn(
            &control_peer(),
            &helper.runtime(),
            &base,
            "alpn1-control-peer",
            None,
            Some(&schedule),
        )?;
        wait_file(&mut origin, &victim.ipc, 20)?;
        wait_file(&mut peer, &helper.ipc, 20)?;
        // B's locator first avoids an artificial failed A dial/backoff.
        import_peer(&nodes[1], &nodes[0], &c)?;
        import_peer(&nodes[0], &nodes[1], &c)?;
        wait_file(&mut peer, &schedule.with_extension("hit"), 25)?;
        origin.alive()?;
        peer.alive()?;
        let status = json_cli(
            &frozen(),
            &[
                "ordered".into(),
                "status".into(),
                "--ipc-file".into(),
                text(&victim.ipc),
            ],
        )?;
        ensure!(
            status["contexts"][0]["phase"].as_u64() == Some(phase),
            "actual frozen partial phase {status}"
        );
        if phase < 3 {
            cli(
                &frozen(),
                &[
                    "ordered".into(),
                    "queue".into(),
                    "--ipc-file".into(),
                    text(&victim.ipc),
                    "--context".into(),
                    c.clone(),
                    "--request-id".into(),
                    "00000000000000000000000000000071".into(),
                    "--text".into(),
                    "queued partial / Привет".into(),
                ],
            )?;
        }
        println!(
            "M04 actual_d9b_phase={phase} role={victim_role} remote_holds={point} origin_pid={} peer_pid={} both_alive=true",
            origin.0.id(),
            peer.0.id()
        );
        origin.kill_wait()?;
        peer.kill_wait()?;
        let claim = std::fs::read(victim.state.join("runtime/od1/profile-claim"))?;
        let value = kilogram_protocol::ordered_dialog_v1::decode(&claim, 1024)?;
        let mut branch = Branch {
            base: base.clone(),
            state: victim.state.clone(),
            account: victim.account.clone(),
            device: victim.device.clone(),
            config: base.join("migration.json"),
            context: c,
            claim: kilogram_cli::ordered_dialog::state::hex(&value.fields(5)?[2].fixed::<32>()?),
        };
        branch.write_config()?;
        let checked = json_cli(&current(), &branch.upgrade(true))?;
        ensure!(
            checked["origins"][0]["native_phase"].as_u64() == Some(phase),
            "production actual Native preflight"
        );
        let migrated = json_cli(&current(), &branch.upgrade(false))?;
        ensure!(
            migrated["storage_stage"] == "Ready"
                && migrated["contexts"][0]["native_phase"].as_u64() == Some(phase)
                && migrated["zero_migration_native_calls"] == true,
            "partial storage ready preserves exact Native phase"
        );
        if phase < 3 {
            ensure!(
                migrated["contexts"][0]["queue"] == 1,
                "actual sealed queue retained without materialization"
            );
        }
        std::fs::write(
            base.join("parent-result.json"),
            serde_json::to_vec_pretty(
                &json!({"remote_holds":point,"parent_killed_both":true,"d9b_before":status,"preflight":checked,"migrated":migrated}),
            )?,
        )?;
        println!(
            "M04 partial phase={phase} actual_queued={} migration_crypto=0 storage_ready=true root={}",
            phase < 3,
            base.display()
        );
    }
    Ok(())
}

fn schedule_file(path: &Path, value: &Value) -> Result<()> {
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("schedule parent")?)?;
    temp.write_all(&serde_json::to_vec(value)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}
fn old_status(node: &OldNode) -> Result<Value> {
    json_cli(
        &frozen(),
        &[
            "ordered".into(),
            "status".into(),
            "--ipc-file".into(),
            text(&node.ipc),
        ],
    )
}
fn wait_old(
    node: &OldNode,
    origin: &mut OwnedChild,
    peer: &mut OwnedChild,
    predicate: impl Fn(&Value) -> bool,
) -> Result<Value> {
    let start = Instant::now();
    loop {
        origin.alive()?;
        peer.alive()?;
        let status = old_status(node)?;
        if predicate(&status["contexts"][0]) {
            return Ok(status);
        }
        ensure!(
            start.elapsed() < Duration::from_secs(25),
            "frozen state deadline {status}"
        );
        std::thread::sleep(Duration::from_millis(60));
    }
}
#[test]
fn migration_s5_process_actual_renewed_pending_and_undelivered() -> Result<()> {
    let base = run_dir("renewed-pending-")?;
    let (nodes, c) = old_pair(&base)?;
    let victim = &nodes[0];
    let helper = &nodes[1];
    let schedule = base.join("control-peer.schedule.json");
    schedule_file(
        &schedule,
        &json!({"hold_lane":1,"hold_ordinals":[1,2],"direct_batch":1,"disable_ack":true,"suspend_prefix_requests":true}),
    )?;
    let mut origin = spawn(
        &frozen(),
        &victim.runtime(),
        &base,
        "d9b-origin",
        None,
        None,
    )?;
    let mut peer = spawn(
        &control_peer(),
        &helper.runtime(),
        &base,
        "alpn1-control-peer",
        None,
        Some(&schedule),
    )?;
    wait_file(&mut origin, &victim.ipc, 20)?;
    wait_file(&mut peer, &helper.ipc, 20)?;
    import_peer(&nodes[1], &nodes[0], &c)?;
    import_peer(&nodes[0], &nodes[1], &c)?;
    wait_old(victim, &mut origin, &mut peer, |s| s["status"] == "Active")?;
    let queue = |node: &OldNode, n: u64| -> Result<()> {
        cli(
            &frozen(),
            &[
                "ordered".into(),
                "queue".into(),
                "--ipc-file".into(),
                text(&node.ipc),
                "--context".into(),
                c.clone(),
                "--request-id".into(),
                format!("{n:032x}"),
                "--text".into(),
                format!("actual-obligation-{n} / Привет"),
            ],
        )?;
        Ok(())
    };
    queue(victim, 101)?;
    wait_old(victim, &mut origin, &mut peer, |s| {
        s["send_next"].as_u64() == Some(2)
    })?;
    queue(helper, 201)?;
    queue(helper, 202)?;
    wait_old(helper, &mut origin, &mut peer, |s| {
        s["send_next"].as_u64() == Some(3)
    })?;
    // Real Root-admin renewal happens outside the running state directory. Both
    // signed Text rows already carry the original snapshot before this renewal.
    cli(
        &frozen(),
        &[
            "device-enroll".into(),
            "--profile-mode".into(),
            "ordered-dialog-v1".into(),
            "--account-dir".into(),
            text(&helper.root),
            "--state-dir".into(),
            text(&base.join("new-authorized-device")),
        ],
    )?;
    let snapshot = base.join("renewed.snapshot");
    cli(
        &frozen(),
        &[
            "account-snapshot".into(),
            "--account-dir".into(),
            text(&helper.root),
            "--snapshot-file".into(),
            text(&snapshot),
        ],
    )?;
    for node in [victim, helper] {
        cli(
            &frozen(),
            &[
                "ordered".into(),
                "authority-import".into(),
                "--ipc-file".into(),
                text(&node.ipc),
                "--snapshot-file".into(),
                text(&snapshot),
            ],
        )?;
    }
    schedule_file(
        &schedule,
        &json!({"hold_lane":1,"hold_ordinals":[1],"direct_batch":1,"disable_ack":true,"suspend_prefix_requests":true}),
    )?;
    wait_old(victim, &mut origin, &mut peer, |s| {
        s["ordinary_pending"].as_u64() == Some(1)
    })?;
    queue(victim, 102)?;
    let status = wait_old(victim, &mut origin, &mut peer, |s| {
        s["ordinary_pending"].as_u64() == Some(1) && s["unacked"].as_u64() == Some(1)
    })?;
    println!(
        "M11 actual_d9b_pending=1 own_unacked=1 renewed_snapshot=true origin_pid={} peer_pid={}",
        origin.0.id(),
        peer.0.id()
    );
    origin.kill_wait()?;
    peer.kill_wait()?;
    let claim = std::fs::read(victim.state.join("runtime/od1/profile-claim"))?;
    let v = kilogram_protocol::ordered_dialog_v1::decode(&claim, 1024)?;
    let mut branch = Branch {
        base: base.clone(),
        state: victim.state.clone(),
        account: victim.account.clone(),
        device: victim.device.clone(),
        config: base.join("migration.json"),
        context: c,
        claim: kilogram_cli::ordered_dialog::state::hex(&v.fields(5)?[2].fixed::<32>()?),
    };
    branch.write_config()?;
    let preview = json_cli(&current(), &branch.upgrade(true))?;
    let pending = &preview["origins"][0]["pending"][0];
    ensure!(
        pending["original"] != pending["admission"] && pending["head"] == false,
        "actual distinct historical O/admission A and ordinary slot"
    );
    let ready = json_cli(&current(), &branch.upgrade(false))?;
    ensure!(
        ready["contexts"][0]["pending"] == 1
            && ready["contexts"][0]["undelivered"] == 1
            && ready["zero_migration_native_calls"] == true,
        "captured pending/debt preserved without Delivered or new F/R"
    );
    std::fs::write(
        base.join("parent-result.json"),
        serde_json::to_vec_pretty(
            &json!({"actual_d9b_before":status,"preflight":preview,"ready":ready,"parent_killed_both":true}),
        )?,
    )?;
    println!(
        "M11 renewed O/A, ordinary pending, own unacked debt storage PASS root={}",
        base.display()
    );
    Ok(())
}

#[test]
fn migration_s5_process_capacity_crash_and_predecessor_fences() -> Result<()> {
    for point in ["capacity-staged", "capacity-committed"] {
        let branch = Branch::from_seed("capacity-version-")?;
        let first = json_cli(&current(), &branch.upgrade(false))?;
        let mut cfg: Value = serde_json::from_slice(&std::fs::read(&branch.config)?)?;
        cfg["logical_quota"][0] = json!(cfg["logical_quota"][0].as_u64().context("quota")? + 4096);
        std::fs::write(&branch.config, serde_json::to_vec_pretty(&cfg)?)?;
        let mut args = branch.args("storage-increase-capacity");
        args.extend([
            "--config-file".into(),
            text(&branch.config),
            "--expected-root-generation".into(),
            first["generation"].to_string(),
            "--expected-config-revision".into(),
            first["config_revision"].to_string(),
            "--expected-config-hash".into(),
            first["config_hash"].as_str().context("config hash")?.into(),
        ]);
        let pause = branch.base.join("pause");
        std::fs::write(&pause, point)?;
        let mut child = spawn(
            &current(),
            &args,
            &branch.base,
            "capacity",
            Some(&pause),
            None,
        )?;
        wait_file(&mut child, &pause.with_extension("hit"), 20)?;
        let pid = child.0.id();
        child.kill_wait()?;
        let candidate =
            kilogram_cli::ordered_dialog::storage_migration::load_config(&branch.config)?;
        let mut recovered = kilogram_state::indexed_dialog::migration7::MigrationRepository::open(
            kilogram_state::StateDirectoryLock::acquire(&branch.state)?,
            kilogram_cli::ordered_dialog::state::parse_id(&branch.account)?,
            kilogram_cli::ordered_dialog::state::parse_id(&branch.device)?,
            Some(&candidate),
        )?;
        let outcome = serde_json::to_value(recovered.status()?)?;
        drop(recovered);
        ensure!(
            outcome["config_revision"].as_u64()
                == Some(if point == "capacity-staged" { 1 } else { 2 }),
            "whole old/new capacity checkpoint {point}"
        );
        let check = json_cli(&current(), &args)?; // exact retry resolves either outcome
        ensure!(
            check["config_revision"].as_u64() == Some(2)
                && check["migration_id"] == first["migration_id"]
                && check["credits"]["repair_free"] == first["credits"]["repair_free"],
            "capacity commit kill preserves identity/funding"
        );
        ensure!(
            json_cli(&current(), &args)?["generation"] == check["generation"],
            "exact capacity retry idempotent"
        );
        let mut stale = args.clone();
        let index = stale
            .iter()
            .position(|v| v == "--expected-config-revision")
            .context("revision argument")?;
        stale[index + 1] = "999".into();
        rejected(&current(), &stale)?;
        let old = std::fs::read(branch.base.join("indexed.profile"))?;
        ensure!(
            rejected(&current(), &branch.upgrade(false))?.contains("ReadyNeedsProfileUpdate"),
            "explicit file CAS after capacity"
        );
        let mut resume = branch.upgrade(false);
        resume.extend([
            "--expected-profile-hash".into(),
            kilogram_cli::ordered_dialog::state::hex(blake3::hash(&old).as_bytes()),
        ]);
        json_cli(&current(), &resume)?;
        let cold = [
            "ordered".into(),
            "inspect".into(),
            "--state-dir".into(),
            text(&branch.state),
        ];
        let error = rejected(&frozen(), &cold)?;
        ensure!(
            error.contains("schema") || error.contains("Unsupported"),
            "actual d9b schema refusal"
        );
        let s4 =
            workspace().join("target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe");
        let s4_error = rejected(
            &s4,
            &[
                "ordered".into(),
                "context2-import".into(),
                "--state-dir".into(),
                text(&branch.state),
                "--expect-account".into(),
                branch.account.clone(),
                "--expect-claim".into(),
                branch.claim.clone(),
                "--invitation-file".into(),
                text(&branch.base.join("must-not-be-read")),
            ],
        )?;
        ensure!(
            s4_error.contains("anchor version") || s4_error.contains("UnsupportedRuntimeProfile"),
            "exact S4 rejects Root7 before missing invitation IO"
        );
        let s4_root = branch.base.join("positive-s4-root");
        cli(
            &s4,
            &[
                "account-create".into(),
                "--account-dir".into(),
                text(&s4_root),
            ],
        )?;
        let s4_state = branch.base.join("positive-s4-state");
        let enrollment = cli(
            &s4,
            &[
                "device-enroll".into(),
                "--profile-mode".into(),
                "ordered-dialog-v2".into(),
                "--account-dir".into(),
                text(&s4_root),
                "--state-dir".into(),
                text(&s4_state),
                "--ordered-context".into(),
                "15".repeat(32),
                "--ordered-context".into(),
                "16".repeat(32),
            ],
        )?;
        let pins: Value = serde_json::from_str(
            enrollment
                .lines()
                .last()
                .context("S4 public enrollment pins")?,
        )?;
        let positive = kilogram_state::indexed_dialog::RuntimeRepository::open(
            kilogram_state::StateDirectoryLock::acquire(&s4_state)?,
            kilogram_state::indexed_dialog::ExpectedOwner {
                account: kilogram_cli::ordered_dialog::state::parse_id(
                    pins["account"].as_str().context("account")?,
                )?,
                claim: kilogram_cli::ordered_dialog::state::parse_id(
                    pins["claim"].as_str().context("claim")?,
                )?,
            },
            kilogram_state::indexed_dialog::Config::default(),
        )?;
        drop(positive);
        let s3 = workspace().join(
            "target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe",
        );
        let config = kilogram_state::indexed_dialog::Config::default();
        let probe = vec![
            "probe".into(),
            text(&branch.state),
            branch.account.clone(),
            branch.claim.clone(),
            config.logical_quota[0].to_string(),
            config.logical_quota[1].to_string(),
            config.physical_quota.to_string(),
        ];
        ensure!(
            cli(&s3, &probe)?.contains("S3_PREDECESSOR_REFUSED"),
            "actual archived S3 refuses Root7"
        );
        let mut positive = probe.clone();
        positive[0] = "positive".into();
        positive[1] = text(&branch.base.join("positive-s3"));
        ensure!(
            cli(&s3, &positive)?.contains("S3_PREDECESSOR_POSITIVE"),
            "correct owner/config S3 positive control"
        );
        let final_status = json_cli(&current(), &branch.args("storage-upgrade-status"))?;
        ensure!(
            final_status["generation"] == check["generation"],
            "predecessor refusal never mutates Root7"
        );
        for corruption in ["schema-relabel", "missing-anchor", "cipher-anchor"] {
            let key = if corruption == "schema-relabel" {
                "snapshot-manifest"
            } else {
                "od2-root-v5"
            };
            let old = edit_metadata(&branch.state, key, |bytes| {
                if corruption == "missing-anchor" {
                    None
                } else {
                    let mut changed = bytes.to_vec();
                    if corruption == "schema-relabel" {
                        changed[0] = 4;
                    } else {
                        changed[32] ^= 1;
                    }
                    Some(changed)
                }
            })?;
            let error = rejected(&current(), &branch.upgrade(true))?;
            ensure!(
                !error.contains("must-not") && !error.contains("CapacityCAS"),
                "malformed Root7 rejected at authoritative header/source"
            );
            edit_metadata(&branch.state, key, |_| Some(old.clone()))?;
            ensure!(
                json_cli(&current(), &branch.args("storage-upgrade-status"))?["generation"]
                    == check["generation"],
                "exact protected root restored after negative fixture"
            );
            println!("M09 actual_root7_corruption={corruption} refused=true");
        }
        let mut profile: Value =
            serde_json::from_slice(&std::fs::read(branch.base.join("indexed.profile"))?)?;
        profile["profile_mode"] = "ordered-dialog-v1".into();
        let relabel = branch.base.join("relabel.profile");
        std::fs::write(&relabel, serde_json::to_vec(&profile)?)?;
        let error = rejected(
            &current(),
            &[
                "runtime-from-profile".into(),
                "--profile-file".into(),
                text(&relabel),
            ],
        )?;
        ensure!(
            error.contains("unknown field") || error.contains("Unsupported"),
            "profile label cannot adopt Root7"
        );
        println!(
            "M08/M09 capacity point={point} parent_alive_killed=true pid={pid} archived_S3/exact_S4/d9b_root7_fences PASS root={}",
            branch.base.display()
        );
    }
    Ok(())
}

fn edit_metadata(
    path: &Path,
    key: &str,
    edit: impl FnOnce(&[u8]) -> Option<Vec<u8>>,
) -> Result<Vec<u8>> {
    use redb::ReadableTable;
    let _lease = kilogram_state::StateDirectoryLock::acquire(path)?;
    let mut builder = redb::Database::builder();
    builder.set_cache_size(64 * 1024 * 1024);
    let database = builder.open(path.join("state-vault.redb"))?;
    let tx = database.begin_write()?;
    let prior = {
        let mut table =
            tx.open_table(redb::TableDefinition::<&str, &[u8]>::new("vault-meta-v1"))?;
        let prior = table
            .get(key)?
            .map(|v| v.value().to_vec())
            .unwrap_or_default();
        ensure!(prior.len() <= 32 * 1024, "metadata negative cap");
        if let Some(next) = edit(&prior) {
            table.insert(key, next.as_slice())?;
        } else {
            table.remove(key)?;
        }
        prior
    };
    tx.commit()?;
    Ok(prior)
}

#[test]
fn migration_s5_process_two_context_escrow_and_history() -> Result<()> {
    let base = run_dir("two-context-")?;
    let (nodes, c0) = old_pair(&base)?;
    let [a, b] = &nodes;
    let invitation = base.join("invitation2");
    let signed = base.join("countersigned2");
    let c1 = field(
        &cli(
            &frozen(),
            &[
                "ordered".into(),
                "context-create".into(),
                "--state-dir".into(),
                text(&a.state),
                "--account-dir".into(),
                text(&a.root),
                "--peer-credentials-file".into(),
                text(&b.credentials),
                "--expect-account".into(),
                b.account.clone(),
                "--output-file".into(),
                text(&invitation),
            ],
        )?,
        "context",
    )?;
    cli(
        &frozen(),
        &[
            "ordered".into(),
            "context-import".into(),
            "--state-dir".into(),
            text(&b.state),
            "--account-dir".into(),
            text(&b.root),
            "--invitation-file".into(),
            text(&invitation),
            "--expect-account".into(),
            a.account.clone(),
            "--output-file".into(),
            text(&signed),
        ],
    )?;
    cli(
        &frozen(),
        &[
            "ordered".into(),
            "context-import".into(),
            "--state-dir".into(),
            text(&a.state),
            "--invitation-file".into(),
            text(&signed),
            "--expect-account".into(),
            b.account.clone(),
        ],
    )?;
    for node in &nodes {
        let mut p: Value = serde_json::from_slice(&std::fs::read(&node.profile)?)?;
        p["enabled_contexts"] = json!([c0, c1]);
        std::fs::write(&node.profile, serde_json::to_vec(&p)?)?;
    }
    let mut first = spawn(&frozen(), &a.runtime(), &base, "d9b-a", None, None)?;
    let mut second = spawn(&frozen(), &b.runtime(), &base, "d9b-b", None, None)?;
    wait_file(&mut first, &a.ipc, 20)?;
    wait_file(&mut second, &b.ipc, 20)?;
    for c in [&c0, &c1] {
        import_peer(b, a, c)?;
        import_peer(a, b, c)?;
    }
    let start = Instant::now();
    loop {
        first.alive()?;
        second.alive()?;
        let status = old_status(a)?;
        if status["contexts"]
            .as_array()
            .context("contexts")?
            .iter()
            .all(|s| s["status"] == "Active")
        {
            break;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(25),
            "two-context admission {status}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    for (n, c) in [&c0, &c1].iter().enumerate() {
        cli(
            &frozen(),
            &[
                "ordered".into(),
                "queue".into(),
                "--ipc-file".into(),
                text(&a.ipc),
                "--context".into(),
                (*c).clone(),
                "--request-id".into(),
                format!("{:032x}", n + 301),
                "--text".into(),
                format!("two-context/{n}/Привет"),
            ],
        )?;
    }
    let start = Instant::now();
    let before = loop {
        first.alive()?;
        second.alive()?;
        let status = old_status(a)?;
        if status["contexts"]
            .as_array()
            .context("contexts")?
            .iter()
            .all(|s| s["send_next"] == 2 && s["unacked"] == 0)
        {
            break status;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(25),
            "two-context delivery {status}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    first.kill_wait()?;
    second.kill_wait()?;
    let claim = std::fs::read(a.state.join("runtime/od1/profile-claim"))?;
    let value = kilogram_protocol::ordered_dialog_v1::decode(&claim, 1024)?;
    let mut branch = Branch {
        base: base.clone(),
        state: a.state.clone(),
        account: a.account.clone(),
        device: a.device.clone(),
        config: base.join("migration.json"),
        context: c0.clone(),
        claim: kilogram_cli::ordered_dialog::state::hex(&value.fields(5)?[2].fixed::<32>()?),
    };
    branch.write_config()?;
    let preview = json_cli(&current(), &branch.upgrade(true))?;
    ensure!(
        preview["origins"].as_array().context("origins")?.len() == 2
            && preview["logical_need"][1].as_u64().context("C1 need")?
                > 147_980_288 + 64 * 1024 * 1024,
        "independent occupied C1 escrow/endowment"
    );
    let mut cfg: Value = serde_json::from_slice(&std::fs::read(&branch.config)?)?;
    cfg["logical_quota"] = preview["logical_need"].clone();
    cfg["logical_quota"][1] = json!(cfg["logical_quota"][1].as_u64().context("quota1")? - 1);
    std::fs::write(&branch.config, serde_json::to_vec(&cfg)?)?;
    ensure!(
        rejected(&current(), &branch.upgrade(false))?.contains("quota"),
        "C1 need-1 cannot borrow owner0 source backing"
    );
    cfg["logical_quota"] = preview["logical_need"].clone();
    std::fs::write(&branch.config, serde_json::to_vec(&cfg)?)?;
    let ready = json_cli(&current(), &branch.upgrade(false))?;
    ensure!(
        ready["credits"]["repair_free"] == json!([64 * 1024 * 1024, 64 * 1024 * 1024]),
        "one funded pool per actual C"
    );
    ensure!(
        rejected(
            &current(),
            &[
                "runtime-from-profile".into(),
                "--profile-file".into(),
                text(&branch.base.join("indexed.profile"))
            ]
        )?
        .contains("live indexed runtime unavailable"),
        "published storage profile cannot launch a live owner"
    );
    for c in [&c0, &c1] {
        let mut args = branch.args("storage-history");
        args.extend(["--context".into(), c.clone(), "--limit".into(), "8".into()]);
        let history = json_cli(&current(), &args)?;
        ensure!(
            history["total"] == 1
                && history["items"][0]["text"]
                    .as_str()
                    .context("text")?
                    .contains("two-context/"),
            "exact C-scoped legacy history"
        );
    }
    std::fs::write(
        base.join("parent-result.json"),
        serde_json::to_vec_pretty(&json!({"before":before,"preflight":preview,"ready":ready}))?,
    )?;
    println!(
        "M01/M08/M10 actual_two_C=true exact_need/need1-1/no_cross_funding/history PASS root={}",
        base.display()
    );
    Ok(())
}
