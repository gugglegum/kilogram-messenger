use super::*;
use kilogram_identity::{AccountAuthoritySnapshot, DeviceCertificate};

#[derive(Clone)]
pub(super) struct Current2 {
    pub peers: [wire::RootVerifiedCredential; 2],
    pub consent: wire::ParsedConsent2,
}
impl Current2 {
    pub fn check_time(&self, s: &db::RuntimeSnapshot, wall: u64) -> Result<()> {
        ensure!(wall >= s.profile.wall_high_water, "ClockBlocked");
        let (nb, expires) = self.consent.interval();
        ensure!(nb <= wall && wall < expires, "ConsentExpired/NotYetValid");
        Ok(())
    }
}
pub(super) fn current(s: &db::RuntimeSnapshot, wall: u64) -> Result<Current2> {
    ensure!(wall >= s.profile.wall_high_water, "ClockBlocked");
    let own = usize::from(s.head.role);
    let peer = 1 - own;
    let mut certificates = [Vec::new(), Vec::new()];
    for (i, raw) in s.spec.credentials.iter().enumerate() {
        let parsed = wire::ParsedCredentialBundle::parse(raw)?;
        ensure!(parsed.cert_ref() == s.spec.certs[i], "certificate pin");
        certificates[i] = parsed.certificate_raw().to_vec();
    }
    let pin = s.profile.peer.as_ref().context("peer pin")?;
    ensure!(
        pin.account == s.spec.accounts[peer] && pin.certificate == s.spec.certs[peer],
        "peer policy pin"
    );
    let snapshots = if own == 0 {
        [&s.profile.enrollment.authority, &pin.authority]
    } else {
        [&pin.authority, &s.profile.enrollment.authority]
    };
    let mut peers = Vec::with_capacity(2);
    for i in 0..2 {
        peers.push(wire::RootVerifiedCredential::from_parts(
            DeviceCertificate::decode_and_verify(&certificates[i])?,
            AccountAuthoritySnapshot::decode_and_verify(snapshots[i])?,
        )?);
    }
    let peers: [wire::RootVerifiedCredential; 2] = peers
        .try_into()
        .map_err(|_| anyhow::anyhow!("peer count"))?;
    for (i, peer) in peers.iter().enumerate() {
        ensure!(
            peer.certificate().account_id().as_bytes() == &s.spec.accounts[i],
            "signed account pin"
        );
    }
    wire::verify_membership(s.head.context, &peers, &s.spec.membership)?;
    let consent = wire::ParsedConsent2::parse(&s.spec.consent)?;
    ensure!(
        consent.c() == s.head.context && consent.origin() == &wire::Origin::Fresh,
        "UnsupportedExistingRuntime"
    );
    consent.verify(&peers)?;
    let (nb, expires) = consent.interval();
    ensure!(nb <= wall && wall < expires, "ConsentExpired/NotYetValid");
    Ok(Current2 { peers, consent })
}
pub(super) fn selected(s: &db::RuntimeSnapshot) -> Result<wire::StaticSelection2> {
    let peers = handshake::pair_credentials(&s.selection)?;
    ensure!(
        peers.iter().map(|p| p.cert_ref()).collect::<Vec<_>>() == s.spec.certs,
        "selected certificate pins"
    );
    let proof = wire::ParsedModeProof2::parse(&s.selection.proof)?;
    ensure!(
        proof.origin() == &wire::Origin::Fresh,
        "UnsupportedExistingRuntime"
    );
    let proof = proof.verify(&wire::SelectionEvidence2 {
        peers,
        membership: s.spec.membership.clone(),
        initial_pair: Some(s.selection.initial_pair.clone()),
        previous: None,
    })?;
    ensure!(s.head.mode_ref == Some(proof.q()), "selected Q Head");
    Ok(wire::StaticSelection2::new(proof, s.head.sid.clone())?)
}
