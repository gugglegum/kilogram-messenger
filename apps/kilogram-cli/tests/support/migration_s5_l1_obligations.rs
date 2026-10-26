//! Genuine source, full64+head, preserved O/A and current F/R, quarantined C1/C2.
use super::*;
use kilogram_cli::ordered_dialog::{runtime7::Profile3, state};
use kilogram_identity::AccountAuthoritySnapshot;
use kilogram_protocol::ordered_dialog_v1 as wire;

pub(super) fn context_between(a: &OldNode, b: &OldNode, base: &Path, name: &str) -> Result<String> {
    ensure!(a.device < b.device, "actual elected roles");
    let invitation = base.join(format!("{name}.invitation"));
    let completed = base.join(format!("{name}.completed"));
    let c = field(
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
            text(&completed),
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
            text(&completed),
            "--expect-account".into(),
            b.account.clone(),
        ],
    )?;
    Ok(c)
}
fn old_view(node: &Node, root: &Path, credentials: &Path) -> OldNode {
    OldNode {
        state: node.branch.state.clone(),
        root: root.into(),
        credentials: credentials.into(),
        account: node.branch.account.clone(),
        device: node.branch.device.clone(),
        profile: node.profile.clone(),
        ipc: node.ipc.clone(),
        locators: node.locators.clone(),
    }
}

#[test]
fn migration_s5_l1_cold_diagnostic_ranges() -> Result<()> {
    let base = PathBuf::from(
        std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("stopped active branch")?,
    );
    for side in ["one", "two"] {
        let path = base.join(side).join("indexed.profile");
        let profile = Profile3::load(&path)?;
        let mut repo = profile.open(&path)?;
        println!(
            "COLD_TAIL side={side} repair={:?} after61={:?}",
            repo.repair_state(0)?.pending,
            repo.debt_page(0, Some(61), 32)?
                .iter()
                .map(|v| v.0)
                .collect::<Vec<_>>()
        );
        let mut debts = Vec::new();
        let mut pending = Vec::new();
        let mut after = None;
        loop {
            let page = repo.debt_page(0, after, 32)?;
            if page.is_empty() {
                break;
            }
            after = page.last().map(|v| v.0);
            debts.extend(page.into_iter().map(|v| v.0));
        }
        after = None;
        loop {
            let page = repo.pending_page(0, after, 32)?;
            if page.is_empty() {
                break;
            }
            after = page.last().map(|v| v.ordinal);
            pending.extend(page.into_iter().map(|v| v.ordinal));
        }
        println!(
            "COLD_DIAGNOSTIC side={side} next={:?} debt={debts:?} pending={pending:?}",
            repo.metadata(0)?.next
        );
    }
    Ok(())
}
pub(super) fn wait_condition(
    node: &Node,
    child: &mut OwnedChild,
    condition: impl Fn(&Value) -> bool,
) -> Result<Value> {
    let started = Instant::now();
    loop {
        child.alive()?;
        let status = node.status()?;
        if condition(node.context(&status)?) {
            return Ok(status);
        }
        ensure!(
            started.elapsed() < Duration::from_secs(25),
            "bounded obligation condition {status}"
        );
        std::thread::sleep(Duration::from_millis(60));
    }
}
pub(super) fn wait_condition_before(
    node: &Node,
    child: &mut OwnedChild,
    condition: impl Fn(&Value) -> bool,
    deadline: Instant,
) -> Result<Value> {
    loop {
        ensure!(
            Instant::now() < deadline,
            "absolute pending64 phase deadline"
        );
        child.alive()?;
        let status = node.status()?;
        ensure!(
            Instant::now() <= deadline,
            "absolute pending64 phase deadline {status}"
        );
        if condition(node.context(&status)?) {
            return Ok(status);
        }
        std::thread::sleep(Duration::from_millis(60));
    }
}
fn slot(status: &Value, c: &str) -> Result<u8> {
    Ok(status["contexts"]
        .as_array()
        .context("context list")?
        .iter()
        .find(|v| v["context"] == c)
        .context("C slot")?["slot"]
        .as_u64()
        .context("slot")? as u8)
}

pub(super) fn preserve_input(base: &Path, case: &str) -> Result<Value> {
    let mut locks = Vec::new();
    let mut paths = Vec::new();
    for side in ["one", "two"] {
        let p = Profile3::load(&base.join(side).join("indexed.profile"))?;
        let path = p.state_dir.join(".kilogram-state.lock");
        let path_meta = std::fs::symlink_metadata(&path)?;
        ensure!(
            path_meta.is_file() && !path_meta.file_type().is_symlink(),
            "regular owned lock path"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                path_meta.file_attributes() & 0x400 == 0,
                "lock reparse point"
            );
        }
        // The OS lock only, with no repository open/recovery before byte copy.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)?;
        file.try_lock()
            .map_err(|e| anyhow::anyhow!("stopped input exclusive lock: {e}"))?;
        let meta = file.metadata()?;
        ensure!(
            meta.is_file() && meta.len() == 0,
            "held handle must prove empty synchronization file"
        );
        locks.push((file, meta.modified()?, meta.created()?));
        paths.push(path);
    }
    let shell = PathBuf::from(
        std::env::var_os("KILOGRAM_S5_POWERSHELL").context("parent PowerShell executable")?,
    );
    let output = workspace().join(".tmp/migration-s5-preserved").join(case);
    let result = json_cli(
        &shell,
        &[
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-File".into(),
            text(&workspace().join("scripts/preserve-migration-s5-l1-input.ps1")),
            "-InputDirectory".into(),
            text(base),
            "-OutputDirectory".into(),
            text(&output),
            "-OwnedLockOne".into(),
            text(&paths[0]),
            "-OwnedLockTwo".into(),
            text(&paths[1]),
        ],
    );
    for (file, modified, created) in &locks {
        let meta = file.metadata()?;
        ensure!(
            meta.is_file()
                && meta.len() == 0
                && meta.modified()? == *modified
                && meta.created()? == *created,
            "held synchronization file changed during preservation"
        );
    }
    drop(locks);
    result
}
fn resumed_cold(node: &Node) -> Result<Value> {
    let p = Profile3::load(&node.profile)?;
    let mut r = p.open(&node.profile)?;
    let snap = r.snapshot(node.slot)?;
    let h = wire::decode(&snap.head, 2048)?;
    let f = h.fields(13)?;
    let mut debt = Vec::new();
    let mut after = None;
    loop {
        let rows = r.debt_page(node.slot, after, 32)?;
        if rows.is_empty() {
            break;
        }
        after = rows.last().map(|v| v.0);
        debt.extend(rows.into_iter().map(|v| v.0));
    }
    let mut pending = Vec::new();
    after = None;
    loop {
        let rows = r.pending_page(node.slot, after, 32)?;
        if rows.is_empty() {
            break;
        }
        after = rows.last().map(|v| v.ordinal);
        ensure!(
            rows.iter().all(|v| !v.quarantined),
            "unexpected prior quarantine"
        );
        pending.extend(rows.into_iter().map(|v| v.ordinal));
    }
    Ok(
        json!({"context":node.branch.context,"role":f[3].uint()?,"d":wire::optional_id(&f[9])?.map(|v|state::hex(&v)),"e":wire::optional_id(&f[10])?.map(|v|state::hex(&v)),"sid":f[11].option()?.map(|v|v.text()).transpose()?,"next":snap.meta.next,"native_hash":snap.native.map(|v|state::hex(&wire::hash(state::NATIVE_TAG,&v))),"policy_generation":snap.meta.policy_generation,"debt":debt,"pending":pending,"counts":r.counts(node.slot)?}),
    )
}

#[test]
fn migration_s5_l1_obligations_full64_head_saved_captures_and_c2() -> Result<()> {
    let seed =
        PathBuf::from(std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged origin")?);
    let source: Value = serde_json::from_slice(&std::fs::read(seed.join("result.json"))?)?;
    let c1 = source["context"].as_str().context("C1")?;
    let a_side = source["elected_a"]["side"].as_str().context("A")?;
    let b_side = source["elected_b"]["side"].as_str().context("B")?;
    let resume = std::env::var_os("KILOGRAM_S5_L1_RESUME_DIR").map(PathBuf::from);
    let case = std::env::var("KILOGRAM_S5_CASE").context("charged case ID")?;
    let base = if let Some(path) = &resume {
        let path = path.canonicalize()?;
        ensure!(
            path.starts_with(
                workspace()
                    .join(".tmp/migration-s5-storage")
                    .canonicalize()?
            ),
            "resume only the stopped active branch"
        );
        ensure!(
            path.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("l1-obligations-")),
            "obligations branch"
        );
        path
    } else {
        run_dir("l1-obligations-")?
    };
    let root_a = base.join("root-a");
    let root_b = base.join("root-b");
    let preserved = if resume.is_some() {
        Some(preserve_input(&base, &case)?)
    } else {
        None
    };
    let (a, b, c2) = if resume.is_some() {
        let a = Node::from_ready(&base.join(a_side), c1)?;
        let b = Node::from_ready(&base.join(b_side), c1)?;
        let profile = Profile3::load(&a.profile)?;
        let c2 = profile
            .enabled_contexts
            .iter()
            .find(|c| c.as_str() != c1)
            .context("existing real C2")?
            .clone();
        (a, b, c2)
    } else {
        let mut a = Node::from_seed(&seed, a_side, &base, c1)?;
        let mut b = Node::from_seed(&seed, b_side, &base, c1)?;
        copy_tree(&seed.join(format!("{a_side}.account")), &root_a)?;
        copy_tree(&seed.join(format!("{b_side}.account")), &root_b)?;
        std::fs::copy(seed.join("countersigned"), base.join("original.invitation"))?;
        let old_a = old_view(&a, &root_a, &seed.join(format!("{a_side}.credentials")));
        let old_b = old_view(&b, &root_b, &seed.join(format!("{b_side}.credentials")));
        let c2 = context_between(&old_a, &old_b, &base, "second")?;
        a.migrate()?;
        b.migrate()?;
        (a, b, c2)
    };
    let starting = if resume.is_some() {
        let a0 = resumed_cold(&a)?;
        let b0 = resumed_cold(&b)?;
        ensure!(
            a0["role"] == 0
                && b0["role"] == 1
                && a0["sid"] == b0["sid"]
                && a0["d"] == b0["d"]
                && a0["e"] == b0["e"],
            "same actual scope/session"
        );
        ensure!(
            a0["next"] == json!([70, 5])
                && b0["next"] == json!([5, 5])
                && a0["debt"] == json!((5..70).collect::<Vec<_>>()),
            "authentic stopped65 debt/frontiers"
        );
        let pending = b0["pending"].as_array().context("pending input")?;
        ensure!(
            (56..=64).contains(&pending.len())
                && pending
                    .iter()
                    .enumerate()
                    .all(|(i, v)| v.as_u64() == Some(6 + i as u64)),
            "actual stopped successors differ; refuse fabricated input"
        );
        let record = json!({"case":case,"binding_case":"F-obligations-02","root_decision_sha256":"8a075a753718605c1dd2659846b54673b2fb3930c0e1f229a662223a1b82d46c","preservation":preserved,"a":a0,"b":b0});
        std::fs::write(
            base.join(format!("{case}-starting.json")),
            serde_json::to_vec_pretty(&record)?,
        )?;
        Some(record)
    } else {
        None
    };
    let sa = base.join(format!("{case}-a.schedule.json"));
    let sb = base.join(format!("{case}-b.schedule.json"));
    schedule_file(
        &sa,
        &json!({"context":c1,"hold_lane":0,"hold_ordinals":[5],"trace_rpc":true}),
    )?;
    schedule_file(
        &sb,
        &json!({"context":c1,"suspend_prefix_requests":true,"trace_rpc":true}),
    )?;
    let mut ca = a.start_controlled(&format!("{case}-a"), Some(&sa), None)?;
    let mut cb = b.start_controlled(&format!("{case}-b"), Some(&sb), None)?;
    let status_a = a.status()?;
    let status_b = b.status()?;
    if resume.is_some() {
        std::fs::write(
            base.join(format!("{case}-starting-runtime.json")),
            serde_json::to_vec_pretty(
                &json!({"a":status_a,"b":status_b,"native_guard_correspondence":"runtime checked actual Native/Guard against committed metadata","starting":starting}),
            )?,
        )?;
    }
    let a2 = a.view(&c2, slot(&status_a, &c2)?);
    let b2 = b.view(&c2, slot(&status_b, &c2)?);
    b.import(&a)?;
    a.import(&b)?;
    b2.import(&a2)?;
    a2.import(&b2)?;
    a2.send(2000, "second context warm A")?;
    b2.send(2001, "second context warm B")?;
    a2.wait(&mut ca, 2, 2)?;
    b2.wait(&mut cb, 2, 2)?;
    let warm = b.status()?;
    let original_native = b.context(&warm)?["native_hash"].clone();
    if resume.is_some() {
        let before = a.status()?;
        let row = a.context(&before)?;
        ensure!(
            row["next"][0] == 70 && row["counts"]["queued"] == 0 && row["counts"]["debt"] == 65,
            "same real materialized65 source"
        );
    } else {
        for i in 0..65 {
            a.send(100 + i, &format!("real gap successor {i}/Привет"))?;
        }
    }
    let pending_started = Instant::now();
    let full = wait_condition_before(
        &b,
        &mut cb,
        |c| c["counts"]["pending"] == 64 && c["head_pending"].is_null() && c["next"][0] == 5,
        pending_started + Duration::from_secs(70),
    )?;
    let pending_seconds = pending_started.elapsed().as_secs_f64();
    std::fs::write(
        base.join(format!("{case}-phase-full64.json")),
        serde_json::to_vec_pretty(
            &json!({"phase":"pending64","deadline_seconds":70,"observed_phase_seconds":pending_seconds,"status":full,"direct_batch":"unset/default8"}),
        )?,
    )?;
    ensure!(
        b.context(&full)?["native_hash"] == original_native
            && full["stats"]["packet_attempts"] == warm["stats"]["packet_attempts"],
        "successors invoked Native before the gap head"
    );
    a2.send(2002, "C2 progresses with C1 full64")?;
    b2.send(2003, "C2 independent reverse")?;
    a2.wait(&mut ca, 3, 3)?;
    let independent = b2.wait(&mut cb, 3, 3)?;
    ensure!(
        b.context(&independent)?["counts"]["pending"] == 64
            && b.context(&independent)?["native_hash"] == original_native,
        "C2 consumed C1 credit/Native"
    );
    ensure!(
        independent["window"][0] == json!([64, 64]),
        "hard shares while C1 blocked"
    );
    let before_pins = b.status()?;
    policy::audit(
        &base,
        &frozen(),
        &[
            "device-enroll".into(),
            "--profile-mode".into(),
            "ordered-dialog-v1".into(),
            "--account-dir".into(),
            text(&root_a),
            "--state-dir".into(),
            text(&base.join("additional-device")),
        ],
    )?;
    let auth_a = policy::snapshot(&base, &root_a, "a-current.snapshot")?;
    let auth_b = policy::snapshot(&base, &root_b, "b-current.snapshot")?;
    policy::import_authority(&b, &base, &auth_a)?;
    let invalidated = b.status()?;
    for (view, old) in [
        (&b, b.context(&before_pins)?),
        (&b2, b2.context(&before_pins)?),
    ] {
        let current = view.context(&invalidated)?;
        ensure!(
            current["policy_generation"].as_u64()
                == Some(old["policy_generation"].as_u64().context("generation")? + 1)
                && current["lease_current"] == false,
            "shared-account lease invalidation for every C"
        );
    }
    policy::import_authority(&a, &base, &auth_a)?;
    let paused = sb.with_extension("hit");
    if paused.exists() {
        std::fs::remove_file(&paused)?;
    }
    schedule_file(
        &sb,
        &json!({"context":c1,"suspend_prefix_requests":true,"pause_point":"l1-before-native-commit"}),
    )?;
    schedule_file(&sa, &json!({}))?;
    wait_file(&mut cb, &paused, 25)?;
    cb.alive()?;
    let killed_pid = cb.0.id();
    cb.kill_wait()?;
    let profile = Profile3::load(&b.profile)?;
    let mut repository = profile.open(&b.profile)?;
    let full65 = repository.counts(b.slot)?;
    let repair = repository.repair_state(b.slot)?;
    ensure!(
        full65.pending == 65 && repair.pending == 64 && repair.head == Some(5),
        "actual full64 plus separate reserved head"
    );
    let successor = repository
        .pending(b.slot, 6)?
        .context("actual saved successor")?;
    let head = repository
        .pending(b.slot, 5)?
        .context("actual reserved head")?;
    ensure!(
        successor.original == successor.admission
            && head.original == successor.original
            && head.admission != successor.admission,
        "actual old/new admission captures"
    );
    drop(repository);
    schedule_file(&sb, &json!({"context":c1,"suspend_prefix_requests":true}))?;
    let drain_started = Instant::now();
    let drain_deadline = drain_started + Duration::from_secs(70);
    cb = b.start_controlled(&format!("{case}-b-head-recovered"), Some(&sb), None)?;
    b.import(&a)?;
    a.import(&b)?;
    b2.import(&a2)?;
    a2.import(&b2)?;
    let debt_zero = a.wait_before(&mut ca, 70, 5, drain_deadline)?;
    let drained = b.wait_before(&mut cb, 5, 70, drain_deadline)?;
    let drain_seconds = drain_started.elapsed().as_secs_f64();
    std::fs::write(
        base.join(format!("{case}-phase-drain65.json")),
        serde_json::to_vec_pretty(
            &json!({"phase":"drain65_and_receipts","shared_absolute_deadline_seconds":70,"observed_phase_seconds":drain_seconds,"sender":debt_zero,"receiver":drained}),
        )?,
    )?;
    let native_after_drain = b.context(&drained)?["native_hash"].clone();
    schedule_file(&sa, &json!({"context":c1,"tamper_event":"invalid-cipher"}))?;
    a.send(999, "actual signed invalid ciphertext negative")?;
    let quarantined = wait_condition(&b, &mut cb, |c| {
        c["counts"]["pending"] == 1 && c["head_pending"] == 70 && c["next"][0] == 70
    })?;
    ensure!(
        b.context(&quarantined)?["native_hash"] == native_after_drain,
        "quarantine changed committed Native"
    );
    // Stop at the actual quarantine flag, not merely pending possession.
    let start = Instant::now();
    loop {
        let status = b.status()?;
        if status["stats"]["aborted_packet_calls"]
            .as_u64()
            .context("aborted calls")?
            > 0
        {
            break;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(10),
            "quarantine Native Input outcome"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let (_, consent) = policy::issue(
        &a,
        &b,
        &base,
        [&root_a, &root_b],
        [&auth_a, &auth_b],
        [&auth_a, &auth_b],
        "quarantine-renewal",
        3600,
    )?;
    let a_status = a.status()?;
    let b_status = b.status()?;
    policy::import_consent(
        &a,
        &base,
        &consent,
        a.context(&a_status)?["current_consent"]
            .as_str()
            .context("A current")?,
    )?;
    policy::import_consent(
        &b,
        &base,
        &consent,
        b.context(&b_status)?["current_consent"]
            .as_str()
            .context("B current")?,
    )?;
    a2.send(2004, "C2 after C1 quarantine and renewal")?;
    b2.send(2005, "C2 after renewal reverse")?;
    a2.wait(&mut ca, 4, 4)?;
    let final_status = b2.wait(&mut cb, 4, 4)?;
    ensure!(
        b.context(&final_status)?["native_hash"] == native_after_drain
            && b.context(&final_status)?["counts"]["pending"] == 1,
        "renewal cleared quarantine/changed C1 Native"
    );
    ca.kill_wait()?;
    cb.kill_wait()?;
    let profile = Profile3::load(&b.profile)?;
    let mut repository = profile.open(&b.profile)?;
    let pending = repository
        .pending(b.slot, 70)?
        .context("durable quarantined ticket")?;
    ensure!(
        pending.quarantined,
        "quarantine did not survive restart boundary"
    );
    let source_event = repository
        .event_at(b.slot, 0, 6)?
        .context("drained actual source")?;
    let proof = repository
        .full_l1_proof(b.slot, wire::hash(wire::EVENT_ID, &source_event))?
        .context("post-Ready full proof")?;
    let actual_credentials =
        Credentials::parse(&std::fs::read(seed.join(format!("{a_side}.credentials")))?)?;
    let current = Credentials::new(
        actual_credentials.cert,
        AccountAuthoritySnapshot::decode_and_verify(&std::fs::read(&auth_a)?)?,
    )?;
    ensure!(
        proof.original == successor.original
            && proof.admission == Some(successor.admission)
            && proof.final_auth == Some(current.reference())
            && proof.final_auth != proof.admission
            && proof.receiver.is_some()
            && proof.receipt == proof.receipt_overlay,
        "saved O/A, actual current F/R and receipt"
    );
    let mut snapshots = 0;
    let mut full_proofs = 0;
    let mut cursor = None;
    loop {
        let page = repository.history(b.slot, cursor.as_ref(), 32)?;
        let page: Value = serde_json::from_slice(&page)?;
        for item in page["items"].as_array().context("history items")? {
            if item["proof_kind"] == "LegacySnapshot" {
                snapshots += 1;
            } else {
                ensure!(item["proof_kind"] == "FullL1", "proof discriminator");
                full_proofs += 1;
            }
        }
        if page["finished"] == true {
            break;
        }
        cursor = Some(kilogram_state::indexed_dialog::HistoryCursor::from_base64(
            page["cursor"].as_str().context("cursor")?,
        )?);
    }
    ensure!(
        snapshots == 8 && full_proofs == 65,
        "imported snapshots changed or quarantine became readable"
    );
    drop(repository);
    std::fs::write(
        base.join(format!("{case}-parent-l1-result.json")),
        serde_json::to_vec_pretty(
            &json!({"full64":full,"phase_seconds":{"pending64":pending_seconds,"drain65_shared":drain_seconds},"direct_batch":"unset/default8","c2_progress":independent,"shared_peer_invalidated":invalidated,"reserved_head":{"actual_pending":full65,"actual_ordinary":repair.pending,"actual_head":repair.head,"parent_killed_pid":killed_pid},"drained":drained,"quarantined":quarantined,"final":final_status,"saved_successor_O":state::hex(&successor.original),"saved_successor_A":state::hex(&successor.admission),"head_A":state::hex(&head.admission),"actual_full_proof":proof,"legacy_snapshot_rows":snapshots,"full_l1_rows":full_proofs,"quarantine_durable":pending.quarantined}),
        )?,
    )?;
    println!(
        "L1_OBLIGATIONS PASS full64_plus_head=true saved_OA_current_FR=true shared_peer_two_C_invalidated=true quarantine_survives_renewal=true C2_progress=true root={}",
        base.display()
    );
    Ok(())
}
