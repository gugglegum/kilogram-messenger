use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FrameKind {
    Hello = 1,
    Proof,
    Admitted,
    OpenIntent,
    Offer,
    Grant,
    EpochCipher,
    Ready,
    Resume,
    Deliver,
    PrefixReq,
    PrefixResp,
    Complete,
    Error,
    AckBatch,
    AckQuery,
    Head,
    Propose,
    Accept,
    Select,
    Committed,
}
impl TryFrom<u64> for FrameKind {
    type Error = Error;
    fn try_from(n: u64) -> Result<Self> {
        Ok(match n {
            1 => Self::Hello,
            2 => Self::Proof,
            3 => Self::Admitted,
            4 => Self::OpenIntent,
            5 => Self::Offer,
            6 => Self::Grant,
            7 => Self::EpochCipher,
            8 => Self::Ready,
            9 => Self::Resume,
            10 => Self::Deliver,
            11 => Self::PrefixReq,
            12 => Self::PrefixResp,
            13 => Self::Complete,
            14 => Self::Error,
            15 => Self::AckBatch,
            16 => Self::AckQuery,
            17 => Self::Head,
            18 => Self::Propose,
            19 => Self::Accept,
            20 => Self::Select,
            21 => Self::Committed,
            _ => return Err(Error::Invalid("Frame kind")),
        })
    }
}
/// A schema-checked body. Public signing accepts this type rather than arbitrary Value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameBody2 {
    kind: FrameKind,
    value: Value,
}
impl FrameBody2 {
    pub fn parse(kind: FrameKind, raw: &[u8]) -> Result<Self> {
        Self::from_value(kind, decode(raw, MAX_WIRE - 4)?)
    }
    pub(super) fn from_value(kind: FrameKind, value: Value) -> Result<Self> {
        validate_body(kind, &value)?;
        Ok(Self { kind, value })
    }
    pub fn kind(&self) -> FrameKind {
        self.kind
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn proof(t: Id, c: Id, certs: [Id; 2]) -> Result<Self> {
        Self::from_value(
            FrameKind::Proof,
            a([b(t), b(c), u(1), u(2), b(certs[0]), b(certs[1])]),
        )
    }
}
#[derive(Clone)]
pub struct ParsedFrame2 {
    raw: Vec<u8>,
    binding: Id,
    request: [u8; 16],
    body: FrameBody2,
    sig: [u8; 64],
}
impl ParsedFrame2 {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let v = decode2(raw, WireLimit2::Frame)?;
        let f = v.fields(6)?;
        require(f[0].uint()? == 2, "Frame version")?;
        Ok(Self {
            raw: raw.to_vec(),
            binding: f[2].fixed()?,
            request: f[3].fixed()?,
            body: FrameBody2::from_value(FrameKind::try_from(f[1].uint()?)?, f[4].clone())?,
            sig: f[5].fixed()?,
        })
    }
    fn unsigned(&self) -> Value {
        a([
            u(2),
            u(self.body.kind as u64),
            b(self.binding),
            b(self.request),
            self.body.value.clone(),
        ])
    }
    fn tag(kind: FrameKind) -> &'static [u8] {
        match kind {
            FrameKind::Hello => b"kg:od2:auth-hello\0",
            FrameKind::Proof => b"kg:od2:auth-proof\0",
            _ => CONTROL_SIGN,
        }
    }
    pub fn sign(
        body: FrameBody2,
        binding: Id,
        request: [u8; 16],
        signer: &DeviceIdentity,
    ) -> Result<Self> {
        let mut frame = Self {
            raw: vec![],
            binding,
            request,
            body,
            sig: [0; 64],
        };
        frame.sig = signer.sign(&signing_bytes(
            Self::tag(frame.body.kind),
            &frame.unsigned(),
            MAX_WIRE - 4,
        )?);
        let mut f = frame.unsigned().list()?.to_vec();
        f.push(b(frame.sig));
        Self::parse(&encode(&a(f), MAX_WIRE - 4)?)
    }
    pub fn verify(self, binding: Id, signer: DeviceId) -> Result<VerifiedFrame2> {
        require(self.binding == binding, "TLS binding")?;
        signer.verify(
            &signing_bytes(Self::tag(self.body.kind), &self.unsigned(), MAX_WIRE - 4)?,
            &self.sig,
        )?;
        Ok(VerifiedFrame2(self))
    }
    pub fn binding(&self) -> Id {
        self.binding
    }
    pub fn request(&self) -> [u8; 16] {
        self.request
    }
    pub fn body(&self) -> &FrameBody2 {
        &self.body
    }
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}
pub struct VerifiedFrame2(ParsedFrame2);
impl VerifiedFrame2 {
    pub fn frame(&self) -> &ParsedFrame2 {
        &self.0
    }
}
pub fn exporter_context(dialer: Id, listener: Id) -> Result<Vec<u8>> {
    Ok(encode(&a([b(ALPN), b(dialer), b(listener)]), 256)?)
}
pub fn exporter_hash(material: &[u8; 32]) -> Id {
    hash(b"kg:od2:channel-binding\0", material)
}
pub const EXPORTER_LABEL: &[u8] = b"kilogram ordered-dialog v2";

fn ids(v: &Value) -> Result<()> {
    let xs = v.list()?;
    require(xs.len() <= 32, "ID count")?;
    for x in xs {
        x.fixed::<32>()?;
    }
    Ok(())
}
fn slot_pairs(v: &Value) -> Result<()> {
    require(v.list()?.len() <= 32, "slots count")?;
    for x in v.list()? {
        let f = x.fields(2)?;
        f[0].uint()?;
        f[1].fixed::<32>()?;
    }
    Ok(())
}
pub fn validate_auth_records(v: &Value) -> Result<Vec<(Id, Vec<u8>)>> {
    require(v.list()?.len() <= 32, "auth records count")?;
    let mut out = vec![];
    let mut prev = None;
    for x in v.list()? {
        let f = x.fields(2)?;
        let id = f[0].fixed::<32>()?;
        let raw = f[1].bytes()?;
        require(
            raw.len() <= MAX_AUTH && hash(b"", raw) == id && prev.is_none_or(|p| p < id),
            "auth records hash/order",
        )?;
        ParsedCredentialBundle::parse(raw)?;
        prev = Some(id);
        out.push((id, raw.to_vec()));
    }
    Ok(out)
}
/// Check supplied records against the references extracted from the selected items.
/// Missing references may be resolved from the owner's authenticated durable catalog.
pub fn auth_records_for(v: &Value, referenced: &[Id]) -> Result<Vec<(Id, Vec<u8>)>> {
    require(referenced.len() <= 64, "referenced auth count")?;
    let records = validate_auth_records(v)?;
    require(
        records.iter().all(|(id, _)| referenced.contains(id)),
        "unused auth record",
    )?;
    Ok(records)
}
pub fn cursor(v: &Value) -> Result<()> {
    let f = v.fields(2)?;
    require(
        (f[0].uint()? == 0) == optional_id(&f[1])?.is_none(),
        "cursor",
    )
}
fn status(v: &Value) -> Result<()> {
    let f = v.fields(2)?;
    require(f[0].uint()? <= 7, "status reason")?;
    if let Some(p) = f[1].option()? {
        p.uint()?;
    }
    Ok(())
}
pub fn credit(v: &Value) -> Result<()> {
    let f = v.fields(7)?;
    require(
        f[0].uint()? <= 64
            && f[1].uint()? <= 4 * 1024 * 1024
            && f[2].uint()? <= 64 * 1024
            && f[3].uint()? <= 32
            && f[4].uint()? <= MAX_WIRE as u64,
        "credit bounds",
    )?;
    f[5].uint()?;
    if let Some(g) = f[6].option()? {
        let x = g.fields(3)?;
        x[0].fixed::<32>()?;
        require(x[1].uint()? < x[2].uint()?, "grant interval")?;
    }
    Ok(())
}
fn sid(v: &Value) -> Result<()> {
    require(v.bytes()?.len() == 43 && v.bytes()?.is_ascii(), "SID")
}
fn scope(f: &[Value]) -> Result<()> {
    for x in &f[..3] {
        x.fixed::<32>()?;
    }
    Ok(())
}
fn validate_body(k: FrameKind, v: &Value) -> Result<()> {
    use FrameKind::*;
    let count = match k {
        Hello => 12,
        Proof => 6,
        Admitted => 4,
        OpenIntent => 6,
        Offer | Grant => 3,
        EpochCipher => 5,
        Ready => 4,
        Resume => 10,
        Deliver => 7,
        PrefixReq => 9,
        PrefixResp => 11,
        Complete => 6,
        Error => 8,
        AckBatch => 8,
        AckQuery => 6,
        Head => 9,
        Propose => 2,
        Accept | Select | Committed => 1,
    };
    let f = v.fields(count)?;
    match k {
        Hello => {
            f[0].fixed::<32>()?;
            require(f[1].uint()? <= 1, "transport role")?;
            for x in &f[2..5] {
                x.fixed::<32>()?;
            }
            require(f[5] == a([u(1)]) && f[6] == a([u(2)]), "suite/limits")?;
            ParsedCredentialBundle::parse(f[7].bytes()?)?;
            WireLimit2::Membership.check_len(f[8].bytes()?.len())?;
            f[9].fixed::<32>()?;
            f[10].fixed::<32>()?;
            Origin::parse(&f[11])?;
        }
        Proof | Admitted => {
            f[0].fixed::<32>()?;
            f[1].fixed::<32>()?;
            require(f[2].uint()? == 1 && f[3].uint()? == 2, "negotiation")?;
            if k == Proof {
                f[4].fixed::<32>()?;
                f[5].fixed::<32>()?;
            }
        }
        OpenIntent => {
            f[0].fixed::<32>()?;
            require(f[1].uint()? <= 3, "phase")?;
            for x in &f[2..] {
                optional_id(x)?;
            }
        }
        Offer => {
            f[0].fixed::<32>()?;
            ParsedControl2::parse(f[1].bytes()?)?.require_kind(1)?;
            WireLimit2::InitialPair.check_len(f[2].bytes()?.len())?;
        }
        Grant => {
            f[0].fixed::<32>()?;
            f[1].fixed::<32>()?;
            ParsedControl2::parse(f[2].bytes()?)?.require_kind(2)?;
        }
        EpochCipher => {
            scope(f)?;
            let format = f[3].uint()?;
            require(
                (1..=2).contains(&format) && f[4].bytes()?.len() <= MAX_EVENT,
                "epoch item",
            )?;
        }
        Ready => {
            scope(f)?;
            ParsedControl2::parse(f[3].bytes()?)?.require_kind(3)?;
        }
        Resume => {
            scope(f)?;
            sid(&f[3])?;
            for x in f[4].fields(6)? {
                optional_id(x)?;
            }
            cursor(&f[5])?;
            cursor(&f[6])?;
            ModeView2::parse(&f[7])?;
            status(&f[8])?;
            credit(&f[9])?;
        }
        Deliver => {
            scope(f)?;
            sid(&f[3])?;
            require(f[4].uint()? <= 1, "lane")?;
            validate_auth_records(&f[5])?;
            tagged_items(&f[6], MAX_EVENT)?;
        }
        PrefixReq => {
            scope(f)?;
            sid(&f[3])?;
            require(
                f[4].uint()? <= 1 && f[5].uint()? < f[6].uint()?,
                "prefix range",
            )?;
            optional_id(&f[7])?;
            credit(&f[8])?;
            let cr = f[8].fields(7)?;
            if cr[0].uint()? == 0 {
                require(
                    cr[3].uint()? == 1
                        && cr[4].uint()? <= 68 * 1024
                        && f[5].uint()?.checked_add(1) == Some(f[6].uint()?),
                    "head only",
                )?;
            }
        }
        PrefixResp => {
            scope(f)?;
            sid(&f[3])?;
            require(
                f[4].uint()? <= 1 && f[5].uint()? <= f[6].uint()? && f[10].uint()? <= 1,
                "prefix response",
            )?;
            cursor(&f[7])?;
            validate_auth_records(&f[8])?;
            tagged_items(&f[9], MAX_EVENT)?;
        }
        Complete => {
            scope(f)?;
            sid(&f[3])?;
            f[4].fixed::<16>()?;
            ids(&f[5])?;
        }
        Error => {
            f[0].fixed::<32>()?;
            optional_id(&f[1])?;
            optional_id(&f[2])?;
            f[3].fixed::<16>()?;
            require(f[4].uint()? <= 2 && f[5].uint()? <= 7, "error enum")?;
            if let Some(x) = f[6].option()? {
                x.uint()?;
            }
            f[7].uint()?;
        }
        AckBatch => {
            scope(f)?;
            sid(&f[3])?;
            require(f[4].uint()? <= 1, "lane")?;
            validate_auth_records(&f[5])?;
            tagged_items(&f[6], MAX_RECEIPT)?;
            slot_pairs(&f[7])?;
        }
        AckQuery => {
            scope(f)?;
            sid(&f[3])?;
            require(f[4].uint()? <= 1, "lane")?;
            slot_pairs(&f[5])?;
        }
        Head => {
            scope(f)?;
            sid(&f[3])?;
            cursor(&f[4])?;
            cursor(&f[5])?;
            ModeView2::parse(&f[6])?;
            status(&f[7])?;
            credit(&f[8])?;
        }
        Propose => {
            ParsedControl2::parse(f[0].bytes()?)?.require_kind(18)?;
            ParsedConsent2::parse(f[1].bytes()?)?;
        }
        Accept | Select | Committed => {
            ParsedControl2::parse(f[0].bytes()?)?.require_kind(k as u8)?;
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct TaggedItem {
    pub format: u8,
    pub raw: Vec<u8>,
}
pub fn tagged_items(v: &Value, cap: usize) -> Result<Vec<TaggedItem>> {
    require(v.list()?.len() <= 32, "item count")?;
    v.list()?
        .iter()
        .map(|x| {
            let f = x.fields(2)?;
            let format = f[0].uint()?;
            require(
                (1..=2).contains(&format) && f[1].bytes()?.len() <= cap,
                "item format/cap",
            )?;
            Ok(TaggedItem {
                format: format as u8,
                raw: f[1].bytes()?.to_vec(),
            })
        })
        .collect()
}
pub struct VerifiedLegacyEvent(pub(super) crate::ordered_dialog_v1::Event);
impl VerifiedLegacyEvent {
    pub fn event(&self) -> &crate::ordered_dialog_v1::Event {
        &self.0
    }
}
pub enum MixedEvent {
    Legacy(VerifiedLegacyEvent),
    Current(VerifiedEvent2),
}
impl TaggedItem {
    pub fn verify_event(
        &self,
        selection: &StaticSelection2,
        author: &RootVerifiedCredential,
    ) -> Result<MixedEvent> {
        match self.format {
            1 => {
                let e = crate::ordered_dialog_v1::Event::parse(&self.raw, author.cert.device_id())?;
                require(
                    selection.proof.format_for(e.core.role as u8, e.core.n)? == 1
                        && e.core.c == selection.c
                        && e.core.d == selection.d
                        && e.core.e == selection.e
                        && selection.sid.as_ref() == Some(&e.core.sid)
                        && e.core.author_auth == author.reference()
                        && selection.proof.certs()[e.core.role as usize] == author.cert_ref(),
                    "legacy mode/scope/author",
                )?;
                Ok(MixedEvent::Legacy(VerifiedLegacyEvent(e)))
            }
            2 => Ok(MixedEvent::Current(
                ParsedEvent2::parse(&self.raw)?.verify(selection, author)?,
            )),
            _ => Err(Error::Invalid("format tag")),
        }
    }
}
