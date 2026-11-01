//! Two actual capacity publication interruptions on one fresh Active lineage.
use super::*;
#[test]
fn s53_capacity_recovery_preserves_validated_route_public() -> Result<()> {
    let managed = Managed::new("F-s53-capacity-recovery-r2-01", 240)?;
    let mut pair = Pair::fresh(&managed.run, "capacity")?;
    pair.exchange2()?;
    pair.upgrade(0, 0xc1)?;
    let original = pair.converge()?;
    pair.stop()?;
    let ready = managed.run.join("relay.ready");
    let stop = managed.run.join("relay.stop");
    let relay_exe =
        workspace().join("target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe");
    let mut relay = OwnedChild(
        command(&relay_exe)
            .arg(&ready)
            .arg(&stop)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(
                managed.run.join("relay.stderr.log"),
            )?))
            .spawn()?,
    );
    wait_file(&mut relay, &ready, 15)?;
    let url = std::fs::read_to_string(&ready)?;
    for node in &pair.nodes {
        route(node, "relay-only", Some(&url))?;
    }
    pair.start(0, None)?;
    pair.start(1, None)?;
    pair.exchange2()?;
    let initial_route = wait_path(&pair.nodes[0], "relay")?;
    let mut observations = Vec::new();
    for (round, point) in ["capacity-committed", "profile-staged"]
        .into_iter()
        .enumerate()
    {
        pair.converge()?;
        pair.stop()?;
        let prior_raw = std::fs::read(&pair.nodes[0].profile)?;
        let prior: Value = serde_json::from_slice(&prior_raw)?;
        let prior_hash = blake3::hash(&prior_raw).to_hex().to_string();
        let pause = managed.run.join(format!("{point}.pause"));
        std::fs::write(&pause, point)?;
        let node = &pair.nodes[0];
        let runtime_args = vec![
            "runtime-from-profile".into(),
            "--profile-file".into(),
            text(&node.profile),
        ];
        if node.ipc.exists() {
            std::fs::remove_file(&node.ipc)?;
        }
        let mut owner = spawn(
            &current(),
            &runtime_args,
            &managed.run,
            &format!("{point}-owner"),
            Some(&pause),
            None,
        )?;
        wait_file(&mut owner, &node.ipc, 15)?;
        let before = node.status()?;
        let mut config: Value = serde_json::from_slice(&std::fs::read(&node.branch.config)?)?;
        let quotas = [
            prior["logical_quota"][0].as_u64().context("quota0")? + 128 * 1024 * 1024,
            prior["logical_quota"][1].as_u64().context("quota1")? + 128 * 1024 * 1024,
        ];
        let physical = prior["physical_capacity"]
            .as_u64()
            .context("physical capacity")?
            + 512 * 1024 * 1024;
        config["logical_quota"] = json!(quotas);
        config["physical_capacity"] = json!(physical);
        std::fs::write(&node.branch.config, serde_json::to_vec_pretty(&config)?)?;
        let args = vec![
            "ordered".into(),
            "storage-increase-capacity-live".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--config-file".into(),
            text(&node.branch.config),
            "--expected-root-generation".into(),
            before["root_generation"].to_string(),
            "--expected-config-revision".into(),
            before["config_revision"].to_string(),
            "--expected-config-hash".into(),
            before["config_hash"]
                .as_str()
                .context("config hash")?
                .into(),
        ];
        let client = spawn(
            &current(),
            &args,
            &managed.run,
            &format!("{point}-cas"),
            None,
            None,
        )?;
        let hit = pause.with_extension("hit");
        wait_file(&mut owner, &hit, 20)?;
        ensure!(
            std::fs::read_to_string(&hit)? == point,
            "actual requested capacity/publication boundary"
        );
        owner.kill_wait()?;
        drop(client);
        ensure!(
            std::fs::read(&node.profile)? == prior_raw,
            "prior routed profile survives interrupted publication"
        );
        let mut resume = node.branch.upgrade(false);
        resume.extend(["--expected-profile-hash".into(), prior_hash.clone()]);
        let resumed = json_cli(&current(), &resume)?;
        let published: Value = serde_json::from_slice(&std::fs::read(&node.profile)?)?;
        ensure!(
            published["route"] == prior["route"]
                && published["logical_quota"] == json!(quotas)
                && published["physical_capacity"] == physical
                && published["config_revision"].as_u64()
                    == prior["config_revision"].as_u64().map(|v| v + 1),
            "public Ready resume publishes committed capacity and exact validated route"
        );
        let mut base_before = prior.clone();
        let mut base_after = published.clone();
        for field in [
            "route",
            "logical_quota",
            "physical_capacity",
            "config_revision",
        ] {
            base_before.as_object_mut().context("base")?.remove(field);
            base_after.as_object_mut().context("base")?.remove(field);
        }
        ensure!(
            base_before == base_after,
            "every immutable profile base field retained"
        );
        pair.start(0, None)?;
        pair.start(1, None)?;
        pair.exchange2()?;
        let route_after = wait_path(&pair.nodes[0], "relay")?;
        pair.nodes[0].send(
            0xc200 + round as u64,
            "real Event2 after interrupted capacity publication and public resume",
        )?;
        let after = pair.converge()?;
        for role in 0..2 {
            same_origin(&original[role], &after[role])?;
            ensure!(
                after[role]["q"] == original[role]["q"]
                    && after[role]["cuts"] == original[role]["cuts"],
                "capacity publication recovery retains the original selected Q/cuts"
            );
            let status = pair.nodes[role].status()?;
            for field in [
                "account_constructors",
                "outbound_constructors",
                "inbound_constructors",
            ] {
                ensure!(
                    status["stats"][field] == 0,
                    "no Native constructor on route recovery: {field}"
                );
            }
        }
        observations.push(json!({"point":point,"prior_profile_hash":prior_hash,"root_before":before,"resume":resumed,"published":published,"actual_route":route_after,"after":after}));
    }
    pair.stop()?;
    relay.kill_wait()?;
    managed.finish(json!({"classification":"two real interrupted capacity publications, public Ready resume and actual RelayOnly Event2 restarts on one original lineage","initial_route":initial_route,"boundaries":observations,"owned_children_cleaned":true}))
}
