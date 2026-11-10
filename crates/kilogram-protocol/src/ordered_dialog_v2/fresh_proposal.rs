//! Read-only public CS1 proposal validation shared by the signer and desktop.
//! The bytes/domain are the existing administrative witness, not a new wire.
use super::*;

pub const FRESH_PROPOSAL_TAG2: &[u8] = b"kg:od2:fresh-consent-proposal/v1";
pub const MAX_FRESH_PROPOSAL2: usize = 96 * 1024;

pub fn verify_fresh_proposal2(
    raw: &[u8],
    c: Id,
    peers: &[RootVerifiedCredential; 2],
    now: u64,
) -> Result<Value> {
    let value = decode(raw, MAX_FRESH_PROPOSAL2)?;
    let f = value.fields(7)?;
    require(
        f[0].bytes()? == FRESH_PROPOSAL_TAG2
            && f[1].bytes()? == peers[0].raw()
            && f[2].bytes()? == peers[1].raw(),
        "Fresh proposal tag/credential pair mismatch",
    )?;
    let body = f[4].fields(10)?;
    let nb = body[8].uint()?;
    let end = body[9].uint()?;
    let expected = a([
        u(2),
        b(c),
        b(peers[0].cert_ref()),
        b(peers[1].cert_ref()),
        u(1),
        u(2),
        Origin::Fresh.value(),
        u(1),
        u(nb),
        u(end),
    ]);
    require(
        f[4] == expected
            && end > nb
            && end - nb <= 30 * 86400
            && now.saturating_add(120) >= nb
            && now < end,
        "Fresh body/context/suite/origin/validity mismatch",
    )?;
    verify_membership(c, peers, f[3].bytes()?)?;
    let key = peers[0].certificate().account_id();
    key.verify_ordered_context_consent2(&encode(&f[4], 4096)?, f[5].bytes()?)?;
    let witness = a([
        b(FRESH_PROPOSAL_TAG2),
        a(peers.each_ref().map(|p| b(hash(b"", p.raw())))),
        b(hash(b"", f[3].bytes()?)),
        f[4].clone(),
        f[5].clone(),
    ]);
    key.verify_ordered_context_consent2(&encode(&witness, 4096)?, f[6].bytes()?)?;
    Ok(value)
}
