//! Offline Root ceremony. No runtime code calls AccountRootState::load.
use super::*;
use kilogram_identity::{
    AccountRootState, ConversationScopeId, DeviceCapability, DeviceEncryptionIdentity,
    DeviceIdentity,
};
use kilogram_state::StateDirectoryLock;
use std::path::Path;
use zeroize::Zeroizing;

pub fn enroll(
    lease: StateDirectoryLock,
    account_dir: &Path,
    contexts: [Id; 2],
    certificate_file: Option<&Path>,
) -> Result<()> {
    let root = lease.root().to_path_buf();
    hooks::validate(&root)?;
    ensure!(
        !account_dir
            .canonicalize()?
            .starts_with(root.canonicalize()?),
        "Root admin directory inside runtime state"
    );
    // Check before generating keys or advancing external Root authority state.
    ensure!(
        std::fs::read_dir(&root)?.all(|e| e.is_ok_and(|e| e.file_name() == ".kilogram-state.lock")),
        "CreationConflict/ExistingStateRequiresMigration"
    );
    let account = AccountRootState::load(account_dir)?;
    let signing = DeviceIdentity::generate()?;
    let encryption = DeviceEncryptionIdentity::generate()?;
    let cert = account.issue_device_certificate(
        signing.device_id(),
        encryption.public_key(),
        &DeviceCapability::MESSAGING,
    )?;
    let authority = account.authority_snapshot()?;
    let claim = shared_admin::random::<32>()?;
    let public = serde_json::json!({"profile_mode":"ordered-dialog-v2","account":hex(account.account_id().as_bytes()),"claim":hex(&claim),"device":hex(signing.device_id().as_bytes()),"contexts":contexts.map(|c|hex(&c)),"status":"enrollment-planned"});
    println!("{public}");
    use std::io::Write;
    std::io::stdout().flush()?;
    hooks::checkpoint("enrollment_before_root", None, public.clone())?;
    let mut hook = |point| -> db::Result<()> {
        let name = match point {
            db::RuntimeBootstrapPoint::Fenced => "enrollment_fenced",
            db::RuntimeBootstrapPoint::Staged => "enrollment_staged",
        };
        hooks::checkpoint(name, None, public.clone())
            .map_err(|_| db::Error::PolicyRejected("bootstrap controller"))
    };
    let mut repository = db::RuntimeRepository::create_with_hook(
        lease,
        db::CreationClaim {
            account: *account.account_id().as_bytes(),
            claim,
            signing_secret: Zeroizing::new(signing.secret_bytes()),
            encryption_secret: Zeroizing::new(encryption.secret_bytes()),
            contexts,
        },
        db::RuntimeEnrollment {
            device: *signing.device_id().as_bytes(),
            certificate: cert.encode()?,
            authority: authority.encode()?,
            authority_revision: authority.revision(),
        },
        db::Config::default(),
        &mut hook,
    )?;
    let material = repository.owner_material()?;
    ensure!(*material.signing == signing.secret_bytes(), "saved keys");
    hooks::checkpoint("enrollment_committed", None, public)?;
    if let Some(file) = certificate_file {
        let credential = wire::RootVerifiedCredential::from_parts(cert, authority)?;
        shared_admin::write_public(file, credential.raw(), &root)?;
    }
    println!(
        "{}",
        serde_json::json!({"profile_mode":"ordered-dialog-v2","account":hex(account.account_id().as_bytes()),"claim":hex(&claim),"device":hex(signing.device_id().as_bytes()),"contexts":contexts.map(|c|hex(&c)),"status":"device-enrolled"})
    );
    Ok(())
}
pub fn create_profile(profile: config::Profile2, path: &Path) -> Result<()> {
    profile.validate()?;
    let mut repository = db::RuntimeRepository::open(
        StateDirectoryLock::acquire(&profile.state_dir)?,
        profile.owner()?,
        profile.db_config(),
    )?;
    let p = repository.profile()?;
    ensure!(
        p.enrollment.device == parse_id(&profile.device)?
            && p.contexts
                == [
                    parse_id(&profile.contexts[0])?,
                    parse_id(&profile.contexts[1])?
                ],
        "profile pins differ from committed claim"
    );
    shared_admin::write_public(
        path,
        &serde_json::to_vec_pretty(&profile)?,
        &profile.state_dir,
    )
}
pub fn resume_enrollment(
    repository: &mut db::RuntimeRepository,
    expected_device: Id,
    certificate_file: &Path,
) -> Result<()> {
    let material = repository.owner_material()?;
    let p = material.profile;
    let device = DeviceIdentity::from_secret_bytes(*material.signing);
    let encryption = DeviceEncryptionIdentity::from_secret_bytes(*material.encryption);
    let cert = kilogram_identity::DeviceCertificate::decode_and_verify(&p.enrollment.certificate)?;
    ensure!(
        p.enrollment.device == expected_device
            && device.device_id().as_bytes() == &expected_device
            && cert.device_id() == device.device_id()
            && cert.encryption_public_key() == encryption.public_key(),
        "resume Device/certificate pins"
    );
    let authority =
        kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&p.enrollment.authority)?;
    let credential = wire::RootVerifiedCredential::from_parts(cert, authority)?;
    if certificate_file.exists() {
        ensure!(
            shared_admin::read_public(certificate_file, wire::MAX_AUTH)? == credential.raw(),
            "public credential conflict"
        );
    } else {
        shared_admin::write_public(
            certificate_file,
            credential.raw(),
            &repository.state_directory().canonicalize()?,
        )?;
    }
    println!(
        "{}",
        serde_json::json!({"status":"enrollment-resumed","account":hex(&p.account),"claim":hex(&p.claim),"device":hex(&expected_device),"contexts":p.contexts.map(|c|hex(&c))})
    );
    Ok(())
}
pub fn context_artifact(
    c: Id,
    credential_files: [&Path; 2],
    account_dirs: [&Path; 2],
    not_before: u64,
    expires: u64,
) -> Result<Vec<u8>> {
    ensure!(not_before < expires, "consent interval");
    let mut credentials = credential_files
        .iter()
        .map(|file| {
            wire::ParsedCredentialBundle::parse(&shared_admin::read_public(file, wire::MAX_AUTH)?)?
                .verify_root()
                .map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    credentials.sort_by_key(|c| *c.certificate().device_id().as_bytes());
    let peers: [wire::RootVerifiedCredential; 2] = credentials
        .try_into()
        .map_err(|_| anyhow::anyhow!("peer pair"))?;
    let roots = [
        AccountRootState::load(account_dirs[0])?,
        AccountRootState::load(account_dirs[1])?,
    ];
    let root_for = |role: usize| {
        roots
            .iter()
            .find(|r| r.account_id() == peers[role].certificate().account_id())
            .context("Root account does not match certificates")
    };
    let membership = match root_for(0)?.create_conversation_membership(
        ConversationScopeId::from_bytes(c),
        &[peers[1].certificate().account_id()],
    ) {
        Ok(snapshot) => snapshot,
        Err(kilogram_identity::IdentityError::ConversationMembershipAlreadyExists(_)) => {
            root_for(0)?.load_root_conversation_membership(ConversationScopeId::from_bytes(c))?
        }
        Err(error) => return Err(error.into()),
    }
    .encode()?;
    wire::verify_membership(c, &peers, &membership)?;
    let body = a([
        u(2),
        b(c),
        b(peers[0].cert_ref()),
        b(peers[1].cert_ref()),
        u(1),
        u(2),
        wire::Origin::Fresh.value(),
        u(1),
        u(not_before),
        u(expires),
    ]);
    let encoded = wire::encode(&body, 4096)?;
    let consent = wire::encode(
        &a([
            body,
            b(root_for(0)?.sign_ordered_context_consent2(&encoded)),
            b(root_for(1)?.sign_ordered_context_consent2(&encoded)),
        ]),
        4096,
    )?;
    wire::ParsedConsent2::parse(&consent)?.verify(&peers)?;
    Ok(wire::encode(
        &a([
            u(2),
            b(c),
            b(peers[0].raw()),
            b(peers[1].raw()),
            b(membership),
            b(consent),
        ]),
        96 * 1024,
    )?)
}
pub fn install(repository: &mut db::RuntimeRepository, raw: &[u8]) -> Result<()> {
    let v = wire::decode(raw, 96 * 1024)?;
    let f = v.fields(6)?;
    ensure!(f[0].uint()? == 2, "context artifact version");
    let c = f[1].fixed::<32>()?;
    let profile = repository.profile()?;
    let slot = profile
        .contexts
        .iter()
        .position(|id| *id == c)
        .context("context outside immutable slots")? as u8;
    let peers = [
        wire::ParsedCredentialBundle::parse(f[2].bytes()?)?.verify_root()?,
        wire::ParsedCredentialBundle::parse(f[3].bytes()?)?.verify_root()?,
    ];
    wire::verify_membership(c, &peers, f[4].bytes()?)?;
    let consent = wire::ParsedConsent2::parse(f[5].bytes()?)?;
    consent.verify(&peers)?;
    ensure!(
        consent.c() == c && consent.origin() == &wire::Origin::Fresh,
        "UnsupportedExistingRuntime"
    );
    let role = peers
        .iter()
        .position(|peer| peer.certificate().device_id().as_bytes() == &profile.enrollment.device)
        .context("Device outside context")? as u8;
    ensure!(
        peers[usize::from(role)]
            .certificate()
            .account_id()
            .as_bytes()
            == &profile.account
            && peers[usize::from(role)].certificate().encode()? == profile.enrollment.certificate,
        "own certificate pin"
    );
    let spec = db::RuntimeContextSpec {
        role,
        accounts: peers
            .clone()
            .map(|c| *c.certificate().account_id().as_bytes()),
        certs: peers.clone().map(|c| c.cert_ref()),
        credentials: peers.clone().map(|c| c.raw().to_vec()),
        membership: f[4].bytes()?.to_vec(),
        consent: f[5].bytes()?.to_vec(),
        own_locator: None,
        peer_locator: None,
        own_locator_revision: 0,
        peer_locator_revision: 0,
    };
    if repository.installed(slot)? {
        let old = repository.snapshot(slot)?;
        ensure!(
            old.spec.certs == spec.certs
                && old.spec.accounts == spec.accounts
                && old.spec.membership == spec.membership
                && old.spec.consent == spec.consent,
            "CreationConflict: context already installed"
        );
        return Ok(());
    }
    repository.commit(
        repository.generation(),
        shared_admin::random()?,
        db::RuntimeOperation::InstallContext {
            context: slot,
            spec,
            peer_revision: peers[usize::from(1 - role)].authority().revision(),
        },
        &mut || Ok(()),
    )?;
    Ok(())
}
