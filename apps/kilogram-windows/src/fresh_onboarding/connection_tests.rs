use super::super::consent::tests::{cli, pair_in, repository, run_isolated, selected};
use super::*;

fn ready_pair(name: &str, missing: bool) -> Result<[Setup; 2]> {
    let parent =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/clean-start-cs3-2b/fixtures");
    ready_pair_at(&parent, name, missing)
}
fn ready_pair_at(parent: &Path, name: &str, missing: bool) -> Result<[Setup; 2]> {
    let mut nodes = pair_in(parent, name)?;
    let proposer = consent::role(&nodes[0])?;
    let signer = 1 - proposer;
    run_isolated(&mut nodes, proposer, consent::Action::Propose)?;
    let proposal = selected(&nodes[proposer])?;
    consent::select(&mut nodes[signer], &proposal)?;
    run_isolated(&mut nodes, signer, consent::Action::Cosign)?;
    let final_file = selected(&nodes[signer])?;
    consent::select(&mut nodes[proposer], &final_file)?;
    if !missing {
        for who in 0..2 {
            run_isolated(&mut nodes, who, consent::Action::Import)?;
        }
    }
    Ok(nodes)
}

#[cfg(windows)]
#[path = "connection_message_tests.rs"]
mod messages;

#[test]
fn cs41_lan_desktop_profile_socket_and_signed_pair() -> Result<()> {
    use kilogram_protocol::ordered_dialog_v2 as wire;

    let Ok(local) = std::env::var("KILOGRAM_CS41_LOCAL_IPV4") else {
        return Ok(());
    };
    let lan = LanSettings {
        local: local.parse()?,
        peer: std::env::var("KILOGRAM_CS41_PEER_IPV4")?.parse()?,
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/clean-start-cs4-1/fixtures");
    let [mut local, mut peer] = ready_pair_at(&root, "lan-listener-", false)?;
    let signal = Arc::new(AtomicBool::new(false));
    let (previous, _) = prepare(&mut local, &cli(), signal.clone(), false)?;
    let previous_raw = model::read(&previous, kilogram_runtime_ipc::ordered2::PROFILE_CAP)?;
    let home = local.home.clone();
    select_route(&mut local, Some(lan))?;
    drop(local);
    let mut local = Setup::open(&home)?;
    ensure!(lan_settings(&local) == Some(lan));
    select_route(
        &mut peer,
        Some(LanSettings {
            local: lan.peer,
            peer: lan.local,
        }),
    )?;
    let signer = {
        let mut repo = repository(&peer)?;
        kilogram_identity::DeviceIdentity::from_secret_bytes(*repo.owner_material()?.signing)
    };

    let mut session = Session::start(
        &mut local,
        &cli(),
        signal.clone(),
        false,
        &Cleanup::default(),
    )?;
    let view = session.poll(&signal)?;
    ensure!(view.running && !view.current(Instant::now(), consent::now()?));
    let path = session.export(&local, &signal)?;
    let raw = model::read(&path, locator_artifact::CAP)?;
    let original = locator_artifact::Envelope::decode(&raw)?.validate(&local, false)?;
    let status = session.last_status.as_ref().context("LAN status")?;
    let endpoint = status.endpoint.as_ref().context("LAN endpoint")?;
    ensure!(status.route.lan_peer_ipv4 == Some(lan.peer) && endpoint.relays.is_empty());
    for (slot, blob) in original.iter().enumerate() {
        let own = wire::ParsedRuntimeLocator2::parse(blob)?;
        let actual = own.direct_lan_socket(lan.local)?;
        ensure!(endpoint.ips == [actual] && actual.port() > 0);
        let row = status.installed(slot)?;
        ensure!(!row.live_admitted && row.selected_lan_peer.is_none());
    }
    let ipc: serde_json::Value =
        serde_json::from_slice(&model::read(&session.doc.ipc_file, 16 * 1024)?)?;
    let address: SocketAddr = ipc["content"]["address"]
        .as_str()
        .context("IPC address")?
        .parse()?;
    ensure!(address.ip().is_loopback() && address.port() > 0);
    let stop = session.finish(&Cleanup::default());
    ensure!(stop.graceful && stop.reaped && stop.eof && !stop.forced && stop.exit == Some(0));
    ensure!(!session.doc.ipc_file.exists());
    ensure!(model::read(&previous, kilogram_runtime_ipc::ordered2::PROFILE_CAP)? == previous_raw);

    let mut fixture = Vec::new();
    for blob in original {
        let mut f = wire::ParsedRuntimeLocator2::parse(&blob)?.fields().clone();
        std::mem::swap(&mut f.cert, &mut f.peer_cert);
        f.endpoint = [0x42; 32];
        f.ips = vec![SocketAddr::from((lan.peer, 39101))];
        fixture.push(f);
    }
    let pair = |fields: &[wire::RuntimeLocatorFields2]| -> Result<[Vec<u8>; 2]> {
        Ok([
            wire::ParsedRuntimeLocator2::sign(fields[0].clone(), &signer)?
                .raw()
                .to_vec(),
            wire::ParsedRuntimeLocator2::sign(fields[1].clone(), &signer)?
                .raw()
                .to_vec(),
        ])
    };
    let envelope = locator_artifact::Envelope::create(&peer, pair(&fixture)?)?;
    let correct = serde_json::to_value(&envelope)?;
    envelope.validate(&local, true)?;
    for case in 0..7 {
        let mut wrong = fixture.clone();
        match case {
            0 => wrong[1].ips = vec![SocketAddr::from((lan.peer, 39102))],
            1 => wrong[1].ips = vec!["127.0.0.1:39101".parse()?],
            2 => wrong[1].ips = vec!["0.0.0.0:39101".parse()?],
            3 => wrong[1].ips = vec!["[fe80::1]:39101".parse()?],
            4 => wrong[1].relay = Some("https://example.invalid/".into()),
            5 => wrong[1].endpoint = [0x13; 32],
            _ => wrong[1].c = wrong[0].c,
        }
        let mut changed = correct.clone();
        changed["locators"] = serde_json::to_value(pair(&wrong)?.map(|blob| artifact::hex(&blob)))?;
        let envelope = locator_artifact::Envelope::decode(&serde_json::to_vec(&changed)?)?;
        ensure!(
            envelope.validate(&local, true).is_err(),
            "LAN negative {case} passed"
        );
    }
    println!(
        "CS41_LAN_DESKTOP_PASS local_udp_snapshot=true tcp_ipc_loopback=true inactive_without_peer=true graceful_stop=true paired_signed_locator_negatives=7 prior_profile_preserved=true"
    );
    Ok(())
}

fn active_pair(sessions: &mut [Session; 2], nodes: &[Setup; 2]) -> Result<()> {
    let end = Instant::now() + Duration::from_secs(90);
    let mut previous = String::new();
    loop {
        let mut views = Vec::new();
        let mut state = Vec::new();
        for (who, session) in sessions.iter_mut().enumerate() {
            let view = session.poll(&AtomicBool::new(false))?;
            let status = session.last_status.as_ref().context("Status")?;
            for slot in 0..2 {
                let c = status.installed(slot)?;
                state.push(format!(
                    "node{who}/slot{slot}:phase={} active={} lease={} policy_error={} rev={}/{}",
                    c.phase,
                    c.active,
                    c.live_admitted,
                    c.policy_error.is_some(),
                    c.own_locator_revision,
                    c.peer_locator_revision
                ));
            }
            views.push(view);
        }
        let summary = state.join("; ");
        if summary != previous {
            println!(
                "CS32B_ACTIVE_PROGRESS {summary}; oldest_request_ms={}",
                views
                    .iter()
                    .map(|v| v.request_started.elapsed().as_millis())
                    .max()
                    .unwrap_or(0)
            );
            previous = summary.clone();
        }
        if views
            .iter()
            .all(|v| v.current(Instant::now(), consent::now().unwrap_or(u64::MAX)))
        {
            return Ok(());
        }
        if Instant::now() >= end {
            for who in 0..2 {
                for (pipe, drain) in sessions[who].drains.iter().enumerate() {
                    let raw = drain
                        .buffer
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Trace lock"))?
                        .iter()
                        .copied()
                        .collect::<Vec<_>>();
                    model::write(
                        &nodes[who].home.join(format!("failed-runtime-{pipe}.log")),
                        &raw,
                        false,
                    )?;
                }
            }
            anyhow::bail!(
                "Actual live Active2 deadline: {summary}; status_request_ages_ms={:?}",
                views
                    .iter()
                    .map(|v| v.request_started.elapsed().as_millis())
                    .collect::<Vec<_>>()
            );
        }
        thread::sleep(Duration::from_millis(150));
    }
}

fn exchange(sessions: &mut [Session; 2], nodes: &mut [Setup; 2]) -> Result<[PathBuf; 2]> {
    let signal = AtomicBool::new(false);
    ensure!(
        prepare(
            &mut nodes[0],
            &cli(),
            Arc::new(AtomicBool::new(false)),
            false
        )
        .is_err(),
        "live store allowed offline recheck"
    );
    ensure!(
        sessions[0].child.try_wait()?.is_none(),
        "lock refusal stopped the owner"
    );
    let paths = [
        sessions[0].export(&nodes[0], &signal)?,
        sessions[1].export(&nodes[1], &signal)?,
    ];
    for who in 0..2 {
        sessions[who].import(&mut nodes[who], &paths[1 - who], &signal)?;
    }
    Ok(paths)
}
#[test]
fn cs32b_request_start_freshness_and_schema_negatives() -> Result<()> {
    let origin = Instant::now();
    let wall = 100;
    let mut view = View {
        generation: 0,
        request_started: origin,
        connected: [true; 2],
        lease_expires: [200; 2],
        consent_expires: 200,
        running: true,
        text: String::new(),
        messages: Messages::default(),
    };
    ensure!(
        view.current(origin + Duration::from_millis(1999), wall),
        "fresh observation"
    );
    // A reply arriving at t=3s still carries the original request start.
    ensure!(
        !view.current(origin + Duration::from_secs(3), wall),
        "delayed reply revived badge"
    );
    ensure!(!view.current(origin, 200), "expiry revived badge");
    view.running = false;
    ensure!(!view.current(origin, wall), "old run revived badge");
    for bad in [
        br#"{"version":1}"#.as_slice(),
        br#"{"version":1,"version":1}"#.as_slice(),
        &vec![b'x'; locator_artifact::CAP + 1],
    ] {
        ensure!(
            locator_artifact::Envelope::decode(bad).is_err(),
            "bad locator shape"
        );
    }
    Ok(())
}

#[test]
fn cs32b_late_status_generation_is_discarded() -> Result<()> {
    let (actions, _action_rx) = mpsc::sync_channel(1);
    let (deliveries, _delivery_rx) = mpsc::sync_channel(1);
    let (views, view_rx) = mpsc::sync_channel(1);
    let (_done_tx, done) = mpsc::sync_channel(1);
    let mut work = Work {
        cancel: Arc::new(AtomicBool::new(false)),
        epoch: Arc::new(AtomicU64::new(2)),
        actions,
        deliveries,
        views: view_rx,
        done,
        cleanup: Arc::new(Cleanup::default()),
        view: View::waiting("pending"),
    };
    let observed = View {
        generation: 1,
        request_started: Instant::now(),
        connected: [true; 2],
        lease_expires: [u64::MAX; 2],
        consent_expires: u64::MAX,
        running: true,
        text: "old response".into(),
        messages: Messages::default(),
    };
    views.send(observed.clone())?;
    let _ = work.poll();
    ensure!(
        !work.view.current(Instant::now(), 0),
        "old action revived badge"
    );
    let mut current = observed;
    current.generation = 2;
    views.send(current.clone())?;
    let _ = work.poll();
    ensure!(
        work.view.current(Instant::now(), 0),
        "current generation rejected"
    );
    work.cancel();
    views.send(current)?;
    let _ = work.poll();
    ensure!(
        !work.view.current(Instant::now(), 0),
        "cancelled response revived badge"
    );
    Ok(())
}
#[test]
fn cs32b_actual_independent_connect_restart_and_failures() -> Result<()> {
    let mut nodes = ready_pair("cs32b-live-", true)?;
    let signal = Arc::new(AtomicBool::new(false));
    // Existing UI acknowledgements are deliberately not used as installation evidence.
    for node in &nodes {
        let mut repo = repository(node)?;
        ensure!(
            !repo.installed(0)? && !repo.installed(1)?,
            "fixture missing installation"
        );
    }
    let peer_signer = {
        let mut repository = repository(&nodes[1])?;
        kilogram_identity::DeviceIdentity::from_secret_bytes(*repository.owner_material()?.signing)
    };
    let mut sessions = [
        Session::start(
            &mut nodes[0],
            &cli(),
            signal.clone(),
            false,
            &Cleanup::default(),
        )?,
        Session::start(
            &mut nodes[1],
            &cli(),
            signal.clone(),
            false,
            &Cleanup::default(),
        )?,
    ];
    let paths = [
        sessions[0].export(&nodes[0], &signal)?,
        sessions[1].export(&nodes[1], &signal)?,
    ];
    let interrupted = sessions[0].import_with(&mut nodes[0], &paths[1], &signal, |slot| {
        ensure!(slot != 0, "controlled lost locator acknowledgement");
        Ok(())
    });
    ensure!(interrupted.is_err(), "interruption hook");
    ensure!(
        nodes[0]
            .record
            .connection
            .as_ref()
            .context("state")?
            .imports
            == [consent::Outcome::Started, consent::Outcome::NotStarted],
        "honest partial progress"
    );
    let _ = sessions[0].poll(&signal)?;
    let state = sessions[0].last_status.as_ref().context("status")?;
    ensure!(
        state.installed(0)?.peer_locator_revision > 0
            && state.installed(1)?.peer_locator_revision == 0,
        "real slot0 commit only"
    );
    let home = nodes[0].home.clone();
    let replacement = Setup::begin(&home.with_extension("closed-placeholder"), None)?;
    drop(std::mem::replace(&mut nodes[0], replacement));
    nodes[0] = Setup::open(&home)?;
    let generation = state.root_generation;
    sessions[0].import(&mut nodes[0], &paths[1], &signal)?;
    let _ = sessions[0].poll(&signal)?;
    ensure!(
        sessions[0]
            .last_status
            .as_ref()
            .context("status")?
            .root_generation
            == generation + 1,
        "retry wrote anything except missing slot1"
    );
    sessions[1].import(&mut nodes[1], &paths[0], &signal)?;
    active_pair(&mut sessions, &nodes)?;
    let original_instances = [sessions[0].instance.clone(), sessions[1].instance.clone()];
    let original_profiles = [
        nodes[0]
            .record
            .connection
            .as_ref()
            .context("state")?
            .profile
            .clone(),
        nodes[1]
            .record
            .connection
            .as_ref()
            .context("state")?
            .profile
            .clone(),
    ];
    let consent_refs = [
        consent::connection_refs(&nodes[0])?,
        consent::connection_refs(&nodes[1])?,
    ];
    // Real rollback/invalid second slot is rejected before either import.
    let original = model::read(&paths[1], locator_artifact::CAP)?;
    let mut bad: serde_json::Value = serde_json::from_slice(&original)?;
    bad["locators"][1] = "00".into();
    let bad_path = nodes[0].home.join("invalid-locators.json");
    model::write(&bad_path, &serde_json::to_vec(&bad)?, false)?;
    let before = nodes[0].record.connection.clone();
    ensure!(
        sessions[0]
            .import(&mut nodes[0], &bad_path, &AtomicBool::new(false))
            .is_err(),
        "invalid locator accepted"
    );
    ensure!(
        nodes[0].record.connection == before,
        "negative changed progress"
    );
    for (field, value) in [
        ("invitation", serde_json::json!("aa".repeat(32))),
        (
            "recipient",
            serde_json::json!({"account":"aa".repeat(32),"device":"bb".repeat(32)}),
        ),
        (
            "contexts",
            serde_json::json!([nodes[0].record.contexts[1], nodes[0].record.contexts[0]]),
        ),
    ] {
        let mut v: serde_json::Value = serde_json::from_slice(&original)?;
        v[field] = value;
        let e = locator_artifact::Envelope::decode(&serde_json::to_vec(&v)?)?;
        ensure!(e.validate(&nodes[0], true).is_err(), "binding accepted");
    }
    for kind in 0..4 {
        let mut v: serde_json::Value = serde_json::from_slice(&original)?;
        for slot in 0..2 {
            let blob = artifact::unhex(v["locators"][slot].as_str().context("blob")?, 8192)?;
            let mut fields =
                kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::parse(&blob)?
                    .fields()
                    .clone();
            let wall = consent::now()?;
            match kind {
                0 => {
                    fields.not_before = wall + 60;
                    fields.expires = wall + 3600;
                }
                1 => {
                    fields.not_before = wall - 100;
                    fields.expires = wall - 1;
                }
                2 => {
                    fields.ips = vec!["192.0.2.1:1234".parse()?];
                }
                _ => {
                    fields.relay = Some("https://example.invalid/".into());
                }
            }
            let loc = kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::sign(
                fields,
                &peer_signer,
            )?;
            v["locators"][slot] = artifact::hex(loc.raw()).into();
        }
        let env = locator_artifact::Envelope::decode(&serde_json::to_vec(&v)?)?;
        ensure!(
            env.validate(&nodes[0], true).is_err(),
            "signed locator policy negative accepted"
        );
    }
    let stops = [
        sessions[0].finish(&Cleanup::default()),
        sessions[1].finish(&Cleanup::default()),
    ];
    for stop in &stops {
        ensure!(
            stop.graceful && stop.reaped && stop.eof && !stop.forced && stop.exit == Some(0),
            "graceful stop"
        );
    }
    drop(sessions);
    for node in &nodes {
        let mut repo = repository(node)?;
        ensure!(
            repo.installed(0)? && repo.installed(1)?,
            "real missing-slot recovery"
        );
    }
    let mut sessions = [
        Session::start(
            &mut nodes[0],
            &cli(),
            signal.clone(),
            false,
            &Cleanup::default(),
        )?,
        Session::start(
            &mut nodes[1],
            &cli(),
            signal.clone(),
            false,
            &Cleanup::default(),
        )?,
    ];
    for who in 0..2 {
        ensure!(
            sessions[who].instance != original_instances[who],
            "runtime instance retained"
        );
        ensure!(
            nodes[who]
                .record
                .connection
                .as_ref()
                .context("state")?
                .profile
                == original_profiles[who]
                && consent::connection_refs(&nodes[who])? == consent_refs[who],
            "restart changed profile/consents"
        );
        let view = sessions[who].poll(&AtomicBool::new(false))?;
        ensure!(
            !view.current(Instant::now(), consent::now()?),
            "historical active mistaken for connection"
        );
    }
    let new_paths = exchange(&mut sessions, &mut nodes)?;
    active_pair(&mut sessions, &nodes)?;
    let before = nodes[0].record.connection.clone();
    ensure!(
        sessions[0]
            .import(&mut nodes[0], &paths[1], &signal)
            .is_err(),
        "stale revision accepted"
    );
    ensure!(
        nodes[0].record.connection == before,
        "stale input replaced selection"
    );
    for who in 0..2 {
        let old =
            locator_artifact::Envelope::decode(&model::read(&paths[who], locator_artifact::CAP)?)?
                .validate(&nodes[who], false)?;
        let new = locator_artifact::Envelope::decode(&model::read(
            &new_paths[who],
            locator_artifact::CAP,
        )?)?
        .validate(&nodes[who], false)?;
        for slot in 0..2 {
            ensure!(
                kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::parse(&new[slot])?
                    .fields()
                    .revision
                    > kilogram_protocol::ordered_dialog_v2::ParsedRuntimeLocator2::parse(
                        &old[slot]
                    )?
                    .fields()
                    .revision,
                "locator revision not refreshed"
            );
        }
    }
    // Replacement does not redirect status/Shutdown to the other owned runtime.
    let replacement = sessions[1]
        .client
        .as_ref()
        .context("peer client")?
        .descriptor()
        .clone();
    fs::write(&sessions[0].doc.ipc_file, serde_json::to_vec(&replacement)?)?;
    ensure!(
        sessions[0].poll(&signal).is_err(),
        "replacement accepted as owned"
    );
    let stop = sessions[0].finish(&Cleanup::default());
    ensure!(
        !stop.graceful && stop.reaped && !stop.forced && stop.exit == Some(0),
        "replacement was hidden by graceful label"
    );
    ensure!(
        sessions[1].child.try_wait()?.is_none(),
        "replacement runtime was stopped"
    );
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let v = sessions[1].poll(&signal)?;
        if !v.current(Instant::now(), consent::now()?) {
            break;
        }
        ensure!(Instant::now() < end, "closed peer still connected");
        thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        sessions[1].finish(&Cleanup::default()).graceful,
        "peer graceful stop"
    );
    // Keep the deliberately replaced public file as fault evidence.
    fs::rename(
        &sessions[0].doc.ipc_file,
        directory(&nodes[0])?.join("foreign-descriptor-retained.json"),
    )?;
    drop(sessions);
    let mut forced = Session::start(
        &mut nodes[0],
        &cli(),
        signal.clone(),
        false,
        &Cleanup::default(),
    )?;
    forced.client = None;
    let stop = forced.finish(&Cleanup::default());
    ensure!(
        stop.forced && stop.reaped && stop.eof && !stop.graceful,
        "failed shutdown falsely graceful"
    );
    drop(forced);
    // An existing malformed profile remains unchanged and explicit fresh-path retry works.
    let profile = nodes[0]
        .record
        .connection
        .as_ref()
        .context("state")?
        .profile
        .clone()
        .context("profile")?;
    let profile_path = directory(&nodes[0])?.join(profile);
    fs::write(&profile_path, b"")?;
    ensure!(
        prepare(&mut nodes[0], &cli(), signal.clone(), false).is_err()
            && fs::read(&profile_path)?.is_empty(),
        "partial profile overwritten"
    );
    prepare(
        &mut nodes[0],
        &cli(),
        signal,
        false, /* refuse recorded conflict */
    )
    .err()
    .context("expected profile conflict")?;
    prepare(
        &mut nodes[0],
        &cli(),
        Arc::new(AtomicBool::new(false)),
        true,
    )?;
    let final_path = nodes[0].home.join("consent/final.kilogram");
    let unavailable = nodes[0].home.join("consent/final-unavailable-retained.bin");
    fs::rename(&final_path, &unavailable)?;
    ensure!(
        prepare(
            &mut nodes[0],
            &cli(),
            Arc::new(AtomicBool::new(false)),
            false
        )
        .is_err(),
        "missing final allowed start"
    );
    fs::rename(&unavailable, &final_path)?;
    println!(
        "CS32B_LIVE_PASS independent_roots=true actual_missing_slots_recovered=4 live_active_slots=4 restart_same_pins=true refreshed_locators=true graceful_stops=3 descriptor_conflict_stop=true forced_stop_classified=true actual_partial_retry=true stale_active_rejected=true"
    );
    Ok(())
}
#[test]
fn cs32b_prepare_ui_fixture() -> Result<()> {
    if std::env::var_os("KILOGRAM_CS32B_UI_FIXTURE").is_none() {
        return Ok(());
    }
    let nodes = ready_pair("cs32b-ui-", false)?;
    let paths = nodes.iter().map(|n| n.home.clone()).collect::<Vec<_>>();
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/clean-start-cs3-2b");
    let out = ui_fixture_index(&base)?;
    model::write(
        &out,
        &serde_json::to_vec_pretty(&serde_json::json!({"homes":paths}))?,
        false,
    )?;
    Ok(())
}

fn ui_fixture_index(base: &Path) -> Result<PathBuf> {
    let Some(tag) = std::env::var_os("KILOGRAM_CS32B_UI_FIXTURE_TAG") else {
        return Ok(base.join("ui-fixture.json"));
    };
    let tag = tag.to_str().context("Fixture tag")?;
    ensure!(
        !tag.is_empty()
            && tag.len() <= 32
            && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Fixture tag"
    );
    Ok(base.join(format!("ui-fixture-{tag}.json")))
}

#[test]
fn cs32b_valid_conflicting_consent_blocks_start() -> Result<()> {
    let mut nodes = ready_pair("cs32b-conflict-", true)?;
    let proposer = consent::role(&nodes[0])?;
    let signer = 1 - proposer;
    let proposal = nodes[proposer].home.join("alternate-proposal.bin");
    let final_file = nodes[signer].home.join("alternate-final.bin");
    let (nb, end) = consent::period(&nodes[0])?;
    for (who, cosign, output) in [(proposer, false, &proposal), (signer, true, &final_file)] {
        let node = &nodes[who];
        let info = node.record.public.as_ref().context("Account")?;
        let peer = node.record.peer.as_ref().context("Peer")?;
        let mut args: Vec<std::ffi::OsString> = vec![
            "ordered".into(),
            if cosign {
                "context2-cosign"
            } else {
                "context2-propose"
            }
            .into(),
            "--account-dir".into(),
            info.account_root_dir.clone().into_os_string(),
            "--expect-account".into(),
            info.account_id.to_string().into(),
            "--expect-device".into(),
            info.device_id.to_string().into(),
            "--expect-peer-account".into(),
            peer.account.to_string().into(),
            "--expect-peer-device".into(),
            peer.device.to_string().into(),
            "--context".into(),
            node.record.contexts[0].clone().into(),
            "--output-file".into(),
            output.as_os_str().into(),
            "--credentials-file".into(),
        ];
        args.extend(
            ["own.credential", "peer.credential"]
                .map(|n| node.home.join("consent").join(n).into_os_string()),
        );
        args.push("--minimum-authority-file".into());
        args.extend(
            ["own.authority", "peer.authority"]
                .map(|n| node.home.join("consent").join(n).into_os_string()),
        );
        if cosign {
            args.extend(["--proposal-file".into(), proposal.clone().into_os_string()]);
        } else {
            args.extend([
                "--not-before".into(),
                nb.to_string().into(),
                "--valid-for-seconds".into(),
                (end - nb - 1).to_string().into(),
            ]);
        }
        helper::public_command(&cli(), &args, Arc::new(AtomicBool::new(false)))?;
    }
    let info = nodes[0].record.public.as_ref().context("Account")?;
    let args = vec![
        "ordered".into(),
        "context2-import".into(),
        "--state-dir".into(),
        info.state_dir.clone().into_os_string(),
        "--expect-account".into(),
        info.account_id.to_string().into(),
        "--expect-claim".into(),
        info.claim.clone().into(),
        "--invitation-file".into(),
        final_file.clone().into_os_string(),
    ];
    helper::public_command(&cli(), &args, Arc::new(AtomicBool::new(false)))?;
    let generation = repository(&nodes[0])?.generation();
    ensure!(
        prepare(
            &mut nodes[0],
            &cli(),
            Arc::new(AtomicBool::new(false)),
            false
        )
        .is_err(),
        "conflicting valid installed consent allowed start"
    );
    let mut repo = repository(&nodes[0])?;
    ensure!(
        repo.generation() == generation && repo.installed(0)? && !repo.installed(1)?,
        "conflict changed store"
    );
    ensure!(
        nodes[0].record.connection.is_none(),
        "profile created before exact recheck"
    );
    println!("CS32B_CONFLICT_PASS alternate_valid_consent_preserved=true start_blocked=true");
    Ok(())
}

#[test]
fn cs32b_owned_start_failure_and_cancel_preserve_setup() -> Result<()> {
    let [mut left, right] = ready_pair("cs32b-start-failure-", false)?;
    let (_, doc) = prepare(&mut left, &cli(), Arc::new(AtomicBool::new(false)), false)?;
    fs::create_dir_all(&doc.locator_directory)?;
    let publication = doc
        .locator_directory
        .join(format!("{}.locator", left.record.contexts[0]));
    model::write(&publication, b"corrupt own publication", false)?;
    let generation = repository(&left)?.generation();
    let error = Session::start(
        &mut left,
        &cli(),
        Arc::new(AtomicBool::new(false)),
        false,
        &Cleanup::default(),
    )
    .err()
    .context("corrupt locator startup must fail")?;
    ensure!(
        error.to_string().contains("exited during startup"),
        "actual spawned child failure was not observed"
    );
    ensure!(
        left.record
            .connection
            .as_ref()
            .and_then(|s| s.last_outcome.as_deref())
            == Some("start-failed")
            && !doc.ipc_file.exists(),
        "failed child reported ready"
    );
    ensure!(
        model::read(&publication, 8192)? == b"corrupt own publication",
        "corrupt publication silently removed"
    );
    let mut reopened = repository(&left)?;
    ensure!(
        reopened.generation() == generation && reopened.installed(0)? && reopened.installed(1)?,
        "startup failure changed store or kept its lock"
    );
    drop(reopened);

    // Hold back the exit observation in the actual failed-start worker, even
    // though the corrupt publication makes the child exit on its own.
    let reveal_exit = Arc::new(AtomicBool::new(false));
    let home = left.home.clone();
    let saved = left.record.clone();
    let mut pending = Work::start_with_cleanup(
        left,
        cli(),
        false,
        None,
        Arc::new(Cleanup {
            reveal_exit: Some(reveal_exit.clone()),
            ..Cleanup::default()
        }),
    )?;
    let held = (|| -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(35);
        loop {
            ensure!(
                pending.poll().is_none(),
                "Setup returned before exit observation"
            );
            if pending.view.text.contains("cleanup unconfirmed") {
                break;
            }
            ensure!(Instant::now() < deadline, "Missing blocked startup view");
            thread::sleep(Duration::from_millis(20));
        }
        let error = Setup::open(&home)
            .err()
            .context("Setup lock was released")?;
        ensure!(
            error.to_string().contains("already open"),
            "Unexpected setup error"
        );
        ensure!(
            pending.action(Action::Export).is_err(),
            "Action accepted during cleanup"
        );
        pending.cancel();
        ensure!(
            pending.poll().is_none()
                && pending.view.text.contains("cleanup unconfirmed")
                && !pending.view.current(Instant::now(), consent::now()?),
            "Cancel cleared blocked ownership or confirmed a connection"
        );
        Ok(())
    })();
    reveal_exit.store(true, Ordering::Release);
    held?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some((returned, outcome)) = pending.poll() {
            let error = outcome
                .err()
                .context("Unconfirmed cleanup reported success")?;
            ensure!(
                error.to_string().contains("forced cleanup was unconfirmed")
                    && error
                        .root_cause()
                        .to_string()
                        .contains("exited during startup"),
                "Lost original start failure or cleanup classification"
            );
            ensure!(
                returned.record.local_id == saved.local_id
                    && returned.record.public == saved.public
                    && returned.record.contexts == saved.contexts
                    && returned
                        .record
                        .connection
                        .as_ref()
                        .and_then(|state| state.last_outcome.as_deref())
                        == Some("start-failed"),
                "Failure changed Setup or lost persisted start outcome"
            );
            let mut repo = repository(&returned)?;
            ensure!(
                repo.generation() == generation && repo.installed(0)? && repo.installed(1)?,
                "Startup cleanup changed durable consent state"
            );
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "Setup did not return after exit observation"
        );
        thread::sleep(Duration::from_millis(20));
    }
    println!(
        "CS32B_UNCONFIRMED_START_CLEANUP_PASS setup_lock_held=true actions_blocked=true error_retained=true"
    );

    let saved = right.record.clone();
    let mut work = Work::start(right, cli(), false, None)?;
    work.cancel();
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some((returned, outcome)) = work.poll() {
            ensure!(
                outcome.is_err() || outcome.is_ok_and(|stop| stop.reaped && stop.graceful),
                "cancel lost owned cleanup"
            );
            ensure!(
                returned.record.local_id == saved.local_id
                    && returned.record.public == saved.public
                    && returned.record.contexts == saved.contexts,
                "cancel replaced account"
            );
            let _ = repository(&returned)?;
            ensure!(
                !work.view.current(Instant::now(), consent::now()?),
                "cancel retained a badge"
            );
            break;
        }
        ensure!(Instant::now() < end, "cancelled startup failed to return");
        thread::sleep(Duration::from_millis(20));
    }
    println!(
        "CS32B_START_FAILURE_PASS actual_child_exited=true source_store_preserved=true cancelled_start_clean=true"
    );
    Ok(())
}

/// Separate read-only native observation during the real GUI walkthrough.
#[test]
fn cs32b_observe_current_ui_runtimes() -> Result<()> {
    let Some(name) = std::env::var_os("KILOGRAM_CS32B_UI_OBSERVATION") else {
        return Ok(());
    };
    let name = name.to_str().context("Observation name")?;
    ensure!(
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Observation name"
    );
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/clean-start-cs3-2b");
    let fixture: serde_json::Value =
        serde_json::from_slice(&model::read(&ui_fixture_index(&base)?, 4096)?)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut observations = Vec::new();
    for home in fixture["homes"].as_array().context("homes")? {
        let home = Path::new(home.as_str().context("home")?);
        let record: super::super::model::Record =
            serde_json::from_slice(&model::read(&home.join("setup.json"), 128 * 1024)?)?;
        let public = record.public.as_ref().context("Public account")?;
        let name = record
            .connection
            .as_ref()
            .and_then(|s| s.profile.as_ref())
            .context("Profile")?;
        let profile = Profile2Document::decode(&model::read(
            &home.join("connection").join(name),
            16 * 1024,
        )?)?;
        let descriptor = RuntimeIpcDescriptor::load(&profile.ipc_file)?;
        let client = Client2::new(
            descriptor,
            public.account_id,
            public.device_id,
            [
                kilogram_runtime_ipc::ordered2::id(&record.contexts[0])?,
                kilogram_runtime_ipc::ordered2::id(&record.contexts[1])?,
            ],
        )?;
        let start = Instant::now();
        let status = rt.block_on(client.status())?;
        ensure!(
            start.elapsed() < Duration::from_secs(2),
            "UI runtime observation already stale"
        );
        let mut contexts = Vec::new();
        for slot in 0..2 {
            let c = status.installed(slot)?;
            ensure!(
                c.phase == 3
                    && c.active
                    && c.live_admitted
                    && c.blocked.is_none()
                    && c.policy_error.is_none()
                    && c.lease_expires
                        .is_some_and(|end| end > consent::now().unwrap_or(u64::MAX)),
                "UI runtime not currently live Active2"
            );
            contexts.push(serde_json::json!({"slot":c.slot,"context":c.context,"active":c.active,"phase":c.phase,"live_admitted":c.live_admitted,"lease_expires":c.lease_expires,"current_consent":c.details.current_consent,"own_locator_revision":c.own_locator_revision,"peer_locator_revision":c.peer_locator_revision}));
        }
        observations.push(serde_json::json!({"home":home,"runtime_pid":status.runtime_pid,"runtime_instance":status.runtime_instance,"root_generation":status.root_generation,"descriptor_identity_verified":true,"contexts":contexts}));
    }
    ensure!(observations.len() == 2, "two UI runtimes");
    let directory = base.join("observations");
    fs::create_dir_all(&directory)?;
    model::write(
        &directory.join(format!("{name}.json")),
        &serde_json::to_vec_pretty(&observations)?,
        false,
    )?;
    println!(
        "CS32B_UI_OBSERVATION_PASS runtimes=2 live_active_contexts=4 same_descriptor_identity=true"
    );
    Ok(())
}
