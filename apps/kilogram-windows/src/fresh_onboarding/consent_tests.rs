use super::super::{artifact::Artifact, model::Setup};
use super::*;
use kilogram_state::{StateDirectoryLock, indexed_dialog as db};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub(in crate::fresh_onboarding) fn cli() -> PathBuf {
    repo().join("target/clean-start-runtime/kilogram-cli.exe")
}
fn bootstrap(setup: &mut Setup) -> Result<()> {
    let out = helper::run(
        &repo().join("target/clean-start-runtime/kilogram-bootstrap.exe"),
        &setup.record.workspace,
        &setup.record.contexts,
        Arc::new(AtomicBool::new(false)),
    )?;
    let generation = setup.record.local_id.clone();
    setup.accept(out, &generation)?;
    setup.acknowledge(true)
}
pub(in crate::fresh_onboarding) fn pair(name: &str) -> Result<[Setup; 2]> {
    let parent = std::env::var_os("KILOGRAM_CS_TEST_FIXTURE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo().join(".tmp/clean-start-cs3-2a/fixtures"));
    pair_in(&parent, name)
}
pub(in crate::fresh_onboarding) fn pair_in(parent: &Path, name: &str) -> Result<[Setup; 2]> {
    fs::create_dir_all(parent)?;
    let base = tempfile::Builder::new()
        .prefix(name)
        .tempdir_in(parent)?
        .keep();
    let mut alice = Setup::begin(&base.join("inviter"), None)?;
    bootstrap(&mut alice)?;
    let invite = Artifact::decode(&model::read(&alice.export()?, artifact::FILE_CAP)?)?;
    let mut bob = Setup::begin(&base.join("joiner"), Some(invite))?;
    bootstrap(&mut bob)?;
    let reply = Artifact::decode(&model::read(&bob.export()?, artifact::FILE_CAP)?)?;
    alice.select_reply(reply, true)?;
    Ok([alice, bob])
}
pub(in crate::fresh_onboarding) fn run_isolated(
    nodes: &mut [Setup; 2],
    who: usize,
    action: Action,
) -> Result<()> {
    let peer_root = nodes[1 - who]
        .record
        .public
        .as_ref()
        .context("Peer")?
        .account_root_dir
        .clone();
    let hidden = peer_root.with_extension("unavailable");
    ensure!(!hidden.exists(), "Owned hidden Root path");
    fs::rename(&peer_root, &hidden)?;
    let result = execute(
        &mut nodes[who],
        &cli(),
        action,
        Arc::new(AtomicBool::new(false)),
    );
    fs::rename(&hidden, &peer_root)?;
    result
}
pub(in crate::fresh_onboarding) fn repository(setup: &Setup) -> Result<db::RuntimeRepository> {
    let info = setup.record.public.as_ref().context("Account")?;
    Ok(db::RuntimeRepository::open(
        StateDirectoryLock::acquire(&info.state_dir)?,
        db::ExpectedOwner {
            account: *info.account_id.as_bytes(),
            claim: parse_ordered2_id(&info.claim)?,
        },
        db::Config::default(),
    )?)
}
pub(in crate::fresh_onboarding) fn selected(setup: &Setup) -> Result<Envelope> {
    Envelope::decode(&model::read(&export(setup)?, artifact::FILE_CAP)?)
}
fn cancelled() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[test]
fn cs32a_actual_both_inviter_roles_independent_import_and_reopen() -> Result<()> {
    let mut seen = [false; 2];
    // Only fixture construction is random; bounded fresh attempts establish both
    // genuine inviter/device orders without changing production identities.
    for attempt in 0..32 {
        let mut nodes = pair("roles-")?;
        let inviter_role = role(&nodes[0])?;
        if seen[inviter_role] {
            continue;
        }
        let proposer = inviter_role;
        let signer = 1 - proposer;
        run_isolated(&mut nodes, proposer, Action::Propose)?;
        let proposal = selected(&nodes[proposer])?;
        let frozen = nodes[proposer].record.consent.clone();
        run_isolated(&mut nodes, proposer, Action::Propose)?;
        ensure!(
            nodes[proposer].record.consent == frozen && selected(&nodes[proposer])? == proposal,
            "same proposal inputs/bytes on retry"
        );
        select(&mut nodes[signer], &proposal)?;
        let path = nodes[signer].home.clone();
        drop(nodes[signer].record.consent.clone());
        // Real close/reopen while preserving the other model and its lock.
        let replacement = Setup::begin(&path.with_extension("temporary-empty"), None)?;
        let old = std::mem::replace(&mut nodes[signer], replacement);
        drop(old);
        nodes[signer] = Setup::open(&path)?;
        run_isolated(&mut nodes, signer, Action::Cosign)?;
        let final_file = selected(&nodes[signer])?;
        select(&mut nodes[proposer], &final_file)?;
        for who in 0..2 {
            run_isolated(&mut nodes, who, Action::Import)?;
            ensure!(
                nodes[who].consent_confirmed,
                "both current imports acknowledged"
            );
            let mut store = repository(&nodes[who])?;
            ensure!(
                store.installed(0)? && store.installed(1)?,
                "actual two-slot repository installation"
            );
            let generation = store.generation();
            drop(store);
            run_isolated(&mut nodes, who, Action::Import)?;
            ensure!(
                repository(&nodes[who])?.generation() == generation,
                "actual idempotent import has no new commit"
            );
        }
        println!(
            "CS32A_ROLES_PASS inviter_role={inviter_role} fixture_attempt={} own_root_only=true both_slots_installed=true idempotent_generation=true",
            attempt + 1
        );
        for node in nodes {
            let path = node.home.clone();
            let expected = node.record.consent.clone();
            drop(node);
            let reopened = Setup::open(&path)?;
            ensure!(
                reopened.record.consent == expected && !reopened.consent_confirmed,
                "reopen keeps evidence, not a false repository proof"
            );
            review(&reopened, true)?;
        }
        seen[inviter_role] = true;
        if seen == [true, true] {
            return Ok(());
        }
    }
    anyhow::bail!("Fixture construction did not produce both genuine inviter roles")
}

#[test]
fn cs32a_partial_output_conflict_ambiguous_import_and_expiry() -> Result<()> {
    let mut nodes = pair("partial-")?;
    let p = role(&nodes[0])?;
    let s = 1 - p;
    let signal = cancelled();
    let interrupted = execute_with(
        &mut nodes[p],
        &cli(),
        Action::Propose,
        signal.clone(),
        |slot| {
            if slot == 1 {
                anyhow::bail!("controlled lost signing result");
            }
            Ok(())
        },
    );
    ensure!(interrupted.is_err(), "interruption exercised");
    let state = nodes[p].record.consent.as_ref().context("State")?;
    ensure!(
        state.slots[0].output.is_some()
            && state.slots[1].output.is_none()
            && state.slots[1].signing == Outcome::Started,
        "first success retained and second ambiguous"
    );
    let failed_path = nodes[p].home.join("consent").join(
        state.slots[1]
            .last_output
            .as_ref()
            .context("Ambiguous path")?,
    );
    let retained = model::read(&failed_path, BLOB_CAP)?;
    run_isolated(&mut nodes, p, Action::Propose)?;
    ensure!(
        model::read(&failed_path, BLOB_CAP)? == retained,
        "ambiguous output retained, fresh retry filename"
    );
    let proposal = selected(&nodes[p])?;
    select(&mut nodes[s], &proposal)?;
    // Existing CLI output reservation conflict is real and precedes signing.
    let inputs = get_inputs(&nodes[s], true)?;
    for (name, raw) in [
        (
            "own.credential",
            artifact::unhex(&inputs.own.credential, 16 * 1024)?,
        ),
        (
            "peer.credential",
            artifact::unhex(&inputs.peer.credential, 16 * 1024)?,
        ),
        (
            "own.authority",
            artifact::unhex(&inputs.own.authority, 16 * 1024)?,
        ),
        (
            "peer.authority",
            artifact::unhex(&inputs.peer.authority, 16 * 1024)?,
        ),
    ] {
        public_file(&nodes[s], name, &raw)?;
    }
    for (slot, raw) in proposal.payloads()?.iter().enumerate() {
        public_file(&nodes[s], &format!("input-{slot}.proposal"), raw)?;
    }
    let conflict = nodes[s].home.join("consent/blocked.bin");
    fs::write(&conflict, b"unrelated")?;
    ensure!(
        helper::public_command(
            &cli(),
            &argv(&inputs, &nodes[s], 0, &conflict, true)?,
            cancelled()
        )
        .is_err()
            && fs::read(&conflict)? == b"unrelated",
        "CLI reserved-output conflict preserves existing bytes"
    );
    // A malformed proposal leaves an actual empty reserved CLI output.
    let malformed = nodes[s].home.join("consent/bad.proposal");
    fs::write(&malformed, b"invalid")?;
    let empty = nodes[s].home.join("consent/empty.bin");
    let mut args = argv(&inputs, &nodes[s], 0, &empty, true)?;
    let at = args
        .iter()
        .position(|a| a == "--proposal-file")
        .context("Proposal argv")?;
    args[at + 1] = malformed.into_os_string();
    ensure!(
        helper::public_command(&cli(), &args, cancelled()).is_err()
            && fs::metadata(&empty)?.len() == 0,
        "failed CLI reservation is not completion"
    );
    run_isolated(&mut nodes, s, Action::Cosign)?;
    let final_file = selected(&nodes[s])?;
    select(&mut nodes[p], &final_file)?;
    let info = nodes[p].record.public.as_ref().context("Account")?;
    let held = StateDirectoryLock::acquire(&info.state_dir)?;
    ensure!(
        execute(&mut nodes[p], &cli(), Action::Import, cancelled()).is_err()
            && !nodes[p].consent_confirmed,
        "actual directory-lock conflict is recoverable, not completion"
    );
    drop(held);
    let interrupted = execute_with(&mut nodes[p], &cli(), Action::Import, cancelled(), |slot| {
        if slot == 0 {
            anyhow::bail!("controlled result loss after durable import");
        }
        Ok(())
    });
    ensure!(
        interrupted.is_err() && !nodes[p].consent_confirmed,
        "ambiguous import not success label"
    );
    let mut store = repository(&nodes[p])?;
    ensure!(
        store.installed(0)? && !store.installed(1)?,
        "actual partial import"
    );
    let generation = store.generation();
    drop(store);
    ensure!(
        nodes[p].record.consent.as_ref().context("State")?.slots[0].importing == Outcome::Started,
        "lost result persisted uncertain"
    );
    let path = nodes[p].home.clone();
    let replacement = Setup::begin(&path.with_extension("reopen-holder"), None)?;
    let old = std::mem::replace(&mut nodes[p], replacement);
    drop(old);
    nodes[p] = Setup::open(&path)?;
    ensure!(
        !nodes[p].consent_confirmed
            && nodes[p].record.consent.as_ref().context("State")?.slots[0].importing
                == Outcome::Started,
        "ambiguous actual import remains uncertain on reopen"
    );
    run_isolated(&mut nodes, p, Action::Import)?;
    ensure!(
        nodes[p].consent_confirmed && repository(&nodes[p])?.generation() == generation + 1,
        "exact retry confirms slot0 idempotently and installs only missing slot1"
    );
    let inputs = get_inputs(&nodes[p], true)?;
    let blobs = final_file.payloads()?;
    ensure!(
        finals(&inputs, &blobs, Some(inputs.not_before + inputs.ttl)).is_err(),
        "exact expiry cannot be current permission"
    );
    ensure!(
        finals(&inputs, &blobs, Some(inputs.not_before.saturating_sub(121))).is_err(),
        "future outside Fresh skew refused"
    );
    ensure!(
        repository(&nodes[p])?.installed(0)?,
        "time refusal does not erase installed state"
    );
    Ok(())
}

#[test]
fn cs32a_public_negative_bindings_crypto_and_aggregate() -> Result<()> {
    let mut nodes = pair("negative-")?;
    let p = role(&nodes[0])?;
    let s = 1 - p;
    run_isolated(&mut nodes, p, Action::Propose)?;
    let proposal = selected(&nodes[p])?;
    let before = fs::read(nodes[s].home.join("setup.json"))?;
    let mut invalid = proposal.clone();
    invalid.contexts.swap(0, 1);
    ensure!(select(&mut nodes[s], &invalid).is_err(), "swapped contexts");
    invalid = proposal.clone();
    invalid.recipient.device = kilogram_identity::DeviceId::from_bytes([7; 32]);
    ensure!(select(&mut nodes[s], &invalid).is_err(), "wrong recipient");
    invalid = proposal.clone();
    invalid.invitation = artifact::id()?;
    ensure!(select(&mut nodes[s], &invalid).is_err(), "wrong invitation");
    invalid = proposal.clone();
    invalid.sender = Setup::identity(nodes[s].record.public.as_ref().context("Account")?)?;
    ensure!(select(&mut nodes[s], &invalid).is_err(), "wrong sender");
    invalid = proposal.clone();
    invalid.sender.authority =
        Setup::identity(nodes[s].record.public.as_ref().context("Account")?)?.authority;
    ensure!(
        select(&mut nodes[s], &invalid).is_err(),
        "wrong authority floor binding"
    );
    invalid = proposal.clone();
    invalid.blobs.swap(0, 1);
    ensure!(
        select(&mut nodes[s], &invalid).is_err(),
        "swapped opaque slots"
    );
    for index in [4usize, 5, 6] {
        let mut invalid = proposal.clone();
        let raw = invalid.payloads()?[0].clone();
        let mut value = wire::decode(&raw, BLOB_CAP)?;
        if let wire::Value::A(ref mut f) = value {
            if index == 4 {
                if let wire::Value::A(ref mut body) = f[index] {
                    body[7] = wire::u(0);
                }
            } else {
                f[index] = wire::b([0u8; 64]);
            }
        }
        invalid.blobs[0] = artifact::hex(&wire::encode(&value, BLOB_CAP)?);
        ensure!(
            select(&mut nodes[s], &invalid).is_err(),
            "wrong body or Root/seal signature refused before selection/signing"
        );
    }
    invalid = proposal.clone();
    let mut value = wire::decode(&invalid.payloads()?[0], BLOB_CAP)?;
    if let wire::Value::A(ref mut f) = value
        && let wire::Value::A(ref mut body) = f[4]
    {
        body[6] = wire::a([wire::u(99)]);
    }
    invalid.blobs[0] = artifact::hex(&wire::encode(&value, BLOB_CAP)?);
    ensure!(
        select(&mut nodes[s], &invalid).is_err(),
        "non-Fresh origin refused before signing"
    );
    ensure!(
        fs::read(nodes[s].home.join("setup.json"))? == before && nodes[s].record.consent.is_none(),
        "negative proposal refusals preserve prior state"
    );
    let raw = proposal.encode()?;
    let mut unknown: serde_json::Value = serde_json::from_slice(&raw)?;
    unknown["unexpected"] = "x".into();
    ensure!(
        Envelope::decode(&serde_json::to_vec(&unknown)?).is_err(),
        "unknown fields"
    );
    let mut duplicate = String::from_utf8(raw.clone())?;
    duplicate = duplicate.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    ensure!(
        Envelope::decode(duplicate.as_bytes()).is_err(),
        "duplicate fields"
    );
    let mut trailing = raw;
    trailing.extend_from_slice(b"{}");
    ensure!(Envelope::decode(&trailing).is_err(), "trailing data");
    invalid = proposal.clone();
    invalid.blobs[0] = "00".repeat(BLOB_CAP + 1);
    ensure!(invalid.payloads().is_err(), "per-blob cap before decoding");
    ensure!(
        Envelope::decode(&vec![b' '; artifact::FILE_CAP + 1]).is_err(),
        "aggregate file cap"
    );
    // Maximum two opaque blobs plus bounded sender overhead fits the512KiB cap.
    invalid = proposal.clone();
    invalid.blobs = ["00".repeat(BLOB_CAP), "00".repeat(BLOB_CAP)];
    ensure!(
        invalid.encode()?.len() <= artifact::FILE_CAP,
        "actual hex-expansion aggregate budget"
    );
    select(&mut nodes[s], &proposal)?;
    run_isolated(&mut nodes, s, Action::Cosign)?;
    let final_file = selected(&nodes[s])?;
    let mut bad = final_file.clone();
    let mut blobs = bad.payloads()?;
    let mut v = wire::decode(&blobs[1], BLOB_CAP)?;
    if let wire::Value::A(ref mut f) = v {
        let mut consent = wire::decode(f[5].bytes()?, 4096)?;
        if let wire::Value::A(ref mut signed) = consent {
            signed[2] = wire::b([0u8; 64]);
        }
        f[5] = wire::b(wire::encode(&consent, 4096)?);
    }
    blobs[1] = wire::encode(&v, BLOB_CAP)?;
    bad.blobs = blobs.each_ref().map(|b| artifact::hex(b));
    ensure!(
        select(&mut nodes[p], &bad).is_err() && !repository(&nodes[p])?.installed(0)?,
        "both finals checked before first import"
    );
    Ok(())
}

#[test]
fn cs32a_prepare_ui_fixture() -> Result<()> {
    if std::env::var_os("KILOGRAM_CS32A_UI_FIXTURE").is_none() {
        return Ok(());
    }
    let nodes = pair("ui-")?;
    let paths = nodes.iter().map(|s| s.home.clone()).collect::<Vec<_>>();
    let roles = nodes.iter().map(role).collect::<Result<Vec<_>>>()?;
    let out = repo().join(".tmp/clean-start-cs3-2a/ui-fixture.json");
    model::write(
        &out,
        &serde_json::to_vec_pretty(&serde_json::json!({"homes":paths,"roles":roles}))?,
        false,
    )?;
    Ok(())
}
