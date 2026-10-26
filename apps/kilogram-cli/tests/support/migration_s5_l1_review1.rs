//! Review1 field controllers: genuine frozen origins only. No private Native,
//! Head, Event or proof rows are injected or repaired by this controller.
use super::*;
use kilogram_cli::ordered_dialog::{runtime7::Profile3, state};
use kilogram_identity::AccountAuthoritySnapshot;
use kilogram_protocol::ordered_dialog_v1 as wire;

fn seed_pair(prefix: &str) -> Result<(PathBuf, Node, Node)> {
    let seed = PathBuf::from(
        std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged genuine origin")?,
    );
    let origin: Value = serde_json::from_slice(&std::fs::read(seed.join("result.json"))?)?;
    let c = origin["context"].as_str().context("C")?;
    let base = run_dir(prefix)?;
    let a = Node::from_seed(
        &seed,
        origin["elected_a"]["side"].as_str().context("A")?,
        &base,
        c,
    )?;
    let b = Node::from_seed(
        &seed,
        origin["elected_b"]["side"].as_str().context("B")?,
        &base,
        c,
    )?;
    Ok((base, a, b))
}
fn cold(node: &Node, own_n: Option<u64>) -> Result<Value> {
    let p = Profile3::load(&node.profile)?;
    let mut r = p.open(&node.profile)?;
    let snapshot = r.snapshot(node.slot)?;
    let proof = if let Some(n) = own_n {
        let role = r.contexts()[node.slot as usize].1;
        let raw = r
            .event_at(node.slot, role, n)?
            .context("actual own source")?;
        r.full_l1_proof(node.slot, wire::hash(wire::EVENT_ID, &raw))?
    } else {
        None
    };
    Ok(
        json!({"next":snapshot.meta.next,"counts":r.counts(node.slot)?,"native_hash":snapshot.native.map(|v|state::hex(&wire::hash(state::NATIVE_TAG,&v))),"proof":proof}),
    )
}
fn save(base: &Path, name: &str, value: &Value) -> Result<()> {
    let path = base.join(name);
    ensure!(!path.exists(), "immutable result path");
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

#[test]
fn migration_s5_l1_review1_genuine_partial_own_f_both_genesis_roles() -> Result<()> {
    let mut outcomes = Vec::new();
    for (point, role, phase) in [("after-grant", 0usize, 1), ("before-init-commit", 1, 2)] {
        let base = run_dir(&format!("l1-review1-own-f-{role}-"))?;
        let (nodes, c) = old_pair(&base)?;
        let victim = &nodes[role];
        let remote = &nodes[1 - role];
        let scheduler = base.join("origin-peer.schedule.json");
        schedule_file(&scheduler, &json!({"pause_point":point}))?;
        let mut origin = spawn(
            &frozen(),
            &victim.runtime(),
            &base,
            "genuine-victim",
            None,
            None,
        )?;
        let mut helper = spawn(
            &control_peer(),
            &remote.runtime(),
            &base,
            "real-control-peer",
            None,
            Some(&scheduler),
        )?;
        wait_file(&mut origin, &victim.ipc, 15)?;
        wait_file(&mut helper, &remote.ipc, 15)?;
        import_peer(&nodes[1], &nodes[0], &c)?;
        import_peer(&nodes[0], &nodes[1], &c)?;
        wait_file(&mut helper, &scheduler.with_extension("hit"), 25)?;
        let before = old_status(victim)?;
        ensure!(
            before["contexts"][0]["phase"] == phase,
            "genuine partial phase"
        );
        origin.kill_wait()?;
        helper.kill_wait()?;
        copy_tree(&victim.state, &base.join("frozen-partial-original"))?;
        let old = Credentials::parse(&std::fs::read(&victim.credentials)?)?;
        policy::audit(
            &base,
            &frozen(),
            &[
                "device-enroll".into(),
                "--profile-mode".into(),
                "ordered-dialog-v1".into(),
                "--account-dir".into(),
                text(&victim.root),
                "--state-dir".into(),
                text(&base.join("additional-device")),
            ],
        )?;
        let advanced = policy::snapshot(&base, &victim.root, "advanced-own.snapshot")?;
        for node in [victim, remote] {
            policy::audit(
                &base,
                &frozen(),
                &[
                    "ordered".into(),
                    "authority-update".into(),
                    "--state-dir".into(),
                    text(&node.state),
                    "--snapshot-file".into(),
                    text(&advanced),
                ],
            )?;
        }
        let current = Credentials::new(
            old.cert.clone(),
            AccountAuthoritySnapshot::decode_and_verify(&std::fs::read(&advanced)?)?,
        )?;
        ensure!(
            old.reference() != current.reference(),
            "real authority advance"
        );
        let mut live = Node::from_old(victim, &c)?;
        let peer = Node::from_old(remote, &c)?;
        live.migrate()?;
        let mut indexed = live.start("indexed-victim")?;
        let mut frozen_peer = peer.start("frozen-completion")?;
        peer.import(&live)?;
        live.import(&peer)?;
        live.send(401, "own Text with explicit current F")?;
        peer.send(402, "actual opposite completion")?;
        let status = live.wait(&mut indexed, 2, 2)?;
        peer.wait(&mut frozen_peer, 2, 2)?;
        ensure!(
            status["stats"]["account_constructors"] == 0,
            "existing original Account replaced"
        );
        let expected = if role == 0 {
            "outbound_constructors"
        } else {
            "inbound_constructors"
        };
        ensure!(
            status["stats"][expected] == 1,
            "first legal absent Session constructor"
        );
        indexed.kill_wait()?;
        frozen_peer.kill_wait()?;
        let genesis = cold(&live, Some(0))?;
        let text_proof = cold(&live, Some(1))?;
        let p = Profile3::load(&live.profile)?;
        let mut r = p.open(&live.profile)?;
        for (n, original) in [(0, old.reference()), (1, current.reference())] {
            let raw = r
                .event_at(live.slot, role as u8, n)?
                .context("own source")?;
            let proof = r
                .full_l1_proof(live.slot, wire::hash(wire::EVENT_ID, &raw))?
                .context("actual full proof")?;
            ensure!(
                proof.original == original
                    && proof.final_auth == Some(current.reference())
                    && proof.admission.is_none()
                    && proof.receiver.is_none()
                    && proof.receipt.is_none(),
                "own O/F links and nonapplicable A/R"
            );
        }
        drop(r);
        let result = json!({"role":role,"point":point,"before":before,"runtime":status,"genesis":genesis,"own_text":text_proof,"Oold":state::hex(&old.reference()),"Fcurrent":state::hex(&current.reference())});
        save(&base, "review1-own-f-result.json", &result)?;
        outcomes.push(result);
    }
    println!(
        "REVIEW1_OWN_F PASS genuine_partial_roles=2 explicit_Oold_Fcurrent=true outcomes={}",
        serde_json::to_string(&outcomes)?
    );
    Ok(())
}

#[test]
fn migration_s5_l1_review1_unacked128_before_native_and_one_exact_receipt() -> Result<()> {
    let (base, mut a, mut b) = seed_pair("l1-review1-unacked-")?;
    a.migrate()?;
    b.migrate()?;
    let schedule = base.join("sender.schedule.json");
    schedule_file(&schedule, &json!({"disable_ack":true}))?;
    let mut ca = a.start_controlled("a-initial", Some(&schedule), None)?;
    let mut cb = b.start("b-initial")?;
    b.import(&a)?;
    a.import(&b)?;
    for i in 0..127 {
        a.send(1000 + i, &format!("genuine unacked {i}"))?;
    }
    let filling = Instant::now();
    let at127 = obligations::wait_condition_before(
        &a,
        &mut ca,
        |c| c["counts"]["debt"] == 127 && c["counts"]["queued"] == 0,
        filling + Duration::from_secs(100),
    )?;
    schedule_file(
        &schedule,
        &json!({"disable_ack":true,"suspend_materialization":true}),
    )?;
    for i in 0..4 {
        a.send(2000 + i, &format!("accepted remainder {i}"))?;
    }
    let queued = obligations::wait_condition(&a, &mut ca, |c| {
        c["counts"]["debt"] == 127 && c["counts"]["queued"] == 4
    })?;
    let native_before = a.context(&queued)?["native_hash"].clone();
    schedule_file(&schedule, &json!({"disable_ack":true}))?;
    let limited = obligations::wait_condition(&a, &mut ca, |c| {
        c["counts"]["debt"] == 128 && c["counts"]["queued"] == 3
    })?;
    ensure!(
        limited["stats"]["committed_encrypt"].as_u64()
            == Some(
                queued["stats"]["committed_encrypt"]
                    .as_u64()
                    .context("encrypt")?
                    + 1
            ),
        "127+4 invoked more than one new packet"
    );
    let at_limit = a.context(&limited)?["native_hash"].clone();
    ensure!(at_limit != native_before, "actual one encryption absent");
    std::thread::sleep(Duration::from_millis(500));
    let another = a.status()?;
    ensure!(
        another["stats"]["packet_attempts"] == limited["stats"]["packet_attempts"]
            && a.context(&another)?["native_hash"] == at_limit
            && a.context(&another)?["counts"]["queued"] == 3,
        "another turn crossed unacked128"
    );
    ca.kill_wait()?;
    cb.kill_wait()?;
    let stopped = cold(&a, None)?;
    ensure!(
        stopped["counts"]["debt"] == 128 && stopped["counts"]["queued"] == 3,
        "durable remainder"
    );
    ca = a.start_controlled("a-reopened", Some(&schedule), None)?;
    cb = b.start("b-reopened")?;
    b.import(&a)?;
    a.import(&b)?;
    std::thread::sleep(Duration::from_millis(500));
    let reopened = a.status()?;
    ensure!(
        reopened["stats"]["packet_attempts"] == 0
            && a.context(&reopened)?["native_hash"] == at_limit
            && a.context(&reopened)?["counts"]["queued"] == 3,
        "reopen performed packet work at unacked128"
    );
    schedule_file(
        &schedule,
        &json!({"ack_query_batch":1,"pause_point":"l1-after-receipt-commit"}),
    )?;
    wait_file(&mut ca, &schedule.with_extension("hit"), 25)?;
    schedule_file(&schedule, &json!({"disable_ack":true}))?;
    let freed = obligations::wait_condition(&a, &mut ca, |c| {
        c["counts"]["debt"] == 128 && c["counts"]["queued"] == 2
    })?;
    ensure!(
        freed["stats"]["committed_encrypt"] == 1,
        "one exact receipt did not free exactly one packet slot"
    );
    ca.kill_wait()?;
    cb.kill_wait()?;
    let p = Profile3::load(&a.profile)?;
    let mut r = p.open(&a.profile)?;
    let receipt = r
        .receipt(a.slot, 0, 5)?
        .context("real first new source receipt")?;
    ensure!(
        r.debt_page(a.slot, Some(4), 1)?
            .first()
            .is_some_and(|v| v.0 == 6),
        "wrong source debt retired"
    );
    drop(r);
    save(
        &base,
        "review1-unacked-result.json",
        &json!({"at127":at127,"queued4":queued,"limited128_3":limited,"another_turn":another,"stopped":stopped,"reopened":reopened,"one_receipt128_2":freed,"receipt_sha_tag":state::hex(&wire::hash(b"",&receipt)),"one_active_genuine_branch":true}),
    )?;
    println!(
        "REVIEW1_UNACKED PASS actual127_queue4=true limit128_queue3=true reopen_no_packets=true exact_receipt_freed_one=true root={}",
        base.display()
    );
    Ok(())
}

#[test]
fn migration_s5_l1_review1_signed_ready_refusal_then_scoped_fork() -> Result<()> {
    let (base, mut a, mut b) = seed_pair("l1-review1-ready-")?;
    let seed = PathBuf::from(std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("origin")?);
    let origin: Value = serde_json::from_slice(&std::fs::read(seed.join("result.json"))?)?;
    let root_a = base.join("root-a");
    let root_b = base.join("root-b");
    copy_tree(
        &seed.join(format!(
            "{}.account",
            origin["elected_a"]["side"].as_str().context("A")?
        )),
        &root_a,
    )?;
    copy_tree(
        &seed.join(format!(
            "{}.account",
            origin["elected_b"]["side"].as_str().context("B")?
        )),
        &root_b,
    )?;
    std::fs::copy(seed.join("countersigned"), base.join("original.invitation"))?;
    a.migrate()?;
    b.migrate()?;
    let schedule = base.join("ready.schedule.json");
    schedule_file(&schedule, &json!({}))?;
    let mut cb = b.start("b-receiver")?;
    let mut cases = Vec::new();
    let mut next = 5;
    for (index, fault) in [
        "arity",
        "d",
        "e",
        "sid",
        "role",
        "suite",
        "format",
        "genesis-shape",
    ]
    .into_iter()
    .enumerate()
    {
        let before = b.status()?;
        let log = b.branch.base.join("b-receiver.stderr.log");
        let refusal_count = |log: &str| {
            log.lines()
                .filter(|line| {
                    line.starts_with("l1_dispatch_rejected ")
                        && line.contains(if fault == "arity" {
                            "OD1 array arity"
                        } else {
                            "Ready shape/scope refusal"
                        })
                })
                .count()
        };
        let previous = refusal_count(&std::fs::read_to_string(&log)?);
        schedule_file(&schedule, &json!({"ready_fault":fault}))?;
        let mut ca = a.start_controlled(&format!("a-fault-{index}"), Some(&schedule), None)?;
        b.import(&a)?;
        a.import(&b)?;
        let started = Instant::now();
        loop {
            ca.alive()?;
            cb.alive()?;
            if refusal_count(&std::fs::read_to_string(&log)?) > previous {
                break;
            }
            ensure!(
                started.elapsed() < Duration::from_secs(25),
                "actual signed Ready refusal {fault}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        let refused = b.status()?;
        ensure!(
            b.context(&refused)?["native_hash"] == b.context(&before)?["native_hash"]
                && b.context(&refused)?["policy_error"].is_null()
                && refused["stats"]["packet_attempts"] == before["stats"]["packet_attempts"],
            "malformed Ready poisoned C or invoked Native"
        );
        schedule_file(&schedule, &json!({}))?;
        a.send(3000 + index as u64, &format!("valid Ready after {fault}"))?;
        next += 1;
        a.wait(&mut ca, next, 5)?;
        let valid = b.wait(&mut cb, 5, next)?;
        cases.push(json!({"fault":fault,"refusal":refused,"valid_after":valid}));
        ca.kill_wait()?;
    }
    schedule_file(&schedule, &json!({"ready_fault":"genesis-conflict"}))?;
    let mut ca = a.start_controlled("a-real-scope-fork", Some(&schedule), None)?;
    b.import(&a)?;
    a.import(&b)?;
    let blocked = obligations::wait_condition(&b, &mut cb, |c| {
        c["policy_error"]
            .as_str()
            .is_some_and(|s| s.contains("signed fork"))
    })?;
    ca.kill_wait()?;
    cb.kill_wait()?;
    let p = Profile3::load(&b.profile)?;
    let mut r = p.open(&b.profile)?;
    let prior = r
        .control(
            b.slot,
            kilogram_state::indexed_dialog::migration7::live1::L1Control::Ready(0),
        )?
        .context("prior peer Ready")?;
    let proof = r
        .control(
            b.slot,
            kilogram_state::indexed_dialog::migration7::live1::L1Control::BlockedFork,
        )?
        .context("durable actual fork")?;
    let fields = wire::decode(&proof, 64 * 1024)?;
    ensure!(
        fields.fields(4)?[2].bytes()? == prior,
        "fork first witness was replaced with genesis"
    );
    drop(r);
    schedule_file(&schedule, &json!({}))?;
    ca = a.start_controlled("a-before-renewal", Some(&schedule), None)?;
    cb = b.start("b-block-before-renewal")?;
    let auth_a = policy::snapshot(&base, &root_a, "a-current.snapshot")?;
    let auth_b = policy::snapshot(&base, &root_b, "b-current.snapshot")?;
    let (_, consent) = policy::issue(
        &a,
        &b,
        &base,
        [&root_a, &root_b],
        [&auth_a, &auth_b],
        [&auth_a, &auth_b],
        "blocked-ready-renewal",
        3600,
    )?;
    for node in [&a, &b] {
        let status = node.status()?;
        policy::import_consent(
            node,
            &base,
            &consent,
            node.context(&status)?["current_consent"]
                .as_str()
                .context("consent ref")?,
        )?;
    }
    let renewed = b.status()?;
    ensure!(
        b.context(&renewed)?["policy_error"]
            .as_str()
            .is_some_and(|s| s.contains("signed fork"))
            && b.context(&renewed)?["native_hash"] == b.context(&blocked)?["native_hash"],
        "renewal cleared fork/changed Native"
    );
    ca.kill_wait()?;
    cb.kill_wait()?;
    cb = b.start("b-block-reopened")?;
    let reopened = b.status()?;
    ensure!(
        b.context(&reopened)?["policy_error"]
            .as_str()
            .is_some_and(|s| s.contains("signed fork")),
        "fork did not survive reopen"
    );
    cb.kill_wait()?;
    save(
        &base,
        "review1-ready-result.json",
        &json!({"signed_malformed_cases":cases,"same_scope_fork":blocked,"public_renewal_still_blocked":renewed,"cold_reopened":reopened,"prior_ready_witness_hash":state::hex(&wire::hash(b"",&prior)),"fork_proof_hash":state::hex(&wire::hash(b"",&proof))}),
    )?;
    println!(
        "REVIEW1_READY PASS actual_owner_dispatch=true signed_malformed8_refused=true valid_after=true actual_scoped_fork=true root={}",
        base.display()
    );
    Ok(())
}
