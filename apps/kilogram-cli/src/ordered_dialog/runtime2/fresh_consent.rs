//! Offline Fresh ceremony. Each call has exactly one local Root path.
use anyhow::{Context, Result, ensure};
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, AccountRootState, ConversationScopeId, DeviceId,
};
use kilogram_protocol::ordered_dialog_v2::{self as wire, Id, Value, a, b, u};
use std::path::Path;
#[cfg(test)]
#[path = "fresh_consent_tests.rs"]
mod tests;

pub const MAX_PROPOSAL: usize = 96 * 1024;
const TAG: &[u8] = b"kg:od2:fresh-consent-proposal/v1";
pub struct Inputs<'a> {
    pub credentials: [&'a [u8]; 2],
    /// Independently retained public authority floors, in either account order.
    pub minimum_authorities: [&'a [u8]; 2],
    pub context: Id,
    pub account: AccountId,
    pub peer: AccountId,
    pub device: DeviceId,
    pub peer_device: DeviceId,
}
fn peers(input: &Inputs<'_>, role: usize) -> Result<[wire::RootVerifiedCredential; 2]> {
    let [a, b] = input
        .credentials
        .map(|raw| wire::ParsedCredentialBundle::parse(raw)?.verify_root());
    let mut peers = [a?, b?];
    peers.sort_by_key(|p| *p.certificate().device_id().as_bytes());
    ensure!(
        peers[0].certificate().device_id() != peers[1].certificate().device_id()
            && peers[0].certificate().account_id() != peers[1].certificate().account_id(),
        "distinct Fresh peers"
    );
    ensure!(
        peers[role].certificate().account_id() == input.account
            && peers[1 - role].certificate().account_id() == input.peer
            && peers[role].certificate().device_id() == input.device
            && peers[1 - role].certificate().device_id() == input.peer_device,
        "Fresh expected account/device/role mismatch: canonical role0 must initiate, role1 must cosign"
    );
    let [floor0, floor1] = input.minimum_authorities.map(|raw| -> Result<_> {
        ensure!(raw.len() <= wire::MAX_AUTH, "authority byte cap");
        let floor = AccountAuthoritySnapshot::decode_and_verify(raw)?;
        ensure!(floor.encode()? == raw, "exact authority encoding");
        Ok(floor)
    });
    let floors = [floor0?, floor1?];
    ensure!(
        floors[0].account_id() != floors[1].account_id(),
        "distinct authority floor accounts"
    );
    for peer in &peers {
        let floor = floors
            .iter()
            .find(|f| f.account_id() == peer.certificate().account_id())
            .context("missing peer authority floor")?;
        let actual = peer.authority();
        ensure!(
            actual.revision() >= floor.revision()
                && (actual.revision() != floor.revision() || actual.encode()? == floor.encode()?),
            "Fresh authority rollback/conflict"
        );
    }
    Ok(peers)
}
fn local(
    root: &AccountRootState,
    input: &Inputs<'_>,
    peers: &[wire::RootVerifiedCredential; 2],
    role: usize,
) -> Result<()> {
    ensure!(
        root.account_id() == input.account
            && root.authority_snapshot()?.encode()? == peers[role].authority().encode()?,
        "Fresh local Root/current authority mismatch"
    );
    Ok(())
}
fn body(c: Id, peers: &[wire::RootVerifiedCredential; 2], not_before: u64, expires: u64) -> Value {
    a([
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
    ])
}
fn interval(
    value: &Value,
    input: &Inputs<'_>,
    peers: &[wire::RootVerifiedCredential; 2],
    now: u64,
) -> Result<()> {
    let f = value.fields(10)?;
    let nb = f[8].uint()?;
    let end = f[9].uint()?;
    ensure!(
        value == &body(input.context, peers, nb, end)
            && end > nb
            && end - nb <= 30 * 86400
            && now.saturating_add(120) >= nb
            && now < end,
        "Fresh body/context/suite/origin/validity mismatch"
    );
    Ok(())
}
fn witness(credentials: [&[u8]; 2], membership: &[u8], body: &Value, signature: &Value) -> Value {
    a([
        b(TAG),
        a(credentials.map(|raw| b(wire::hash(b"", raw)))),
        b(wire::hash(b"", membership)),
        body.clone(),
        signature.clone(),
    ])
}
pub fn propose(
    input: Inputs<'_>,
    root_dir: &Path,
    not_before: u64,
    valid_for: u64,
    now: u64,
) -> Result<Vec<u8>> {
    let peers = peers(&input, 0)?;
    ensure!((1..=30 * 86400).contains(&valid_for), "Fresh TTL1..2592000");
    let expires = not_before
        .checked_add(valid_for)
        .context("Fresh interval overflow")?;
    let body = body(input.context, &peers, not_before, expires);
    interval(&body, &input, &peers, now)?;
    let root = AccountRootState::load(root_dir)?;
    local(&root, &input, &peers, 0)?;
    // This is the first authority mutation. Invalid pins, roles and times have
    // already refused. Existing membership is verified, never replaced/extended.
    let membership = match root.create_conversation_membership(
        ConversationScopeId::from_bytes(input.context),
        &[input.peer],
    ) {
        Ok(m) => m,
        Err(kilogram_identity::IdentityError::ConversationMembershipAlreadyExists(_)) => {
            root.load_root_conversation_membership(ConversationScopeId::from_bytes(input.context))?
        }
        Err(e) => return Err(e.into()),
    }
    .encode()?;
    wire::verify_membership(input.context, &peers, &membership)?;
    let signature = b(root.sign_ordered_context_consent2(&wire::encode(&body, 4096)?));
    // A tagged bounded administrative witness seals the exact public bytes;
    // the ordinary consent signature/body and final artifact remain unchanged.
    let seal = root.sign_ordered_context_consent2(&wire::encode(
        &witness(
            peers.each_ref().map(|p| p.raw()),
            &membership,
            &body,
            &signature,
        ),
        4096,
    )?);
    Ok(wire::encode(
        &a([
            b(TAG),
            b(peers[0].raw()),
            b(peers[1].raw()),
            b(membership),
            body,
            signature,
            b(seal),
        ]),
        MAX_PROPOSAL,
    )?)
}
pub fn cosign(input: Inputs<'_>, proposal: &[u8], root_dir: &Path, now: u64) -> Result<Vec<u8>> {
    let peers = peers(&input, 1)?;
    let value = wire::decode(proposal, MAX_PROPOSAL)?;
    let f = value.fields(7)?;
    ensure!(
        f[0].bytes()? == TAG && f[1].bytes()? == peers[0].raw() && f[2].bytes()? == peers[1].raw(),
        "Fresh proposal tag/credential pair mismatch"
    );
    interval(&f[4], &input, &peers, now)?;
    wire::verify_membership(input.context, &peers, f[3].bytes()?)?;
    let unsigned = wire::encode(&f[4], 4096)?;
    peers[0]
        .certificate()
        .account_id()
        .verify_ordered_context_consent2(&unsigned, f[5].bytes()?)?;
    peers[0]
        .certificate()
        .account_id()
        .verify_ordered_context_consent2(
            &wire::encode(
                &witness(
                    peers.each_ref().map(|p| p.raw()),
                    f[3].bytes()?,
                    &f[4],
                    &f[5],
                ),
                4096,
            )?,
            f[6].bytes()?,
        )?;
    let root = AccountRootState::load(root_dir)?;
    local(&root, &input, &peers, 1)?;
    let consent = wire::encode(
        &a([
            f[4].clone(),
            f[5].clone(),
            b(root.sign_ordered_context_consent2(&unsigned)),
        ]),
        4096,
    )?;
    wire::ParsedConsent2::parse(&consent)?.verify(&peers)?;
    Ok(wire::encode(
        &a([
            u(2),
            b(input.context),
            b(peers[0].raw()),
            b(peers[1].raw()),
            f[3].clone(),
            b(consent),
        ]),
        MAX_PROPOSAL,
    )?)
}
