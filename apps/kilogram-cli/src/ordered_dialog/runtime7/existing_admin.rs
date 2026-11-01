//! Offline existing-origin consent. Each step loads only its own account Root.
//! No state-directory lock, messenger repository or Native operation is used here.
use anyhow::{Context, Result, ensure};
use kilogram_identity::{AccountAuthoritySnapshot, AccountId, AccountRootState};
use kilogram_protocol::ordered_dialog_v2::{self as wire, Id, Value, a, b, u};
use std::path::Path;

pub struct ExistingConsentInputs<'a> {
    pub origin: &'a [u8],
    pub authorities: [&'a [u8]; 2],
    pub minimum_authorities: [&'a [u8]; 2],
    pub context: Id,
    pub account: AccountId,
    pub peer: AccountId,
}

fn authority(raw: &[u8]) -> Result<AccountAuthoritySnapshot> {
    ensure!(raw.len() <= wire::MAX_AUTH, "authority byte cap");
    let authority = AccountAuthoritySnapshot::decode_and_verify(raw)?;
    ensure!(authority.encode()? == raw, "authority exact encoding");
    Ok(authority)
}

fn current(
    inputs: &ExistingConsentInputs<'_>,
    role: usize,
    root: &AccountRootState,
) -> Result<(
    wire::VerifiedExistingScope1,
    [wire::RootVerifiedCredential; 2],
)> {
    let scope = wire::VerifiedExistingScope1::verify_export(inputs.origin)?;
    ensure!(
        scope.c() == inputs.context
            && scope.peers()[role].certificate().account_id() == inputs.account
            && scope.peers()[1 - role].certificate().account_id() == inputs.peer
            && root.account_id() == inputs.account,
        "existing consent context/account/role/Root mismatch"
    );
    let mut peers = scope.peers().clone();
    for (r, peer) in peers.iter_mut().enumerate() {
        let authority = authority(inputs.authorities[r])?;
        let floor = self::authority(inputs.minimum_authorities[r])?;
        ensure!(
            authority.account_id() == peer.certificate().account_id()
                && floor.account_id() == authority.account_id()
                && authority.revision() >= floor.revision(),
            "existing consent authority account/rollback"
        );
        ensure!(
            authority.revision() != floor.revision() || authority.encode()? == floor.encode()?,
            "existing consent authority revision conflict"
        );
        if r == role {
            ensure!(
                root.authority_snapshot()?.encode()? == authority.encode()?,
                "existing consent own authority differs from current Root"
            );
        }
        *peer = wire::RootVerifiedCredential::from_parts(peer.certificate().clone(), authority)?;
    }
    wire::verify_membership(scope.c(), &peers, scope.membership())?;
    Ok((scope, peers))
}

fn body(scope: &wire::VerifiedExistingScope1, not_before: u64, expires: u64) -> Value {
    let certs = scope.certs();
    a([
        u(2),
        b(scope.c()),
        b(certs[0]),
        b(certs[1]),
        u(1),
        u(2),
        scope.origin().value(),
        u(1),
        u(not_before),
        u(expires),
    ])
}

fn interval(value: &Value, scope: &wire::VerifiedExistingScope1, now: u64) -> Result<()> {
    let f = value.fields(10)?;
    let not_before = f[8].uint()?;
    let expires = f[9].uint()?;
    ensure!(
        value == &body(scope, not_before, expires)
            && expires > not_before
            && expires - not_before <= 30 * 86400
            && now.saturating_add(120) >= not_before
            && now < expires,
        "existing consent scope/suite/interval"
    );
    Ok(())
}

pub fn propose(
    inputs: ExistingConsentInputs<'_>,
    account_dir: &Path,
    not_before: u64,
    valid_for: u64,
    now: u64,
) -> Result<Vec<u8>> {
    ensure!(
        (1..=30 * 86400).contains(&valid_for),
        "consent TTL1..2592000"
    );
    let root = AccountRootState::load(account_dir)?;
    let (scope, _) = current(&inputs, 0, &root)?;
    let expires = not_before
        .checked_add(valid_for)
        .context("consent interval overflow")?;
    let body = body(&scope, not_before, expires);
    interval(&body, &scope, now)?;
    let sig = root.sign_ordered_context_consent2(&wire::encode(&body, 4096)?);
    // Public witness hashes bind the proposal to the exact export and both
    // current authority snapshots without putting their unbounded bytes in it.
    Ok(wire::encode(
        &a([
            u(2),
            b(wire::hash(b"", inputs.origin)),
            a(inputs.authorities.map(|raw| b(wire::hash(b"", raw)))),
            body,
            b(sig),
        ]),
        4096,
    )?)
}

pub fn cosign(
    inputs: ExistingConsentInputs<'_>,
    proposal: &[u8],
    account_dir: &Path,
    now: u64,
) -> Result<Vec<u8>> {
    let root = AccountRootState::load(account_dir)?;
    let (scope, peers) = current(&inputs, 1, &root)?;
    let value = wire::decode(proposal, 4096)?;
    let f = value.fields(5)?;
    ensure!(
        f[0].uint()? == 2
            && f[1].fixed::<32>()? == wire::hash(b"", inputs.origin)
            && f[2] == a(inputs.authorities.map(|raw| b(wire::hash(b"", raw))))
            && f[4].bytes()?.len() == 64,
        "existing consent proposal differs from exact current scope/authorities"
    );
    interval(&f[3], &scope, now)?;
    let unsigned = wire::encode(&f[3], 4096)?;
    peers[0]
        .certificate()
        .account_id()
        .verify_ordered_context_consent2(&unsigned, f[4].bytes()?)?;
    let sig_b = root.sign_ordered_context_consent2(&unsigned);
    let consent = wire::encode(&a([f[3].clone(), f[4].clone(), b(sig_b)]), 4096)?;
    scope.verify_consent(&wire::ParsedConsent2::parse(&consent)?, &peers)?;
    Ok(consent)
}
