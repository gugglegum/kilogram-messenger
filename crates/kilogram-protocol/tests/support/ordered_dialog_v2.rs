// Shared test construction only. No production dependency edge; public test keys.
use super::wire;
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, ConversationScopeId, DeviceCapability, DeviceCertificate,
    DeviceEncryptionIdentity, DeviceId, DeviceIdentity, DeviceRevocation, EncryptionPublicKey,
};
use serde::Serialize;
use wire::{Id, a, b, encode, u};
pub type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Serialize)]
struct Signed<T> {
    content: T,
    signature: Vec<u8>,
}
#[derive(Serialize)]
struct CertContent {
    version: u8,
    account_id: AccountId,
    device_id: DeviceId,
    encryption_public_key: EncryptionPublicKey,
    authority_sequence: u64,
    capabilities: Vec<DeviceCapability>,
}
#[derive(Serialize)]
struct AuthorityContent {
    version: u8,
    account_id: AccountId,
    revision: u64,
    revocations: Vec<DeviceRevocation>,
}
#[derive(Serialize)]
struct MemberContent {
    version: u8,
    conversation_id: ConversationScopeId,
    revision: u64,
    owner_account_id: AccountId,
    members: Vec<AccountId>,
}
fn signed<T: Serialize>(content: T, root: &DeviceIdentity, tag: &[u8]) -> TestResult<Vec<u8>> {
    let mut bytes = tag.to_vec();
    bytes.extend(postcard::to_allocvec(&content)?);
    let signature = root.sign(&bytes).to_vec();
    Ok(postcard::to_allocvec(&Signed { content, signature })?)
}
pub struct Actors {
    pub devices: [DeviceIdentity; 2],
    pub roots: [DeviceIdentity; 2],
    pub credentials: [wire::RootVerifiedCredential; 2],
    pub membership: Vec<u8>,
    pub c: Id,
}
impl Actors {
    pub fn new(c: Id) -> TestResult<Self> {
        let mut devices = [
            DeviceIdentity::from_secret_bytes([0x31; 32]),
            DeviceIdentity::from_secret_bytes([0x52; 32]),
        ];
        if devices[0].device_id() > devices[1].device_id() {
            devices.swap(0, 1);
        }
        let roots = [
            DeviceIdentity::from_secret_bytes([91; 32]),
            DeviceIdentity::from_secret_bytes([92; 32]),
        ];
        let mut creds = vec![];
        for role in 0..2 {
            let account = AccountId::from_bytes(*roots[role].device_id().as_bytes());
            let cert_raw = signed(
                CertContent {
                    version: 2,
                    account_id: account,
                    device_id: devices[role].device_id(),
                    encryption_public_key: DeviceEncryptionIdentity::from_secret_bytes(
                        [31 + role as u8; 32],
                    )
                    .public_key(),
                    authority_sequence: 1,
                    capabilities: DeviceCapability::MESSAGING.to_vec(),
                },
                &roots[role],
                b"kilogram:device-certificate-signature:v2\0",
            )?;
            let cert = DeviceCertificate::decode_and_verify(&cert_raw)?;
            let authority_raw = signed(
                AuthorityContent {
                    version: 1,
                    account_id: account,
                    revision: 2,
                    revocations: vec![],
                },
                &roots[role],
                b"kilogram:account-authority-snapshot-signature:v1\0",
            )?;
            creds.push(wire::RootVerifiedCredential::from_parts(
                cert,
                AccountAuthoritySnapshot::decode_and_verify(&authority_raw)?,
            )?);
        }
        let credentials: [wire::RootVerifiedCredential; 2] =
            creds.try_into().map_err(|_| "credential pair")?;
        let owner = credentials[0].certificate().account_id();
        let mut members = vec![owner, credentials[1].certificate().account_id()];
        members.sort_by_key(|id| *id.as_bytes());
        let membership = signed(
            MemberContent {
                version: 1,
                conversation_id: ConversationScopeId::from_bytes(c),
                revision: 1,
                owner_account_id: owner,
                members,
            },
            &roots[0],
            b"kilogram:conversation-membership-signature:v1\0",
        )?;
        wire::verify_membership(c, &credentials, &membership)?;
        Ok(Self {
            devices,
            roots,
            credentials,
            membership,
            c,
        })
    }
    pub fn renewal(&self, role: usize, revision: u64) -> TestResult<wire::RootVerifiedCredential> {
        let authority = signed(
            AuthorityContent {
                version: 1,
                account_id: self.credentials[role].certificate().account_id(),
                revision,
                revocations: vec![],
            },
            &self.roots[role],
            b"kilogram:account-authority-snapshot-signature:v1\0",
        )?;
        Ok(wire::RootVerifiedCredential::from_parts(
            self.credentials[role].certificate().clone(),
            AccountAuthoritySnapshot::decode_and_verify(&authority)?,
        )?)
    }
    pub fn consent(&self, origin: wire::Origin) -> TestResult<Vec<u8>> {
        let body = a([
            u(2),
            b(self.c),
            b(self.credentials[0].cert_ref()),
            b(self.credentials[1].cert_ref()),
            u(1),
            u(2),
            origin.value(),
            u(1),
            u(1699999000),
            u(1700010000),
        ]);
        let bytes = wire::signing_bytes(wire::CONSENT, &body, 4096)?;
        Ok(encode(
            &a([
                body,
                b(self.roots[0].sign(&bytes)),
                b(self.roots[1].sign(&bytes)),
            ]),
            4096,
        )?)
    }
    pub fn fresh(&self, native_a: Id, native_b: Id, otk: Id) -> TestResult<Fresh> {
        let consent = self.consent(wire::Origin::Fresh)?;
        let consent_ref = wire::hash(wire::CONSENT, &consent);
        let binding = [17; 32];
        let endpoints = [[11; 32], [12; 32]];
        let mut hellos = vec![];
        for role in 0..2 {
            let body = a([
                b(self.c),
                u(role as u64),
                b(endpoints[role]),
                b(endpoints[1 - role]),
                b([21 + role as u8; 32]),
                a([u(1)]),
                a([u(2)]),
                b(self.credentials[role].raw()),
                b(&self.membership),
                b(consent_ref),
                b([51 + role as u8; 32]),
                wire::Origin::Fresh.value(),
            ]);
            let body = wire::FrameBody2::parse(wire::FrameKind::Hello, &encode(&body, 80 * 1024)?)?;
            hellos.push(wire::ParsedFrame2::sign(
                body,
                binding,
                [role as u8; 16],
                &self.devices[role],
            )?);
        }
        let t = wire::initial_transcript2(binding, hellos[0].raw(), hellos[1].raw())?;
        let certs = [
            self.credentials[0].cert_ref(),
            self.credentials[1].cert_ref(),
        ];
        let mut proofs = vec![];
        for role in 0..2 {
            proofs.push(wire::ParsedFrame2::sign(
                wire::FrameBody2::proof(t, self.c, certs)?,
                binding,
                [role as u8 + 2; 16],
                &self.devices[role],
            )?);
        }
        let pair = encode(
            &a([
                u(2),
                b(t),
                b(hellos[0].raw()),
                b(hellos[1].raw()),
                b(proofs[0].raw()),
                b(proofs[1].raw()),
            ]),
            192 * 1024,
        )?;
        let offer = wire::ParsedControl2::sign(
            1,
            &encode(
                &a([
                    b(self.c),
                    b(certs[0]),
                    b(certs[1]),
                    b(t),
                    b([0x17; 32]),
                    b(native_a),
                    u(1),
                    u(2),
                ]),
                4096,
            )?,
            &self.devices[0],
        )?;
        let grant = wire::ParsedControl2::sign(
            2,
            &encode(
                &a([
                    b(self.c),
                    b(offer.id()),
                    b(t),
                    b([0x29; 32]),
                    b(native_b),
                    b(otk),
                    u(1),
                    u(2),
                ]),
                4096,
            )?,
            &self.devices[1],
        )?;
        let raw = encode(
            &a([
                u(2),
                u(0),
                b(self.c),
                b(&consent),
                b(offer.raw()),
                b(grant.raw()),
            ]),
            wire::MAX_PROOF,
        )?;
        let parsed = wire::ParsedModeProof2::parse(&raw)?;
        let proof = parsed.verify(&wire::SelectionEvidence2 {
            peers: self.credentials.clone(),
            membership: self.membership.clone(),
            initial_pair: Some(pair.clone()),
            previous: None,
        })?;
        Ok(Fresh {
            proof,
            raw,
            consent,
            pair,
            offer,
            grant,
            hellos,
            proofs,
        })
    }
    pub fn existing(
        &self,
        d: Id,
        e: Id,
        sid: &str,
        genesis: [Vec<u8>; 2],
        cuts: [u64; 2],
        last: [Id; 2],
    ) -> TestResult<Existing> {
        let consent = self.consent(wire::Origin::Existing {
            d,
            e,
            sid: sid.into(),
        })?;
        let p = wire::ParsedControl2::sign(
            18,
            &encode(
                &a([
                    b(self.c),
                    b(d),
                    b(e),
                    b(sid.as_bytes()),
                    b(wire::hash(wire::CONSENT, &consent)),
                    b([44; 32]),
                    u(cuts[0]),
                    b(last[0]),
                ]),
                4096,
            )?,
            &self.devices[0],
        )?;
        let accept = wire::ParsedControl2::sign(
            19,
            &encode(&a([b(self.c), b(p.id()), u(cuts[1]), b(last[1])]), 4096)?,
            &self.devices[1],
        )?;
        let select = wire::ParsedControl2::sign(
            20,
            &encode(&a([b(self.c), b(p.id()), b(accept.id())]), 4096)?,
            &self.devices[0],
        )?;
        let raw = encode(
            &a([
                u(2),
                u(1),
                b(p.raw()),
                b(accept.raw()),
                b(select.raw()),
                b(&consent),
            ]),
            wire::MAX_PROOF,
        )?;
        let ids = [
            wire::hash(b"kg:od1:event-id\0", &genesis[0]),
            wire::hash(b"kg:od1:event-id\0", &genesis[1]),
        ];
        let mut ready = vec![];
        for role in 0..2 {
            ready.push(kilogram_protocol_v1_ready(
                d,
                e,
                sid,
                ids,
                role,
                &self.devices[role],
            )?);
        }
        let previous = wire::PreviousSession2 {
            genesis,
            ready: ready.try_into().map_err(|_| "ready pair")?,
        };
        let proof = wire::ParsedModeProof2::parse(&raw)?.verify(&wire::SelectionEvidence2 {
            peers: self.credentials.clone(),
            membership: self.membership.clone(),
            initial_pair: None,
            previous: Some(previous),
        })?;
        Ok(Existing {
            proof,
            raw,
            consent,
            propose: p,
            accept,
            select,
        })
    }
}
fn kilogram_protocol_v1_ready(
    d: Id,
    e: Id,
    sid: &str,
    ids: [Id; 2],
    role: usize,
    signer: &DeviceIdentity,
) -> TestResult<Vec<u8>> {
    // Exact legacy Control3, signed without invoking any protocol2 decoder.
    let body = a([
        b(d),
        b(e),
        b(sid.as_bytes()),
        b(ids[0]),
        b(ids[1]),
        u(role as u64),
        u(1),
        u(1),
    ]);
    let unsigned = a([u(1), u(3), body.clone()]);
    let sig = signer.sign(&wire::signing_bytes(
        b"kg:od1:control-sign\0",
        &unsigned,
        32 * 1024,
    )?);
    Ok(encode(&a([u(1), u(3), body, b(sig)]), 32 * 1024)?)
}
pub struct Existing {
    pub proof: wire::VerifiedModeProof2,
    pub raw: Vec<u8>,
    pub consent: Vec<u8>,
    pub propose: wire::ParsedControl2,
    pub accept: wire::ParsedControl2,
    pub select: wire::ParsedControl2,
}
pub struct Fresh {
    pub proof: wire::VerifiedModeProof2,
    pub raw: Vec<u8>,
    pub consent: Vec<u8>,
    pub pair: Vec<u8>,
    pub offer: wire::ParsedControl2,
    pub grant: wire::ParsedControl2,
    pub hellos: Vec<wire::ParsedFrame2>,
    pub proofs: Vec<wire::ParsedFrame2>,
}
pub fn hex(raw: &[u8]) -> String {
    raw.iter().map(|v| format!("{v:02x}")).collect()
}
pub fn unhex(s: &str) -> TestResult<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return Err("odd hex".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&s[i..i + 2], 16)?))
        .collect()
}
