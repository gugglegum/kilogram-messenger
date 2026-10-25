//! Generated accepted OD1 checkpoints, not a file/profile migration or arbitrary pickle adoption.
use super::super::{
    handshake as hs,
    ratchet::Guard as Guard1,
    state::{Head, PrivateState},
};
use super::*;
use kilogram_protocol::ordered_dialog_v1 as old;

struct LegacyFixture {
    actors: support::Actors,
    states: [PrivateState; 2],
    heads: [Head; 2],
    blobs: [Vec<u8>; 2],
    offer: Vec<u8>,
    grant: Vec<u8>,
    pair: Vec<u8>,
    spec: hs::Spec,
    trace: Vec<old::Event>,
    nonce: Id,
    scope: db::HistoryScope,
    keys: [[u8; 32]; 3],
    calls: u64,
}
#[derive(Clone, Copy)]
struct OriginFact {
    known_constructor: bool,
    config: u8,
}
impl OriginFact {
    const CONFIG1: Self = Self {
        known_constructor: true,
        config: 1,
    };
}
#[derive(Default, Debug, PartialEq)]
struct ImportProbe {
    boundaries: Vec<&'static str>,
}
type ImportedFixture = (Native, BTreeMap<(u8, Id), guard::Seen>);
fn guard_lane(l: &super::super::ratchet::Lane) -> guard::Lane {
    guard::Lane {
        next: l.next,
        last: l.last,
        g: l.g,
        key: l.key,
        index: l.index,
        first: l.first,
        parent: l.parent,
        sequence: l.sequence,
    }
}
fn compact(g: &Guard1) -> TestResult<Vec<u8>> {
    Ok(guard::compact_l1_checkpoint(guard::L1CheckpointFacts {
        role: g.role as u8,
        own: guard_lane(&g.own),
        peer: guard_lane(&g.peer),
        rotate_due: g.rotate_due,
        received_new_first: g.received_new_first,
        used_own_first: g.used_own_first,
    })?)
}
fn event_meta(e: &old::Event) -> guard::EventMeta {
    guard::EventMeta {
        role: e.core.role as u8,
        n: e.core.n,
        previous: e.core.previous,
        sequence: e.core.sequence,
        kind: e.core.kind as u8,
        native_kind: e.native_kind as u8,
        key: e.key,
        index: e.index,
        expected: guard::Expected {
            g: e.core.expected.g,
            index: e.core.expected.index,
            parent: e.core.expected.parent,
            tail: e.core.expected.tail.as_ref().map(|t| guard::ClosedTail {
                g: t.g,
                key: t.key,
                index: t.index,
                last: t.last,
            }),
        },
    }
}
impl LegacyFixture {
    fn new() -> TestResult<Self> {
        let actors = support::Actors::new([0x93; 32])?;
        let body = a([
            u(1),
            b(actors.c),
            b(actors.credentials[0].cert_ref()),
            b(actors.credentials[1].cert_ref()),
            u(1),
            u(1),
            u(1699999000),
            u(1700010000),
        ]);
        let bytes = old::signing_bytes(hs::CONSENT, &body, 4096)?;
        let consent = old::encode(
            &a([
                body,
                b(actors.roots[0].sign(&bytes)),
                b(actors.roots[1].sign(&bytes)),
            ]),
            4096,
        )?;
        let spec = hs::Spec {
            c: actors.c,
            peers: [
                hs::Credentials::parse(actors.credentials[0].raw())?,
                hs::Credentials::parse(actors.credentials[1].raw())?,
            ],
            membership: actors.membership.clone(),
            consent,
        };
        spec.validate()?;
        spec.valid_now(&spec.consent, 1700000000)?;
        let binding = [17; 32];
        let endpoints = [[11; 32], [12; 32]];
        let mut hello = vec![];
        for role in 0..2 {
            hello.push(
                old::Frame::signed(
                    1,
                    binding,
                    [role as u8; 16],
                    a([
                        b(spec.c),
                        u(role as u64),
                        b(endpoints[role]),
                        b(endpoints[1 - role]),
                        b([22 + role as u8; 32]),
                        a([u(1)]),
                        a([u(1)]),
                        b(&spec.peers[role].raw),
                        b(&spec.membership),
                        b(old::hash(hs::CONSENT, &spec.consent)),
                        b([44 + role as u8; 32]),
                    ]),
                    &actors.devices[role],
                )?
                .raw()?,
            );
        }
        let t = hs::transcript(binding, &hello[0], &hello[1])?;
        let refs = spec.cert_refs()?;
        let proof_body = a([b(t), b(spec.c), u(1), u(1), b(refs[0]), b(refs[1])]);
        let proofs = [
            old::Frame::signed(2, binding, [3; 16], proof_body.clone(), &actors.devices[0])?
                .raw()?,
            old::Frame::signed(2, binding, [4; 16], proof_body, &actors.devices[1])?.raw()?,
        ];
        let pair = hs::initial_pair(
            t,
            hello[0].clone(),
            hello[1].clone(),
            proofs[0].clone(),
            proofs[1].clone(),
        )?;
        assert_eq!(
            hs::verify_initial_pair(&spec, &pair, Some(old::hash(hs::CONSENT, &spec.consent)))?,
            t
        );
        let (na, _) = PrivateState::fresh(false)?;
        let (mut nb, otk) = PrivateState::fresh(true)?;
        let otk = otk.ok_or("OTK")?;
        let offer = old::signed_control(
            1,
            a([
                b(spec.c),
                b(refs[0]),
                b(refs[1]),
                b(t),
                b([0x17; 32]),
                b(na.account.curve_key()),
                u(1),
                u(1),
            ]),
            &actors.devices[0],
        )?;
        let offer_id = old::hash(old::CONTROL_ID, &offer);
        let grant = old::signed_control(
            2,
            a([
                b(spec.c),
                b(offer_id),
                b(t),
                b([0x29; 32]),
                b(nb.account.curve_key()),
                b(otk),
                u(1),
                u(1),
            ]),
            &actors.devices[1],
        )?;
        let grant_id = old::hash(old::CONTROL_ID, &grant);
        let (d, e) = hs::epoch(&spec, &offer, &grant)?;
        let nonce = [0x61; 32];
        let mut heads = [
            Head::absent(spec.c, refs[0], 0),
            Head::absent(spec.c, refs[1], 1),
        ];
        heads[0].phase = 1;
        heads[0].generation = 1;
        heads[0].native_generation = 1;
        heads[0].offer = Some(offer_id);
        heads[1].phase = 2;
        heads[1].generation = 1;
        heads[1].native_generation = 1;
        heads[1].offer = Some(offer_id);
        heads[1].grant = Some(grant_id);
        heads[1].d = Some(d);
        heads[1].e = Some(e);
        nb.reservation = Some((offer_id, grant_id, otk, 0));
        let mut states = [na, nb];
        for role in 0..2 {
            let raw = states[role].seal(
                &mut heads[role],
                nonce,
                actors.credentials[role]
                    .certificate()
                    .encryption_public_key(),
            )?;
            let restored = PrivateState::open(
                &Head::parse(&heads[role].encode()?, spec.c, refs[role], role as u64)?,
                nonce,
                &DeviceEncryptionIdentity::from_secret_bytes([31 + role as u8; 32]),
                &raw,
            )?;
            assert!(restored.session.is_none());
            assert!(restored.guard.is_none());
            assert!(heads[role].sid.is_none());
            assert_eq!(
                restored.account.curve_key(),
                states[role].account.curve_key()
            );
            if role == 0 {
                assert!(heads[role].d.is_none());
                assert!(heads[role].e.is_none());
                assert!(restored.reservation.is_none());
            } else {
                assert_eq!(restored.reservation, Some((offer_id, grant_id, otk, 0)));
            }
            states[role] = restored;
        }
        states[0].session = Some(
            states[0]
                .account
                .outbound(states[1].account.curve_key(), otk)?,
        );
        let sid = states[0].session.as_ref().ok_or("session")?.sid();
        let keys = states[0].session.as_ref().ok_or("session")?.keys();
        for (role, state) in states.iter_mut().enumerate() {
            state.guard = Some(Guard1::new(role as u64));
        }
        let scope = db::HistoryScope {
            context: spec.c,
            dialog: d,
            epoch: e,
            session_id: sid,
        };
        let mut f = Self {
            actors,
            states,
            heads,
            blobs: [vec![], vec![]],
            offer,
            grant,
            pair,
            spec,
            trace: vec![],
            nonce,
            scope,
            keys,
            calls: 0,
        };
        f.step(0)?;
        f.step(1)?;
        for role in 0..2 {
            for _ in 0..8 {
                f.step(role)?;
            }
        }
        for role in 0..2 {
            let h = &mut f.heads[role];
            h.phase = 3;
            h.generation = 20;
            h.native_generation = 20;
            h.offer = Some(offer_id);
            h.grant = Some(grant_id);
            h.d = Some(d);
            h.e = Some(e);
            h.sid = Some(f.scope.session_id.clone());
            f.blobs[role] = f.states[role].seal(
                h,
                nonce,
                f.actors.credentials[role]
                    .certificate()
                    .encryption_public_key(),
            )?;
        }
        Ok(f)
    }
    fn step(&mut self, role: usize) -> TestResult<()> {
        let guard = self.states[role].guard.as_ref().ok_or("guard")?;
        let n = guard.own.next;
        let expected = guard.expected_send()?;
        let offer = old::hash(old::CONTROL_ID, &self.offer);
        let grant = old::hash(old::CONTROL_ID, &self.grant);
        let mut request = [0; 16];
        let kind = if n == 0 { role as u64 + 1 } else { 3 };
        if n == 0 {
            request.copy_from_slice(&if role == 0 { offer } else { grant }[..16]);
        } else {
            request[..4].copy_from_slice(&3u32.to_be_bytes());
            request[4..8].copy_from_slice(&(role as u32).to_be_bytes());
            request[8..].copy_from_slice(&n.to_be_bytes());
        }
        let mut parents = vec![];
        for lane in [&guard.own, &guard.peer] {
            if lane.next > 1 {
                parents.push(lane.last.ok_or("parent")?);
            }
        }
        parents.sort();
        parents.dedup();
        let core = old::Core {
            c: self.scope.context,
            d: self.scope.dialog,
            e: self.scope.epoch,
            sid: self.scope.session_id.clone(),
            role: role as u64,
            n,
            previous: guard.own.last,
            sequence: n,
            kind,
            request,
            parents,
            expected,
            author_auth: self.actors.credentials[role].reference(),
        };
        let payload = match kind {
            1 => a([
                b(offer),
                b(grant),
                old::control(&self.offer, 1, self.actors.devices[0].device_id())?.fields(8)?[3]
                    .clone(),
                a(self.keys.iter().map(b)),
            ]),
            2 => a([
                b(offer),
                b(grant),
                b(self.scope.session_id.as_bytes()),
                b(self.trace[0].id()?),
            ]),
            _ => a([b(format!("S3:3:{role}:{n}:Привет").as_bytes())]),
        };
        let inner = old::Event::inner(&core, payload)?;
        let mut trial = self.states[role].fork();
        self.calls += 1;
        let packet = trial
            .session
            .as_mut()
            .ok_or("send session")?
            .encrypt(&inner)?;
        let header = packet.header()?;
        let mut event = old::Event {
            core,
            native_kind: u64::from(packet.kind),
            key: header.key,
            index: header.index,
            ciphertext: packet.bytes.clone(),
            signature: vec![],
        };
        event.sign(&self.actors.devices[role])?;
        let event = old::Event::parse(&event.raw()?, self.actors.devices[role].device_id())?;
        let id = event.id()?;
        trial
            .guard
            .as_mut()
            .ok_or("guard")?
            .commit_send(&event, id)?;
        self.states[role] = trial;
        let mut trial = self.states[1 - role].fork();
        trial.guard.as_ref().ok_or("guard")?.check_receive(&event)?;
        self.calls += 1;
        let plain = if let Some(s) = trial.session.as_mut() {
            s.decrypt(&packet)?
        } else {
            let (s, p) = trial
                .account
                .inbound(self.states[role].account.curve_key(), &packet)?;
            trial.session = Some(s);
            let r = trial.reservation.as_mut().ok_or("reservation")?;
            r.3 = 1;
            p
        };
        assert_eq!(plain, inner);
        event.verify_inner(&plain)?;
        trial
            .guard
            .as_mut()
            .ok_or("guard")?
            .commit_receive(&event, id)?;
        self.states[1 - role] = trial;
        self.trace.push(event);
        Ok(())
    }
    fn import(
        &self,
        role: usize,
        head: Option<&Head>,
        raw: &[u8],
        origin: OriginFact,
        probe: &mut ImportProbe,
    ) -> TestResult<ImportedFixture> {
        probe.boundaries.push("owner-verifier");
        if origin.config != 1 {
            return Err("UnsupportedOrigin/config".into());
        }
        if !origin.known_constructor {
            return Err("unknown/libolm constructor origin".into());
        }
        let head = head.ok_or("authenticated Head missing")?;
        let crypto = DeviceEncryptionIdentity::from_secret_bytes([31 + role as u8; 32]);
        let head = Head::parse(
            &head.encode()?,
            self.scope.context,
            self.actors.credentials[role].cert_ref(),
            role as u64,
        )?;
        probe.boundaries.push("native-restore/open");
        let accepted = PrivateState::open(&head, self.nonce, &crypto, raw)?;
        let selected = hs::epoch(&self.spec, &self.offer, &self.grant)?;
        hs::verify_initial_pair(
            &self.spec,
            &self.pair,
            Some(old::hash(hs::CONSENT, &self.spec.consent)),
        )?;
        if selected != (self.scope.dialog, self.scope.epoch)
            || head.d != Some(selected.0)
            || head.e != Some(selected.1)
            || head.sid.as_deref() != Some(self.scope.session_id.as_str())
            || accepted.account.curve_key() != self.states[role].account.curve_key()
            || accepted.session.as_ref().ok_or("session")?.keys() != self.keys
        {
            return Err("L1 accepted origin/scope/SID".into());
        }
        let mut steps = vec![];
        for event in &self.trace {
            let event = old::Event::parse(
                &event.raw()?,
                self.actors.devices[event.core.role as usize].device_id(),
            )?;
            let c = &event.core;
            if c.c != self.scope.context
                || c.d != self.scope.dialog
                || c.e != self.scope.epoch
                || c.sid != self.scope.session_id
            {
                return Err("L1 source scope".into());
            }
            if c.n == 0 {
                let control = if c.role == 0 {
                    &self.offer
                } else {
                    &self.grant
                };
                if c.request.as_slice() != &old::hash(old::CONTROL_ID, control)[..16]
                    || !c.parents.is_empty()
                {
                    return Err("L1 genesis R4".into());
                }
            }
            steps.push((event_meta(&event), event.id()?));
        }
        let old_guard = accepted.guard.as_ref().ok_or("guard")?;
        let compact = compact(old_guard)?;
        probe.boundaries.push("guard-import");
        let imported = guard::import_l1(role as u8, &steps, &compact)?;
        for (lane, source_role) in [
            (&old_guard.own, role as u8),
            (&old_guard.peer, 1 - role as u8),
        ] {
            let actual = imported
                .seen
                .iter()
                .filter(|(r, _, _)| *r == source_role)
                .map(|(_, key, _)| *key)
                .collect::<std::collections::BTreeSet<_>>();
            if actual != lane.seen {
                return Err("L1 authenticated Seen/first provenance".into());
            }
        }
        let seen = imported
            .seen
            .into_iter()
            .map(|(r, k, s)| ((r, k), s))
            .collect();
        Ok((
            Native {
                account: accepted.account,
                session: accepted.session,
                guard: imported.guard,
                generation: head.native_generation,
                key: accepted.key,
            },
            seen,
        ))
    }
}
#[test]
fn a3_s3_n03_generated_l1_provenance_and_continuation() -> TestResult {
    let timer = Instant::now();
    let f = LegacyFixture::new()?;
    assert_eq!(f.calls, 36);
    assert_eq!(f.trace.len(), 18);
    let mut probes = [ImportProbe::default(), ImportProbe::default()];
    let (na, sa) = f.import(
        0,
        Some(&f.heads[0]),
        &f.blobs[0],
        OriginFact::CONFIG1,
        &mut probes[0],
    )?;
    let (nb, sb) = f.import(
        1,
        Some(&f.heads[1]),
        &f.blobs[1],
        OriginFact::CONFIG1,
        &mut probes[1],
    )?;
    for probe in probes {
        assert_eq!(
            probe.boundaries,
            vec!["owner-verifier", "native-restore/open", "guard-import"]
        );
    }
    let mut nodes = [na, nb];
    let mut seen = [sa, sb];
    let initial_keys = nodes.each_ref().map(|n| n.account.curve_key());
    let old_raw = f
        .trace
        .iter()
        .map(old::Event::raw)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let genesis = [f.trace[0].raw()?, f.trace[1].raw()?];
    let lasts = [
        nodes[0].guard.own().last.ok_or("A8")?,
        nodes[1].guard.own().last.ok_or("B8")?,
    ];
    let mode = f.actors.existing(
        f.scope.dialog,
        f.scope.epoch,
        &f.scope.session_id,
        genesis,
        [9, 9],
        lasts,
    )?;
    let selection =
        wire::StaticSelection2::new(mode.proof.clone(), Some(f.scope.session_id.clone()))?;
    let mut calls = f.calls;
    let mut sources = old_raw.clone();
    for role in 0..2 {
        let n = nodes[role].guard.own().next;
        assert_eq!(n, 9);
        let mut parents = vec![
            nodes[role].guard.own().last.ok_or("own parent")?,
            nodes[role].guard.peer().last.ok_or("peer parent")?,
        ];
        parents.sort();
        parents.dedup();
        let mut request = [0; 16];
        request[..4].copy_from_slice(&3u32.to_be_bytes());
        request[4..8].copy_from_slice(&(role as u32).to_be_bytes());
        request[8..].copy_from_slice(&n.to_be_bytes());
        let core = wire::Core2 {
            c: f.scope.context,
            d: f.scope.dialog,
            e: f.scope.epoch,
            sid: f.scope.session_id.clone(),
            role: role as u8,
            n,
            previous: nodes[role].guard.own().last,
            sequence: n,
            kind: 3,
            request,
            parents,
            expected: nodes[role].guard.expected_send()?,
            author_auth: f.actors.credentials[role].reference(),
            q: mode.proof.q(),
        };
        let text = format!("S3:3:{role}:{n}:Привет");
        let inner = wire::inner2(&core, a([b(text.as_bytes())]))?;
        let intent = guard::EventMeta {
            role: role as u8,
            n,
            previous: core.previous,
            sequence: n,
            kind: 3,
            native_kind: 1,
            key: [0; 32],
            index: core.expected.index,
            expected: core.expected.clone(),
        };
        let step = nodes[role]
            .guard
            .preflight_send(&intent, 0, nodes[role].generation)?;
        let mut trial = nodes[role].fork();
        calls += 1;
        let packet = trial.session.as_mut().ok_or("session")?.encrypt(&inner)?;
        let event = wire::sign_event(core, &packet, &f.actors.devices[role])?;
        let meta = event.meta();
        let observation = seen[role]
            .get(&(role as u8, meta.key))
            .copied()
            .map_or(SeenObservation::Absent, SeenObservation::Present);
        trial.guard.apply(step, &meta, event.id(), observation)?;
        trial.generation += 1;
        nodes[role] = trial;
        let verified = event
            .clone()
            .verify(&selection, &f.actors.credentials[role])?;
        let observation = seen[1 - role]
            .get(&(role as u8, meta.key))
            .copied()
            .map_or(SeenObservation::Absent, SeenObservation::Present);
        let step = nodes[1 - role].guard.preflight_receive(
            &meta,
            observation,
            0,
            nodes[1 - role].generation,
        )?;
        let mut trial = nodes[1 - role].fork();
        calls += 1;
        let plain = trial.session.as_mut().ok_or("session")?.decrypt(&packet)?;
        assert_eq!(plain, inner);
        assert_eq!(
            verified.verify_inner(&plain)?.value().fields(1)?[0].text()?,
            text
        );
        trial.guard.apply(step, &meta, event.id(), observation)?;
        trial.generation += 1;
        nodes[1 - role] = trial;
        for catalog in &mut seen {
            catalog
                .entry((role as u8, meta.key))
                .or_insert(guard::Seen {
                    generation: meta.expected.g,
                    first: event.id(),
                });
        }
        sources.push(event.raw().to_vec());
    }
    assert_eq!(calls, 40);
    assert_eq!(sources.len(), 20);
    assert_eq!(
        old_raw,
        f.trace
            .iter()
            .map(old::Event::raw)
            .collect::<std::result::Result<Vec<_>, _>>()?
    );
    for role in 0..2 {
        assert_eq!(nodes[role].account.curve_key(), initial_keys[role]);
        assert_eq!(
            nodes[role].session.as_ref().ok_or("session")?.sid(),
            f.scope.session_id
        );
        assert_eq!(
            nodes[role].session.as_ref().ok_or("session")?.keys(),
            f.keys
        );
        assert_eq!(nodes[role].guard.own().g, 2);
        assert_eq!(nodes[role].guard.peer().g, 2);
        let crypto = DeviceEncryptionIdentity::from_secret_bytes([31 + role as u8; 32]);
        let blob = nodes[role].seal(
            &f.scope,
            mode.proof.q(),
            role as u8,
            &crypto,
            [0x78; 32],
            f.actors.credentials[role].cert_ref(),
        )?;
        let restored = Native::open(
            &blob,
            &f.scope,
            mode.proof.q(),
            role as u8,
            &crypto,
            [0x78; 32],
            f.actors.credentials[role].cert_ref(),
        )?;
        assert_eq!(restored.guard, nodes[role].guard);
    }
    println!(
        "N03 sources=20 Text=18 native_calls={calls} cutA=9 cutB=9 pre_session_shapes=2 elapsed_seconds={:.3} in_memory_only=true",
        timer.elapsed().as_secs_f64()
    );
    Ok(())
}
#[test]
fn a3_s3_l1_import_eight_provenance_categories() -> TestResult {
    let f = LegacyFixture::new()?;
    let mut probe = ImportProbe::default();
    assert!(
        f.import(0, None, &f.blobs[0], OriginFact::CONFIG1, &mut probe)
            .is_err()
    );
    let mut wrong_aad = f.heads[0].clone();
    wrong_aad.generation += 1;
    wrong_aad.native_generation += 1;
    assert!(
        f.import(
            0,
            Some(&wrong_aad),
            &f.blobs[0],
            OriginFact::CONFIG1,
            &mut probe
        )
        .is_err()
    );
    assert!(
        f.import(
            0,
            Some(&f.heads[0]),
            &f.blobs[0],
            OriginFact {
                known_constructor: false,
                config: 1
            },
            &mut probe
        )
        .is_err()
    );
    let mut wrong_sid = f.heads[0].clone();
    wrong_sid.sid = Some("Z".repeat(43));
    assert!(
        f.import(
            0,
            Some(&wrong_sid),
            &f.blobs[0],
            OriginFact::CONFIG1,
            &mut probe
        )
        .is_err()
    );
    for damage in 0..4 {
        let mut state = f.states[0].fork();
        let g = state.guard.as_mut().ok_or("guard")?;
        match damage {
            0 => g.own.first = Some([99; 32]),
            1 => g.own.index += 1,
            2 => {
                let key = f.trace[0].key;
                g.own.seen.remove(&key);
            }
            _ => g.used_own_first = Some([99; 32]),
        }
        let mut head = f.heads[0].clone();
        let raw = state.seal(
            &mut head,
            f.nonce,
            f.actors.credentials[0]
                .certificate()
                .encryption_public_key(),
        )?;
        assert!(
            f.import(0, Some(&head), &raw, OriginFact::CONFIG1, &mut probe)
                .is_err(),
            "damage {damage}"
        );
    }
    let before = (
        f.heads[0].encode()?,
        f.blobs[0].clone(),
        postcard::to_allocvec(f.states[0].guard.as_ref().ok_or("guard")?)?,
    );
    let mut config_probe = ImportProbe::default();
    let error = f
        .import(
            0,
            Some(&f.heads[0]),
            &f.blobs[0],
            OriginFact {
                known_constructor: true,
                config: 2,
            },
            &mut config_probe,
        )
        .err()
        .ok_or("config accepted")?;
    assert_eq!(error.to_string(), "UnsupportedOrigin/config");
    assert_eq!(config_probe.boundaries, vec!["owner-verifier"]);
    assert_eq!(
        before,
        (
            f.heads[0].encode()?,
            f.blobs[0].clone(),
            postcard::to_allocvec(f.states[0].guard.as_ref().ok_or("guard")?)?
        )
    );
    let mut control = ImportProbe::default();
    f.import(
        0,
        Some(&f.heads[0]),
        &f.blobs[0],
        OriginFact::CONFIG1,
        &mut control,
    )?;
    assert_eq!(
        control.boundaries,
        vec!["owner-verifier", "native-restore/open", "guard-import"]
    );
    println!(
        "a3_s3_import_negative_categories=8 rejection_calls=9 missing_head_aad_calls=2 config2_probe={config_probe:?} config1_control={control:?} writes_capability=false actual_config2_pickle=false"
    );
    Ok(())
}
