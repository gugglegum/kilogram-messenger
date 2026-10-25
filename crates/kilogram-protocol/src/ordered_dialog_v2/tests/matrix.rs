use super::*;
use crate::ordered_dialog_v1 as old;
use kilogram_ratchet::{
    ordered::{OrderedAccount, OrderedSession, Packet},
    ordered_v2::{Guard2, Seen, SeenObservation},
};
use std::collections::BTreeMap;

fn old_core(c: &Core2) -> old::Core {
    old::Core {
        c: c.c,
        d: c.d,
        e: c.e,
        sid: c.sid.clone(),
        role: u64::from(c.role),
        n: c.n,
        previous: c.previous,
        sequence: c.sequence,
        kind: u64::from(c.kind),
        request: c.request,
        parents: c.parents.clone(),
        expected: old::Expected {
            g: c.expected.g,
            index: c.expected.index,
            parent: c.expected.parent,
            tail: c.expected.tail.as_ref().map(|t| old::ClosedTail {
                g: t.g,
                key: t.key,
                index: t.index,
                last: t.last,
            }),
        },
        author_auth: c.author_auth,
    }
}
fn signed_raw_event(
    core: &Core2,
    packet: &Packet,
    signer: &kilogram_identity::DeviceIdentity,
) -> TestResult<Vec<u8>> {
    let h = packet.header()?;
    let unsigned = a([
        u(2),
        core.value(),
        a([u(u64::from(packet.kind)), b(h.key), u(h.index)]),
        b(&packet.bytes),
    ]);
    let sig = signer.sign(&signing_bytes(EVENT_SIGN, &unsigned, MAX_EVENT)?);
    let mut fields = unsigned.list()?.to_vec();
    fields.push(b(sig));
    Ok(encode(&a(fields), MAX_EVENT)?)
}
struct Fixture {
    actors: support::Actors,
    fresh: support::Fresh,
    selection: StaticSelection2,
    events: Vec<ParsedEvent2>,
    inners: Vec<Vec<u8>>,
    packets: Vec<Packet>,
    legacy: Vec<old::Event>,
    existing: support::Existing,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let actors = support::Actors::new([0xc1; 32])?;
        let (aa, _) = OrderedAccount::fresh(false);
        let (mut bb, otk) = OrderedAccount::fresh(true);
        let fresh = actors.fresh(aa.curve_key(), bb.curve_key(), otk.ok_or("OTK")?)?;
        let sa = aa.outbound(bb.curve_key(), otk.ok_or("OTK")?)?;
        let sid = sa.sid();
        let selection = StaticSelection2::new(fresh.proof.clone(), Some(sid.clone()))?;
        let mut sessions: [Option<OrderedSession>; 2] = [Some(sa), None];
        let mut guards = [Guard2::new(0)?, Guard2::new(1)?];
        let mut seen = BTreeMap::new();
        let mut events: Vec<ParsedEvent2> = vec![];
        let mut inners = vec![];
        let mut packets = vec![];
        for role in [0usize, 1, 0] {
            let n = guards[role].own().next;
            let id = if role == 0 {
                fresh.offer.id()
            } else {
                fresh.grant.id()
            };
            let mut request = [3; 16];
            if n == 0 {
                request.copy_from_slice(&id[..16]);
            }
            let core = Core2 {
                c: actors.c,
                d: selection.d(),
                e: selection.e(),
                sid: sid.clone(),
                role: role as u8,
                n,
                previous: guards[role].own().last,
                sequence: n,
                kind: if n == 0 { role as u8 + 1 } else { 3 },
                request,
                parents: vec![],
                expected: guards[role].expected_send()?,
                author_auth: actors.credentials[role].reference(),
                q: fresh.proof.q(),
            };
            let payload = match core.kind {
                1 => a([
                    b(fresh.offer.id()),
                    b(fresh.grant.id()),
                    fresh.offer.body().fields(8)?[3].clone(),
                    a(sessions[role]
                        .as_ref()
                        .ok_or("session")?
                        .keys()
                        .iter()
                        .map(b)),
                ]),
                2 => a([
                    b(fresh.offer.id()),
                    b(fresh.grant.id()),
                    b(sid.as_bytes()),
                    b(events.first().ok_or("A0")?.id()),
                ]),
                _ => a([b("S3:codec:Привет".as_bytes())]),
            };
            let inner = inner2(&core, payload)?;
            let mut meta = kilogram_ratchet::ordered_v2::EventMeta {
                role: role as u8,
                n,
                previous: core.previous,
                sequence: n,
                kind: core.kind,
                native_kind: if core.kind == 1 { 0 } else { 1 },
                key: [0; 32],
                index: core.expected.index,
                expected: core.expected.clone(),
            };
            let step = guards[role].preflight_send(&meta, 0, 0)?;
            let packet = sessions[role].as_mut().ok_or("session")?.encrypt(&inner)?;
            let event = sign_event(core, &packet, &actors.devices[role])?;
            meta = event.meta();
            let observation = seen
                .get(&(role as u8, meta.key))
                .copied()
                .map_or(SeenObservation::Absent, SeenObservation::Present);
            guards[role].apply(step, &meta, event.id(), observation)?;
            let receive = guards[1 - role].preflight_receive(&meta, observation, 0, 0)?;
            let plain = if let Some(session) = sessions[1 - role].as_mut() {
                session.decrypt(&packet)?
            } else {
                let (s, p) = bb.inbound(aa.curve_key(), &packet)?;
                sessions[1 - role] = Some(s);
                p
            };
            assert_eq!(plain, inner);
            event
                .clone()
                .verify(&selection, &actors.credentials[role])?
                .verify_inner(&plain)?;
            guards[1 - role].apply(receive, &meta, event.id(), observation)?;
            seen.entry((role as u8, meta.key)).or_insert(Seen {
                generation: meta.expected.g,
                first: event.id(),
            });
            events.push(event);
            inners.push(inner);
            packets.push(packet);
        }
        // Independently generated config1 legacy packets/Inner1 for mixed decoder coverage.
        let (la, _) = OrderedAccount::fresh(false);
        let (mut lb, otk) = OrderedAccount::fresh(true);
        let lsa = la.outbound(lb.curve_key(), otk.ok_or("OTK")?)?;
        let lsid = lsa.sid();
        let mut ls = [Some(lsa), None];
        let mut legacy: Vec<old::Event> = vec![];
        for (i, role) in [0usize, 1, 0].into_iter().enumerate() {
            let mut c = events[i].core().clone();
            c.d = [71; 32];
            c.e = [72; 32];
            c.sid = lsid.clone();
            c.previous = if i == 2 { Some(legacy[0].id()?) } else { None };
            c.expected.parent = if i == 1 {
                Some(legacy[0].id()?)
            } else if i == 2 {
                Some(legacy[1].id()?)
            } else {
                None
            };
            c.expected.tail = if i == 2 {
                Some(ClosedTail {
                    g: 0,
                    key: legacy[0].key,
                    index: 0,
                    last: legacy[0].id()?,
                })
            } else {
                None
            };
            let core = old_core(&c);
            let inner = old::Event::inner(&core, a([b(b"legacy")]))?;
            let packet = ls[role].as_mut().ok_or("legacy session")?.encrypt(&inner)?;
            let h = packet.header()?;
            let mut event = old::Event {
                core,
                native_kind: u64::from(packet.kind),
                key: h.key,
                index: h.index,
                ciphertext: packet.bytes.clone(),
                signature: vec![],
            };
            event.sign(&actors.devices[role])?;
            let plain = if let Some(s) = ls[1 - role].as_mut() {
                s.decrypt(&packet)?
            } else {
                let (s, p) = lb.inbound(la.curve_key(), &packet)?;
                ls[1 - role] = Some(s);
                p
            };
            assert_eq!(plain, inner);
            let event = old::Event::parse(&event.raw()?, actors.devices[role].device_id())?;
            event.verify_inner(&plain)?;
            legacy.push(event);
        }
        let existing = actors.existing(
            [71; 32],
            [72; 32],
            &lsid,
            [legacy[0].raw()?, legacy[1].raw()?],
            [9, 9],
            [legacy[2].id()?, legacy[1].id()?],
        )?;
        Ok(Self {
            actors,
            fresh,
            selection,
            events,
            inners,
            packets,
            legacy,
            existing,
        })
    }
    fn receipt(&self) -> TestResult<ParsedReceipt2> {
        let e = &self.events[2];
        let c = e.core();
        Ok(ParsedReceipt2::sign(
            ReceiptFields2 {
                c: c.c,
                d: c.d,
                e: c.e,
                sid: c.sid.clone(),
                q: c.q,
                format: 2,
                lane: c.role,
                n: c.n,
                event: e.id(),
                receiver_cert: self.actors.credentials[1].cert_ref(),
                receiver_auth: self.actors.credentials[1].reference(),
            },
            &self.actors.devices[1],
        )?)
    }
    fn ready(&self, q: Id) -> TestResult<ParsedControl2> {
        Ok(ParsedControl2::sign(
            3,
            &encode(
                &a([
                    b(self.selection.d()),
                    b(self.selection.e()),
                    b(self.selection.sid().ok_or("SID")?.as_bytes()),
                    b(self.events[0].id()),
                    b(self.events[1].id()),
                    u(0),
                    u(1),
                    u(2),
                    b(q),
                ]),
                4096,
            )?,
            &self.actors.devices[0],
        )?)
    }
}
fn frame_bodies(f: &Fixture) -> TestResult<Vec<Value>> {
    let c = f.selection.c();
    let d = f.selection.d();
    let e = f.selection.e();
    let sid = f.selection.sid().ok_or("SID")?;
    let t = f.fresh.offer.body().fields(8)?[3].clone();
    let cursor = a([u(1), opt(Some(b(f.events[0].id())))]);
    let mode = a([
        u(4),
        opt(Some(b(f.fresh.proof.q()))),
        a([opt(Some(u(0))), opt(Some(u(0)))]),
    ]);
    let status = a([u(0), opt(None)]);
    let credit = a([
        u(64),
        u(4 * 1024 * 1024),
        u(64 * 1024),
        u(32),
        u(MAX_WIRE as u64),
        u(100),
        opt(None),
    ]);
    let records = |role: usize| {
        a([a([
            b(f.actors.credentials[role].reference()),
            b(f.actors.credentials[role].raw()),
        ])])
    };
    let ready = f.ready(f.fresh.proof.q())?;
    let receipt = f.receipt()?;
    let committed = ParsedControl2::sign(
        21,
        &encode(&a([b(c), b(f.existing.proof.q())]), 1024)?,
        &f.actors.devices[1],
    )?;
    Ok(vec![
        f.fresh.hellos[0].body().value().clone(),
        f.fresh.proofs[0].body().value().clone(),
        a([t, b(c), u(1), u(2)]),
        a([
            b(c),
            u(3),
            opt(Some(b(f.fresh.offer.id()))),
            opt(Some(b(f.fresh.grant.id()))),
            opt(Some(b(d))),
            opt(Some(b(e))),
        ]),
        a([b(c), b(f.fresh.offer.raw()), b(&f.fresh.pair)]),
        a([b(c), b(f.fresh.offer.id()), b(f.fresh.grant.raw())]),
        a([b(c), b(d), b(e), u(2), b(f.events[0].raw())]),
        a([b(c), b(d), b(e), b(ready.raw())]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            a((0..6).map(|_| opt(None))),
            cursor.clone(),
            cursor.clone(),
            mode.clone(),
            status.clone(),
            credit.clone(),
        ]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            u(0),
            records(0),
            a([a([u(2), b(f.events[2].raw())])]),
        ]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            u(0),
            u(1),
            u(2),
            opt(Some(b(f.events[0].id()))),
            credit.clone(),
        ]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            u(0),
            u(1),
            u(2),
            cursor.clone(),
            records(0),
            a([a([u(2), b(f.events[2].raw())])]),
            u(0),
        ]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            b([3; 16]),
            a([b(f.events[2].id())]),
        ]),
        a([
            b(c),
            opt(Some(b(d))),
            opt(Some(b(e))),
            b([3; 16]),
            u(0),
            u(0),
            opt(None),
            u(1),
        ]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            u(0),
            records(1),
            a([a([u(2), b(receipt.raw())])]),
            a([]),
        ]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            u(0),
            a([a([u(1), b(f.events[2].id())])]),
        ]),
        a([
            b(c),
            b(d),
            b(e),
            b(sid.as_bytes()),
            cursor.clone(),
            cursor,
            mode,
            status,
            credit,
        ]),
        a([b(f.existing.propose.raw()), b(&f.existing.consent)]),
        a([b(f.existing.accept.raw())]),
        a([b(f.existing.select.raw())]),
        a([b(committed.raw())]),
    ])
}
#[test]
fn a3_s3_c01_roundtrip_28_and_mixed_receipts() -> TestResult {
    let f = Fixture::new()?;
    let mut count = 0;
    for (i, body) in frame_bodies(&f)?.into_iter().enumerate() {
        let kind = FrameKind::try_from((i + 1) as u64)?;
        let raw = encode(&body, MAX_WIRE - 4)?;
        let frame = ParsedFrame2::sign(
            FrameBody2::parse(kind, &raw)?,
            [17; 32],
            [3; 16],
            &f.actors.devices[0],
        )?;
        let parsed = ParsedFrame2::parse(frame.raw())?;
        assert_eq!(encode(parsed.body().value(), MAX_WIRE - 4)?, raw);
        assert_eq!(parsed.raw(), frame.raw());
        parsed.verify([17; 32], f.actors.devices[0].device_id())?;
        count += 1;
    }
    for (event, inner) in f.events.iter().zip(&f.inners) {
        let parsed = ParsedEvent2::parse(event.raw())?;
        assert_eq!(parsed.id(), event.id());
        assert_eq!(parsed.raw(), event.raw());
        let verified = parsed.verify(
            &f.selection,
            &f.actors.credentials[usize::from(event.core().role)],
        )?;
        let payload = verified.verify_inner(inner)?;
        assert_eq!(inner2(event.core(), payload.value().clone())?, *inner);
        count += 1;
    }
    for raw in [&f.fresh.raw, &f.existing.raw] {
        let p = ParsedModeProof2::parse(raw)?;
        assert_eq!(p.raw(), raw);
        assert_eq!(p.q(), hash(MODE, raw));
        count += 1;
    }
    let current = f.receipt()?;
    let verified = f.events[2]
        .clone()
        .verify(&f.selection, &f.actors.credentials[0])?;
    assert_eq!(ParsedReceipt2::parse(current.raw())?.id(), current.id());
    current
        .clone()
        .verify(&verified, &f.selection, &f.actors.credentials[1])?;
    count += 1;
    let selection =
        StaticSelection2::new(f.existing.proof.clone(), Some(f.legacy[0].core.sid.clone()))?;
    let source = TaggedItem {
        format: 1,
        raw: f.legacy[2].raw()?,
    }
    .verify_event(&selection, &f.actors.credentials[0])?;
    let body = a([
        u(1),
        b(selection.d()),
        b(selection.e()),
        u(0),
        u(1),
        b(f.legacy[2].id()?),
        b(f.actors.credentials[1].cert_ref()),
        b(f.actors.credentials[1].reference()),
    ]);
    let sig = f.actors.devices[1].sign(&signing_bytes(old::RECEIPT_SIGN, &body, MAX_RECEIPT)?);
    let raw = encode(&a([body, b(sig)]), MAX_RECEIPT)?;
    let receipt = ParsedReceipt1::parse(&raw)?;
    assert_eq!(receipt.raw(), raw);
    assert_eq!(receipt.id(), hash(old::RECEIPT_ID, &raw));
    TaggedItem {
        format: 1,
        raw: raw.clone(),
    }
    .verify_receipt(&source, &selection, &f.actors.credentials[1])?;
    count += 1;
    // New receipt referencing an exact legacy source under the selected Q is also permitted.
    let fields = ReceiptFields2 {
        c: selection.c(),
        d: selection.d(),
        e: selection.e(),
        sid: selection.sid().ok_or("SID")?.into(),
        q: f.existing.proof.q(),
        format: 1,
        lane: 0,
        n: 1,
        event: f.legacy[2].id()?,
        receiver_cert: f.actors.credentials[1].cert_ref(),
        receiver_auth: f.actors.credentials[1].reference(),
    };
    let receipt = ParsedReceipt2::sign(fields, &f.actors.devices[1])?;
    TaggedItem {
        format: 2,
        raw: receipt.raw().to_vec(),
    }
    .verify_receipt(&source, &selection, &f.actors.credentials[1])?;
    assert!(
        TaggedItem { format: 2, raw }
            .verify_receipt(&source, &selection, &f.actors.credentials[1])
            .is_err()
    );
    assert_eq!(count, 28);
    // Largest reachable text tuple, with exact UTF-8 payload and no padding fields.
    let text = "я".repeat(MAX_TEXT / 2);
    let raw = inner2(f.events[2].core(), a([b(text.as_bytes())]))?;
    verified.verify_inner(&raw)?;
    assert!(inner2(f.events[2].core(), a([b(vec![b'x'; MAX_TEXT + 1])])).is_err());
    println!(
        "a3_s3_C01_roundtrips={count} mixed_receipt2_legacy=1 text_boundary_bytes={}",
        text.len()
    );
    Ok(())
}
#[test]
fn a3_s3_c04_c05_c06_c07_c09_static_verification() -> TestResult {
    let f = Fixture::new()?;
    let author = &f.actors.credentials[0];
    let signer = &f.actors.devices[0];
    let existing =
        StaticSelection2::new(f.existing.proof.clone(), Some(f.legacy[0].core.sid.clone()))?;
    TaggedItem {
        format: 1,
        raw: f.legacy[2].raw()?,
    }
    .verify_event(&existing, author)?;
    TaggedItem {
        format: 2,
        raw: f.events[2].raw().to_vec(),
    }
    .verify_event(&f.selection, author)?;
    for (format, raw) in [(1, f.events[2].raw().to_vec()), (2, f.legacy[2].raw()?)] {
        assert!(
            TaggedItem { format, raw }
                .verify_event(&f.selection, author)
                .is_err()
        );
    }
    let verified = f.events[2].clone().verify(&f.selection, author)?;
    let mut inner = decode(&f.inners[2], MAX_INNER)?.list()?.to_vec();
    inner[0] = u(1);
    assert!(
        verified
            .verify_inner(&encode(&a(inner), MAX_INNER)?)
            .is_err()
    );
    let mut legacy_inner = decode(
        &old::Event::inner(&f.legacy[2].core, a([b(b"old")]))?,
        MAX_INNER,
    )?
    .list()?
    .to_vec();
    legacy_inner[0] = u(2);
    assert!(
        f.legacy[2]
            .verify_inner(&encode(&a(legacy_inner), MAX_INNER)?)
            .is_err()
    );
    let mut c = f.events[2].core().clone();
    c.n = 512;
    c.sequence = 512;
    c.parents = vec![c.previous.ok_or("previous")?];
    Core2::parse(&c.value())?;
    assert!(old::Core::parse(&old_core(&c).value()).is_err());
    let mut disagreements = 0;
    for field in 0..7 {
        let mut c = f.events[2].core().clone();
        match field {
            0 => c.c = [99; 32],
            1 => c.d = [99; 32],
            2 => c.e = [99; 32],
            3 => c.sid = "Z".repeat(43),
            4 => c.role = 1,
            5 => c.q = [99; 32],
            6 => c.author_auth = [99; 32],
            _ => {}
        }
        let raw = signed_raw_event(&c, &f.packets[2], signer)?;
        assert!(
            ParsedEvent2::parse(&raw)?
                .verify(&f.selection, author)
                .is_err()
        );
        disagreements += 1;
    }
    let mut receipt_fields = f.receipt()?.fields().clone();
    receipt_fields.receiver_cert = [99; 32];
    assert!(
        ParsedReceipt2::sign(receipt_fields, &f.actors.devices[1])?
            .verify(&verified, &f.selection, &f.actors.credentials[1])
            .is_err()
    );
    disagreements += 1;
    for field in 0..3 {
        let event = &f.events[2];
        let mut fields = decode(event.raw(), MAX_EVENT)?.list()?.to_vec();
        let mut h = fields[2].list()?.to_vec();
        h[field] = match field {
            0 => u(0),
            1 => b([99; 32]),
            _ => u(1),
        };
        fields[2] = a(h);
        let unsigned = a(fields[..4].iter().cloned());
        fields[4] = b(signer.sign(&signing_bytes(EVENT_SIGN, &unsigned, MAX_EVENT)?));
        let raw = encode(&a(fields), MAX_EVENT)?;
        assert!(
            ParsedEvent2::parse(&raw)
                .and_then(|e| e.verify(&f.selection, author))
                .is_err()
        );
        disagreements += 1;
    }
    let mut inner = decode(&f.inners[2], MAX_INNER)?.list()?.to_vec();
    let mut c = f.events[2].core().clone();
    c.request = [88; 16];
    inner[1] = c.value();
    assert!(
        verified
            .verify_inner(&encode(&a(inner), MAX_INNER)?)
            .is_err()
    );
    disagreements += 1;
    assert_eq!(disagreements, 12);
    // Eight cryptographic/binding failures. Correctly signed disagreements above reach later seams.
    let mut raw = f.events[2].raw().to_vec();
    let end = raw.len() - 1;
    raw[end] ^= 1;
    assert!(
        ParsedEvent2::parse(&raw)?
            .verify(&f.selection, author)
            .is_err()
    );
    let frame = ParsedFrame2::sign(
        FrameBody2::proof([4; 32], f.actors.c, f.fresh.proof.certs())?,
        [17; 32],
        [0; 16],
        signer,
    )?;
    let mut raw = frame.raw().to_vec();
    let end = raw.len() - 1;
    raw[end] ^= 1;
    assert!(
        ParsedFrame2::parse(&raw)?
            .verify([17; 32], signer.device_id())
            .is_err()
    );
    let receipt = f.receipt()?;
    let mut raw = receipt.raw().to_vec();
    let end = raw.len() - 1;
    raw[end] ^= 1;
    assert!(
        ParsedReceipt2::parse(&raw)?
            .verify(&verified, &f.selection, &f.actors.credentials[1])
            .is_err()
    );
    assert!(
        frame
            .clone()
            .verify([17; 32], f.actors.devices[1].device_id())
            .is_err()
    );
    assert!(frame.verify([18; 32], signer.device_id()).is_err());
    assert!(f.fresh.offer.require_kind(3).is_err());
    let mut consent = f.fresh.consent.clone();
    let end = consent.len() - 1;
    consent[end] ^= 1;
    assert!(
        ParsedConsent2::parse(&consent)?
            .verify(&f.actors.credentials)
            .is_err()
    );
    let mut pair = decode(&f.fresh.pair, 192 * 1024)?.list()?.to_vec();
    pair.swap(4, 5);
    assert!(
        ParsedModeProof2::parse(&f.fresh.raw)?
            .verify(&SelectionEvidence2 {
                peers: f.actors.credentials.clone(),
                membership: f.actors.membership.clone(),
                initial_pair: Some(encode(&a(pair), 192 * 1024)?),
                previous: None
            })
            .is_err()
    );
    assert_eq!(
        ParsedModeProof2::parse(&f.fresh.raw)?.q(),
        f.fresh.proof.q()
    );
    assert_eq!(
        ParsedModeProof2::parse(&f.existing.raw)?.q(),
        f.existing.proof.q()
    );
    let mut c = f.events[2].core().clone();
    c.q = [88; 32];
    assert!(
        ParsedEvent2::parse(&signed_raw_event(&c, &f.packets[2], signer)?)?
            .verify(&f.selection, author)
            .is_err()
    );
    let genesis = [f.events[0].id(), f.events[1].id()];
    f.ready(f.fresh.proof.q())?
        .verify_ready(&f.selection, 0, genesis, author)?;
    assert!(
        f.ready([88; 32])?
            .verify_ready(&f.selection, 0, genesis, author)
            .is_err()
    );
    let hint = ModeView2::parse(&a([
        u(5),
        opt(Some(b([88; 32]))),
        a([opt(Some(u(0))), opt(Some(u(0)))]),
    ]))?;
    assert!(hint.check_selected(&f.fresh.proof).is_err());
    let mut fields = f.receipt()?.fields().clone();
    fields.q = [88; 32];
    assert!(
        ParsedReceipt2::sign(fields, &f.actors.devices[1])?
            .verify(&verified, &f.selection, &f.actors.credentials[1])
            .is_err()
    );
    let renewed = f.actors.renewal(0, 3)?;
    assert_eq!(renewed.cert_ref(), author.cert_ref());
    assert_ne!(renewed.reference(), author.reference());
    assert_eq!(f.selection.proof().q(), hash(MODE, &f.fresh.raw));
    assert!(
        TaggedItem {
            format: 1,
            raw: f.legacy[2].raw()?
        }
        .verify_event(&f.selection, author)
        .is_err()
    );
    let mut genesis_negatives = 0;
    for role in 0..2 {
        let mut c = f.events[role].core().clone();
        c.request[0] ^= 1;
        assert!(
            ParsedEvent2::parse(&signed_raw_event(
                &c,
                &f.packets[role],
                &f.actors.devices[role]
            )?)?
            .verify(&f.selection, &f.actors.credentials[role])
            .is_err()
        );
        genesis_negatives += 1;
        let mut c = f.events[role].core().clone();
        c.parents.push([99; 32]);
        let raw = signed_raw_event(&c, &f.packets[role], &f.actors.devices[role])?;
        assert!(ParsedEvent2::parse(&raw).is_err());
        genesis_negatives += 1;
    }
    assert_eq!(genesis_negatives, 4);
    println!("a3_s3_C04=8 C05=12 C06=8 C07=8 C09=4 static_reject_native_calls=0");
    Ok(())
}
#[test]
fn a3_s3_c02_c03_c08_bounds_canonical_collections() -> TestResult {
    let malformed: [Vec<u8>; 12] = [
        vec![0x18, 0],
        vec![0x58, 0],
        vec![0x9f, 0xff],
        vec![0x5f, 0xff],
        vec![0xa0],
        vec![0x60],
        vec![0x20],
        vec![0xc0, 0],
        vec![0xf9, 0, 0],
        vec![0xf6],
        vec![0, 0],
        vec![0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
    ];
    for raw in &malformed {
        assert!(decode(raw, MAX_WIRE).is_err());
    }
    for limit in [
        WireLimit2::Core,
        WireLimit2::Event,
        WireLimit2::Inner,
        WireLimit2::Olm,
        WireLimit2::Text,
        WireLimit2::Auth,
        WireLimit2::Membership,
        WireLimit2::Consent,
        WireLimit2::ModeProof,
        WireLimit2::Frame,
        WireLimit2::Descriptor,
        WireLimit2::Receipt,
        WireLimit2::InitialPair,
    ] {
        limit.check_len(limit.maximum())?;
        assert!(limit.check_len(limit.maximum() + 1).is_err());
    }
    let actors = support::Actors::new([8; 32])?;
    let refs = &actors.credentials;
    let mut records = refs
        .iter()
        .map(|c| (c.reference(), c.raw()))
        .collect::<Vec<_>>();
    records.sort_by_key(|(id, _)| *id);
    let records = a(records.iter().map(|(id, raw)| a([b(id), b(raw)])));
    let mut duplicate = records.list()?.to_vec();
    duplicate.push(duplicate[1].clone());
    assert!(validate_auth_records(&a(duplicate)).is_err());
    let mut unsorted = records.list()?.to_vec();
    unsorted.reverse();
    assert!(validate_auth_records(&a(unsorted)).is_err());
    assert!(auth_records_for(&records, &[refs[0].reference()]).is_err());
    auth_records_for(
        &records,
        &refs
            .iter()
            .map(RootVerifiedCredential::reference)
            .collect::<Vec<_>>(),
    )?;
    let mut core = Core2 {
        c: [1; 32],
        d: [2; 32],
        e: [3; 32],
        sid: "S".repeat(43),
        role: 0,
        n: 2,
        previous: Some([1; 32]),
        sequence: 2,
        kind: 3,
        request: [4; 16],
        parents: vec![[1; 32], [1; 32]],
        expected: Expected {
            g: 1,
            index: 1,
            parent: Some([2; 32]),
            tail: None,
        },
        author_auth: [5; 32],
        q: [6; 32],
    };
    assert!(Core2::parse(&core.value()).is_err());
    core.parents = vec![[2; 32], [1; 32]];
    assert!(Core2::parse(&core.value()).is_err());
    assert!(tagged_items(&a((0..33).map(|_| a([u(2), b([])]))), MAX_EVENT).is_err());
    println!("a3_s3_C02=12 C03_cap_pairs=13 cap_vectors=26 C08=6 native_calls=0");
    Ok(())
}
#[test]
fn a3_s3_c03_largest_structural_tuples_and_descriptor() -> TestResult {
    let f = Fixture::new()?;
    let m = u64::MAX;
    let mut c = f.events[2].core().clone();
    c.role = 1;
    c.n = m;
    c.sequence = m;
    c.previous = Some([1; 32]);
    c.parents = vec![[1; 32], [2; 32]];
    c.expected = Expected {
        g: m,
        index: m,
        parent: Some([3; 32]),
        tail: Some(ClosedTail {
            g: m,
            key: [4; 32],
            index: m,
            last: [5; 32],
        }),
    };
    let core = encode(&c.value(), MAX_CORE)?;
    Core2::parse(&c.value())?;
    // All specified fields, no extra fields/padding. Opaque Olm is structural only.
    let event = encode(
        &a([
            u(2),
            c.value(),
            a([u(1), b([6; 32]), u(m)]),
            b(vec![0x44; MAX_OLM]),
            b([0; 64]),
        ]),
        MAX_EVENT,
    )?;
    ParsedEvent2::parse(&event)?;
    assert!(event.len() < MAX_EVENT);
    let mut r = f.receipt()?.fields().clone();
    r.n = m;
    let receipt = ParsedReceipt2::sign(r, &f.actors.devices[1])?;
    assert!(receipt.raw().len() < MAX_RECEIPT);
    let origin = Origin::Existing {
        d: [71; 32],
        e: [72; 32],
        sid: f.legacy[0].core.sid.clone(),
    };
    let consent_body = a([
        u(2),
        b(f.actors.c),
        b(f.actors.credentials[0].cert_ref()),
        b(f.actors.credentials[1].cert_ref()),
        u(1),
        u(2),
        origin.value(),
        u(1),
        u(m - 1),
        u(m),
    ]);
    let msg = signing_bytes(CONSENT, &consent_body, 4096)?;
    let consent = encode(
        &a([
            consent_body,
            b(f.actors.roots[0].sign(&msg)),
            b(f.actors.roots[1].sign(&msg)),
        ]),
        4096,
    )?;
    ParsedConsent2::parse(&consent)?.verify(&f.actors.credentials)?;
    let p = ParsedControl2::sign(
        18,
        &encode(
            &a([
                b(f.actors.c),
                b([71; 32]),
                b([72; 32]),
                b(f.legacy[0].core.sid.as_bytes()),
                b(hash(CONSENT, &consent)),
                b([7; 32]),
                u(512),
                b(f.legacy[2].id()?),
            ]),
            4096,
        )?,
        &f.actors.devices[0],
    )?;
    let accept = ParsedControl2::sign(
        19,
        &encode(
            &a([b(f.actors.c), b(p.id()), u(512), b(f.legacy[1].id()?)]),
            4096,
        )?,
        &f.actors.devices[1],
    )?;
    let select = ParsedControl2::sign(
        20,
        &encode(&a([b(f.actors.c), b(p.id()), b(accept.id())]), 4096)?,
        &f.actors.devices[0],
    )?;
    let proof = encode(
        &a([
            u(2),
            u(1),
            b(p.raw()),
            b(accept.raw()),
            b(select.raw()),
            b(consent),
        ]),
        MAX_PROOF,
    )?;
    ParsedModeProof2::parse(&proof)?;
    assert!(proof.len() < MAX_PROOF);
    let body = a([
        u(2),
        b(f.actors.c),
        b(f.actors.credentials[0].cert_ref()),
        b(f.actors.credentials[1].cert_ref()),
        b([11; 32]),
        a((0u8..16).map(|i| {
            let mut ip = [0xff; 16];
            ip[15] = i;
            a([u(6), b(ip), u(65535)])
        })),
        u(m),
        u(m - 1),
        u(m),
    ]);
    let sig = f.actors.devices[0].sign(&signing_bytes(DESCRIPTOR, &body, 8192)?);
    let raw = encode(&a([body, b(sig)]), 8192)?;
    let descriptor = ParsedDescriptor2::parse(&raw)?;
    descriptor.verify(
        f.actors.c,
        &f.actors.credentials[0],
        &f.actors.credentials[1],
    )?;
    assert_eq!(descriptor.raw(), raw);
    assert_eq!(descriptor.endpoint(), [11; 32]);
    assert_eq!(
        exporter_context([11; 32], [12; 32])?,
        encode(&a([b(ALPN), b([11; 32]), b([12; 32])]), 256)?
    );
    assert_ne!(exporter_hash(&[17; 32]), exporter_hash(&[18; 32]));
    println!(
        "a3_s3_C03_largest_structural core={} Event={} Receipt={} ModeProof={} Descriptor={} descriptor_static_verified=true fixture_native_calls=12 huge_native_state=false",
        core.len(),
        event.len(),
        receipt.raw().len(),
        proof.len(),
        raw.len()
    );
    Ok(())
}
