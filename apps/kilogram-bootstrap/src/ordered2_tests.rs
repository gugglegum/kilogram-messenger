use super::*;
use kilogram_identity::AccountRecoveryPhrase;

#[test]
fn cs2_direct_runtime6_reopens_and_recovers_root_identity() -> Result<()> {
    let parent = tempfile::tempdir()?;
    let workspace = parent.path().join("fresh");
    let output = create(&workspace, [[1; 32], [2; 32]])?;
    let info = output.public();
    let root = AccountRootState::load(&info.account_root_dir)?;
    let phrase = AccountRecoveryPhrase::parse(output.recovery_phrase())?;
    ensure!(
        phrase.account_id()? == info.account_id,
        "phrase key derivation/account identity"
    );
    let (package, witness) = root.export_recovery()?;
    let recovered = AccountRootState::recover(
        parent.path().join("recovered-root"),
        &phrase,
        &package,
        &witness,
    )?;
    ensure!(
        recovered.account_id() == info.account_id,
        "actual existing Root recovery primitive"
    );
    let cert = fs::read(&info.certificate_file)?;
    let authority = fs::read(&info.authority_file)?;
    verify(&info.state_dir, info, &cert, &authority)?;
    let mut repository = db::RuntimeRepository::open(
        StateDirectoryLock::acquire(&info.state_dir)?,
        db::ExpectedOwner {
            account: *info.account_id.as_bytes(),
            claim: kilogram_bootstrap_contract::parse_ordered2_id(&info.claim)?,
        },
        db::Config::default(),
    )?;
    ensure!(
        repository.profile()?.peer.is_none()
            && !repository.installed(0)?
            && !repository.installed(1)?,
        "bootstrap invents no peer or context ceremony"
    );
    drop(repository);
    let credential =
        wire::ParsedCredentialBundle::parse(&fs::read(&info.credential_file)?)?.verify_root()?;
    ensure!(
        credential.certificate().account_id() == info.account_id
            && credential.certificate().device_id() == info.device_id,
        "real exported CS1 credential"
    );
    let receipt = fs::read_to_string(&info.receipt_file)?;
    ensure!(
        receipt.contains("ready-runtime6")
            && !receipt.contains(output.recovery_phrase())
            && !receipt.contains("recovery_phrase"),
        "public receipt excludes secret result"
    );
    ensure!(
        !format!("{output:?}").contains(output.recovery_phrase()),
        "redacted Debug"
    );
    ensure!(
        info.account_root_dir.parent() == info.state_dir.parent()
            && info.account_root_dir != info.state_dir,
        "separate sibling Root/runtime"
    );
    ensure!(
        !info.state_dir.join("device-secret.key").exists()
            && !workspace.join("public/prekey-pool.bin").exists(),
        "no legacy device/prekey initialization"
    );
    ensure!(
        fs::read(info.state_dir.join(kilogram_state::STATE_VAULT_KEY_FILE))?.len() != 32,
        "enveloped vault key"
    );
    #[cfg(windows)]
    ensure!(
        info.root_key_protection == "windows-dpapi-current-user",
        "protected Root key provider"
    );
    let encoded = output.encode()?;
    let decoded = Ordered2BootstrapOutput::decode(&encoded)?;
    ensure!(
        decoded.public() == info,
        "structured final-path output roundtrip"
    );
    Ok(())
}
#[test]
fn cs2_existing_invalid_conflict_and_interruption_refuse() -> Result<()> {
    let parent = tempfile::tempdir()?;
    let old = parent.path().join("unknown-existing");
    fs::create_dir(&old)?;
    fs::write(old.join("opaque"), b"unchanged old/unknown state")?;
    ensure!(
        create(&old, [[1; 32], [2; 32]]).is_err()
            && fs::read(old.join("opaque"))? == b"unchanged old/unknown state"
            && fs::read_dir(&old)?.count() == 1,
        "existing destination unchanged"
    );
    let invalid = parent.path().join("not-created/invalid");
    ensure!(
        create(&invalid, [[1; 32], [1; 32]]).is_err()
            && !parent.path().join("not-created").exists(),
        "invalid contexts before any filesystem/secret creation"
    );
    let destination = parent.path().join("raced-empty-directory");
    let conflict = create_with(&destination, [[1; 32], [2; 32]], |point, dest| {
        if matches!(point, Point::BeforeReserve) {
            fs::create_dir(dest)?;
        }
        Ok(())
    });
    ensure!(
        conflict.is_err() && destination.is_dir() && fs::read_dir(&destination)?.count() == 0,
        "atomic reservation never replaces even an empty appeared directory"
    );
    let interrupted = parent.path().join("interrupted-before");
    let result = create_with(&interrupted, [[1; 32], [2; 32]], |point, _| {
        if matches!(point, Point::BeforeReserve) {
            anyhow::bail!("synthetic interruption before reservation");
        }
        Ok(())
    });
    ensure!(
        result.is_err() && !interrupted.exists(),
        "pre-reservation failure creates no workspace"
    );
    let interrupted = parent.path().join("interrupted-after");
    let result = create_with(&interrupted, [[1; 32], [2; 32]], |point, _| {
        if matches!(point, Point::AfterReserve) {
            anyhow::bail!("synthetic interruption after reservation");
        }
        Ok(())
    });
    ensure!(
        result.is_err()
            && interrupted.join("bootstrap-pending.json").is_file()
            && !interrupted.join("bootstrap-ready.json").exists(),
        "post-reservation interruption cannot acquire ready receipt"
    );
    ensure!(
        create(&interrupted, [[1; 32], [2; 32]]).is_err(),
        "incomplete workspace never silently reused"
    );
    let incomplete = parent.path().join("interrupted-before-commit");
    let result = create_with(&incomplete, [[1; 32], [2; 32]], |point, _| {
        if matches!(point, Point::BeforeCommit) {
            anyhow::bail!("synthetic interruption before final receipt");
        }
        Ok(())
    });
    ensure!(
        result.is_err()
            && incomplete.join("bootstrap-candidate.json").is_file()
            && !incomplete.join("bootstrap-ready.json").exists(),
        "complete candidate retained without successful commit/output"
    );
    let conflict = parent.path().join("ready-receipt-conflict");
    let result = create_with(&conflict, [[1; 32], [2; 32]], |point, path| {
        if matches!(point, Point::BeforeCommit) {
            fs::write(
                path.join("bootstrap-ready.json"),
                b"foreign public sentinel",
            )?;
        }
        Ok(())
    });
    ensure!(
        result.is_err()
            && fs::read(conflict.join("bootstrap-ready.json"))? == b"foreign public sentinel",
        "appeared ready receipt never overwritten or accepted as success"
    );
    Ok(())
}
