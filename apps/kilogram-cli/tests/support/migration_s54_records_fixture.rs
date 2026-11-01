//! Signed public fixtures only. Header parsing is real; no Native constructor/packet crypto.
use super::*;
use kilogram_identity::DeviceIdentity;
use kilogram_protocol::ordered_dialog_v2 as wire;
use kilogram_ratchet::ordered::Packet;
use records::{Credentials, Expected, Projection, Record, verify};
#[allow(dead_code)]
mod support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/kilogram-protocol/tests/support/ordered_dialog_v2.rs"
    ));
}
pub(super) use support::TestResult;
pub(super) struct Fixture {
    pub actors: support::Actors,
    pub selected: wire::StaticSelection2,
    pub contexts: [old::Id; 2],
    pub index: u8,
    pub normal: Packet,
    pub genesis: [Vec<u8>; 2],
}
pub(super) struct Case {
    pub expected: oracle::Request,
    pub core: wire::Core2,
    pub raw: Vec<u8>,
    pub receipt: Vec<u8>,
    pub id: old::Id,
    pub body: String,
    pub author: wire::RootVerifiedCredential,
    pub receiver: wire::RootVerifiedCredential,
}
fn legacy(core: &wire::Core2, packet: &Packet, signer: &DeviceIdentity) -> TestResult<Vec<u8>> {
    let h = packet.header()?;
    let mut event = old::Event {
        core: old::Core {
            c: core.c,
            d: core.d,
            e: core.e,
            sid: core.sid.clone(),
            role: u64::from(core.role),
            n: core.n,
            previous: core.previous,
            sequence: core.sequence,
            kind: u64::from(core.kind),
            request: core.request,
            parents: core.parents.clone(),
            expected: old::Expected {
                g: core.expected.g,
                index: core.expected.index,
                parent: None,
                tail: None,
            },
            author_auth: core.author_auth,
        },
        native_kind: u64::from(packet.kind),
        key: h.key,
        index: h.index,
        ciphertext: packet.bytes.clone(),
        signature: vec![],
    };
    event.sign(signer)?;
    Ok(event.raw()?)
}
impl Fixture {
    pub fn new(index: u8) -> TestResult<Self> {
        let contexts = [[41; 32], [42; 32]];
        let actors = support::Actors::new(contexts[index as usize])?;
        // vodozemac0.10 Message codec vector: valid public protobuf/header+opaque MAC.
        // It supplies syntax, never a claim that projection matches encrypted payload.
        let normal = Packet {
            kind: 1,
            bytes: b"\x03\n\x20ratchetkeyhereprettyplease123456\x10\x02\"\nciphertextMACHEREE"
                .to_vec(),
        };
        let mut bytes = vec![3];
        for (tag, key) in [(0x0a, [1; 32]), (0x12, [2; 32]), (0x1a, [3; 32])] {
            bytes.extend([tag, 32]);
            bytes.extend(key);
        }
        bytes.extend([0x22, u8::try_from(normal.bytes.len())?]);
        bytes.extend(&normal.bytes);
        let prekey = Packet { kind: 0, bytes };
        let sid = prekey
            .header()?
            .prekey_sid
            .ok_or("actual prekey header SID")?;
        let d = [51 + index; 32];
        let e = [61 + index; 32];
        let mut genesis = Vec::new();
        for role in 0..2 {
            let packet = if role == 0 { &prekey } else { &normal };
            let core = wire::Core2 {
                c: actors.c,
                d,
                e,
                sid: sid.clone(),
                role: role as u8,
                n: 0,
                previous: None,
                sequence: 0,
                kind: role as u8 + 1,
                request: [90; 16],
                parents: vec![],
                expected: wire::Expected {
                    g: 0,
                    index: packet.header()?.index,
                    parent: None,
                    tail: None,
                },
                author_auth: actors.credentials[role].reference(),
                q: [0; 32],
            };
            genesis.push(legacy(&core, packet, &actors.devices[role])?);
        }
        let genesis: [Vec<u8>; 2] = genesis.try_into().map_err(|_| "genesis pair")?;
        let cuts = if index == 0 { [25, 25] } else { [33, 33] };
        let selected = actors.existing(d, e, &sid, genesis.clone(), cuts, [[81; 32], [82; 32]])?;
        let selected = wire::StaticSelection2::new(selected.proof, Some(sid))?;
        Ok(Self {
            actors,
            selected,
            contexts,
            index,
            normal,
            genesis,
        })
    }
    pub fn core(&self, role: u8, n: u16) -> TestResult<wire::Core2> {
        let request = oracle::Request::new(self.index, role, n)?;
        Ok(wire::Core2 {
            c: self.selected.c(),
            d: self.selected.d(),
            e: self.selected.e(),
            sid: self.selected.sid().ok_or("selected SID")?.into(),
            role,
            n: u64::from(n),
            previous: Some([80; 32]),
            sequence: u64::from(n),
            kind: 3,
            request: source::parse_request_id(&source::request_id(request))?,
            parents: if n > 1 { vec![[80; 32]] } else { vec![] },
            expected: wire::Expected {
                g: 1,
                index: self.normal.header()?.index,
                parent: None,
                tail: None,
            },
            author_auth: self.actors.credentials[role as usize].reference(),
            q: self.selected.proof().q(),
        })
    }
    pub fn receipt(
        &self,
        core: &wire::Core2,
        format: u8,
        id: old::Id,
        encoding: u8,
        receiver: &wire::RootVerifiedCredential,
    ) -> TestResult<Vec<u8>> {
        let signer = &self.actors.devices[usize::from(1 - core.role)];
        if encoding == 1 {
            let body = old::a([
                old::u(1),
                old::b(self.selected.d()),
                old::b(self.selected.e()),
                old::u(u64::from(core.role)),
                old::u(core.n),
                old::b(id),
                old::b(receiver.cert_ref()),
                old::b(receiver.reference()),
            ]);
            let sig = signer.sign(&old::signing_bytes(
                old::RECEIPT_SIGN,
                &body,
                wire::MAX_RECEIPT,
            )?);
            Ok(old::encode(
                &old::a([body, old::b(sig)]),
                wire::MAX_RECEIPT,
            )?)
        } else {
            Ok(wire::ParsedReceipt2::sign(
                wire::ReceiptFields2 {
                    c: self.selected.c(),
                    d: self.selected.d(),
                    e: self.selected.e(),
                    sid: self.selected.sid().ok_or("SID")?.into(),
                    q: self.selected.proof().q(),
                    format,
                    lane: core.role,
                    n: core.n,
                    event: id,
                    receiver_cert: receiver.cert_ref(),
                    receiver_auth: receiver.reference(),
                },
                signer,
            )?
            .raw()
            .to_vec())
        }
    }
    pub fn record(
        &self,
        expected: oracle::Request,
        core: wire::Core2,
        format: u8,
        encoding: u8,
        packet: &Packet,
    ) -> TestResult<Case> {
        let author = self.actors.credentials[core.role as usize].clone();
        let receiver = self.actors.credentials[usize::from(1 - core.role)].clone();
        let raw = if format == 1 {
            legacy(&core, packet, &self.actors.devices[core.role as usize])?
        } else {
            wire::sign_event(
                core.clone(),
                packet,
                &self.actors.devices[core.role as usize],
            )?
            .raw()
            .to_vec()
        };
        let id = old::hash(
            if format == 1 {
                old::EVENT_ID
            } else {
                wire::EVENT_ID
            },
            &raw,
        );
        let receipt = self.receipt(&core, format, id, encoding, &receiver)?;
        Ok(Case {
            expected,
            core,
            raw,
            receipt,
            id,
            body: expected.body(),
            author,
            receiver,
        })
    }
    pub fn case(&self, role: u8, n: u16, encoding: u8) -> TestResult<Case> {
        let expected = oracle::Request::new(self.index, role, n)?;
        self.record(
            expected,
            self.core(role, n)?,
            if self.index == 0 && n >= 25 { 2 } else { 1 },
            encoding,
            &self.normal,
        )
    }
    pub fn expected<'a>(&'a self, case: &Case) -> Expected<'a> {
        Expected {
            contexts: &self.contexts,
            request: case.expected,
            event: case.id,
            d: self.selected.d(),
            e: self.selected.e(),
            sid: self.selected.sid().unwrap_or_default(),
            selection: self.selected.proof().q(),
        }
    }
    pub fn projection<'a>(&self, case: &'a Case) -> Result<Projection<'a>> {
        Ok(Projection {
            event: case.id,
            request: source::parse_request_id(&source::request_id(case.expected))?,
            text: &case.body,
        })
    }
    pub fn check(&self, case: &Case) -> Result<Record> {
        verify(
            &self.expected(case),
            &self.projection(case)?,
            &self.selected,
            Credentials {
                author: &case.author,
                receiver: &case.receiver,
            },
            &case.raw,
            &case.receipt,
        )
    }
}
