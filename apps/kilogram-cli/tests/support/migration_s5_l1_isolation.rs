//! Actual d9b signed-fork origin with a third independent peer in C2.
use super::*;

fn old_queue(node: &OldNode, c: &str, id: u64) -> Result<()> {
    cli(
        &frozen(),
        &[
            "ordered".into(),
            "queue".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--context".into(),
            c.into(),
            "--request-id".into(),
            format!("ec00000000000000{id:016x}"),
            "--text".into(),
            format!("genuine C source {id}"),
        ],
    )?;
    Ok(())
}
fn wait_old_context(node: &OldNode, c: &str, child: &mut OwnedChild, next: u64) -> Result<Value> {
    let start = Instant::now();
    loop {
        child.alive()?;
        let s = old_status(node)?;
        let row = s["contexts"]
            .as_array()
            .context("contexts")?
            .iter()
            .find(|v| v["context"] == c)
            .context("C")?;
        if row["phase"] == 3 && row["send_next"] == next && row["unacked"] == 0 {
            return Ok(s);
        }
        ensure!(
            start.elapsed() < Duration::from_secs(25),
            "old actual C deadline {s}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn migration_s5_l1_different_peer_c2_survives_signed_fork_and_account_pin() -> Result<()> {
    let base = run_dir("l1-isolation-")?;
    let mut nodes = [
        OldNode::enrolled(&base, 0)?,
        OldNode::enrolled(&base, 1)?,
        OldNode::enrolled(&base, 2)?,
    ];
    nodes.sort_by(|a, b| a.device.cmp(&b.device));
    let [y, x, z] = &nodes; // X is B in C1 and A in C2.
    let c1 = obligations::context_between(y, x, &base, "first")?;
    let c2 = obligations::context_between(x, z, &base, "second")?;
    x.profile(&c1)?;
    y.profile(&c1)?;
    z.profile(&c2)?;
    let mut p: Value = serde_json::from_slice(&std::fs::read(&x.profile)?)?;
    p["enabled_contexts"] = json!([c1, c2]);
    std::fs::write(&x.profile, serde_json::to_vec_pretty(&p)?)?;
    let mut cx = spawn(&frozen(), &x.runtime(), &base, "d9b-x", None, None)?;
    let mut cy = spawn(&frozen(), &y.runtime(), &base, "d9b-y", None, None)?;
    let mut cz = spawn(&frozen(), &z.runtime(), &base, "d9b-z", None, None)?;
    wait_file(&mut cx, &x.ipc, 15)?;
    wait_file(&mut cy, &y.ipc, 15)?;
    wait_file(&mut cz, &z.ipc, 15)?;
    import_peer(x, y, &c1)?;
    import_peer(y, x, &c1)?;
    import_peer(z, x, &c2)?;
    import_peer(x, z, &c2)?;
    old_queue(x, &c1, 1)?;
    // Two genuine C1 rows make its one-item page produce a real scoped cursor.
    old_queue(y, &c1, 4)?;
    old_queue(x, &c2, 2)?;
    wait_old_context(x, &c1, &mut cx, 2)?;
    wait_old_context(y, &c1, &mut cy, 2)?;
    let before = wait_old_context(x, &c2, &mut cx, 2)?;
    cy.kill_wait()?;
    let schedule = base.join("signed-fork.schedule.json");
    schedule_file(&schedule, &json!({"context":c1,"conflicting_offer":true}))?;
    cy = spawn(
        &control_peer(),
        &y.runtime(),
        &base,
        "actual-malicious-signer",
        None,
        Some(&schedule),
    )?;
    // Existing public IPC file is stale until the new helper replaces it.
    let start = Instant::now();
    loop {
        cy.alive()?;
        if cli(
            &frozen(),
            &[
                "ordered".into(),
                "status".into(),
                "--ipc-file".into(),
                text(&y.ipc),
            ],
        )
        .is_ok()
        {
            break;
        }
        ensure!(start.elapsed() < Duration::from_secs(15), "helper IPC");
        std::thread::sleep(Duration::from_millis(50));
    }
    import_peer(x, y, &c1)?;
    import_peer(y, x, &c1)?;
    let fork = x
        .state
        .join(format!("runtime/od1/contexts/{c1}/blocked-fork"));
    wait_file(&mut cx, &fork, 25)?;
    old_queue(x, &c2, 3)?;
    wait_old_context(x, &c2, &mut cx, 3)?;
    cx.kill_wait()?;
    cy.kill_wait()?;
    cz.kill_wait()?;
    copy_tree(&x.state, &base.join("frozen-x-original"))?;
    let mut live = Node::from_old(x, &c1)?;
    let peer = Node::from_old(z, &c2)?;
    let ready = live.migrate()?;
    cx = live.start("indexed-x")?;
    cz = peer.start("frozen-z")?;
    let status = live.status()?;
    let second_slot = status["contexts"]
        .as_array()
        .context("contexts")?
        .iter()
        .find(|v| v["context"] == c2)
        .context("C2")?["slot"]
        .as_u64()
        .context("slot")? as u8;
    let second = live.view(&c2, second_slot);
    peer.import(&second)?;
    second.import(&peer)?;
    second.send(50, "C2 continues beside imported signed fork")?;
    let warm = second.wait(&mut cx, 4, 1)?;
    peer.wait(&mut cz, 1, 4)?;
    let c1_native = live.context(&warm)?["native_hash"].clone();
    let c2_generation = second.context(&warm)?["policy_generation"].clone();
    let c2_lease = second.context(&warm)?["lease_generation"].clone();
    let c1_generation = live.context(&warm)?["policy_generation"]
        .as_u64()
        .context("C1 policy")?;
    ensure!(
        live.context(&warm)?["policy_error"]
            .as_str()
            .is_some_and(|s| s.contains("signed fork")),
        "imported block absent"
    );
    let history = live.history(1, None)?;
    let cursor = history["cursor"].as_str().context("C1 cursor")?;
    ensure!(
        second.history(1, Some(cursor)).is_err(),
        "cross-C cursor accepted"
    );
    policy::audit(
        &base,
        &frozen(),
        &[
            "device-enroll".into(),
            "--profile-mode".into(),
            "ordered-dialog-v1".into(),
            "--account-dir".into(),
            text(&y.root),
            "--state-dir".into(),
            text(&base.join("additional-y-device")),
        ],
    )?;
    let snapshot = policy::snapshot(&base, &y.root, "advanced-y.snapshot")?;
    policy::import_authority(&live, &base, &snapshot)?;
    let changed = live.status()?;
    ensure!(
        live.context(&changed)?["policy_generation"].as_u64() == Some(c1_generation + 1)
            && second.context(&changed)?["policy_generation"] == c2_generation
            && second.context(&changed)?["lease_generation"] == c2_lease,
        "unrelated peer pin invalidated C2 token"
    );
    second.send(51, "C2 after unrelated peer authority advance")?;
    let after = second.wait(&mut cx, 5, 1)?;
    peer.wait(&mut cz, 1, 5)?;
    ensure!(
        live.context(&after)?["native_hash"] == c1_native
            && second.context(&after)?["lease_generation"] == c2_lease,
        "C1 block changed Native or C2 lease"
    );
    ensure!(
        live.send(52, "blocked C1 must refuse").is_err(),
        "fork cleared by pin advancement"
    );
    for name in [
        "account_constructors",
        "outbound_constructors",
        "inbound_constructors",
    ] {
        ensure!(
            after["stats"][name] == 0,
            "existing dialogs recreated {name}"
        );
    }
    cx.kill_wait()?;
    cz.kill_wait()?;
    std::fs::write(
        base.join("parent-l1-result.json"),
        serde_json::to_vec_pretty(
            &json!({"actual_d9b_before":before,"ready":ready,"warm":warm,"pin_changed":changed,"after":after,"blocked_C1_history":history,"actual_signed_fork":true,"different_peer_C2_token_preserved":true,"parent_killed_and_waited":true}),
        )?,
    )?;
    println!(
        "L1_ISOLATION PASS genuine_signed_fork=true C2_different_peer_progress=true C2_lease_unchanged=true cross_C_cursor_rejected=true root={}",
        base.display()
    );
    Ok(())
}
