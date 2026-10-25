use super::*;

#[derive(Clone)]
pub struct ParsedControl2 {
    raw: Vec<u8>,
    kind: u8,
    body: Value,
    sig: [u8; 64],
}
impl ParsedControl2 {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode(raw, 32 * 1024)?;
        let f = v.fields(4)?;
        require(f[0].uint()? == 2, "Control version")?;
        let kind = f[1].uint()?;
        let count = match kind {
            1 | 2 => 8,
            3 => 9,
            18 => 8,
            19 => 4,
            20 => 3,
            21 => 2,
            _ => return Err(Error::Invalid("Control kind")),
        };
        let t = f[2].fields(count)?;
        match kind {
            1 | 2 => {
                for x in &t[..6] {
                    x.fixed::<32>()?;
                }
                require(
                    t[6].uint()? == 1 && t[7].uint()? == 2,
                    "Control negotiation",
                )?;
            }
            3 => {
                t[0].fixed::<32>()?;
                t[1].fixed::<32>()?;
                require(
                    t[2].bytes()?.len() == 43 && t[2].bytes()?.is_ascii(),
                    "Ready SID",
                )?;
                t[3].fixed::<32>()?;
                t[4].fixed::<32>()?;
                require(
                    t[5].uint()? <= 1 && t[6].uint()? == 1 && t[7].uint()? == 2,
                    "Ready fields",
                )?;
                t[8].fixed::<32>()?;
            }
            18 => {
                for x in &t[..3] {
                    x.fixed::<32>()?;
                }
                require(
                    t[3].bytes()?.len() == 43 && t[3].bytes()?.is_ascii(),
                    "P SID",
                )?;
                t[4].fixed::<32>()?;
                t[5].fixed::<32>()?;
                require((1..=512).contains(&t[6].uint()?), "cutA")?;
                t[7].fixed::<32>()?;
            }
            19 => {
                t[0].fixed::<32>()?;
                t[1].fixed::<32>()?;
                require((1..=512).contains(&t[2].uint()?), "cutB")?;
                t[3].fixed::<32>()?;
            }
            20 | 21 => {
                for x in t {
                    x.fixed::<32>()?;
                }
            }
            _ => return Err(Error::Invalid("Control kind")),
        }
        Ok(Self {
            raw: raw.to_vec(),
            kind: kind as u8,
            body: f[2].clone(),
            sig: f[3].fixed()?,
        })
    }
    pub fn sign(kind: u8, body_raw: &[u8], signer: &DeviceIdentity) -> Result<Self> {
        let body = decode(body_raw, 32 * 1024)?;
        let unsigned = a([u(2), u(u64::from(kind)), body.clone()]);
        let sig = signer.sign(&signing_bytes(CONTROL_SIGN, &unsigned, 32 * 1024)?);
        Self::parse(&encode(
            &a([u(2), u(u64::from(kind)), body, b(sig)]),
            32 * 1024,
        )?)
    }
    pub fn require_kind(&self, kind: u8) -> Result<()> {
        require(self.kind == kind, "Control kind binding")
    }
    pub fn verify(&self, signer: DeviceId) -> Result<()> {
        signer.verify(
            &signing_bytes(
                CONTROL_SIGN,
                &a([u(2), u(u64::from(self.kind)), self.body.clone()]),
                32 * 1024,
            )?,
            &self.sig,
        )?;
        Ok(())
    }
    pub fn body(&self) -> &Value {
        &self.body
    }
    pub fn kind(&self) -> u8 {
        self.kind
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn id(&self) -> Id {
        hash(CONTROL_ID, &self.raw)
    }
}
#[derive(Clone)]
pub struct ParsedModeProof2 {
    raw: Vec<u8>,
    origin: Origin,
    c: Id,
    certs: [Id; 2],
    consent: ParsedConsent2,
    controls: Vec<ParsedControl2>,
    cuts: [u64; 2],
    offer_id: Option<Id>,
    grant_id: Option<Id>,
    native_a: Option<Id>,
    otk: Option<Id>,
    d: Id,
    e: Id,
}
impl ParsedModeProof2 {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::ModeProof)?;
        let f = v.fields(6)?;
        require(f[0].uint()? == 2, "proof version")?;
        match f[1].uint()? {
            0 => {
                let c = f[2].fixed::<32>()?;
                let consent = ParsedConsent2::parse(f[3].bytes()?)?;
                require(
                    consent.c == c && consent.origin == Origin::Fresh,
                    "fresh proof consent",
                )?;
                let offer = ParsedControl2::parse(f[4].bytes()?)?;
                offer.require_kind(1)?;
                let grant = ParsedControl2::parse(f[5].bytes()?)?;
                grant.require_kind(2)?;
                let oa = offer.body.fields(8)?;
                let gb = grant.body.fields(8)?;
                require(
                    oa[0].fixed::<32>()? == c
                        && oa[1].fixed::<32>()? == consent.certs[0]
                        && oa[2].fixed::<32>()? == consent.certs[1]
                        && gb[0].fixed::<32>()? == c
                        && gb[1].fixed::<32>()? == offer.id()
                        && oa[3] == gb[2],
                    "selected Offer/Grant",
                )?;
                let e = hash(
                    b"kg:od2:epoch\0",
                    &encode(
                        &a([
                            u(2),
                            b(c),
                            b(consent.certs[0]),
                            b(consent.certs[1]),
                            b(offer.raw()),
                            b(grant.raw()),
                        ]),
                        4096,
                    )?,
                );
                let d = hash(
                    b"kg:od2:domain\0",
                    &encode(
                        &a([
                            u(2),
                            u(1),
                            u(2),
                            b(c),
                            b(consent.certs[0]),
                            b(consent.certs[1]),
                            b(e),
                        ]),
                        256,
                    )?,
                );
                Ok(Self {
                    raw: raw.to_vec(),
                    origin: Origin::Fresh,
                    c,
                    certs: consent.certs,
                    consent,
                    cuts: [0, 0],
                    offer_id: Some(offer.id()),
                    grant_id: Some(grant.id()),
                    native_a: Some(oa[5].fixed()?),
                    otk: Some(gb[5].fixed()?),
                    controls: vec![offer, grant],
                    d,
                    e,
                })
            }
            1 => {
                let p = ParsedControl2::parse(f[2].bytes()?)?;
                p.require_kind(18)?;
                let accept = ParsedControl2::parse(f[3].bytes()?)?;
                accept.require_kind(19)?;
                let k = ParsedControl2::parse(f[4].bytes()?)?;
                k.require_kind(20)?;
                let consent = ParsedConsent2::parse(f[5].bytes()?)?;
                let pb = p.body.fields(8)?;
                let ab = accept.body.fields(4)?;
                let kb = k.body.fields(3)?;
                let c = pb[0].fixed::<32>()?;
                let d = pb[1].fixed::<32>()?;
                let e = pb[2].fixed::<32>()?;
                let origin = Origin::Existing {
                    d,
                    e,
                    sid: pb[3].text()?.into(),
                };
                require(
                    consent.c == c
                        && consent.origin == origin
                        && pb[4].fixed::<32>()? == consent.reference()
                        && ab[0].fixed::<32>()? == c
                        && ab[1].fixed::<32>()? == p.id()
                        && kb[0].fixed::<32>()? == c
                        && kb[1].fixed::<32>()? == p.id()
                        && kb[2].fixed::<32>()? == accept.id(),
                    "existing proof chain/origin",
                )?;
                Ok(Self {
                    raw: raw.to_vec(),
                    origin,
                    c,
                    certs: consent.certs,
                    consent,
                    cuts: [pb[6].uint()?, ab[2].uint()?],
                    offer_id: None,
                    grant_id: None,
                    native_a: None,
                    otk: None,
                    controls: vec![p, accept, k],
                    d,
                    e,
                })
            }
            _ => Err(Error::Invalid("proof origin")),
        }
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn q(&self) -> Id {
        hash(MODE, &self.raw)
    }
    pub fn c(&self) -> Id {
        self.c
    }
    pub fn certs(&self) -> [Id; 2] {
        self.certs
    }
    pub fn d(&self) -> Id {
        self.d
    }
    pub fn e(&self) -> Id {
        self.e
    }
    pub fn cuts(&self) -> [u64; 2] {
        self.cuts
    }
    pub fn offer_id(&self) -> Option<Id> {
        self.offer_id
    }
    pub fn grant_id(&self) -> Option<Id> {
        self.grant_id
    }
    pub fn prekey_selection(&self) -> Option<(Id, Id)> {
        self.native_a.zip(self.otk)
    }
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    pub fn verify(self, evidence: &SelectionEvidence2) -> Result<VerifiedModeProof2> {
        verify_membership(self.c, &evidence.peers, &evidence.membership)?;
        self.consent.verify(&evidence.peers)?;
        let mut initial_auth = None;
        match &self.origin {
            Origin::Fresh => {
                for (control, peer) in self.controls.iter().zip(&evidence.peers) {
                    control.verify(peer.cert.device_id())?;
                }
                let pair = evidence
                    .initial_pair
                    .as_deref()
                    .ok_or(Error::Invalid("initial pair required"))?;
                let (t, refs) = verify_initial_pair2(
                    self.c,
                    &self.consent,
                    pair,
                    &evidence.peers,
                    &evidence.membership,
                )?;
                initial_auth = Some(refs);
                require(
                    self.controls[0].body.fields(8)?[3].fixed::<32>()? == t,
                    "selected initial transcript",
                )?;
            }
            Origin::Existing { d, e, sid } => {
                for (i, control) in self.controls.iter().enumerate() {
                    control.verify(evidence.peers[i % 2].cert.device_id())?;
                }
                let old = evidence
                    .previous
                    .as_ref()
                    .ok_or(Error::Invalid("old Ready/genesis required"))?;
                for role in 0..2 {
                    let ev = crate::ordered_dialog_v1::Event::parse(
                        &old.genesis[role],
                        evidence.peers[role].cert.device_id(),
                    )?;
                    require(
                        ev.core.c == self.c
                            && ev.core.d == *d
                            && ev.core.e == *e
                            && ev.core.sid == *sid
                            && ev.core.n == 0
                            && ev.core.role == role as u64
                            && ev.core.kind == role as u64 + 1,
                        "old genesis scope",
                    )?;
                    let body = crate::ordered_dialog_v1::control(
                        &old.ready[role],
                        3,
                        evidence.peers[role].cert.device_id(),
                    )?;
                    require(
                        body == a([
                            b(d),
                            b(e),
                            b(sid.as_bytes()),
                            b(hash(crate::ordered_dialog_v1::EVENT_ID, &old.genesis[0])),
                            b(hash(crate::ordered_dialog_v1::EVENT_ID, &old.genesis[1])),
                            u(role as u64),
                            u(1),
                            u(1),
                        ]),
                        "old Ready pair",
                    )?;
                }
            }
        }
        Ok(VerifiedModeProof2(self, initial_auth))
    }
}
impl ParsedControl2 {
    pub fn verify_ready(
        self,
        selection: &StaticSelection2,
        role: u8,
        genesis: [Id; 2],
        signer: &RootVerifiedCredential,
    ) -> Result<VerifiedReady2> {
        self.require_kind(3)?;
        require(role <= 1, "Ready role")?;
        let sid = selection
            .sid
            .as_deref()
            .ok_or(Error::Invalid("Ready before selected SID"))?;
        require(
            signer.cert_ref() == selection.proof.certs()[usize::from(role)]
                && self.body
                    == a([
                        b(selection.d),
                        b(selection.e),
                        b(sid.as_bytes()),
                        b(genesis[0]),
                        b(genesis[1]),
                        u(u64::from(role)),
                        u(1),
                        u(2),
                        b(selection.proof.q()),
                    ]),
            "Ready selected Q/genesis/scope",
        )?;
        self.verify(signer.cert.device_id())?;
        Ok(VerifiedReady2(self))
    }
    pub fn verify_committed(
        &self,
        selection: &StaticSelection2,
        signer: &RootVerifiedCredential,
    ) -> Result<()> {
        self.require_kind(21)?;
        require(
            signer.cert_ref() == selection.proof.certs()[1]
                && self.body == a([b(selection.c), b(selection.proof.q())]),
            "Committed Q/signer",
        )?;
        self.verify(signer.cert.device_id())
    }
}
pub struct VerifiedReady2(ParsedControl2);
impl VerifiedReady2 {
    pub fn control(&self) -> &ParsedControl2 {
        &self.0
    }
}
pub struct PreviousSession2 {
    pub genesis: [Vec<u8>; 2],
    pub ready: [Vec<u8>; 2],
}
pub struct SelectionEvidence2 {
    pub peers: [RootVerifiedCredential; 2],
    pub membership: Vec<u8>,
    pub initial_pair: Option<Vec<u8>>,
    pub previous: Option<PreviousSession2>,
}
#[derive(Clone)]
pub struct VerifiedModeProof2(pub(super) ParsedModeProof2, pub(super) Option<[Id; 2]>);
impl VerifiedModeProof2 {
    pub fn q(&self) -> Id {
        self.0.q()
    }
    pub fn certs(&self) -> [Id; 2] {
        self.0.certs
    }
    pub fn proof(&self) -> &ParsedModeProof2 {
        &self.0
    }
    pub fn format_for(&self, role: u8, n: u64) -> Result<u8> {
        require(role <= 1, "role")?;
        Ok(if n < self.0.cuts[usize::from(role)] {
            1
        } else {
            2
        })
    }
}
#[derive(Clone)]
pub struct StaticSelection2 {
    pub(super) c: Id,
    pub(super) d: Id,
    pub(super) e: Id,
    pub(super) sid: Option<String>,
    pub(super) proof: VerifiedModeProof2,
}
impl StaticSelection2 {
    pub fn c(&self) -> Id {
        self.c
    }
    pub fn d(&self) -> Id {
        self.d
    }
    pub fn e(&self) -> Id {
        self.e
    }
    pub fn sid(&self) -> Option<&str> {
        self.sid.as_deref()
    }
    pub fn proof(&self) -> &VerifiedModeProof2 {
        &self.proof
    }
    pub fn new(proof: VerifiedModeProof2, sid: Option<String>) -> Result<Self> {
        if let Some(s) = &sid {
            require(s.len() == 43 && s.is_ascii(), "selection SID")?;
        }
        if let Origin::Existing { sid: old, .. } = &proof.0.origin {
            require(sid.as_ref() == Some(old), "existing selected SID")?;
        }
        Ok(Self {
            c: proof.0.c,
            d: proof.0.d,
            e: proof.0.e,
            sid,
            proof,
        })
    }
}
pub fn initial_transcript2(binding: Id, hello_a: &[u8], hello_b: &[u8]) -> Result<Id> {
    Ok(hash(
        AUTH_PAIR,
        &encode(&a([b(binding), b(hello_a), b(hello_b)]), 192 * 1024)?,
    ))
}
/// Validate the selected historical pair before a runtime allocates its sole native account/OTK.
pub fn verify_initial_pair2(
    c: Id,
    consent: &ParsedConsent2,
    raw: &[u8],
    peers: &[RootVerifiedCredential; 2],
    membership: &[u8],
) -> Result<(Id, [Id; 2])> {
    let v = decode(raw, 192 * 1024)?;
    let f = v.fields(6)?;
    require(f[0].uint()? == 2, "pair version")?;
    let ha = ParsedFrame2::parse(f[2].bytes()?)?;
    let hb = ParsedFrame2::parse(f[3].bytes()?)?;
    let binding = ha.binding();
    require(binding == hb.binding(), "pair binding")?;
    let headers = [ha.body().value().clone(), hb.body().value().clone()];
    let mut refs = [[0; 32]; 2];
    for (frame, role) in [(ha, 0), (hb, 1)] {
        require(frame.body().kind() == FrameKind::Hello, "initial Hello")?;
        frame.verify(binding, peers[role].cert.device_id())?;
        let h = headers[role].fields(12)?;
        let cred = ParsedCredentialBundle::parse(h[7].bytes()?)?.verify_root()?;
        refs[role] = cred.reference();
        require(
            h[0].fixed::<32>()? == c
                && h[1].uint()? == role as u64
                && cred.cert_ref() == peers[role].cert_ref()
                && h[8].bytes()? == membership
                && h[9].fixed::<32>()? == consent.reference()
                && Origin::parse(&h[11])? == consent.origin,
            "selected Hello",
        )?;
    }
    let h0 = headers[0].fields(12)?;
    let h1 = headers[1].fields(12)?;
    require(h0[2] == h1[3] && h0[3] == h1[2], "endpoint pair")?;
    let t = initial_transcript2(binding, f[2].bytes()?, f[3].bytes()?)?;
    require(f[1].fixed::<32>()? == t, "initial transcript")?;
    for (raw, role) in [(f[4].bytes()?, 0), (f[5].bytes()?, 1)] {
        let proof = ParsedFrame2::parse(raw)?;
        require(
            proof.body().kind() == FrameKind::Proof
                && proof.body().value()
                    == &a([
                        b(t),
                        b(c),
                        u(1),
                        u(2),
                        b(consent.certs[0]),
                        b(consent.certs[1]),
                    ]),
            "initial Proof",
        )?;
        proof.verify(binding, peers[role].cert.device_id())?;
    }
    Ok((t, refs))
}
/// Peer hint only. It never installs a selected proof or changes local policy.
pub struct ModeView2 {
    pub stage: u8,
    pub q: Option<Id>,
    pub cuts: [Option<u64>; 2],
}
impl ModeView2 {
    pub fn parse(v: &Value) -> Result<Self> {
        let f = v.fields(3)?;
        let cuts = f[2].fields(2)?;
        require(f[0].uint()? <= 5, "mode stage")?;
        let optional =
            |x: &Value| -> Result<Option<u64>> { Ok(x.option()?.map(Value::uint).transpose()?) };
        Ok(Self {
            stage: f[0].uint()? as u8,
            q: optional_id(&f[1])?,
            cuts: [optional(&cuts[0])?, optional(&cuts[1])?],
        })
    }
    pub fn check_selected(&self, proof: &VerifiedModeProof2) -> Result<()> {
        require(
            self.q == Some(proof.q()) && self.cuts == proof.0.cuts.map(Some),
            "mode Q/cuts",
        )
    }
}
