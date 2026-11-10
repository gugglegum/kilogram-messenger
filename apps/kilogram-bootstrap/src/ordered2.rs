//! Fresh recoverable Root + Runtime6. No legacy device/ratchet initialization.
use super::{resolve_new_workspace, write_new_file};
use anyhow::{Context, Result, ensure};
use kilogram_bootstrap_contract::{
    ORDERED2_BOOTSTRAP_MODE, ORDERED2_RECOVERY_SCOPE, Ordered2BootstrapOutput,
    Ordered2BootstrapPublic,
};
use kilogram_identity::{
    AccountRootState, DeviceCapability, DeviceEncryptionIdentity, DeviceIdentity,
};
use kilogram_protocol::ordered_dialog_v2 as wire;
use kilogram_state::{StateDirectoryLock, indexed_dialog as db};
use serde::Serialize;
use std::{fs, path::Path};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy)]
enum Point {
    BeforeReserve,
    AfterReserve,
    BeforeCommit,
}
#[derive(Serialize)]
struct Receipt<'a> {
    version: u8,
    profile_mode: &'static str,
    status: &'static str,
    public: &'a Ordered2BootstrapPublic,
    recovery_scope: &'static str,
}
fn receipt(info: &Ordered2BootstrapPublic, status: &'static str) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(&Receipt {
        version: 1,
        profile_mode: ORDERED2_BOOTSTRAP_MODE,
        status,
        public: info,
        recovery_scope: ORDERED2_RECOVERY_SCOPE,
    })?)
}
fn verify(
    path: &Path,
    info: &Ordered2BootstrapPublic,
    certificate: &[u8],
    authority: &[u8],
) -> Result<()> {
    let mut repository = db::RuntimeRepository::open(
        StateDirectoryLock::acquire(path)?,
        db::ExpectedOwner {
            account: *info.account_id.as_bytes(),
            claim: kilogram_bootstrap_contract::parse_ordered2_id(&info.claim)?,
        },
        db::Config::default(),
    )?;
    let material = repository.owner_material()?;
    let contexts = [
        kilogram_bootstrap_contract::parse_ordered2_id(&info.contexts[0])?,
        kilogram_bootstrap_contract::parse_ordered2_id(&info.contexts[1])?,
    ];
    let signing = DeviceIdentity::from_secret_bytes(*material.signing);
    let encryption = DeviceEncryptionIdentity::from_secret_bytes(*material.encryption);
    let cert = kilogram_identity::DeviceCertificate::decode_and_verify(certificate)?;
    ensure!(
        material.profile.enrollment.device == *info.device_id.as_bytes()
            && material.profile.contexts == contexts
            && material.profile.enrollment.certificate == certificate
            && material.profile.enrollment.authority == authority
            && signing.device_id() == info.device_id
            && cert.account_id() == info.account_id
            && cert.device_id() == info.device_id
            && cert.encryption_public_key() == encryption.public_key(),
        "Runtime6 final owner/enrollment/key binding"
    );
    let _ = wire::RootVerifiedCredential::from_parts(
        cert,
        kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(authority)?,
    )?;
    Ok(())
}
pub fn create(
    workspace_dir: impl AsRef<Path>,
    contexts: [[u8; 32]; 2],
) -> Result<Ordered2BootstrapOutput> {
    create_with(workspace_dir.as_ref(), contexts, |_, _| Ok(()))
}
fn create_with(
    requested: &Path,
    contexts: [[u8; 32]; 2],
    mut hook: impl FnMut(Point, &Path) -> Result<()>,
) -> Result<Ordered2BootstrapOutput> {
    ensure!(
        contexts[0] != contexts[1],
        "two distinct immutable context IDs required"
    );
    let absent = |path: &Path| -> Result<()> {
        match fs::symlink_metadata(path) {
            Ok(_) => anyhow::bail!("bootstrap destination already exists"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    };
    absent(requested)?;
    let workspace = resolve_new_workspace(requested)?;
    absent(&workspace)?;
    ensure!(
        workspace.as_os_str().len() <= 4096,
        "bounded public workspace path"
    );
    let _ = serde_json::to_vec(&workspace)
        .context("workspace path must be JSON encodable before secret creation")?;
    hook(Point::BeforeReserve, &workspace)?;
    // Atomic create_dir is the no-clobber reservation, not an exists check or a
    // replacing directory rename. Every subsequent failure retains this owned
    // incomplete workspace; another create can never silently reuse it.
    fs::create_dir(&workspace).context("reserve new Ordered2 workspace")?;
    write_new_file(
        &workspace.join("bootstrap-pending.json"),
        br#"{"version":1,"profile_mode":"ordered-dialog-v2","status":"incomplete-reservation"}"#,
    )?;
    hook(Point::AfterReserve, &workspace)?;
    let root_path = workspace.join("account-root");
    let state = workspace.join("device");
    let public = workspace.join("public");
    fs::create_dir(&public)?;
    let (root, phrase) = AccountRootState::create_recoverable(&root_path)?;
    let signing = DeviceIdentity::generate()?;
    let encryption = DeviceEncryptionIdentity::generate()?;
    let cert = root.issue_device_certificate(
        signing.device_id(),
        encryption.public_key(),
        &DeviceCapability::MESSAGING,
    )?;
    let authority = root.authority_snapshot()?;
    let devices = root.publish_device_list(std::slice::from_ref(&cert))?;
    let credential = wire::RootVerifiedCredential::from_parts(cert.clone(), authority.clone())?;
    let mut claim = [0; 32];
    getrandom::fill(&mut claim)?;
    ensure!(claim != [0; 32], "nonzero creation claim");
    let certificate = cert.encode()?;
    let authority_raw = authority.encode()?;
    let mut repository = db::RuntimeRepository::create(
        StateDirectoryLock::acquire(&state)?,
        db::CreationClaim {
            account: *root.account_id().as_bytes(),
            claim,
            signing_secret: Zeroizing::new(signing.secret_bytes()),
            encryption_secret: Zeroizing::new(encryption.secret_bytes()),
            contexts,
        },
        db::RuntimeEnrollment {
            device: *signing.device_id().as_bytes(),
            certificate: certificate.clone(),
            authority: authority_raw.clone(),
            authority_revision: authority.revision(),
        },
        db::Config::default(),
    )?;
    ensure!(
        repository.profile()?.contexts == contexts,
        "committed immutable slots"
    );
    drop(repository);
    let hex = |id: &[u8; 32]| id.iter().map(|n| format!("{n:02x}")).collect::<String>();
    let info = Ordered2BootstrapPublic {
        account_id: root.account_id(),
        device_id: signing.device_id(),
        claim: hex(&claim),
        contexts: contexts.each_ref().map(hex),
        workspace_dir: workspace.clone(),
        account_root_dir: workspace.join("account-root"),
        state_dir: workspace.join("device"),
        credential_file: workspace.join("public/credential.bin"),
        certificate_file: workspace.join("public/device-certificate.cert"),
        authority_file: workspace.join("public/authority.snapshot"),
        device_list_file: workspace.join("public/device-list.snapshot"),
        receipt_file: workspace.join("bootstrap-ready.json"),
        root_key_protection: root.key_protection().as_str().into(),
    };
    write_new_file(&public.join("credential.bin"), credential.raw())?;
    write_new_file(&public.join("device-certificate.cert"), &certificate)?;
    write_new_file(&public.join("authority.snapshot"), &authority_raw)?;
    write_new_file(&public.join("device-list.snapshot"), &devices.encode()?)?;
    verify(&state, &info, &certificate, &authority_raw)?;
    let output = Ordered2BootstrapOutput::new(info, phrase.expose_secret().to_owned())?;
    let mut encoded = output.encode()?;
    encoded.zeroize(); // Validate before ready commit; never persist this secret result.
    write_new_file(
        &workspace.join("bootstrap-candidate.json"),
        &receipt(output.public(), "candidate-validated-before-commit")?,
    )?;
    drop(root);
    drop(signing);
    drop(encryption);
    drop(phrase);
    verify(
        &output.public().state_dir,
        output.public(),
        &certificate,
        &authority_raw,
    )?;
    ensure!(
        AccountRootState::load(&output.public().account_root_dir)?.account_id()
            == output.public().account_id,
        "Root reopened at final path"
    );
    hook(Point::BeforeCommit, &workspace)?;
    // This is the final fallible commit: a complete, synced public receipt
    // appears without clobber only after the final-path repository/Root reopen.
    write_new_file(
        &output.public().receipt_file,
        &receipt(output.public(), "ready-runtime6")?,
    )?;
    Ok(output)
}

#[cfg(test)]
#[path = "ordered2_tests.rs"]
mod tests;
