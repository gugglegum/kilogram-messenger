//! Actual public process continuation. Fixtures contain real d9b state only.
use super::*;
#[path = "migration_s5_l1_atomic.rs"]
mod atomic;
#[path = "attempt_result.rs"]
mod attempt_result;
#[path = "migration_s5_l1_exhausted.rs"]
mod exhausted;
#[path = "migration_s5_l1_isolation.rs"]
mod isolation;
#[path = "migration_s5_l1_obligations.rs"]
mod obligations;
#[path = "migration_s5_l1_policy.rs"]
mod policy;
#[path = "migration_s5_l1_review1.rs"]
mod review1;

struct Node {
    branch: Branch,
    profile: PathBuf,
    ipc: PathBuf,
    locators: PathBuf,
    indexed: bool,
    slot: u8,
}
impl Node {
    fn from_ready(base: &Path, context: &str) -> Result<Self> {
        let profile_path = base.join("indexed.profile");
        let profile = kilogram_cli::ordered_dialog::runtime7::Profile3::load(&profile_path)?;
        let repository = profile.open(&profile_path)?;
        let slot = repository
            .contexts()
            .iter()
            .position(|(c, _)| kilogram_cli::ordered_dialog::state::hex(c) == context)
            .context("existing indexed C")? as u8;
        drop(repository);
        Ok(Self {
            branch: Branch {
                base: base.into(),
                state: profile.state_dir,
                account: profile.account,
                device: profile.device,
                config: base.join("migration.json"),
                context: context.into(),
                claim: profile.claim,
            },
            profile: profile_path,
            ipc: profile.ipc_file,
            locators: profile.locator_directory,
            indexed: true,
            slot,
        })
    }
    fn from_old(old: &OldNode, context: &str) -> Result<Self> {
        let base = old
            .state
            .parent()
            .context("origin parent")?
            .join(format!("live-{}", &old.device[..8]));
        std::fs::create_dir(&base)?;
        let raw = std::fs::read(old.state.join("runtime/od1/profile-claim"))?;
        let claim = kilogram_protocol::ordered_dialog_v1::decode(&raw, 1024)?;
        let mut branch = Branch {
            base: base.clone(),
            state: old.state.clone(),
            account: old.account.clone(),
            device: old.device.clone(),
            config: base.join("migration.json"),
            context: context.into(),
            claim: kilogram_cli::ordered_dialog::state::hex(&claim.fields(5)?[2].fixed::<32>()?),
        };
        branch.write_config()?;
        Ok(Self {
            branch,
            profile: old.profile.clone(),
            ipc: old.ipc.clone(),
            locators: old.locators.clone(),
            indexed: false,
            slot: 0,
        })
    }
    fn exe(&self) -> PathBuf {
        if self.indexed { current() } else { frozen() }
    }
    fn from_seed(seed: &Path, side: &str, base: &Path, context: &str) -> Result<Self> {
        let base = base.join(side);
        std::fs::create_dir(&base)?;
        let state = base.join("state");
        copy_tree(&seed.join(format!("{side}.state")), &state)?;
        let cert = Credentials::parse(&std::fs::read(seed.join(format!("{side}.credentials")))?)?;
        let raw = std::fs::read(state.join("runtime/od1/profile-claim"))?;
        let claim = kilogram_protocol::ordered_dialog_v1::decode(&raw, 1024)?;
        let mut branch = Branch {
            state,
            account: cert.cert.account_id().to_string(),
            device: cert.cert.device_id().to_string(),
            context: context.into(),
            claim: kilogram_cli::ordered_dialog::state::hex(&claim.fields(5)?[2].fixed::<32>()?),
            config: base.join("migration.json"),
            base: base.clone(),
        };
        branch.write_config()?;
        let profile = base.join("legacy.profile");
        let ipc = base.join("legacy.ipc");
        let locators = base.join("legacy.locators");
        let mut cfg: Value =
            serde_json::from_slice(&std::fs::read(seed.join(format!("{side}.profile")))?)?;
        cfg["state_dir"] = text(&branch.state).into();
        cfg["ipc_file"] = text(&ipc).into();
        cfg["locator_directory"] = text(&locators).into();
        std::fs::write(&profile, serde_json::to_vec_pretty(&cfg)?)?;
        Ok(Self {
            branch,
            profile,
            ipc,
            locators,
            indexed: false,
            slot: 0,
        })
    }
    fn migrate(&mut self) -> Result<Value> {
        let ready = json_cli(&current(), &self.branch.upgrade(false))?;
        ensure!(
            ready["storage_stage"] == "Ready" && ready["zero_migration_native_calls"] == true,
            "real storage Ready"
        );
        self.indexed = true;
        self.slot = ready["contexts"]
            .as_array()
            .context("Ready contexts")?
            .iter()
            .position(|v| v["context"] == self.branch.context)
            .context("actual C slot")? as u8;
        self.profile = self.branch.base.join("indexed.profile");
        self.ipc = self.branch.base.join("indexed.ipc");
        self.locators = self.branch.base.join("indexed.locators");
        Ok(ready)
    }
    fn context<'a>(&self, status: &'a Value) -> Result<&'a Value> {
        status["contexts"]
            .as_array()
            .context("status contexts")?
            .iter()
            .find(|v| v["context"] == self.branch.context)
            .context("status C scope")
    }
    fn view(&self, context: &str, slot: u8) -> Self {
        Self {
            branch: Branch {
                base: self.branch.base.clone(),
                state: self.branch.state.clone(),
                account: self.branch.account.clone(),
                device: self.branch.device.clone(),
                config: self.branch.config.clone(),
                context: context.into(),
                claim: self.branch.claim.clone(),
            },
            profile: self.profile.clone(),
            ipc: self.ipc.clone(),
            locators: self.locators.clone(),
            indexed: self.indexed,
            slot,
        }
    }
    fn start(&self, name: &str) -> Result<OwnedChild> {
        self.start_controlled(name, None, None)
    }
    fn start_controlled(
        &self,
        name: &str,
        schedule: Option<&Path>,
        clock: Option<&Path>,
    ) -> Result<OwnedChild> {
        if self.ipc.exists() {
            std::fs::remove_file(&self.ipc)?;
        }
        let mut cmd = command(&self.exe());
        cmd.args(["runtime-from-profile", "--profile-file"])
            .arg(&self.profile)
            .stdout(Stdio::from(std::fs::File::create(
                self.branch.base.join(format!("{name}.stdout.log")),
            )?))
            .stderr(Stdio::from(std::fs::File::create(
                self.branch.base.join(format!("{name}.stderr.log")),
            )?));
        if let Some(path) = schedule {
            cmd.env("KILOGRAM_TEST_OD1_SCHEDULE", path);
        }
        if let Some(path) = clock {
            cmd.env("KILOGRAM_TEST_OD1_CLOCK_FILE", path);
        }
        let mut child = OwnedChild(cmd.spawn()?);
        wait_file(&mut child, &self.ipc, 15)
            .with_context(|| format!("runtime startup logs at {}", self.branch.base.display()))?;
        Ok(child)
    }
    fn status(&self) -> Result<Value> {
        json_cli(
            &self.exe(),
            &[
                "ordered".into(),
                if self.indexed { "status3" } else { "status" }.into(),
                "--ipc-file".into(),
                text(&self.ipc),
            ],
        )
    }
    fn import(&self, peer: &Node) -> Result<()> {
        let mut args = vec![
            "ordered".into(),
            if self.indexed {
                "locator3-import"
            } else {
                "locator-import"
            }
            .into(),
            "--ipc-file".into(),
            text(&self.ipc),
        ];
        args.extend(if self.indexed {
            vec!["--slot".into(), self.slot.to_string()]
        } else {
            vec!["--context".into(), self.branch.context.clone()]
        });
        args.extend([
            "--locator-file".into(),
            text(
                &peer
                    .locators
                    .join(format!("{}.locator", self.branch.context)),
            ),
        ]);
        cli(&self.exe(), &args)?;
        Ok(())
    }
    fn send(&self, id: u64, body: &str) -> Result<String> {
        let mut args = vec![
            "ordered".into(),
            if self.indexed { "send3" } else { "queue" }.into(),
            "--ipc-file".into(),
            text(&self.ipc),
        ];
        args.extend(if self.indexed {
            vec!["--slot".into(), self.slot.to_string()]
        } else {
            vec!["--context".into(), self.branch.context.clone()]
        });
        args.extend([
            "--request-id".into(),
            format!("fa00000000000000{id:016x}"),
            "--text".into(),
            body.into(),
        ]);
        cli(&self.exe(), &args)
    }
    fn wait(&self, child: &mut OwnedChild, own: u64, peer: u64) -> Result<Value> {
        let start = Instant::now();
        loop {
            child.alive()?;
            let status = self.status()?;
            if self.reached(&status, own, peer)? {
                return Ok(status);
            }
            ensure!(
                // Frozen A can first spend its fixed15s admission timeout on
                // the copied old Locator. Match the accepted wait_old25s gate;
                // retain every assertion and charge the whole actual elapsed.
                start.elapsed() < Duration::from_secs(25),
                "continuation deadline {status}; logs={}",
                self.branch.base.display()
            );
            std::thread::sleep(Duration::from_millis(70));
        }
    }
    fn reached(&self, status: &Value, own: u64, peer: u64) -> Result<bool> {
        let c = self.context(status)?;
        Ok(if self.indexed {
            let r = c["role"].as_u64().context("role")? as usize;
            c["next"][r] == own
                && c["next"][1 - r] == peer
                && c["counts"]["debt"] == 0
                && c["counts"]["queued"] == 0
        } else {
            c["send_next"] == own && c["receive_next"] == peer && c["unacked"] == 0
        })
    }
    fn wait_before(
        &self,
        child: &mut OwnedChild,
        own: u64,
        peer: u64,
        deadline: Instant,
    ) -> Result<Value> {
        loop {
            ensure!(
                Instant::now() < deadline,
                "absolute large-drain phase deadline"
            );
            child.alive()?;
            let status = self.status()?;
            ensure!(
                Instant::now() <= deadline,
                "absolute large-drain phase deadline {status}"
            );
            if self.reached(&status, own, peer)? {
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(70));
        }
    }
    fn history(&self, limit: usize, cursor: Option<&str>) -> Result<Value> {
        ensure!(self.indexed, "IPC3 history");
        let mut args = vec![
            "ordered".into(),
            "history3".into(),
            "--ipc-file".into(),
            text(&self.ipc),
            "--slot".into(),
            self.slot.to_string(),
            "--limit".into(),
            limit.to_string(),
        ];
        if let Some(cursor) = cursor {
            args.extend(["--cursor".into(), cursor.into()]);
        }
        json_cli(&current(), &args)
    }
}

#[test]
fn migration_s5_l1_five_real_partial_origins_continue_once() -> Result<()> {
    for (point, role, phase, account_ctor, outbound_ctor, inbound_ctor) in [
        ("after-offer", 1, 0, 1, 0, 1),
        ("after-grant", 0, 1, 0, 1, 0),
        ("before-init-commit", 1, 2, 0, 0, 1),
        ("before-confirm-commit", 0, 3, 0, 0, 0),
        ("after-receive-confirm", 1, 3, 0, 0, 0),
    ] {
        let base = run_dir(&format!("l1-partial-{point}-"))?;
        let (nodes, c) = old_pair(&base)?;
        let victim = &nodes[role];
        let helper = &nodes[1 - role];
        let schedule = base.join("peer.schedule.json");
        schedule_file(&schedule, &json!({"pause_point":point}))?;
        let mut origin = spawn(
            &frozen(),
            &victim.runtime(),
            &base,
            "genuine-d9b",
            None,
            None,
        )?;
        let mut peer = spawn(
            &control_peer(),
            &helper.runtime(),
            &base,
            "control-peer",
            None,
            Some(&schedule),
        )?;
        wait_file(&mut origin, &victim.ipc, 15)?;
        wait_file(&mut peer, &helper.ipc, 15)?;
        import_peer(&nodes[1], &nodes[0], &c)?;
        import_peer(&nodes[0], &nodes[1], &c)?;
        wait_file(&mut peer, &schedule.with_extension("hit"), 20)?;
        let before = old_status(victim)?;
        ensure!(
            before["contexts"][0]["phase"] == phase,
            "actual d9b phase {before}"
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
                    "queued before migration / Привет".into(),
                ],
            )?;
        }
        origin.kill_wait()?;
        peer.kill_wait()?;
        copy_tree(&victim.state, &base.join("frozen-victim-original"))?;
        let mut live = Node::from_old(victim, &c)?;
        let old = Node::from_old(helper, &c)?;
        let ready = live.migrate()?;
        ensure!(
            ready["contexts"][0]["native_phase"] == phase,
            "unchanged storage phase"
        );
        let mut indexed = live.start("indexed-victim")?;
        if phase < 3 {
            ensure!(
                live.status()?["stats"]["packet_attempts"] == 0,
                "no pre-admission Native"
            );
        }
        let mut remote = old.start("frozen-completion-peer")?;
        if role == 0 {
            old.import(&live)?;
            live.import(&old)?;
        } else {
            live.import(&old)?;
            old.import(&live)?;
        }
        live.send(11, "indexed continuation of actual partial")?;
        old.send(12, "frozen opposite lane")?;
        let own = if phase < 3 { 3 } else { 2 };
        let after = live.wait(&mut indexed, own, 2)?;
        old.wait(&mut remote, 2, own)?;
        for (name, expected) in [
            ("account_constructors", account_ctor),
            ("outbound_constructors", outbound_ctor),
            ("inbound_constructors", inbound_ctor),
        ] {
            ensure!(
                after["stats"][name] == expected,
                "constructor boundary {point}/{name}: {after}"
            );
        }
        let history = live.history(32, None)?;
        ensure!(
            history["finished"] == true
                && history["items"]
                    .as_array()
                    .context("partial history")?
                    .len()
                    == (own as usize),
            "actual completed partial history"
        );
        indexed.kill_wait()?;
        remote.kill_wait()?;
        std::fs::write(
            base.join("parent-l1-result.json"),
            serde_json::to_vec_pretty(
                &json!({"point":point,"victim_role":role,"d9b_before":before,"ready":ready,"continued":after,"history":history,"parent_killed_and_waited":true}),
            )?,
        )?;
        println!(
            "L1_PARTIAL PASS point={point} original_phase={phase} exact_constructor_boundary=true queued_preserved={} root={}",
            phase < 3,
            base.display()
        );
    }
    Ok(())
}

#[test]
fn migration_s5_l1_small_real_d9b_then_both_indexed() -> Result<()> {
    let seed = PathBuf::from(std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged seed")?);
    let origin: Value = serde_json::from_slice(&std::fs::read(seed.join("result.json"))?)?;
    let context = origin["context"].as_str().context("C")?;
    let base = run_dir("l1-small-")?;
    println!(
        "L1_SMALL active_branch={} origin={} frozen_original_never_opened=true",
        base.display(),
        seed.display()
    );
    let mut a = Node::from_seed(
        &seed,
        origin["elected_a"]["side"].as_str().context("A")?,
        &base,
        context,
    )?;
    let mut b = Node::from_seed(
        &seed,
        origin["elected_b"]["side"].as_str().context("B")?,
        &base,
        context,
    )?;
    let ready = a.migrate()?;
    let mut ca = a.start("indexed-a")?;
    let mut cb = b.start("frozen-b")?;
    b.import(&a)?;
    a.import(&b)?;
    a.send(1, "indexed A → real d9b B / Привет")?;
    b.send(2, "real d9b B → indexed A")?;
    let sa = a.wait(&mut ca, 6, 6)?;
    b.wait(&mut cb, 6, 6)?;
    ensure!(
        sa["contexts"][0]["sid"]
            == origin[format!(
                "final_{}",
                origin["elected_a"]["side"].as_str().context("A side")?
            )]["contexts"][0]["session_id"],
        "same real original Session"
    );
    for name in [
        "account_constructors",
        "outbound_constructors",
        "inbound_constructors",
    ] {
        ensure!(sa["stats"][name] == 0, "existing object recreated: {name}");
    }
    let first = a.history(2, None)?;
    ensure!(
        first["items"].as_array().context("items")?.len() == 2 && first["finished"] == false,
        "bounded imported page"
    );
    let cursor = first["cursor"].as_str().context("cursor")?.to_owned();
    ca.kill_wait()?;
    cb.kill_wait()?;
    b.migrate()?;
    ca = a.start("reopened-indexed-a")?;
    cb = b.start("indexed-b")?;
    b.import(&a)?;
    a.import(&b)?;
    a.send(1, "indexed A → real d9b B / Привет")?;
    a.send(3, "both indexed A / \"\\\nПривет")?;
    b.send(4, "both indexed B")?;
    let sa = a.wait(&mut ca, 7, 7)?;
    let sb = b.wait(&mut cb, 7, 7)?;
    let suffix = a.history(32, Some(&cursor))?;
    ensure!(
        suffix["F"] == first["F"] && suffix["finished"] == true,
        "old snapshot stable suffix after append/reopen"
    );
    ensure!(
        suffix["items"].as_array().context("suffix")?.len() == 8,
        "exact frozen10 rows minus2"
    );
    let ha = a.history(32, None)?;
    let hb = b.history(32, None)?;
    let ids = |h: &Value| -> Result<Vec<String>> {
        let mut ids = h["items"]
            .as_array()
            .context("history rows")?
            .iter()
            .map(|v| v["id"].as_str().context("event ID").map(ToOwned::to_owned))
            .collect::<Result<Vec<_>>>()?;
        ids.sort();
        Ok(ids)
    };
    ensure!(
        ids(&ha)? == ids(&hb)? && ids(&ha)?.len() == 12,
        "same real12 text events"
    );
    ensure!(
        ha["items"]
            .as_array()
            .context("history rows")?
            .iter()
            .any(|v| v["proof_kind"] == "FullL1"),
        "post-Ready capture history"
    );
    let native = sa["contexts"][0]["native_hash"].clone();
    let mut conflict = vec![
        "ordered".into(),
        "send3".into(),
        "--ipc-file".into(),
        text(&a.ipc),
        "--slot".into(),
        "0".into(),
        "--request-id".into(),
        "fa000000000000000000000000000003".into(),
        "--text".into(),
        "changed".into(),
    ];
    ensure!(
        rejected(&current(), &conflict)?.contains("Conflict"),
        "changed body conflict"
    );
    conflict.clear();
    ensure!(
        a.status()?["contexts"][0]["native_hash"] == native,
        "retry/conflict no Native"
    );
    ca.kill_wait()?;
    cb.kill_wait()?;
    std::fs::write(
        base.join("parent-l1-result.json"),
        serde_json::to_vec_pretty(
            &json!({"ready":ready,"a":sa,"b":sb,"a_history":ha,"b_history":hb,"old_suffix":suffix,"actual_frozen_interop":true,"both_indexed":true,"parent_killed_and_waited":true}),
        )?,
    )?;
    println!(
        "L1_SMALL PASS actual_frozen_interop=true both_indexed=true same_sid=true no_bootstrap_constructor=true history_suffix=true root={}",
        base.display()
    );
    Ok(())
}
