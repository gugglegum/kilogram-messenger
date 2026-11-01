//! Two real publication cuts on one fresh pair, with explicit delivery evidence.
use super::*;
use kilogram_cli::ordered_dialog::{runtime7::Profile3, state};
use kilogram_protocol::{ordered_dialog_v1 as old, ordered_dialog_v2 as wire};
use kilogram_state::indexed_dialog::{self as db, migration7::live1 as store};
use std::io::Write;
#[path = "migration_s53_capacity_oracle.rs"]
mod oracle;
#[path = "migration_s53_capacity_witness.rs"]
mod witness;

fn hash(raw: &[u8]) -> String {
    blake3::hash(raw).to_hex().to_string()
}
fn phase(run: &Path, name: &str, value: Value) -> Result<Value> {
    phase_record(run, name, value, "completed G2 phase; not whole-case PASS")
}
fn phase_record(run: &Path, name: &str, mut value: Value, classification: &str) -> Result<Value> {
    s53_deadline::check()?;
    public_fingerprints(&mut value)?;
    let raw = serde_json::to_vec(
        &json!({"case":"F-s53-capacity-recovery-r2-01","attempt":run.file_name().context("G2 attempt")?.to_str().context("G2 attempt UTF8")?,"inventory_identity":std::env::var("KILOGRAM_S53_INVENTORY_ID")?,"phase":name,"classification":classification,"observations":value}),
    )?;
    ensure!(raw.len() <= 128 * 1024, "G2 phase cap");
    let path = run.join(format!("capacity-{name}.json"));
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    f.write_all(&raw)?;
    f.sync_all()?;
    Ok(json!({"path":path,"bytes":raw.len(),"blake3":hash(&raw)}))
}
fn stopped_observation(
    managed: &Managed,
    phases: &mut Vec<Value>,
    pair: &Pair,
    requests: &[oracle::Request],
    name: &str,
) -> Result<[Value; 2]> {
    let reports = witness::stopped(pair, requests)?;
    phases.push(phase_record(
        &managed.run,
        &format!("{name}-observed"),
        json!({"state":reports}),
        "observed G2 stopped state; comparisons not evaluated; not PASS",
    )?);
    witness::check(&reports)?;
    Ok(reports)
}
fn absent(path: &Path) -> Result<()> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(_) => anyhow::bail!("G2 unexpected file {}", path.display()),
    }
}
fn clear_locators(pair: &Pair) -> Result<Value> {
    ensure!(
        pair.children.iter().all(Option::is_none),
        "G2 stopped before locator removal"
    );
    let mut pins = Vec::new();
    for node in &pair.nodes {
        let path = node.locators.join(format!("{}.locator2", pair.c));
        match std::fs::read(&path) {
            Ok(raw) => {
                pins.push(json!({"path":path,"bytes":raw.len(),"blake3":hash(&raw)}));
                std::fs::remove_file(path)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                pins.push(json!({"path":path,"absent":true}))
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(json!(pins))
}
fn start_routed(pair: &mut Pair, url: &str) -> Result<Value> {
    ensure!(
        pair.children.iter().all(Option::is_none),
        "G2 stopped for stored locator capture"
    );
    let previous = pair
        .nodes
        .iter()
        .map(|node| -> Result<_> {
            let profile = Profile3::load(&node.profile)?;
            let mut repo = profile.open(&node.profile)?;
            let raw = repo.existing_locator(node.slot, true)?;
            if let Some(raw) = &raw {
                ensure!(
                    wire::ParsedRuntimeLocator2::parse(raw)?.fields().revision
                        == repo.existing_state(node.slot)?.locator_revision[0],
                    "G2 stored prior locator revision"
                );
            }
            Ok(raw)
        })
        .collect::<Result<Vec<_>>>()?;
    let removed = clear_locators(pair)?;
    pair.start(0, None)?;
    pair.start(1, None)?;
    pair.exchange2()?;
    pair.converge()?;
    let scope = wire::VerifiedExistingScope1::verify_export(&std::fs::read(
        pair.base.join("initial.origin.cbor"),
    )?)?;
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let mut reports = Vec::new();
    let mut revisions = Vec::new();
    for (local, node) in pair.nodes.iter().enumerate() {
        let full = wait_path(node, "relay")?;
        let row = oracle::admitted(&full, node)?;
        let role = oracle::number(&row, "role")? as usize;
        let route = Profile3::load(&node.profile)?
            .route
            .context("G2 explicit route")?;
        ensure!(
            route.effective()? == kilogram_transport_iroh::RoutePolicy::RelayOnly
                && route.relay.as_deref() == Some(url)
                && route.bind.is_none(),
            "G2 admitted exact relay"
        );
        let raw = std::fs::read(node.locators.join(format!("{}.locator2", pair.c)))?;
        let locator = wire::ParsedRuntimeLocator2::parse(&raw)?;
        locator.verify(
            state::parse_id(&pair.c)?,
            &scope.peers()[role],
            &scope.peers()[1 - role],
        )?;
        let f = locator.fields();
        if let Some(old) = &previous[local] {
            oracle::locator_revision(
                wire::ParsedRuntimeLocator2::parse(old)?.fields().revision,
                f.revision,
                old,
                &raw,
            )?;
        }
        ensure!(
            f.relay.as_deref() == Some(url)
                && f.ips.is_empty()
                && f.not_before <= wall
                && wall < f.expires,
            "G2 current relay locator"
        );
        ensure!(
            oracle::number(&row, "own_locator_revision")? == f.revision,
            "G2 current own revision"
        );
        let routes = oracle::field(&full, "routes")?
            .as_array()
            .context("G2 routes")?;
        let actual = routes.get(node.slot as usize).context("G2 route")?;
        ensure!(
            oracle::number(actual, "slot")? == u64::from(node.slot)
                && oracle::field(actual, "policy")?.as_str() == Some("relay-only")
                && oracle::field(actual, "selected_path")?.as_str() == Some("relay"),
            "G2 actual relay transport"
        );
        revisions.push(f.revision);
        reports.push(json!({"pid":pair.children[local].as_ref().context("G2 owned process")?.0.id(),"status":full,"raw":hash(&raw),"revision":f.revision}));
    }
    for local in 0..2 {
        ensure!(
            oracle::number(
                pair.nodes[local].context(&reports[local]["status"])?,
                "peer_locator_revision"
            )? == revisions[1 - local],
            "G2 peer revision"
        );
    }
    Ok(json!({"removed_public_pins":removed,"current_publications":reports}))
}
fn live(pair: &Pair, requests: &[oracle::Request]) -> Result<([Value; 2], [oracle::History; 2])> {
    let full = [pair.nodes[0].status()?, pair.nodes[1].status()?];
    let mut histories = Vec::new();
    for (local, status) in full.iter().enumerate() {
        let row = oracle::admitted(status, &pair.nodes[local])?;
        let next: [u64; 2] = serde_json::from_value(oracle::field(&row, "next")?.clone())?;
        histories.push(oracle::history(
            pair.nodes[local].history(32, None)?,
            next,
            2 + requests.len(),
        )?);
    }
    ensure!(histories[0] == histories[1], "G2 identical full histories");
    for (round, saved) in requests.iter().enumerate() {
        let actual = oracle::request(serde_json::from_str(&pair.command(
            0,
            "request-status3",
            &["--request-id".into(), oracle::request_id(round)],
        )?)?)?
        .context("G2 existing own request")?;
        ensure!(&actual == saved, "G2 exact earlier own request");
    }
    Ok((
        full,
        histories
            .try_into()
            .map_err(|_| anyhow::anyhow!("G2 two histories"))?,
    ))
}
fn native_matches(pair: &Pair, full: &[Value; 2], saved: &[Value; 2]) -> Result<()> {
    for local in 0..2 {
        for name in ["native_hash", "native_generation"] {
            oracle::exact(
                oracle::field(pair.nodes[local].context(&full[local])?, name)?,
                oracle::field(oracle::field(&saved[local], "stable")?, name)?,
            )?;
        }
    }
    Ok(())
}
fn deliver(
    pair: &mut Pair,
    round: usize,
    original: &[oracle::History; 2],
    original_live: &oracle::LiveSnapshot,
    saved: &mut Vec<oracle::Request>,
) -> Result<(Value, oracle::LiveSnapshot)> {
    let (before, history_before) = live(pair, saved)?;
    for status in &before {
        oracle::zero_work(status)?;
    }
    let row = pair.nodes[0].context(&before[0])?;
    let role = oracle::number(row, "role")? as u8;
    let mut next: [u64; 2] = serde_json::from_value(oracle::field(row, "next")?.clone())?;
    let ordinal = next[role as usize];
    next[role as usize] += 1;
    let send = pair.nodes[0].send(0xc200 + round as u64, oracle::body(round))?;
    let mut prior = oracle::send_result(&send)?;
    let until = Instant::now() + Duration::from_secs(25);
    let delivered = loop {
        s53_deadline::check()?;
        for child in pair.children.iter_mut().flatten() {
            child.alive()?;
        }
        let observed = oracle::request(serde_json::from_str(&pair.command(
            0,
            "request-status3",
            &["--request-id".into(), oracle::request_id(round)],
        )?)?)?
        .context("G2 queued request disappeared")?;
        oracle::stable(&prior, &observed)?;
        if observed.delivered {
            break observed;
        }
        prior = observed;
        ensure!(Instant::now() < until, "G2 delivery deadline");
        std::thread::sleep(Duration::from_millis(50));
    };
    saved.push(delivered.clone());
    pair.converge()?;
    let (after, history) = live(pair, saved)?;
    for local in 0..2 {
        oracle::preserved(&original[local], &history[local])?;
        oracle::preserved(&history_before[local], &history[local])?;
        oracle::message(&history[local], &delivered, round, role, ordinal)?;
        let row = pair.nodes[local].context(&after[local])?;
        ensure!(
            serde_json::from_value::<[u64; 2]>(oracle::field(row, "next")?.clone())? == next,
            "G2 exact author advance"
        );
        for name in [
            "account_constructors",
            "outbound_constructors",
            "inbound_constructors",
            "aborted_packet_calls",
            "ambiguous_packet_calls",
        ] {
            ensure!(
                oracle::number(oracle::field(&after[local], "stats")?, name)? == 0,
                "G2 no constructors/aborts"
            );
        }
        for (name, delta) in [
            ("committed_encrypt", u64::from(local == 0)),
            ("materialized", u64::from(local == 0)),
            ("committed_decrypt", u64::from(local == 1)),
            ("readable", u64::from(local == 1)),
        ] {
            ensure!(
                oracle::number(oracle::field(&after[local], "stats")?, name)?
                    == oracle::number(oracle::field(&before[local], "stats")?, name)? + delta,
                "G2 exact message work {name}"
            );
        }
        let old = pair.nodes[local].context(&before[local])?;
        ensure!(
            oracle::lineage(old)? == oracle::lineage(row)?
                && oracle::number(row, "native_generation")?
                    > oracle::number(old, "native_generation")?,
            "G2 same selection, legitimate Native advance"
        );
    }
    let observed = oracle::LiveSnapshot::capture(pair, &after, history.clone(), saved)?;
    observed.messages(original_live)?;
    Ok((
        json!({"send":serde_json::from_str::<Value>(&send)?,"request":delivered,"full":after,"history":history,"live":observed}),
        observed,
    ))
}

#[test]
fn s53_capacity_recovery_preserves_validated_route_public() -> Result<()> {
    let managed = Managed::new("F-s53-capacity-recovery-r2-01", 240)?;
    let mut phases = Vec::new();
    let mut requests = Vec::new();
    let mut pair = Pair::fresh(&managed.run, "capacity")?;
    pair.exchange2()?;
    pair.upgrade(0, 0xc1)?;
    pair.converge()?;
    let (initial, original) = live(&pair, &requests)?;
    let original_live =
        oracle::LiveSnapshot::capture(&pair, &initial, original.clone(), &requests)?;
    let mut latest = original_live.clone();
    for h in &original {
        ensure!(
            h.values().all(|r| r.format == 1)
                && h.values()
                    .map(|r| r.text.as_str())
                    .collect::<std::collections::BTreeSet<_>>()
                    == ["genuine C source 1", "genuine C source 2"]
                        .into_iter()
                        .collect(),
            "G2 two original texts"
        );
    }
    pair.stop()?;
    let fixture_stopped = stopped_observation(&managed, &mut phases, &pair, &requests, "fixture")?;
    latest.join(&fixture_stopped)?;
    phases.push(phase(
        &managed.run,
        "fixture",
        json!({"full":initial,"original":original,"live":latest,"stopped":fixture_stopped}),
    )?);
    let ready = managed.run.join("relay.ready");
    let stop = managed.run.join("relay.stop");
    absent(&ready)?;
    absent(&stop)?;
    absent(&ready.with_extension("pending"))?;
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
    wire::canonical_runtime_relay(&url)?;
    let address: std::net::SocketAddr = url
        .strip_prefix("http://")
        .and_then(|s| s.strip_suffix('/'))
        .context("G2 HTTP ready URL")?
        .parse()?;
    ensure!(
        address.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) && address.port() != 0,
        "G2 own loopback URL"
    );
    for node in &pair.nodes {
        route(node, "relay-only", Some(&url))?;
    }
    phases.push(phase(&managed.run,"routed",json!({"relay_pid":relay.0.id(),"relay_image":hash(&std::fs::read(&relay_exe)?),"url":url,"admission":start_routed(&mut pair,&url)?}))?);
    for (round, point) in ["capacity-committed", "profile-staged"]
        .into_iter()
        .enumerate()
    {
        relay.alive()?;
        pair.converge()?;
        pair.stop()?;
        let before_saved = stopped_observation(
            &managed,
            &mut phases,
            &pair,
            &requests,
            &format!("{point}-before-cut"),
        )?;
        latest.join(&before_saved)?;
        let removed = clear_locators(&pair)?;
        let node = &pair.nodes[0];
        let prior_raw = std::fs::read(&node.profile)?;
        let prior: Value = serde_json::from_slice(&prior_raw)?;
        let prior_hash = hash(&prior_raw);
        let pause = managed.run.join(format!("{point}.pause"));
        std::fs::write(&pause, point)?;
        if node.ipc.try_exists()? {
            std::fs::remove_file(&node.ipc)?;
        }
        let mut owner = spawn(
            &current(),
            &[
                "runtime-from-profile".into(),
                "--profile-file".into(),
                text(&node.profile),
            ],
            &managed.run,
            &format!("{point}-owner"),
            Some(&pause),
            None,
        )?;
        wait_file(&mut owner, &node.ipc, 15)?;
        let before = node.status()?;
        let mut config: db::migration7::MigrationConfig =
            serde_json::from_slice(&std::fs::read(&node.branch.config)?)?;
        for quota in &mut config.logical_quota {
            *quota = quota
                .checked_add(128 * 1024 * 1024)
                .context("G2 quota increase")?;
        }
        config.physical_capacity = config
            .physical_capacity
            .checked_add(512 * 1024 * 1024)
            .context("G2 physical increase")?;
        let revision = oracle::number(&before, "config_revision")?
            .checked_add(1)
            .context("G2 revision")?;
        let mut desired = prior.clone();
        desired["logical_quota"] = json!(config.logical_quota);
        desired["physical_capacity"] = json!(config.physical_capacity);
        desired["config_revision"] = json!(revision);
        let mut desired_raw = serde_json::to_vec_pretty(&desired)?;
        desired_raw.push(b'\n');
        std::fs::write(&node.branch.config, serde_json::to_vec_pretty(&config)?)?;
        phases.push(phase(&managed.run,&format!("{point}-before-cut"),json!({"before":before,"stopped":before_saved,"prior_profile":prior_hash,"removed":removed,"desired":desired,"owner_pid":owner.0.id()}))?);
        let args = vec![
            "ordered".into(),
            "storage-increase-capacity-live".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--config-file".into(),
            text(&node.branch.config),
            "--expected-root-generation".into(),
            oracle::number(&before, "root_generation")?.to_string(),
            "--expected-config-revision".into(),
            oracle::number(&before, "config_revision")?.to_string(),
            "--expected-config-hash".into(),
            oracle::field(&before, "config_hash")?
                .as_str()
                .context("G2 CAS hash")?
                .into(),
        ];
        let mut client = spawn(
            &current(),
            &args,
            &managed.run,
            &format!("{point}-cas"),
            None,
            None,
        )?;
        let hit = pause.with_extension("hit");
        wait_file(&mut owner, &hit, 20)?;
        owner.alive()?;
        ensure!(
            std::fs::read_to_string(&hit)? == point && std::fs::read(&node.profile)? == prior_raw,
            "G2 exact hit/old profile"
        );
        let mut stage_name = node
            .profile
            .file_name()
            .context("G2 profile name")?
            .to_os_string();
        stage_name.push(".kilogram-migration-stage");
        let stage = node.profile.with_file_name(stage_name);
        if round == 0 {
            absent(&stage)?;
        } else {
            ensure!(
                std::fs::read(&stage)? == desired_raw,
                "G2 exact staged serializer/overlay"
            );
        }
        phases.push(phase(&managed.run,&format!("{point}-hit"),json!({"point":point,"owner_pid":owner.0.id(),"client_pid":client.0.id(),"profile":prior_hash,"stage":if round==0{None}else{Some(hash(&desired_raw))}}))?);
        owner.kill_wait()?;
        let client_exit = if let Some(exit) = client.0.try_wait()? {
            ensure!(!exit.success(), "G2 interrupted client cannot succeed");
            format!("{exit}")
        } else {
            client.kill_wait()?;
            "parent-killed-and-reaped".into()
        };
        let committed = stopped_observation(
            &managed,
            &mut phases,
            &pair,
            &requests,
            &format!("{point}-committed"),
        )?;
        ensure!(
            std::fs::read(&node.profile)? == prior_raw,
            "G2 prior profile after kill"
        );
        latest.join(&committed)?;
        witness::capacity_only(&before_saved, &committed)?;
        ensure!(
            oracle::number(&committed[0]["capacity"], "revision")? == revision
                && committed[0]["capacity"]["hash"] != before["config_hash"],
            "G2 committed revision/hash"
        );
        phases.push(phase(
            &managed.run,
            &format!("{point}-committed"),
            json!({"state":committed,"client_reaped":client_exit}),
        )?);
        let mut resume = node.branch.upgrade(false);
        resume.extend(["--expected-profile-hash".into(), prior_hash]);
        let resumed = json_cli(&current(), &resume)?;
        let resumed_saved = stopped_observation(
            &managed,
            &mut phases,
            &pair,
            &requests,
            &format!("{point}-resumed"),
        )?;
        ensure!(
            oracle::field(&resumed, "zero_migration_native_calls")?.as_bool() == Some(true)
                && oracle::field(&resumed, "storage_stage")?.as_str() == Some("Ready")
                && oracle::number(&resumed, "config_revision")? == revision,
            "G2 Ready-only resume"
        );
        ensure!(
            std::fs::read(&node.profile)? == desired_raw,
            "G2 exact latest profile/route/base"
        );
        absent(&stage)?;
        latest.join(&resumed_saved)?;
        witness::resumed(&committed, &resumed_saved)?;
        oracle::exact(&committed[0]["capacity"], &resumed_saved[0]["capacity"])?;
        phases.push(phase(
            &managed.run,
            &format!("{point}-resumed"),
            json!({"resume":resumed,"state":resumed_saved}),
        )?);
        let admission = start_routed(&mut pair, &url)?;
        let (full, histories) = live(&pair, &requests)?;
        let admitted_live =
            oracle::LiveSnapshot::capture(&pair, &full, histories.clone(), &requests)?;
        admitted_live.messages(&original_live)?;
        latest.same(&admitted_live)?;
        native_matches(&pair, &full, &resumed_saved)?;
        for local in 0..2 {
            ensure!(
                oracle::lineage(pair.nodes[local].context(&full[local])?)?
                    == oracle::lineage(pair.nodes[local].context(&initial[local])?)?,
                "G2 original selected lineage"
            );
            oracle::preserved(&original[local], &histories[local])?;
        }
        phases.push(phase(
            &managed.run,
            &format!("{point}-admitted"),
            json!({"admission":admission,"history":histories,"requests":requests}),
        )?);
        let (delivery, observed) =
            deliver(&mut pair, round, &original, &original_live, &mut requests)?;
        latest = observed;
        phases.push(phase(
            &managed.run,
            &format!("{point}-delivered"),
            delivery,
        )?);
        pair.stop()?;
        let delivered_stopped = stopped_observation(
            &managed,
            &mut phases,
            &pair,
            &requests,
            &format!("{point}-stopped"),
        )?;
        latest.join(&delivered_stopped)?;
        phases.push(phase(
            &managed.run,
            &format!("{point}-stopped"),
            json!({"state":delivered_stopped,"live":latest}),
        )?);
        if round == 0 {
            start_routed(&mut pair, &url)?;
        }
    }
    let saved = stopped_observation(
        &managed,
        &mut phases,
        &pair,
        &requests,
        "before-final-restart",
    )?;
    latest.join(&saved)?;
    phases.push(phase(
        &managed.run,
        "before-final-restart",
        json!({"state":saved,"requests":requests}),
    )?);
    let admission = start_routed(&mut pair, &url)?;
    let (full, history) = live(&pair, &requests)?;
    let reopened_live = oracle::LiveSnapshot::capture(&pair, &full, history.clone(), &requests)?;
    reopened_live.messages(&original_live)?;
    latest.same(&reopened_live)?;
    reopened_live.join(&saved)?;
    native_matches(&pair, &full, &saved)?;
    for status in &full {
        oracle::zero_work(status)?;
    }
    pair.stop()?;
    let reopened = stopped_observation(&managed, &mut phases, &pair, &requests, "final-reopened")?;
    reopened_live.join(&reopened)?;
    witness::persisted(&saved, &reopened)?;
    phases.push(phase(&managed.run,"final-restarted",json!({"admission":admission,"full":full,"history":history,"state":reopened,"requests":requests}))?);
    relay.kill_wait()?;
    managed.finish(json!({"classification":"G2 two actual cuts, exact two deliveries and final persistence","phases":phases,"owned_children_cleaned":true}))
}
