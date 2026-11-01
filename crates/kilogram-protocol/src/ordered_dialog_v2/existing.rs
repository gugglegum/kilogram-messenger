//! Verified old scope for existing admission before selection. No live permission
//! or Native provenance is inferred from this public, signed evidence.
use super::*;
use crate::ordered_dialog_v1 as old;

pub const MAX_EXISTING_ORIGIN: usize = 512 * 1024;

#[derive(Clone)]
pub struct VerifiedExistingScope1 {
    raw: Vec<u8>,
    spec: Vec<u8>,
    c: Id,
    d: Id,
    e: Id,
    sid: String,
    peers: [RootVerifiedCredential; 2],
    membership: Vec<u8>,
    offer: Vec<u8>,
    grant: Vec<u8>,
    genesis: [Vec<u8>; 2],
    ready: [Vec<u8>; 2],
}

impl VerifiedExistingScope1 {
    /// The sole constructor verifies all signed bytes, including the original
    /// consent and both Hello/Proof participants. A DTO cannot assert this type.
    pub fn verify_export(raw: &[u8]) -> Result<Self> {
        let value = decode(raw, MAX_EXISTING_ORIGIN)?;
        let f = value.fields(7)?;
        require(f[0].uint()? == 1, "existing-origin version")?;
        let spec = decode(f[1].bytes()?, 96 * 1024)?;
        let s = spec.fields(6)?;
        require(s[0].uint()? == 1, "existing Spec1")?;
        let c = s[1].fixed()?;
        let peers = [
            ParsedCredentialBundle::parse(s[2].bytes()?)?.verify_root()?,
            ParsedCredentialBundle::parse(s[3].bytes()?)?.verify_root()?,
        ];
        verify_membership(c, &peers, s[4].bytes()?)?;
        let certs = peers.each_ref().map(RootVerifiedCredential::cert_ref);
        let consent = decode(s[5].bytes()?, 4096)?;
        let cf = consent.fields(3)?;
        let cb = cf[0].fields(8)?;
        require(
            cb[0].uint()? == 1
                && cb[1].fixed::<32>()? == c
                && cb[2].fixed::<32>()? == certs[0]
                && cb[3].fixed::<32>()? == certs[1]
                && cb[4].uint()? == 1
                && cb[5].uint()? == 1
                && cb[6].uint()? < cb[7].uint()?,
            "existing historical Consent1 scope",
        )?;
        let consent_body = encode(&cf[0], 4096)?;
        for role in 0..2 {
            peers[role]
                .certificate()
                .account_id()
                .verify_ordered_context_consent(&consent_body, cf[role + 1].bytes()?)?;
        }
        let pair = decode(f[4].bytes()?, 192 * 1024)?;
        let pf = pair.fields(6)?;
        require(pf[0].uint()? == 1, "existing initial pair")?;
        let hello = [
            old::Frame::parse(pf[2].bytes()?)?,
            old::Frame::parse(pf[3].bytes()?)?,
        ];
        require(
            hello[0].binding == hello[1].binding,
            "existing Hello binding",
        )?;
        let binding = hello[0].binding;
        let selected_consent = hello[0].body.fields(11)?[9].fixed::<32>()?;
        for role in 0..2 {
            let h = &hello[role];
            h.verify(binding, peers[role].certificate().device_id())?;
            let hf = h.body.fields(11)?;
            let credential = ParsedCredentialBundle::parse(hf[7].bytes()?)?.verify_root()?;
            require(
                h.kind == 1
                    && hf[0].fixed::<32>()? == c
                    && hf[1].uint()? == role as u64
                    && credential.cert_ref() == certs[role]
                    && hf[8].bytes()? == s[4].bytes()?
                    && hf[9].fixed::<32>()? == selected_consent
                    && hf[5] == a([u(1)])
                    && hf[6] == a([u(1)]),
                "existing initial Hello scope/consent/role",
            )?;
            hf[2].fixed::<32>()?;
            hf[3].fixed::<32>()?;
            hf[4].fixed::<32>()?;
        }
        require(
            hello[0].body.list()?[2] == hello[1].body.list()?[3]
                && hello[0].body.list()?[3] == hello[1].body.list()?[2],
            "existing initial endpoint roles",
        )?;
        let transcript = hash(
            b"kg:od1:auth-pair\0",
            &encode(
                &a([b(binding), b(pf[2].bytes()?), b(pf[3].bytes()?)]),
                192 * 1024,
            )?,
        );
        require(pf[1].fixed::<32>()? == transcript, "existing transcript")?;
        for role in 0..2 {
            let proof = old::Frame::parse(pf[4 + role].bytes()?)?;
            proof.verify(binding, peers[role].certificate().device_id())?;
            require(
                proof.kind == 2
                    && proof.body == a([b(transcript), b(c), u(1), u(1), b(certs[0]), b(certs[1])]),
                "existing initial Proof",
            )?;
        }
        let offer = f[2].bytes()?.to_vec();
        let grant = f[3].bytes()?.to_vec();
        let ob = old::control(&offer, 1, peers[0].certificate().device_id())?;
        let gb = old::control(&grant, 2, peers[1].certificate().device_id())?;
        let o = ob.fields(8)?;
        let g = gb.fields(8)?;
        require(
            o[0].fixed::<32>()? == c
                && o[1].fixed::<32>()? == certs[0]
                && o[2].fixed::<32>()? == certs[1]
                && o[3].fixed::<32>()? == transcript
                && g[0].fixed::<32>()? == c
                && g[1].fixed::<32>()? == hash(old::CONTROL_ID, &offer)
                && g[2] == o[3]
                && o[6].uint()? == 1
                && o[7].uint()? == 1
                && g[6].uint()? == 1
                && g[7].uint()? == 1,
            "existing original Offer/Grant",
        )?;
        for x in [&o[4], &o[5], &g[3], &g[4], &g[5]] {
            x.fixed::<32>()?;
        }
        let e = hash(
            b"kg:od1:epoch\0",
            &encode(
                &a([u(1), b(c), b(certs[0]), b(certs[1]), b(&offer), b(&grant)]),
                4096,
            )?,
        );
        let d = hash(
            b"kg:od1:domain\0",
            &encode(&a([u(1), u(1), b(c), b(certs[0]), b(certs[1]), b(e)]), 256)?,
        );
        let gs = f[5].fields(2)?;
        let rs = f[6].fields(2)?;
        let genesis = [gs[0].bytes()?.to_vec(), gs[1].bytes()?.to_vec()];
        let ready = [rs[0].bytes()?.to_vec(), rs[1].bytes()?.to_vec()];
        let mut sid = None;
        let ids = genesis.each_ref().map(|raw| hash(old::EVENT_ID, raw));
        for role in 0..2 {
            let event = old::Event::parse(&genesis[role], peers[role].certificate().device_id())?;
            require(
                event.core.c == c
                    && event.core.d == d
                    && event.core.e == e
                    && event.core.role == role as u64
                    && event.core.n == 0
                    && event.core.kind == role as u64 + 1,
                "existing readable genesis",
            )?;
            if let Some(prior) = &sid {
                require(prior == &event.core.sid, "existing SID agreement")?;
            } else {
                sid = Some(event.core.sid.clone());
            }
            let body = old::control(&ready[role], 3, peers[role].certificate().device_id())?;
            require(
                body == a([
                    b(d),
                    b(e),
                    b(event.core.sid.as_bytes()),
                    b(ids[0]),
                    b(ids[1]),
                    u(role as u64),
                    u(1),
                    u(1),
                ]),
                "existing Ready1 pair",
            )?;
        }
        Ok(Self {
            raw: raw.to_vec(),
            spec: f[1].bytes()?.to_vec(),
            c,
            d,
            e,
            sid: sid.ok_or(Error::Invalid("existing SID"))?,
            peers,
            membership: s[4].bytes()?.to_vec(),
            offer,
            grant,
            genesis,
            ready,
        })
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn spec_raw(&self) -> &[u8] {
        &self.spec
    }
    pub fn c(&self) -> Id {
        self.c
    }
    pub fn d(&self) -> Id {
        self.d
    }
    pub fn e(&self) -> Id {
        self.e
    }
    pub fn sid(&self) -> &str {
        &self.sid
    }
    pub fn certs(&self) -> [Id; 2] {
        self.peers.each_ref().map(RootVerifiedCredential::cert_ref)
    }
    pub fn peers(&self) -> &[RootVerifiedCredential; 2] {
        &self.peers
    }
    pub fn membership(&self) -> &[u8] {
        &self.membership
    }
    pub fn old_control_ids(&self) -> [Id; 4] {
        [
            hash(old::CONTROL_ID, &self.offer),
            hash(old::CONTROL_ID, &self.grant),
            hash(old::EVENT_ID, &self.genesis[0]),
            hash(old::EVENT_ID, &self.genesis[1]),
        ]
    }
    pub fn origin(&self) -> Origin {
        Origin::Existing {
            d: self.d,
            e: self.e,
            sid: self.sid.clone(),
        }
    }
    pub fn verify_consent(
        &self,
        consent: &ParsedConsent2,
        peers: &[RootVerifiedCredential; 2],
    ) -> Result<()> {
        require(
            consent.c() == self.c
                && consent.certs() == self.certs()
                && consent.origin() == &self.origin(),
            "existing Consent2 scope",
        )?;
        consent.verify(peers)
    }
    pub fn verify_event(
        &self,
        raw: &[u8],
        author: &RootVerifiedCredential,
    ) -> Result<VerifiedLegacyEvent> {
        let event = old::Event::parse(raw, author.certificate().device_id())?;
        let r =
            usize::try_from(event.core.role).map_err(|_| Error::Invalid("existing event role"))?;
        require(
            r < 2
                && self.certs()[r] == author.cert_ref()
                && event.core.author_auth == author.reference()
                && event.core.c == self.c
                && event.core.d == self.d
                && event.core.e == self.e
                && event.core.sid == self.sid
                && event.core.n < 512,
            "existing legacy source scope",
        )?;
        Ok(VerifiedLegacyEvent(event))
    }
    pub fn verify_open_intent(&self, body: &FrameBody2, sender_role: u8) -> Result<()> {
        require(
            body.kind() == FrameKind::OpenIntent && sender_role == 1,
            "existing B notification role",
        )?;
        let ids = self.old_control_ids();
        require(
            body.value()
                == &a([
                    b(self.c),
                    u(3),
                    opt(Some(b(ids[0]))),
                    opt(Some(b(ids[1]))),
                    opt(Some(b(ids[2]))),
                    opt(Some(b(ids[3]))),
                ]),
            "existing OpenIntent exact refs/phase",
        )
    }
    pub fn selection(&self, proof: ParsedModeProof2) -> Result<StaticSelection2> {
        require(
            proof.c() == self.c
                && proof.certs() == self.certs()
                && proof.origin() == &self.origin(),
            "existing selected scope",
        )?;
        StaticSelection2::new(
            proof.verify(&SelectionEvidence2 {
                peers: self.peers.clone(),
                membership: self.membership.clone(),
                initial_pair: None,
                previous: Some(PreviousSession2 {
                    genesis: self.genesis.clone(),
                    ready: self.ready.clone(),
                }),
            })?,
            Some(self.sid.clone()),
        )
    }
}
