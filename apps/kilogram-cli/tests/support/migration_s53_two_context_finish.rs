//! Complete G3 capacity/Auto/final-reopen continuation. Source only until admission.
use super::*;
use persistence as persist;

struct Run<'a> {
    managed: &'a Managed,
    owners: &'a mut Owners,
    nodes: &'a [Node; 3],
    maps: &'a [oracle::Mapping],
    contexts: &'a [String; 2],
    base: PathBuf,
    url: String,
}
impl Run<'_> {
    fn checkpoint(
        &mut self,
        label: &str,
        expected: &[oracle::Directed],
        prior: &witness::Checkpoint,
    ) -> Result<witness::Checkpoint> {
        phase(
            self.managed,
            &format!("{label}-before-stop"),
            json!({"owned":self.owners.pids(),"relay_alive":self.owners.relay.is_some()}),
        )?;
        self.owners.stop()?;
        self.owners.alive()?;
        let saved = witness::collect(
            self.owners,
            self.nodes,
            self.maps,
            &self.base,
            self.contexts,
            expected,
            Some(prior),
        )?;
        phase(
            self.managed,
            &format!("{label}-stopped-observed"),
            json!({"state":saved,"Y":"preserved stopped witness; not reopened"}),
        )?;
        self.owners.alive()?;
        Ok(saved)
    }
    fn c1(&self, before: &witness::Checkpoint, after: &witness::Checkpoint) -> Result<()> {
        oracle::exact(
            "X persisted authority pins",
            oracle::field(&before[1], "authority")?,
            oracle::field(&after[1], "authority")?,
        )?;
        oracle::exact(
            "X/C1 complete stopped state",
            persist::context(&before[1], &self.contexts[0])?,
            persist::context(&after[1], &self.contexts[0])?,
        )?;
        witness::same_repair_slot(
            before,
            after,
            1,
            mapping(self.maps, 1, &self.contexts[0])?.slot,
        )
    }
    fn route(
        &mut self,
        prior: &witness::Checkpoint,
        mode: routes::Mode,
        rewrite: bool,
        label: &str,
    ) -> Result<(Live, [Vec<u8>; 2])> {
        self.owners.stopped()?;
        self.owners.alive()?;
        let c2 = &self.contexts[1];
        let profiles: [Value; 2] = [
            serde_json::from_slice(&std::fs::read(&self.nodes[1].profile)?)?,
            serde_json::from_slice(&std::fs::read(&self.nodes[2].profile)?)?,
        ];
        let previous: [routes::Stored; 2] = [
            serde_json::from_value(persist::context(&prior[1], c2)?["routing"].clone())?,
            serde_json::from_value(persist::context(&prior[2], c2)?["routing"].clone())?,
        ];
        phase(
            self.managed,
            &format!("{label}-route-input"),
            json!({"profiles":profiles,"state":prior,"mode":mode.policy(),"relay_url":self.url,"rewrite_overlay":rewrite}),
        )?;
        routes::remove_public(self.owners, self.nodes, self.maps, c2)?;
        for (i, owner) in [1, 2].into_iter().enumerate() {
            let desired = routes::overlay(&profiles[i], &self.url, mode)?;
            if rewrite {
                std::fs::write(
                    &self.nodes[owner].profile,
                    serde_json::to_vec_pretty(&desired)?,
                )?;
            }
            let actual: Value =
                serde_json::from_slice(&std::fs::read(&self.nodes[owner].profile)?)?;
            phase(
                self.managed,
                &format!("{label}-profile{owner}"),
                json!({"profile":actual}),
            )?;
            oracle::exact(
                "G3 route-only/exact unchanged profile",
                if rewrite { &desired } else { &profiles[i] },
                &actual,
            )?;
            ensure!(
                Profile3::load(&self.nodes[owner].profile)?
                    .route_config()?
                    .effective()?
                    .as_str()
                    == mode.policy(),
                "G3 effective route mode"
            );
        }
        self.owners
            .start_subset(self.nodes, label, [false, true, true])?;
        let nodes = self.nodes;
        self.owners.wait(15, || {
            Ok([1, 2]
                .iter()
                .all(|i| nodes[*i].locators.join(format!("{c2}.locator2")).is_file())
                .then_some(()))
        })?;
        let public = [
            routes::read_public(&nodes[1], c2)?,
            routes::read_public(&nodes[2], c2)?,
        ];
        phase(
            self.managed,
            &format!("{label}-public-observed"),
            json!({"public_C2":public,"owned":self.owners.pids(),"recreated":true}),
        )?;
        let scope = wire::VerifiedExistingScope1::verify_export(&std::fs::read(
            self.base.join("c2.origin.cbor"),
        )?)?;
        let locators = [
            routes::route_locator(
                &public[0],
                scope.c(),
                scope.peers(),
                mapping(self.maps, 1, c2)?.role,
                &self.url,
                routes::wall()?,
                mode,
            )?,
            routes::route_locator(
                &public[1],
                scope.c(),
                scope.peers(),
                mapping(self.maps, 2, c2)?.role,
                &self.url,
                routes::wall()?,
                mode,
            )?,
        ];
        for i in 0..2 {
            routes::revision(&previous[i].own, &public[i])?;
        }
        for (i, owner) in [1, 2].into_iter().enumerate() {
            self.owners.alive()?;
            cli(
                &current(),
                &[
                    "ordered".into(),
                    "locator2-import".into(),
                    "--existing".into(),
                    "--ipc-file".into(),
                    text(&nodes[owner].ipc),
                    "--slot".into(),
                    mapping(self.maps, owner, c2)?.slot.to_string(),
                    "--locator-file".into(),
                    text(&nodes[3 - owner].locators.join(format!("{c2}.locator2"))),
                ],
            )?;
            routes::revision(&previous[i].peer, &public[1 - i])?;
        }
        let maps = self.maps;
        let mut last = None;
        let mut diagnostic = None;
        let waiting = self.owners.wait(45, || {
            let status = samples(nodes, [false, true, true])?;
            let checked = routes::both_mode(&status, maps, c2, &locators, mode);
            diagnostic = checked.as_ref().err().map(|e| e.to_string());
            last = Some(status);
            Ok(checked.is_ok().then_some(()))
        });
        phase(
            self.managed,
            &format!("{label}-paths-observed"),
            json!({"status":last,"diagnostic":diagnostic}),
        )?;
        waiting?;
        admitted(self.owners, self.nodes, self.maps, Some(c2))?;
        let observed = live(self.owners, self.nodes, self.maps, self.managed, label)?;
        phase(
            self.managed,
            &format!("{label}-live-observed"),
            serde_json::to_value(&observed)?,
        )?;
        routes::both_mode(&observed.status, self.maps, c2, &locators, mode)?;
        joined(&observed, prior, self.maps)?;
        ensure!(
            oracle::paused(oracle::row(observed.status(1)?, &self.contexts[0])?),
            "G3 persistent X/C1 pause"
        );
        persist::no_work(&observed)?;
        Ok((observed, public))
    }
    fn capacity(
        &mut self,
        prior: &witness::Checkpoint,
        expected: &[oracle::Directed],
    ) -> Result<witness::Checkpoint> {
        self.owners.stopped()?;
        let c2 = &self.contexts[1];
        let node = &self.nodes[1];
        phase(
            self.managed,
            "capacity-solo-route-input",
            json!({"state":prior,"Z_stopped":true}),
        )?;
        std::fs::remove_file(node.locators.join(format!("{c2}.locator2")))?;
        self.owners
            .start_subset(self.nodes, "capacity-x", [false, true, false])?;
        let mut last = None;
        let waiting = self.owners.wait(20, || {
            let status = node.status()?;
            let row = oracle::row(&status, c2)?;
            let ready = row["publication_ready"] == true
                && row["own_locator_revision"].as_u64().is_some()
                && node.locators.join(format!("{c2}.locator2")).is_file();
            last = Some(status);
            Ok(ready.then_some(()))
        });
        phase(
            self.managed,
            "capacity-solo-public-status",
            json!({"status":last}),
        )?;
        waiting?;
        let public = routes::read_public(node, c2)?;
        phase(
            self.managed,
            "capacity-solo-publication",
            json!({"own":public,"owned":self.owners.pids()}),
        )?;
        let scope = wire::VerifiedExistingScope1::verify_export(&std::fs::read(
            self.base.join("c2.origin.cbor"),
        )?)?;
        routes::relay_locator(
            &public,
            scope.c(),
            scope.peers(),
            mapping(self.maps, 1, c2)?.role,
            &self.url,
            routes::wall()?,
        )?;
        let old: routes::Stored =
            serde_json::from_value(persist::context(&prior[1], c2)?["routing"].clone())?;
        routes::revision(&old.own, &public)?;
        let observed = live(
            self.owners,
            self.nodes,
            self.maps,
            self.managed,
            "capacity-solo",
        )?;
        phase(
            self.managed,
            "capacity-solo-live-observed",
            serde_json::to_value(&observed)?,
        )?;
        joined(&observed, prior, self.maps)?;
        persist::no_work(&observed)?;
        ensure!(
            oracle::paused(oracle::row(observed.status(1)?, &self.contexts[0])?),
            "G3 C1 pause before CAS"
        );
        let candidate = persist::candidate(&prior[1])?;
        let input = self.base.join("g3-capacity-candidate.json");
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&input)?
            .write_all(&serde_json::to_vec_pretty(&candidate)?)?;
        let before = node.status()?;
        let bridge = persist::solo_bridge(&prior[1], &before, c2, &public)?;
        phase(
            self.managed,
            "capacity-pre-cas-observed",
            json!({"live":before,"candidate":candidate,"derived_pre_cas":bridge,"classification":"source-derived stopped-to-solo routing bridge; no online repository opened","owned":self.owners.pids()}),
        )?;
        let args = vec![
            "ordered".into(),
            "storage-increase-capacity-live".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--config-file".into(),
            text(&input),
            "--expected-root-generation".into(),
            oracle::number(&before, "root_generation")?.to_string(),
            "--expected-config-revision".into(),
            oracle::number(&before, "config_revision")?.to_string(),
            "--expected-config-hash".into(),
            oracle::field(&before, "config_hash")?
                .as_str()
                .context("G3 CAS hash")?
                .into(),
        ];
        self.owners.alive()?;
        let result = cli(&current(), &args);
        phase(
            self.managed,
            "capacity-cas-result",
            json!({"args":args,"ok":result.is_ok(),"output":result.as_ref().ok(),"error":result.as_ref().err().map(|e|e.to_string().chars().take(512).collect::<String>())}),
        )?;
        result?;
        let after = node.status()?;
        let profile: Value = serde_json::from_slice(&std::fs::read(&node.profile)?)?;
        phase(
            self.managed,
            "capacity-after-live-observed",
            json!({"status":after,"profile":profile}),
        )?;
        oracle::exact(
            "G3 capacity profile publication",
            &persist::desired_profile(&prior[1], &candidate)?,
            &profile,
        )?;
        ensure!(
            after["config_revision"] == profile["config_revision"]
                && after["backend"]["physical_quota"] == candidate.physical_capacity,
            "G3 live capacity actual revision/backend"
        );
        // The candidate becomes the sole config used by subsequent stopped reads.
        std::fs::write(&node.branch.config, serde_json::to_vec_pretty(&candidate)?)?;
        let saved = self.checkpoint("capacity", expected, prior)?;
        persist::capacity(&bridge, &saved[1], &candidate, c2)?;
        witness::same_repair(prior, &saved, 1)?;
        witness::same_repair(prior, &saved, 2)?;
        persist::data(&prior[2], &saved[2], c2, false)?;
        for key in [
            "accounting",
            "credits",
            "config",
            "capacity",
            "profile",
            "generation",
        ] {
            oracle::exact(
                &format!("G3 stopped Z/{key} unaffected by X capacity"),
                oracle::field(&prior[2], key)?,
                oracle::field(&saved[2], key)?,
            )?;
        }
        ensure!(
            prior[2]["backend"]["physical_quota"] == saved[2]["backend"]["physical_quota"],
            "G3 stopped Z physical quota exact"
        );
        self.c1(prior, &saved)?;
        phase(
            self.managed,
            "capacity-asserted",
            json!({"state":saved,"routing_bridge_then_capacity":true,"private_repair_equal":true}),
        )?;
        Ok(saved)
    }
}
pub(super) fn run(
    managed: &Managed,
    owners: &mut Owners,
    nodes: &[Node; 3],
    maps: &[oracle::Mapping],
    contexts: &[String; 2],
    before: witness::Checkpoint,
    mut expected: Vec<oracle::Directed>,
) -> Result<Value> {
    let url = std::fs::read_to_string(managed.run.join("two-c/g3-relay.ready"))?;
    let mut run = Run {
        managed,
        owners,
        nodes,
        maps,
        contexts,
        base: managed.run.join("two-c"),
        url,
    };
    let capacity = run.capacity(&before, &expected)?;
    let (pre502, auto_public) = run.route(&capacity, routes::Mode::Auto, true, "auto502")?;
    let c2 = &contexts[1];
    let sender = view(nodes, maps, 1, c2)?;
    let role = mapping(maps, 1, c2)?.role;
    let next: [u64; 2] =
        serde_json::from_value(oracle::row(pre502.status(1)?, c2)?["next"].clone())?;
    let tails: [Option<String>; 2] =
        serde_json::from_value(capacity[1]["contexts"][c2]["stable"]["tails"].clone())?;
    ensure!(request(&sender, 502)?.is_none(), "G3 distinct request502");
    let mut delivered = oracle::Directed {
        context: c2.clone(),
        author: 1,
        role,
        id: 502,
        body: "G3 C2 Auto delivery".into(),
        ordinal: next[role as usize],
        sequence: oracle::number(&capacity[1], "next_author")?,
        previous: tails[role as usize]
            .as_deref()
            .map(state::parse_id)
            .transpose()?,
        saved: oracle::sent(&sender.send(502, "G3 C2 Auto delivery")?)?,
    };
    let mut prior = delivered.saved.clone();
    delivered.saved = run.owners.wait(25, || {
        let actual = request(&sender, 502)?.context("G3 request502 disappeared")?;
        oracle::request_stable(&prior, &actual)?;
        prior = actual.clone();
        Ok(actual.delivered.then_some(actual))
    })?;
    admitted(run.owners, nodes, maps, Some(c2))?;
    let after502 = live(run.owners, nodes, maps, managed, "delivery502")?;
    phase(
        managed,
        "delivery502-live-observed",
        json!({"expectation":delivered,"live":after502}),
    )?;
    oracle::progress(&pre502.status, &after502.status, maps, &delivered)?;
    for m in maps.iter().filter(|m| m.context == *c2) {
        delivered.history(
            &pre502.history[&key(m.owner, c2)],
            &after502.history[&key(m.owner, c2)],
        )?;
        ensure!(
            after502.history[&key(m.owner, c2)].len() == 4,
            "G3 exactly original/500/501/502"
        );
    }
    ensure!(
        oracle::paused(oracle::row(after502.status(1)?, &contexts[0])?),
        "G3 C1 paused after502"
    );
    expected.push(delivered);
    // Only public IPC delivery/quiescence is observed here. Actual ACK/source
    // settling is proved by the single stopped metadata checkpoint below.
    let mut last = None;
    let waiting = run.owners.wait(30, || {
        let status = samples(nodes, [false, true, true])?;
        let mut complete = true;
        for e in &expected {
            let actual = request(&view(nodes, maps, e.author, c2)?, e.id)?
                .context("G3 prior delivered request lost")?;
            oracle::request_stable(&e.saved, &actual)?;
            complete &= actual.delivered;
        }
        for owner in [1, 2] {
            let row = oracle::row(status[owner].as_ref().context("G3 settled live owner")?, c2)?;
            for name in ["queued", "pending", "debt"] {
                complete &= row["counts"][name] == 0;
            }
            complete &= row["control_debt"] == false;
        }
        last = Some(status);
        Ok(complete.then_some(()))
    });
    phase(
        managed,
        "pre-final-live-delivery-barrier",
        json!({"status":last,"classification":"public delivery/quiescence only; stopped ACK/source assertions follow"}),
    )?;
    waiting?;
    let pre_final = run.checkpoint("pre-final", &expected, &capacity)?;
    joined(&after502, &pre_final, maps)?;
    routes::stored_join(&pre_final, c2, &auto_public)?;
    persist::settled(&pre_final, maps, c2)?;
    run.c1(&before, &pre_final)?;
    phase(
        managed,
        "pre-final-settled-asserted",
        json!({"state":pre_final,"ack_and_source_settled":true,"no_retry":true}),
    )?;
    let (final_live, final_public) =
        run.route(&pre_final, routes::Mode::Auto, false, "final-restart")?;
    for e in &expected {
        let actual =
            request(&view(nodes, maps, e.author, c2)?, e.id)?.context("G3 final request absent")?;
        oracle::request_stable(&e.saved, &actual)?;
        ensure!(actual == e.saved, "G3 exact final request binding/delivery");
    }
    let saved = run.checkpoint("final", &expected, &pre_final)?;
    joined(&final_live, &saved, maps)?;
    routes::stored_join(&saved, c2, &final_public)?;
    persist::settled(&saved, maps, c2)?;
    for owner in [1, 2] {
        persist::routing(&pre_final[owner], &saved[owner], c2, true)?;
        witness::same_repair(&pre_final, &saved, owner)?;
    }
    run.c1(&before, &saved)?;
    oracle::exact("G3 frozen Y evidence", &before[0], &saved[0])?;
    phase(
        managed,
        "final-invariants-asserted",
        json!({"state":saved,"exact_record_whitelist":true,"private_repair_equal":true,"actual_four_record_deltas_reconciled":true}),
    )?;
    run.owners
        .relay
        .take()
        .context("G3 actual relay to reap")?
        .kill_wait()?;
    run.owners.require([false; 3])?;
    phase(
        managed,
        "cleanup-complete",
        json!({"node_owners_reaped":true,"relay_reaped":true}),
    )?;
    let mut phases = Vec::new();
    for entry in std::fs::read_dir(&managed.run)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("G3 phase path")?;
        if name.starts_with("g3-") && name.ends_with(".json") {
            let raw = std::fs::read(&path)?;
            ensure!(raw.len() <= 128 * 1024, "G3 phase cap");
            phases.push(json!({"file":name,"bytes":raw.len(),"blake3":hash(&raw)}));
        }
    }
    phases.sort_by_key(|p| p["file"].as_str().unwrap_or_default().to_owned());
    ensure!(phases.len() <= 80, "G3 bounded phase references");
    Ok(
        json!({"state":saved,"mappings":maps,"delivered":expected,"phases":phases,"node_owners_reaped":true,"relay_reaped":true,"ordinary_occupancy_only":true,"private_repair_compared_in_memory":true,"routing_and_capacity_accounting_separate":true}),
    )
}
