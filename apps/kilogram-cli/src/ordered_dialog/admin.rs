//! Explicit owner enrollment/consent. Runtime paths never load an Account Root.
use super::{
    handshake::{Credentials, Spec},
    state::{Head, head_path, hex},
};
use anyhow::{Context, Result, ensure};
use kilogram_crypto::DeviceEncryptionIdentity;
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, AccountRootState, ConversationScopeId, DeviceCapability,
    DeviceCertificate, DeviceIdentity, DeviceState,
};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Id, a, b, opt, u};
use kilogram_state::{
    EncryptedStateVault, OrderedMutation, OrderedVault, STATE_VAULT_FILE, StateDirectoryLock,
};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use zeroize::Zeroizing;

pub const CLAIM: &str = "runtime/od1/profile-claim";
pub fn required(store: &OrderedVault, path: &str) -> Result<Vec<u8>> {
    store
        .get(path)?
        .with_context(|| format!("missing OD1 record {path}"))
}
pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes)?;
    Ok(bytes)
}
pub fn commit(store: &mut OrderedVault, mutations: Vec<OrderedMutation>) -> Result<()> {
    let receipt = store.commit(store.generation(), mutations)?;
    if !receipt.visible {
        store
            .recover()
            .context("OD1 committed primary needs shadow recovery")?;
    }
    Ok(())
}
pub fn claim(store: &OrderedVault) -> Result<(Id, AccountId, Id)> {
    let v = wire::decode(&required(store, CLAIM)?, 1024)?;
    let f = v.fields(5)?;
    ensure!(
        f[0].uint()? == 1 && f[1].uint()? == 4,
        "OD1 profile claim version"
    );
    Ok((
        f[2].fixed()?,
        AccountId::from_bytes(f[3].fixed()?),
        f[4].fixed()?,
    ))
}
pub fn device(store: &OrderedVault) -> Result<DeviceState> {
    let sign = Zeroizing::new(required(store, "device-secret.key")?);
    let encrypt = Zeroizing::new(required(store, "device-encryption-secret.key")?);
    let state = DeviceState::from_secret_material(
        store.root(),
        sign.as_slice()
            .try_into()
            .context("OD1 signing secret length")?,
        encrypt
            .as_slice()
            .try_into()
            .context("OD1 encryption secret length")?,
    );
    ensure!(
        state.identity().device_id().as_bytes() == &claim(store)?.2,
        "OD1 claim Device mismatch"
    );
    Ok(state)
}
pub fn credentials(store: &OrderedVault) -> Result<Credentials> {
    Credentials::new(
        DeviceCertificate::decode_and_verify(&required(store, "device-certificate.cert")?)?,
        AccountAuthoritySnapshot::decode_and_verify(&required(
            store,
            "account-authority.snapshot",
        )?)?,
    )
}
pub fn enroll(
    lease: StateDirectoryLock,
    account_dir: &Path,
    certificate_file: Option<&Path>,
) -> Result<()> {
    let account = AccountRootState::load(account_dir)?;
    let root = lease.root().to_path_buf();
    let existing = if root.join(STATE_VAULT_FILE).exists() {
        EncryptedStateVault::open_existing(&root)?.is_ordered_profile()?
    } else {
        false
    };
    let mut store = if existing {
        let store = OrderedVault::open(lease)?;
        ensure!(
            claim(&store)?.1 == account.account_id(),
            "CreationConflict: different owner"
        );
        ensure!(
            store.get("device-certificate.cert")?.is_none(),
            "CreationConflict: profile is already enrolled"
        );
        store
    } else {
        let signing = DeviceIdentity::generate()?;
        let encryption = DeviceEncryptionIdentity::generate()?;
        let profile_claim = wire::encode(
            &a([
                u(1),
                u(4),
                b(random::<32>()?),
                b(account.account_id().as_bytes()),
                b(signing.device_id().as_bytes()),
            ]),
            1024,
        )?;
        OrderedVault::create(
            lease,
            vec![
                OrderedMutation::put(CLAIM, profile_claim),
                OrderedMutation::put("device-secret.key", signing.secret_bytes().to_vec()),
                OrderedMutation::put(
                    "device-encryption-secret.key",
                    encryption.secret_bytes().to_vec(),
                ),
            ],
        )?
    };
    let device = device(&store)?;
    let cert = account.issue_device_certificate(
        device.identity().device_id(),
        device.encryption().public_key(),
        &DeviceCapability::MESSAGING,
    )?;
    let authority = account.authority_snapshot()?;
    let credential = Credentials::new(cert, authority)?;
    commit(
        &mut store,
        vec![
            OrderedMutation::put("device-certificate.cert", credential.cert.encode()?),
            OrderedMutation::put("account-authority.snapshot", credential.authority.encode()?),
            OrderedMutation::put(auth_path(credential.reference()), credential.raw.clone()),
        ],
    )?;
    if let Some(path) = certificate_file {
        write_public(path, &credential.cert.encode()?, store.root())?;
    }
    println!("account_id={}", credential.cert.account_id());
    println!("device_id={}", credential.cert.device_id());
    println!("profile_mode=ordered-dialog-v1");
    println!("status=device-enrolled");
    Ok(())
}
pub fn auth_path(id: Id) -> String {
    format!("runtime/od1/auth/{}", hex(&id))
}
pub fn spec_path(c: Id) -> String {
    format!("runtime/od1/contexts/{}/spec", hex(&c))
}
pub fn consent_path(c: Id) -> String {
    format!("runtime/od1/contexts/{}/consent-current", hex(&c))
}
pub fn specs(store: &OrderedVault) -> Result<Vec<Spec>> {
    store
        .paths("runtime/od1/contexts/")
        .into_iter()
        .filter(|p| p.ends_with("/spec"))
        .map(|p| Spec::parse(&required(store, &p)?))
        .collect()
}
pub fn read_public(path: &Path, cap: usize) -> Result<Vec<u8>> {
    ensure!(
        fs::metadata(path)?.len() <= cap as u64,
        "public artifact byte cap"
    );
    let mut raw = Vec::new();
    fs::File::open(path)?
        .take(cap as u64 + 1)
        .read_to_end(&mut raw)?;
    ensure!(raw.len() <= cap, "public artifact grew beyond byte cap");
    Ok(raw)
}
pub fn write_public(path: &Path, raw: &[u8], root: &Path) -> Result<()> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = absolute.parent().context("artifact parent")?;
    let parent = fs::canonicalize(parent)?;
    ensure!(
        !parent.starts_with(root),
        "public artifact must be outside protected state"
    );
    let output = parent.join(absolute.file_name().context("artifact filename")?);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(raw)?;
    file.sync_all()?;
    Ok(())
}

/// The invitation is administrative material, never an alternate network handshake.
pub fn create_context(
    store: &OrderedVault,
    account_dir: &Path,
    peer_raw: &[u8],
    expected_peer: AccountId,
    now: u64,
) -> Result<(Id, Vec<u8>)> {
    ensure!(
        specs(store)?.len() < 2,
        "UpgradeRequired: two contexts/profile"
    );
    let own = credentials(store)?;
    let peer = Credentials::parse(peer_raw)?;
    ensure!(
        peer.cert.account_id() == expected_peer
            && own.cert.device_id().as_bytes() < peer.cert.device_id().as_bytes(),
        "context creator must be elected A with the pinned peer"
    );
    ensure!(
        own.cert.account_id() != peer.cert.account_id(),
        "same-account multi-device unsupported"
    );
    let account = AccountRootState::load(account_dir)?;
    ensure!(
        account.account_id() == own.cert.account_id(),
        "consent owner mismatch"
    );
    let c = random::<32>()?;
    let membership = account
        .create_conversation_membership(
            ConversationScopeId::from_bytes(c),
            &[peer.cert.account_id()],
        )?
        .encode()?;
    let body = a([
        u(1),
        b(c),
        b(own.cert_ref()?),
        b(peer.cert_ref()?),
        u(1),
        u(1),
        u(now),
        u(now.checked_add(30 * 86400).context("consent time")?),
    ]);
    let signature = account.sign_ordered_context_consent(&wire::encode(&body, 4096)?);
    let invitation = wire::encode(
        &a([
            u(1),
            b(c),
            b(&own.raw),
            b(&peer.raw),
            b(membership),
            body,
            b(signature),
            opt(None),
        ]),
        96 * 1024,
    )?;
    Ok((c, invitation))
}

pub fn import_context(
    store: &mut OrderedVault,
    account_dir: Option<&Path>,
    raw: &[u8],
    expected_peer: AccountId,
    now: u64,
) -> Result<(Id, Vec<u8>)> {
    let v = wire::decode(raw, 96 * 1024)?;
    let f = v.fields(8)?;
    ensure!(f[0].uint()? == 1, "OD1 invitation version");
    let c = f[1].fixed()?;
    let peers = [
        Credentials::parse(f[2].bytes()?)?,
        Credentials::parse(f[3].bytes()?)?,
    ];
    let own = credentials(store)?;
    let role = if own.cert_ref()? == peers[0].cert_ref()? {
        0
    } else {
        ensure!(
            own.cert_ref()? == peers[1].cert_ref()?,
            "invitation is for another device"
        );
        1
    };
    ensure!(
        peers[1 - role].cert.account_id() == expected_peer,
        "unapproved peer account"
    );
    let mut signature_b = f[7]
        .option()?
        .map(|v| v.bytes().map(ToOwned::to_owned))
        .transpose()?;
    let unsigned = wire::encode(&f[5], 4096)?;
    peers[0]
        .cert
        .account_id()
        .verify_ordered_context_consent(&unsigned, f[6].bytes()?)?;
    if signature_b.is_none() {
        ensure!(role == 1, "A cannot countersign for B");
        let account =
            AccountRootState::load(account_dir.context("B owner account directory required")?)?;
        ensure!(
            account.account_id() == own.cert.account_id(),
            "B consent owner mismatch"
        );
        signature_b = Some(account.sign_ordered_context_consent(&wire::encode(&f[5], 4096)?));
    }
    let sig_b = signature_b.context("B signature")?;
    let consent = wire::encode(&a([f[5].clone(), f[6].clone(), b(&sig_b)]), 4096)?;
    let spec = Spec {
        c,
        peers,
        membership: f[4].bytes()?.to_vec(),
        consent,
    };
    spec.validate()?;
    spec.valid_now(&spec.consent, now)?;
    let mut mutations = Vec::new();
    if let Some(existing) = store.get(&spec_path(c))? {
        let original = Spec::parse(&existing)?;
        ensure!(
            original.cert_refs()? == spec.cert_refs()? && original.membership == spec.membership,
            "conflicting context cannot replace its epoch"
        );
        let mut head = Head::parse(
            &required(store, &head_path(c))?,
            c,
            own.cert_ref()?,
            role as u64,
        )?;
        head.bump(false)?;
        mutations.push(OrderedMutation::put(head_path(c), head.encode()?));
    } else {
        ensure!(
            specs(store)?.len() < 2,
            "UpgradeRequired: two contexts/profile"
        );
        mutations.push(OrderedMutation::put(spec_path(c), spec.raw()?));
        mutations.push(OrderedMutation::put(
            head_path(c),
            Head::absent(c, own.cert_ref()?, role as u64).encode()?,
        ));
    }
    mutations.push(OrderedMutation::put(consent_path(c), spec.consent.clone()));
    // Initial credentials already live in the immutable Spec/InitialPair.
    // The permanent catalog is populated when an encrypted frame captures a
    // reference, so consent renewals cannot consume lifetime frame credit.
    let peer = &spec.peers[1 - role];
    let peer_path = format!("peer-authority/{}.snapshot", peer.cert.account_id());
    if let Some(current) = store.get(&peer_path)? {
        let current = AccountAuthoritySnapshot::decode_and_verify(&current)?;
        ensure!(
            peer.authority.revision() >= current.revision(),
            "authority rollback"
        );
    }
    mutations.push(OrderedMutation::put(peer_path, peer.authority.encode()?));
    commit(store, mutations)?;
    let complete = wire::encode(
        &a([
            u(1),
            b(c),
            f[2].clone(),
            f[3].clone(),
            f[4].clone(),
            f[5].clone(),
            f[6].clone(),
            opt(Some(b(sig_b))),
        ]),
        96 * 1024,
    )?;
    Ok((c, complete))
}
