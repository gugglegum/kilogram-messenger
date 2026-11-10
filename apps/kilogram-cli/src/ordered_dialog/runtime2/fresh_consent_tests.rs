use super::*;
use kilogram_identity::{DeviceCapability, DeviceEncryptionIdentity, DeviceIdentity};
struct Fixture {
    _dir: tempfile::TempDir,
    roots: [AccountRootState; 2],
    paths: [std::path::PathBuf; 2],
    credentials: [Vec<u8>; 2],
    floors: [Vec<u8>; 2],
    devices: [DeviceId; 2],
}
impl Fixture {
    fn new() -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let make = |n| -> Result<_> {
            let path = dir.path().join(format!("root{n}"));
            let root = AccountRootState::create(&path)?;
            let device = DeviceIdentity::generate()?;
            let enc = DeviceEncryptionIdentity::generate()?;
            let cert = root.issue_device_certificate(
                device.device_id(),
                enc.public_key(),
                &DeviceCapability::MESSAGING,
            )?;
            let credential =
                wire::RootVerifiedCredential::from_parts(cert, root.authority_snapshot()?)?;
            Ok((root, credential.raw().to_vec(), device.device_id(), path))
        };
        let mut rows = [make(0)?, make(1)?];
        rows.sort_by_key(|r| *r.2.as_bytes());
        let [(r0, c0, d0, p0), (r1, c1, d1, p1)] = rows;
        let floors = [
            r0.authority_snapshot()?.encode()?,
            r1.authority_snapshot()?.encode()?,
        ];
        Ok(Self {
            _dir: dir,
            roots: [r0, r1],
            paths: [p0, p1],
            credentials: [c0, c1],
            floors,
            devices: [d0, d1],
        })
    }
    fn input(&self, role: usize) -> Inputs<'_> {
        Inputs {
            credentials: self.credentials.each_ref().map(|c| c.as_slice()),
            minimum_authorities: self.floors.each_ref().map(|f| f.as_slice()),
            context: [9; 32],
            account: self.roots[role].account_id(),
            peer: self.roots[1 - role].account_id(),
            device: self.devices[role],
            peer_device: self.devices[1 - role],
        }
    }
    fn root(&self, n: usize) -> std::path::PathBuf {
        self.paths[n].clone()
    }
}
#[test]
fn cs1_fresh_verifier_interop_and_rejections() -> Result<()> {
    let f = Fixture::new()?;
    let r0 = f.root(0);
    let r1 = f.root(1);
    let now = 1000;
    let mut input = f.input(0);
    input.credentials.reverse();
    input.minimum_authorities.reverse();
    let proposal = propose(input, &r0, now, 3600, now)?;
    let final_artifact = cosign(f.input(1), &proposal, &r1, now)?;
    let v = wire::decode(&final_artifact, MAX_PROPOSAL)?;
    let final_fields = v.fields(6)?;
    let verified = peers(&f.input(1), 1)?;
    wire::verify_membership([9; 32], &verified, final_fields[4].bytes()?)?;
    let consent = wire::ParsedConsent2::parse(final_fields[5].bytes()?)?;
    consent.verify(&verified)?;
    ensure!(
        consent.origin() == &wire::Origin::Fresh,
        "unchanged final Fresh verifier"
    );
    let before = f.roots[1].authority_snapshot()?.encode()?;
    let original = wire::decode(&proposal, MAX_PROPOSAL)?;
    for case in 0..12 {
        let mut rows = original.list()?.to_vec();
        match case {
            0 => rows[0] = b(b"existing proposal"),
            1 => rows.swap(1, 2),
            2 => {
                let mut raw = rows[2].bytes()?.to_vec();
                raw[0] ^= 1;
                rows[2] = b(raw);
            }
            3 => {
                let mut raw = rows[3].bytes()?.to_vec();
                raw[0] ^= 1;
                rows[3] = b(raw);
            }
            4..=8 => {
                let mut body = rows[4].list()?.to_vec();
                match case {
                    4 => body[1] = b([8; 32]),
                    5 => body[6] = a([u(1), b([1; 32]), b([2; 32]), b([b'a'; 43])]),
                    6 => body[4] = u(9),
                    7 => {
                        body[8] = u(1);
                        body[9] = u(2);
                    }
                    _ => body[8] = u(now + 121),
                };
                rows[4] = a(body);
            }
            9 | 10 => {
                let n = if case == 9 { 5 } else { 6 };
                let mut sig = rows[n].bytes()?.to_vec();
                sig[0] ^= 1;
                rows[n] = b(sig);
            }
            _ => rows.push(u(0)),
        }
        ensure!(
            cosign(f.input(1), &wire::encode(&a(rows), MAX_PROPOSAL)?, &r1, now).is_err(),
            "tampered proposal case {case}"
        );
    }
    for raw in [
        &proposal[..proposal.len() - 1],
        &vec![0; MAX_PROPOSAL + 1][..],
        &final_artifact[..],
    ] {
        ensure!(
            cosign(f.input(1), raw, &r1, now).is_err(),
            "incomplete/oversized/final artifact cannot be a proposal"
        );
    }
    let mut input = f.input(1);
    input.peer_device = DeviceId::from_bytes([3; 32]);
    ensure!(
        cosign(input, &proposal, &r1, now).is_err(),
        "explicit peer device pin"
    );
    let mut input = f.input(1);
    input.peer = f.roots[1].account_id();
    ensure!(
        cosign(input, &proposal, &r1, now).is_err(),
        "explicit peer account pin"
    );
    ensure!(
        cosign(f.input(1), &proposal, &r0, now).is_err(),
        "wrong local Root"
    );
    ensure!(
        propose(f.input(1), &r1, now, 3600, now).is_err(),
        "role1 cannot initiate"
    );
    ensure!(
        f.roots[1].authority_snapshot()?.encode()? == before,
        "rejected cosigns did not mutate authority"
    );
    ensure!(
        propose(f.input(0), &r0, now, 3600, now)? == proposal,
        "safe identical membership/proposal retry"
    );
    Ok(())
}
#[test]
fn cs1_fresh_refuses_stale_authority_and_membership_conflict() -> Result<()> {
    let mut f = Fixture::new()?;
    let r0 = f.root(0);
    let r1 = f.root(1);
    let proposal = propose(f.input(0), &r0, 1000, 3600, 1000)?;
    let extra = DeviceIdentity::generate()?;
    let enc = DeviceEncryptionIdentity::generate()?;
    f.roots[0].issue_device_certificate(
        extra.device_id(),
        enc.public_key(),
        &DeviceCapability::MESSAGING,
    )?;
    f.floors[0] = f.roots[0].authority_snapshot()?.encode()?;
    ensure!(
        cosign(f.input(1), &proposal, &r1, 1000).is_err(),
        "retained peer authority floor rejects rollback"
    );
    f.floors[0] = wire::ParsedCredentialBundle::parse(&f.credentials[0])?
        .authority_raw()
        .to_vec();
    ensure!(
        propose(f.input(0), &r0, 1000, 3600, 1000).is_err(),
        "own Root current authority rejects old local credential"
    );
    let cert = wire::ParsedCredentialBundle::parse(&f.credentials[0])?
        .verify_root()?
        .certificate()
        .clone();
    f.credentials[0] =
        wire::RootVerifiedCredential::from_parts(cert.clone(), f.roots[0].authority_snapshot()?)?
            .raw()
            .to_vec();
    let mut rewritten = wire::decode(&proposal, MAX_PROPOSAL)?.list()?.to_vec();
    rewritten[1] = b(&f.credentials[0]);
    ensure!(
        cosign(
            f.input(1),
            &wire::encode(&a(rewritten), MAX_PROPOSAL)?,
            &r1,
            1000
        )
        .is_err(),
        "proposal seal binds exact authority/credential bytes despite unchanged certificate and valid consent signature"
    );
    f.roots[0].revoke_device(f.devices[0])?;
    f.floors[0] = f.roots[0].authority_snapshot()?.encode()?;
    f.credentials[0] = wire::encode(
        &a([u(1), b(cert.encode()?), b(&f.floors[0])]),
        wire::MAX_AUTH,
    )?;
    ensure!(
        cosign(f.input(1), &proposal, &r1, 1000).is_err(),
        "revoked peer certificate refuses"
    );
    let f = Fixture::new()?;
    let r0 = f.root(0);
    let other = AccountRootState::create(f._dir.path().join("other"))?;
    let membership = f.roots[0]
        .create_conversation_membership(
            ConversationScopeId::from_bytes([9; 32]),
            &[other.account_id()],
        )?
        .encode()?;
    ensure!(
        propose(f.input(0), &r0, 1000, 3600, 1000).is_err(),
        "existing different membership refuses"
    );
    ensure!(
        f.roots[0]
            .load_root_conversation_membership(ConversationScopeId::from_bytes([9; 32]))?
            .encode()?
            == membership,
        "conflict did not replace membership"
    );
    Ok(())
}
