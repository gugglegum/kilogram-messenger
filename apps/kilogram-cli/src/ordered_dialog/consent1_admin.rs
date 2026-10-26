//! Offline, bilateral renewal of an existing wire1 scope. No messenger vault IO.
use super::handshake::{Credentials, Spec};
use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountAuthoritySnapshot, AccountId, AccountRootState};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Id, Value, a, b, opt, u};
use std::path::Path;

pub const INVITATION_CAP: usize = 96 * 1024;

/// Public inputs only. Each command supplies its own Root separately, never its peer's.
pub struct RenewalInputs<'a> {
    pub original: &'a [u8],
    pub authorities: [&'a [u8]; 2],
    pub minimum_authorities: [&'a [u8]; 2],
    pub context: Id,
    pub account: AccountId,
    pub peer: AccountId,
}

fn authority(raw: &[u8]) -> Result<AccountAuthoritySnapshot> {
    ensure!(raw.len() <= wire::MAX_AUTH, "authority byte cap");
    let value = AccountAuthoritySnapshot::decode_and_verify(raw)?;
    ensure!(value.encode()? == raw, "authority exact encoding");
    Ok(value)
}

fn original(raw: &[u8]) -> Result<Spec> {
    let value = wire::decode(raw, INVITATION_CAP)?;
    let f = value.fields(8)?;
    ensure!(f[0].uint()? == 1, "renewal invitation version");
    let sig_b = f[7]
        .option()?
        .context("completed original invitation required")?;
    ensure!(
        f[6].bytes()?.len() == 64 && sig_b.bytes()?.len() == 64,
        "original consent signature size"
    );
    let scope = Spec {
        c: f[1].fixed()?,
        // These are historical scope witnesses. Current authority is checked below.
        peers: [
            Credentials::decode(f[2].bytes()?)?,
            Credentials::decode(f[3].bytes()?)?,
        ],
        membership: f[4].bytes()?.to_vec(),
        consent: wire::encode(&a([f[5].clone(), f[6].clone(), sig_b.clone()]), 4096)?,
    };
    scope.validate()?;
    Ok(scope)
}

fn current(inputs: &RenewalInputs<'_>, role: usize, root: &AccountRootState) -> Result<Spec> {
    let mut scope = original(inputs.original)?;
    ensure!(
        scope.c == inputs.context
            && scope.peers[role].cert.account_id() == inputs.account
            && scope.peers[1 - role].cert.account_id() == inputs.peer
            && root.account_id() == inputs.account,
        "renewal original context/account/role/Root mismatch"
    );
    for r in 0..2 {
        let auth = authority(inputs.authorities[r])?;
        let floor = authority(inputs.minimum_authorities[r])?;
        ensure!(
            auth.account_id() == scope.peers[r].cert.account_id()
                && floor.account_id() == auth.account_id()
                && auth.revision() >= floor.revision(),
            "renewal authority account/rollback"
        );
        ensure!(
            auth.revision() != floor.revision() || auth.encode()? == floor.encode()?,
            "renewal authority revision conflict"
        );
        if r == role {
            ensure!(
                root.authority_snapshot()?.encode()? == auth.encode()?,
                "renewal own authority differs from current Root"
            );
        }
        scope.peers[r] = Credentials::new(scope.peers[r].cert.clone(), auth)?;
    }
    scope.validate()?;
    Ok(scope)
}

fn body(scope: &Spec, not_before: u64, expires: u64) -> Result<Value> {
    let refs = scope.cert_refs()?;
    Ok(a([
        u(1),
        b(scope.c),
        b(refs[0]),
        b(refs[1]),
        u(1),
        u(1),
        u(not_before),
        u(expires),
    ]))
}

fn interval(value: &Value, scope: &Spec, now: u64) -> Result<()> {
    let f = value.fields(8)?;
    let not_before = f[6].uint()?;
    let expires = f[7].uint()?;
    ensure!(
        value == &body(scope, not_before, expires)?
            && expires > not_before
            && expires - not_before <= 30 * 86400
            && now.saturating_add(120) >= not_before
            && now < expires,
        "renewal scope/suite/interval"
    );
    Ok(())
}

fn invitation(
    scope: &Spec,
    body: Value,
    sig_a: Vec<u8>,
    sig_b: Option<Vec<u8>>,
) -> Result<Vec<u8>> {
    Ok(wire::encode(
        &a([
            u(1),
            b(scope.c),
            b(&scope.peers[0].raw),
            b(&scope.peers[1].raw),
            b(&scope.membership),
            body,
            b(sig_a),
            opt(sig_b.map(b)),
        ]),
        INVITATION_CAP,
    )?)
}

pub fn propose(
    inputs: RenewalInputs<'_>,
    account_dir: &Path,
    not_before: u64,
    valid_for: u64,
    now: u64,
) -> Result<Vec<u8>> {
    ensure!(
        (1..=30 * 86400).contains(&valid_for),
        "renewal TTL1..2592000"
    );
    let root = AccountRootState::load(account_dir)?;
    let scope = current(&inputs, 0, &root)?;
    let expires = not_before
        .checked_add(valid_for)
        .context("renewal interval overflow")?;
    let body = body(&scope, not_before, expires)?;
    interval(&body, &scope, now)?;
    let signature = root.sign_ordered_context_consent(&wire::encode(&body, 4096)?);
    invitation(&scope, body, signature, None)
}

pub struct CompletedRenewal {
    pub invitation: Vec<u8>,
    pub consent: Vec<u8>,
}

pub fn cosign(
    inputs: RenewalInputs<'_>,
    proposal: &[u8],
    account_dir: &Path,
    now: u64,
) -> Result<CompletedRenewal> {
    let root = AccountRootState::load(account_dir)?;
    let mut scope = current(&inputs, 1, &root)?;
    let value = wire::decode(proposal, INVITATION_CAP)?;
    let f = value.fields(8)?;
    ensure!(
        f[0].uint()? == 1
            && f[1].fixed::<32>()? == scope.c
            && f[2].bytes()? == scope.peers[0].raw
            && f[3].bytes()? == scope.peers[1].raw
            && f[4].bytes()? == scope.membership
            && f[6].bytes()?.len() == 64
            && f[7].option()?.is_none(),
        "renewal proposal differs from exact current scope/credentials"
    );
    interval(&f[5], &scope, now)?;
    let unsigned = wire::encode(&f[5], 4096)?;
    scope.peers[0]
        .cert
        .account_id()
        .verify_ordered_context_consent(&unsigned, f[6].bytes()?)?;
    let sig_b = root.sign_ordered_context_consent(&unsigned);
    let consent = wire::encode(&a([f[5].clone(), f[6].clone(), b(&sig_b)]), 4096)?;
    scope.consent = consent.clone();
    scope.validate()?;
    scope.valid_now(&scope.consent, now)?;
    let completed = invitation(&scope, f[5].clone(), f[6].bytes()?.to_vec(), Some(sig_b))?;
    // A complete output must parse exactly as the frozen wire1 administrative shape.
    let verified = original(&completed)?;
    ensure!(
        verified.raw()? == scope.raw()?,
        "completed renewal correspondence"
    );
    Ok(CompletedRenewal {
        invitation: completed,
        consent,
    })
}
