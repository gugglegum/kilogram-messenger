//! Prepare disposable terminal Setups without starting a runtime.
use anyhow::{Context, Result, ensure};
use kilogram_first_dialog::{
    artifact::{Artifact, FILE_CAP},
    connection::{LanSettings, State},
    consent::{self, Action},
    consent_artifact::Envelope,
    helper, model,
    model::Setup,
};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::Write,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

fn bootstrap(setup: &mut Setup, image: &Path) -> Result<()> {
    let output = helper::run(
        &image.join("kilogram-bootstrap.exe"),
        &setup.record.workspace,
        &setup.record.contexts,
        Arc::new(AtomicBool::new(false)),
    )?;
    setup.accept(output, &setup.record.local_id.clone())?;
    // The helper only creates disposable laboratory identities.
    setup.acknowledge(true)
}

fn configure_lan(setup: Setup, settings: LanSettings) -> Result<Setup> {
    let state = State {
        version: 1,
        lan: Some(settings),
        ..State::default()
    };
    state.validate()?;
    ensure!(
        setup.record.connection.is_none(),
        "fixture already configured"
    );
    let home = setup.home.clone();
    // Stage the same initial connection record as the frontend, under the Setup lock.
    let mut record = setup.record.clone();
    record.connection = Some(state);
    let mut next = tempfile::NamedTempFile::new_in(&home)?;
    serde_json::to_writer_pretty(&mut next, &record)?;
    next.flush()?;
    next.as_file().sync_all()?;
    next.persist(home.join("setup.json"))?;
    drop(setup);
    let reopened = Setup::open(&home)?;
    ensure!(reopened.record == record, "saved LAN fixture mismatch");
    Ok(reopened)
}

fn run() -> Result<Value> {
    let args: Vec<_> = env::args().skip(1).collect();
    ensure!(
        args.len() >= 3,
        "fixture action, setup and image directory required"
    );
    let (op, home, image) = (&args[0][..], Path::new(&args[1]), Path::new(&args[2]));
    ensure!(home.is_absolute() && home.file_name().is_some_and(|name| name == "setup"));
    ensure!(image.is_absolute());
    let exe = image.join("kilogram-cli.exe");
    let cancel = Arc::new(AtomicBool::new(false));
    let mut setup = match (op, &args[3..]) {
        ("invite", []) => {
            ensure!(!home.exists(), "expected a fresh Setup path");
            let mut setup = Setup::begin(home, None)?;
            bootstrap(&mut setup, image)?;
            return Ok(json!({"artifact": setup.export()?}));
        }
        ("join", [path]) => {
            ensure!(!home.exists(), "expected a fresh Setup path");
            let invite = Artifact::decode(&model::read(Path::new(path), FILE_CAP)?)?;
            let mut setup = Setup::begin(home, Some(invite))?;
            bootstrap(&mut setup, image)?;
            return Ok(json!({"artifact": setup.export()?, "role": consent::role(&setup)?}));
        }
        ("reply" | "propose" | "cosign" | "import", _) => {
            ensure!(
                fs::metadata(home.join("setup.json")).is_ok(),
                "missing fixture"
            );
            Setup::open(home)?
        }
        _ => anyhow::bail!("unknown fixture action or arguments"),
    };
    match (op, &args[3..]) {
        ("reply", [path]) => {
            let reply = Artifact::decode(&model::read(Path::new(path), FILE_CAP)?)?;
            setup.select_reply(reply, true)?;
            Ok(json!({"role": consent::role(&setup)?}))
        }
        ("propose", []) => {
            consent::execute(&mut setup, &exe, Action::Propose, cancel)?;
            Ok(json!({"artifact": consent::export(&setup)?}))
        }
        ("cosign", [path]) => {
            let proposal = Envelope::decode(&model::read(Path::new(path), FILE_CAP)?)?;
            consent::select(&mut setup, &proposal)?;
            consent::execute(&mut setup, &exe, Action::Cosign, cancel)?;
            Ok(json!({"artifact": consent::export(&setup)?}))
        }
        ("import", [inputs @ .., local, peer]) => {
            let role = consent::role(&setup)?;
            match (role, inputs) {
                (0, [path]) => {
                    let artifact = Envelope::decode(&model::read(Path::new(path), FILE_CAP)?)?;
                    consent::select(&mut setup, &artifact)?;
                }
                (1, []) => {} // Cosign already retained this host's final artifact.
                _ => anyhow::bail!("consent input does not match fixture role"),
            }
            consent::execute(&mut setup, &exe, Action::Import, cancel)?;
            ensure!(setup.consent_confirmed, "both consent imports required");
            let setup = configure_lan(
                setup,
                LanSettings {
                    local: local.parse().context("local IPv4")?,
                    peer: peer.parse().context("peer IPv4")?,
                },
            )?;
            let identity = setup.ready()?;
            Ok(json!({"contexts": setup.record.contexts, "role": role,
                      "account": identity.account_id, "device": identity.device_id}))
        }
        _ => anyhow::bail!("invalid arguments for fixture action"),
    }
}

fn main() -> Result<()> {
    let result = run()?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
