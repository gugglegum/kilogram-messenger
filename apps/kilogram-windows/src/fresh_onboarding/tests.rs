use super::*;
use anyhow::Context as _;
use artifact::{Artifact, Kind};
use model::{Record, Setup};
use std::{fs, time::Duration};

fn fixture() -> Result<PathBuf> {
    let parent =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/clean-start-cs3-1/fixtures");
    fs::create_dir_all(&parent)?;
    Ok(tempfile::Builder::new()
        .prefix("CS31-")
        .tempdir_in(parent)?
        .keep())
}
fn exe() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/clean-start-runtime/kilogram-bootstrap.exe")
}
fn produce(setup: &Setup) -> Result<Ordered2BootstrapOutput> {
    helper::run(
        &exe(),
        &setup.record.workspace,
        &setup.record.contexts,
        Arc::new(AtomicBool::new(false)),
    )
}

#[test]
fn cs31_actual_invite_join_reopen_ack_public_only_and_frozen_bindings() -> Result<()> {
    let base = fixture()?;
    let mut alice = Setup::begin(&base.join("alice"), None)?;
    ensure!(
        !alice.record.workspace.exists() && alice.home.join("setup.json").is_file(),
        "pending saved outside absent destination before producer"
    );
    let output = produce(&alice)?;
    let phrase = zeroize::Zeroizing::new(output.recovery_phrase().to_owned());
    let generation = alice.record.local_id.clone();
    ensure!(
        alice
            .accept(
                Ordered2BootstrapOutput::decode(&output.encode()?)?,
                "stale-generation"
            )
            .is_err(),
        "stale result refused"
    );
    ensure!(
        alice.record.public.is_none() && alice.phrase.is_none(),
        "stale output cannot mutate setup"
    );
    alice.accept(output, &generation)?;
    ensure!(
        !alice.record.acknowledged && alice.export().is_err() && alice.acknowledge(false).is_err(),
        "success/export do not acknowledge words"
    );
    ensure!(
        !String::from_utf8_lossy(&fs::read(alice.home.join("setup.json"))?)
            .contains(phrase.as_str()),
        "no secret in progress"
    );
    drop(alice);
    let mut alice = Setup::open(&base.join("alice"))?;
    ensure!(
        !alice.incomplete && !alice.record.acknowledged && alice.phrase.is_none(),
        "ready reopen has no phrase and no automatic acknowledgement"
    );
    alice.acknowledge(true)?;
    let invitation = alice.export()?;
    let original = fs::read(&invitation)?;
    ensure!(
        alice.export()? == invitation && fs::read(&invitation)? == original,
        "same export idempotent, unchanged bytes"
    );
    ensure!(
        !String::from_utf8_lossy(&original).contains(phrase.as_str())
            && !String::from_utf8_lossy(&original).contains("claim")
            && !String::from_utf8_lossy(&original).contains("workspace"),
        "public-only exchange"
    );
    let invite = Artifact::decode(&original)?;
    let mut bob = Setup::begin(&base.join("bob"), Some(invite.clone()))?;
    ensure!(
        bob.record.contexts == alice.record.contexts
            && bob.record.peer.as_ref() == Some(&invite.sender),
        "join uses exact ordered slots/peer before creation"
    );
    let out = produce(&bob)?;
    let generation = bob.record.local_id.clone();
    bob.accept(out, &generation)?;
    bob.acknowledge(true)?;
    ensure!(bob.phrase.is_none(), "ack clears secret view state");
    let reply = Artifact::decode(&fs::read(bob.export()?)?)?;
    alice.check_reply(&reply)?;
    ensure!(
        alice.select_reply(reply.clone(), false).is_err(),
        "human verification required"
    );
    alice.select_reply(reply.clone(), true)?;
    let mut wrong = reply.clone();
    wrong.invitation = artifact::id()?;
    ensure!(
        alice.check_reply(&wrong).is_err(),
        "wrong invitation refused"
    );
    wrong = reply.clone();
    wrong.contexts.swap(0, 1);
    ensure!(
        alice.check_reply(&wrong).is_err(),
        "ordered contexts immutable"
    );
    wrong = reply.clone();
    wrong.recipient.as_mut().context("reply recipient")?.device =
        kilogram_identity::DeviceId::from_bytes([9; 32]);
    ensure!(
        alice.check_reply(&wrong).is_err(),
        "wrong recipient refused"
    );
    let record = alice.record.clone();
    drop(alice);
    let alice = Setup::open(&base.join("alice"))?;
    ensure!(
        alice.record == record
            && alice.record.peer.as_ref() == Some(&reply.sender)
            && alice.record.acknowledged
            && alice.phrase.is_none(),
        "awaiting state durable and public-bound"
    );
    // Existing export with unrelated data cannot be replaced.
    fs::write(&invitation, b"unrelated public file")?;
    ensure!(
        alice.export().is_err() && fs::read(&invitation)? == b"unrelated public file",
        "no clobber export"
    );
    drop(bob);
    let bob = Setup::open(&base.join("bob"))?;
    ensure!(
        bob.record.joined && bob.record.acknowledged && bob.record.contexts == record.contexts,
        "joined awaiting state reopens"
    );
    println!(
        "CS31_ACTUAL_INVITE_JOIN_PASS shared_contexts=true recovery_ack_required=true public_only=true reopened_both=true no_runtime_started=true"
    );
    Ok(())
}

#[test]
fn cs31_artifact_rejection_and_pending_incomplete_no_reuse() -> Result<()> {
    let base = fixture()?;
    let setup = Setup::begin(&base.join("pending"), None)?;
    let workspace = setup.record.workspace.clone();
    let record = fs::read(setup.home.join("setup.json"))?;
    drop(setup);
    ensure!(!workspace.exists(), "pending fixture has no helper/store");
    let mut reopened = Setup::open(&base.join("pending"))?;
    ensure!(
        reopened.incomplete
            && reopened.record.public.is_none()
            && reopened.acknowledge(true).is_err(),
        "incomplete cannot become ready"
    );
    drop(reopened);
    ensure!(
        Setup::begin(&base.join("pending"), None).is_err()
            && fs::read(base.join("pending/setup.json"))? == record,
        "existing pending record preserved"
    );
    fs::create_dir(&workspace)?;
    fs::write(workspace.join("unknown"), b"preserve")?;
    ensure!(
        Setup::open(&base.join("pending"))?.incomplete
            && fs::read(workspace.join("unknown"))? == b"preserve",
        "unknown account destination not reset"
    );
    for bytes in [
        b"{}".to_vec(),
        br#"{"version":1,"version":1}"#.to_vec(),
        vec![b' '; artifact::FILE_CAP + 1],
    ] {
        ensure!(
            Artifact::decode(&bytes).is_err(),
            "malformed/duplicate/oversize public input refused"
        );
    }
    let mut panel = Panel::new(base.join("untouched"), exe());
    panel.verified = true;
    panel.input = base.join("bad.kilogram").display().to_string();
    fs::write(&panel.input, b"not JSON")?;
    ensure!(
        panel.preview().is_err()
            && !panel.verified
            && panel.start(true).is_err()
            && !base.join("untouched").exists(),
        "invalid invitation before creation and stale confirmation cleared"
    );
    // Unknown future kinds and unknown fields are rejected, without bootstrap.
    let source = Setup::begin(&base.join("source"), None)?;
    let out = produce(&source)?;
    let info = out.public();
    let sender = artifact::Identity::new(
        &fs::read(&info.credential_file)?,
        &fs::read(&info.authority_file)?,
    )?;
    let invite = Artifact {
        version: 1,
        kind: Kind::Invite,
        mode: "ordered-dialog-v2".into(),
        invitation: source.record.invitation.clone(),
        contexts: source.record.contexts.clone(),
        sender,
        recipient: None,
    };
    let encoded = invite.encode()?;
    let value: serde_json::Value = serde_json::from_slice(&encoded)?;
    for key in ["proposal", "final", "locators"] {
        let mut bad = value.clone();
        bad["kind"] = key.into();
        ensure!(
            Artifact::decode(&serde_json::to_vec(&bad)?).is_err(),
            "future kind refused"
        );
    }
    let mut bad = value.clone();
    bad["path"] = "untrusted".into();
    ensure!(
        Artifact::decode(&serde_json::to_vec(&bad)?).is_err(),
        "unknown remote path refused"
    );
    let mut bad = invite.clone();
    bad.sender.device = kilogram_identity::DeviceId::from_bytes([7; 32]);
    ensure!(bad.validate().is_err(), "credential pins verified");
    bad = invite.clone();
    bad.contexts[1] = bad.contexts[0].clone();
    ensure!(
        Setup::begin(&base.join("invalid"), Some(bad)).is_err() && !base.join("invalid").exists(),
        "invalid contexts before account creation"
    );
    let mut bad_json = String::from_utf8(encoded)?;
    bad_json = bad_json.replacen("\"version\": 1", "\"version\": 1, \"version\": 1", 1);
    ensure!(
        Artifact::decode(bad_json.as_bytes()).is_err(),
        "actual duplicate field rejected"
    );
    // Preserve selected identity against a distinct, valid peer reply.
    let mut target = Setup::begin(&base.join("target"), None)?;
    let output = produce(&target)?;
    let gen_id = target.record.local_id.clone();
    target.accept(output, &gen_id)?;
    target.acknowledge(true)?;
    let own = target.record.public.as_ref().context("ready target")?;
    let mut reply = invite.clone();
    reply.kind = Kind::Reply;
    reply.invitation = target.record.invitation.clone();
    reply.contexts = target.record.contexts.clone();
    reply.recipient = Some(artifact::Recipient {
        account: own.account_id,
        device: own.device_id,
    });
    target.select_reply(reply.clone(), true)?;
    // A fully valid but different device/certificate from another account.
    let extra = Setup::begin(&base.join("extra"), None)?;
    let output = produce(&extra)?;
    reply.sender = artifact::Identity::new(
        &fs::read(&output.public().credential_file)?,
        &fs::read(&output.public().authority_file)?,
    )?;
    ensure!(
        reply.validate().is_ok() && target.select_reply(reply, true).is_err(),
        "confirmed peer cannot be replaced"
    );
    // Receipt existence is insufficient; public binding corruption is refused.
    drop(target);
    let record_path = base.join("target/setup.json");
    let mut rec: Record = serde_json::from_slice(&fs::read(&record_path)?)?;
    rec.contexts.swap(0, 1);
    fs::write(&record_path, serde_json::to_vec(&rec)?)?;
    ensure!(
        Setup::open(&base.join("target")).is_err(),
        "tampered progress must match public receipt"
    );
    Ok(())
}

#[test]
fn cs31_panel_cancel_late_result_and_secret_drop() -> Result<()> {
    let base = fixture()?;
    let mut panel = Panel::new(base.join("panel"), exe());
    panel.start(false)?;
    panel.pause(); // cancellation joins and reaps helper; no secret enters panel
    ensure!(
        panel.work.is_none()
            && panel
                .setup
                .as_ref()
                .context("active setup")?
                .phrase
                .is_none(),
        "cancel discards result"
    );
    panel.open();
    ensure!(
        panel.setup.is_some(),
        "saved pending state can be inspected after cancellation"
    );
    // Exercise successful asynchronous integration, not the legacy response path.
    let mut panel = Panel::new(base.join("normal"), exe());
    panel.start(false)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(65);
    while panel.work.is_some() && std::time::Instant::now() < deadline {
        panel.poll();
        thread::sleep(Duration::from_millis(20));
    }
    ensure!(
        panel.work.is_none()
            && panel
                .setup
                .as_ref()
                .is_some_and(|s| s.phrase.is_some() && !s.record.acknowledged),
        "actual panel receives private one-shot result"
    );
    let setup = panel.setup.as_ref().context("active setup")?;
    let words = zeroize::Zeroizing::new(
        setup
            .phrase
            .as_ref()
            .context("private phrase")?
            .as_str()
            .to_owned(),
    );
    ensure!(
        !panel
            .notice
            .as_ref()
            .is_some_and(|s| s.contains(words.as_str()))
            && !format!("{:?}", setup.record).contains(words.as_str()),
        "no secret in ordinary notices/progress Debug"
    );
    // Verify actual egui text shapes, without screenshots or printing words.
    let context = egui::Context::default();
    let shows_words = |panel: &mut Panel| -> bool {
        let frame = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| panel.draw(ui));
        });
        frame.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::Text(text) if text.galley.job.text.contains(words.as_str())))
    };
    ensure!(
        !shows_words(&mut panel),
        "masked by default in the actual view"
    );
    panel.reveal = true;
    ensure!(
        shows_words(&mut panel),
        "explicit reveal paints the real words"
    );
    panel
        .setup
        .as_mut()
        .context("active setup")?
        .acknowledge(true)?;
    panel.reveal = false;
    ensure!(
        !shows_words(&mut panel)
            && panel
                .setup
                .as_ref()
                .context("active setup")?
                .phrase
                .is_none(),
        "ack removes words from state and rendered view"
    );
    panel.pause();
    ensure!(
        panel
            .setup
            .as_ref()
            .context("active setup")?
            .phrase
            .is_none()
            && !panel.reveal,
        "leaving view drops words"
    );
    panel.open();
    ensure!(
        panel
            .setup
            .as_ref()
            .is_some_and(|s| !s.incomplete && s.record.acknowledged && s.phrase.is_none()),
        "acknowledged reopen does not re-output words"
    );
    Ok(())
}
