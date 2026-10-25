use super::*;
use kilogram_identity::{
    AccountAuthoritySnapshot, ConversationMembershipSnapshot, DeviceCapability, DeviceCertificate,
    verify_device_authorization_with_snapshot,
};

#[derive(Clone)]
pub struct ParsedCredentialBundle {
    raw: Vec<u8>,
    cert: Vec<u8>,
    authority: Vec<u8>,
}
impl ParsedCredentialBundle {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::Auth)?;
        let f = v.fields(3)?;
        require(f[0].uint()? == 1, "credential version")?;
        Ok(Self {
            raw: raw.to_vec(),
            cert: f[1].bytes()?.to_vec(),
            authority: f[2].bytes()?.to_vec(),
        })
    }
    pub fn reference(&self) -> Id {
        hash(b"", &self.raw)
    }
    pub fn cert_ref(&self) -> Id {
        hash(b"", &self.cert)
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn certificate_raw(&self) -> &[u8] {
        &self.cert
    }
    pub fn authority_raw(&self) -> &[u8] {
        &self.authority
    }
    pub fn verify_root(self) -> Result<RootVerifiedCredential> {
        let cert = DeviceCertificate::decode_and_verify(&self.cert)?;
        let authority = AccountAuthoritySnapshot::decode_and_verify(&self.authority)?;
        require(
            cert.encode()? == self.cert && authority.encode()? == self.authority,
            "exact identity encoding",
        )?;
        verify_device_authorization_with_snapshot(
            cert.account_id(),
            &cert,
            &authority,
            &DeviceCapability::MESSAGING,
        )?;
        Ok(RootVerifiedCredential {
            parsed: self,
            cert,
            authority,
        })
    }
}
#[derive(Clone)]
pub struct RootVerifiedCredential {
    parsed: ParsedCredentialBundle,
    pub(crate) cert: DeviceCertificate,
    pub(crate) authority: AccountAuthoritySnapshot,
}
impl RootVerifiedCredential {
    pub fn from_parts(
        cert: DeviceCertificate,
        authority: AccountAuthoritySnapshot,
    ) -> Result<Self> {
        ParsedCredentialBundle::parse(&encode(
            &a([u(1), b(cert.encode()?), b(authority.encode()?)]),
            MAX_AUTH,
        )?)?
        .verify_root()
    }
    pub fn reference(&self) -> Id {
        self.parsed.reference()
    }
    pub fn cert_ref(&self) -> Id {
        self.parsed.cert_ref()
    }
    pub fn raw(&self) -> &[u8] {
        self.parsed.raw()
    }
    pub fn certificate(&self) -> &DeviceCertificate {
        &self.cert
    }
    pub fn authority(&self) -> &AccountAuthoritySnapshot {
        &self.authority
    }
}
pub fn verify_membership(c: Id, peers: &[RootVerifiedCredential; 2], raw: &[u8]) -> Result<()> {
    WireLimit2::Membership.check_len(raw.len())?;
    let m = ConversationMembershipSnapshot::decode_and_verify(raw)?;
    require(
        m.encode()? == raw
            && m.conversation_id().as_bytes() == &c
            && m.owner_account_id() == peers[0].cert.account_id()
            && m.members().len() == 2
            && peers[0].cert.device_id().as_bytes() < peers[1].cert.device_id().as_bytes()
            && peers[0].cert.account_id() != peers[1].cert.account_id(),
        "membership/role scope",
    )?;
    for p in peers {
        m.require_member(p.cert.account_id())?;
    }
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Origin {
    Fresh,
    Existing { d: Id, e: Id, sid: String },
}
impl Origin {
    pub fn value(&self) -> Value {
        match self {
            Self::Fresh => a([u(0)]),
            Self::Existing { d, e, sid } => a([u(1), b(d), b(e), b(sid.as_bytes())]),
        }
    }
    pub fn parse(v: &Value) -> Result<Self> {
        let f = v.list()?;
        match f.first().ok_or(Error::Invalid("origin"))?.uint()? {
            0 => {
                v.fields(1)?;
                Ok(Self::Fresh)
            }
            1 => {
                let f = v.fields(4)?;
                require(
                    f[3].bytes()?.len() == 43 && f[3].bytes()?.is_ascii(),
                    "origin SID",
                )?;
                Ok(Self::Existing {
                    d: f[1].fixed()?,
                    e: f[2].fixed()?,
                    sid: f[3].text()?.into(),
                })
            }
            _ => Err(Error::Invalid("origin tag")),
        }
    }
}
#[derive(Clone)]
pub struct ParsedConsent2 {
    raw: Vec<u8>,
    pub(super) c: Id,
    pub(super) certs: [Id; 2],
    pub(super) origin: Origin,
    not_before: u64,
    expires: u64,
    body: Value,
    sigs: [[u8; 64]; 2],
}
impl ParsedConsent2 {
    pub fn c(&self) -> Id {
        self.c
    }
    pub fn certs(&self) -> [Id; 2] {
        self.certs
    }
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    pub fn interval(&self) -> (u64, u64) {
        (self.not_before, self.expires)
    }
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::Consent)?;
        let f = v.fields(3)?;
        let t = f[0].fields(10)?;
        require(
            t[0].uint()? == 2 && t[4].uint()? == 1 && t[5].uint()? == 2 && t[7].uint()? == 1,
            "consent version/suite/limits/permission",
        )?;
        let not_before = t[8].uint()?;
        let expires = t[9].uint()?;
        require(not_before < expires, "consent interval")?;
        Ok(Self {
            raw: raw.to_vec(),
            c: t[1].fixed()?,
            certs: [t[2].fixed()?, t[3].fixed()?],
            origin: Origin::parse(&t[6])?,
            not_before,
            expires,
            body: f[0].clone(),
            sigs: [f[1].fixed()?, f[2].fixed()?],
        })
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn reference(&self) -> Id {
        hash(CONSENT, &self.raw)
    }
    pub fn verify(&self, peers: &[RootVerifiedCredential; 2]) -> Result<()> {
        require(
            self.certs == [peers[0].cert_ref(), peers[1].cert_ref()],
            "consent certificates",
        )?;
        let msg = signing_bytes(CONSENT, &self.body, 4096)?;
        for (p, sig) in peers.iter().zip(&self.sigs) {
            let key = ed25519_dalek::VerifyingKey::from_bytes(p.cert.account_id().as_bytes())
                .map_err(|_| Error::Invalid("Root key"))?;
            key.verify_strict(&msg, &ed25519_dalek::Signature::from_bytes(sig))
                .map_err(|_| Error::Invalid("consent Root signature"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptFields2 {
    pub c: Id,
    pub d: Id,
    pub e: Id,
    pub sid: String,
    pub q: Id,
    pub format: u8,
    pub lane: u8,
    pub n: u64,
    pub event: Id,
    pub receiver_cert: Id,
    pub receiver_auth: Id,
}
impl ReceiptFields2 {
    pub fn value(&self) -> Value {
        a([
            u(2),
            b(self.c),
            b(self.d),
            b(self.e),
            b(self.sid.as_bytes()),
            b(self.q),
            u(u64::from(self.format)),
            u(u64::from(self.lane)),
            u(self.n),
            b(self.event),
            b(self.receiver_cert),
            b(self.receiver_auth),
        ])
    }
}
#[derive(Clone)]
pub struct ParsedReceipt2 {
    raw: Vec<u8>,
    fields: ReceiptFields2,
    sig: [u8; 64],
}
impl ParsedReceipt2 {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::Receipt)?;
        let f = v.fields(2)?;
        let t = f[0].fields(12)?;
        require(
            t[0].uint()? == 2
                && (1..=2).contains(&t[6].uint()?)
                && t[7].uint()? <= 1
                && t[8].uint()? > 0,
            "receipt fields",
        )?;
        require(
            t[4].bytes()?.len() == 43 && t[4].bytes()?.is_ascii(),
            "receipt SID",
        )?;
        Ok(Self {
            raw: raw.to_vec(),
            fields: ReceiptFields2 {
                c: t[1].fixed()?,
                d: t[2].fixed()?,
                e: t[3].fixed()?,
                sid: t[4].text()?.into(),
                q: t[5].fixed()?,
                format: t[6].uint()? as u8,
                lane: t[7].uint()? as u8,
                n: t[8].uint()?,
                event: t[9].fixed()?,
                receiver_cert: t[10].fixed()?,
                receiver_auth: t[11].fixed()?,
            },
            sig: f[1].fixed()?,
        })
    }
    pub fn sign(fields: ReceiptFields2, signer: &DeviceIdentity) -> Result<Self> {
        let sig = signer.sign(&signing_bytes(RECEIPT_SIGN, &fields.value(), MAX_RECEIPT)?);
        Self::parse(&encode(&a([fields.value(), b(sig)]), MAX_RECEIPT)?)
    }
    pub fn fields(&self) -> &ReceiptFields2 {
        &self.fields
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn id(&self) -> Id {
        hash(RECEIPT_ID, &self.raw)
    }
    pub fn verify(
        self,
        source: &VerifiedEvent2,
        selection: &StaticSelection2,
        receiver: &RootVerifiedCredential,
    ) -> Result<VerifiedReceipt2> {
        self.verify_mixed(
            &MixedEvent::Current(VerifiedEvent2(source.event().clone())),
            selection,
            receiver,
        )
    }
    pub fn verify_mixed(
        self,
        source: &MixedEvent,
        selection: &StaticSelection2,
        receiver: &RootVerifiedCredential,
    ) -> Result<VerifiedReceipt2> {
        let source = receipt_source(source, selection)?;
        let f = &self.fields;
        require(
            (f.c, f.d, f.e, f.q, f.lane, f.n, f.event)
                == (
                    selection.c,
                    selection.d,
                    selection.e,
                    selection.proof.q(),
                    source.role,
                    source.n,
                    source.id,
                )
                && f.sid == source.sid
                && f.format == source.format,
            "receipt source correspondence",
        )?;
        require(
            f.receiver_cert == selection.proof.certs()[usize::from(1 - source.role)]
                && receiver.cert_ref() == f.receiver_cert
                && receiver.reference() == f.receiver_auth,
            "opposite receipt credential",
        )?;
        receiver.cert.device_id().verify(
            &signing_bytes(RECEIPT_SIGN, &f.value(), MAX_RECEIPT)?,
            &self.sig,
        )?;
        Ok(VerifiedReceipt2(self))
    }
}
struct ReceiptSource<'a> {
    format: u8,
    role: u8,
    n: u64,
    id: Id,
    sid: &'a str,
}
fn receipt_source<'a>(
    source: &'a MixedEvent,
    selection: &StaticSelection2,
) -> Result<ReceiptSource<'a>> {
    let (format, c, d, e, sid, role, n, kind, id, q) = match source {
        MixedEvent::Legacy(verified) => {
            let event = verified.event();
            let c = &event.core;
            (
                1,
                c.c,
                c.d,
                c.e,
                c.sid.as_str(),
                c.role as u8,
                c.n,
                c.kind as u8,
                event.id()?,
                None,
            )
        }
        MixedEvent::Current(event) => {
            let c = event.event().core();
            (
                2,
                c.c,
                c.d,
                c.e,
                c.sid.as_str(),
                c.role,
                c.n,
                c.kind,
                event.event().id(),
                Some(c.q),
            )
        }
    };
    require(
        kind == 3
            && n > 0
            && c == selection.c
            && d == selection.d
            && e == selection.e
            && selection.sid.as_deref() == Some(sid)
            && selection.proof.format_for(role, n)? == format
            && q.is_none_or(|q| q == selection.proof.q()),
        "receipt selected source",
    )?;
    Ok(ReceiptSource {
        format,
        role,
        n,
        id,
        sid,
    })
}
/// Exact legacy receipt decoder, used only by explicit receipt_format=1 dispatch.
#[derive(Clone)]
pub struct ParsedReceipt1 {
    raw: Vec<u8>,
    body: Value,
    signature: [u8; 64],
}
impl ParsedReceipt1 {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode(raw, MAX_RECEIPT)?;
        let f = v.fields(2)?;
        let t = f[0].fields(8)?;
        require(
            t[0].uint()? == 1 && t[3].uint()? <= 1 && t[4].uint()? > 0,
            "legacy receipt fields",
        )?;
        for i in [1, 2, 5, 6, 7] {
            t[i].fixed::<32>()?;
        }
        Ok(Self {
            raw: raw.to_vec(),
            body: f[0].clone(),
            signature: f[1].fixed()?,
        })
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn id(&self) -> Id {
        hash(crate::ordered_dialog_v1::RECEIPT_ID, &self.raw)
    }
    pub fn verify(
        self,
        source: &MixedEvent,
        selection: &StaticSelection2,
        receiver: &RootVerifiedCredential,
    ) -> Result<VerifiedReceipt1> {
        let source = receipt_source(source, selection)?;
        let t = self.body.fields(8)?;
        require(
            source.format == 1
                && self.body
                    == a([
                        u(1),
                        b(selection.d),
                        b(selection.e),
                        u(u64::from(source.role)),
                        u(source.n),
                        b(source.id),
                        b(receiver.cert_ref()),
                        b(receiver.reference()),
                    ])
                && selection.proof.certs()[usize::from(1 - source.role)] == receiver.cert_ref(),
            "legacy receipt source/receiver",
        )?;
        // Widths and source scope are checked before signature verification.
        t[7].fixed::<32>()?;
        receiver.cert.device_id().verify(
            &signing_bytes(
                crate::ordered_dialog_v1::RECEIPT_SIGN,
                &self.body,
                MAX_RECEIPT,
            )?,
            &self.signature,
        )?;
        Ok(VerifiedReceipt1(self))
    }
}
pub struct VerifiedReceipt1(ParsedReceipt1);
impl VerifiedReceipt1 {
    pub fn receipt(&self) -> &ParsedReceipt1 {
        &self.0
    }
}
pub enum MixedReceipt {
    Legacy(VerifiedReceipt1),
    Current(Box<VerifiedReceipt2>),
}
impl TaggedItem {
    pub fn verify_receipt(
        &self,
        source: &MixedEvent,
        selection: &StaticSelection2,
        receiver: &RootVerifiedCredential,
    ) -> Result<MixedReceipt> {
        match self.format {
            1 => Ok(MixedReceipt::Legacy(
                ParsedReceipt1::parse(&self.raw)?.verify(source, selection, receiver)?,
            )),
            2 => Ok(MixedReceipt::Current(Box::new(
                ParsedReceipt2::parse(&self.raw)?.verify_mixed(source, selection, receiver)?,
            ))),
            _ => Err(Error::Invalid("receipt format tag")),
        }
    }
}
pub struct VerifiedReceipt2(ParsedReceipt2);
impl VerifiedReceipt2 {
    pub fn receipt(&self) -> &ParsedReceipt2 {
        &self.0
    }
}

pub struct ParsedDescriptor2 {
    raw: Vec<u8>,
    body: Value,
    c: Id,
    cert: Id,
    peer_cert: Id,
    endpoint: Id,
}
impl ParsedDescriptor2 {
    pub fn c(&self) -> Id {
        self.c
    }
    pub fn cert_ref(&self) -> Id {
        self.cert
    }
    pub fn peer_cert_ref(&self) -> Id {
        self.peer_cert
    }
    pub fn endpoint(&self) -> Id {
        self.endpoint
    }
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::Descriptor)?;
        let f = v.fields(2)?;
        let t = f[0].fields(9)?;
        f[1].fixed::<64>()?;
        require(
            t[0].uint()? == 2 && t[6].uint()? > 0 && t[7].uint()? < t[8].uint()?,
            "descriptor fields",
        )?;
        let addresses = t[5].list()?;
        require(!addresses.is_empty() && addresses.len() <= 16, "addresses")?;
        let mut prior: Option<Vec<u8>> = None;
        for addr in addresses {
            let a = addr.fields(3)?;
            let len = match a[0].uint()? {
                4 => 4,
                6 => 16,
                _ => return Err(Error::Invalid("IP family")),
            };
            require(
                a[1].bytes()?.len() == len && (1..=65535).contains(&a[2].uint()?),
                "IP/port",
            )?;
            let bytes = encode(addr, 64)?;
            require(
                prior.as_ref().is_none_or(|p| p < &bytes),
                "sorted addresses",
            )?;
            prior = Some(bytes);
        }
        Ok(Self {
            raw: raw.to_vec(),
            body: f[0].clone(),
            c: t[1].fixed()?,
            cert: t[2].fixed()?,
            peer_cert: t[3].fixed()?,
            endpoint: t[4].fixed()?,
        })
    }
    pub fn verify(
        &self,
        c: Id,
        own: &RootVerifiedCredential,
        peer: &RootVerifiedCredential,
    ) -> Result<()> {
        require(
            self.c == c && self.cert == own.cert_ref() && self.peer_cert == peer.cert_ref(),
            "descriptor scope",
        )?;
        let v = decode(&self.raw, 8192)?;
        own.cert.device_id().verify(
            &signing_bytes(DESCRIPTOR, &self.body, 8192)?,
            v.fields(2)?[1].bytes()?,
        )?;
        Ok(())
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn id(&self) -> Id {
        hash(DESCRIPTOR, &self.raw)
    }
}
