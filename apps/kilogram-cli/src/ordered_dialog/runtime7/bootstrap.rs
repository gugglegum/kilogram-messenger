//! The wire1 bootstrap algorithms over the closed Root7 purposes.
use super::super::ratchet::Guard;
use super::*;
use kilogram_ratchet::ordered::Packet;
use owner::Owner7;

impl Owner7 {
    pub(super) fn original_auth(&mut self, c: u8, role: u64) -> Result<Vec<u8>> {
        let pair = self.control(c, store::L1Control::InitialPair)?;
        let value = wire::decode(&pair, 192 * 1024)?;
        let hello = Frame::parse(value.fields(6)?[2 + role as usize].bytes()?)?;
        Ok(hello.body.fields(11)?[7].bytes()?.to_vec())
    }
    pub(super) fn readable_row(
        &mut self,
        c: u8,
        event: &Event,
        text: Option<String>,
        queue: Option<u64>,
        original: Vec<u8>,
        receipt: Option<Vec<u8>>,
    ) -> Result<store::L1Readable> {
        let local = self.role(c)?;
        let role = event.core.role;
        let peer = 1 - local;
        let auth = |raw: Vec<u8>| db::HistoryAuth {
            id: wire::hash(b"", &raw),
            bytes: raw,
        };
        let (admission, final_auth, receiver) = if role == local {
            (None, Some(auth(self.credentials(c, local)?.raw)), None)
        } else {
            let lease = self.leases[usize::from(c)]
                .as_ref()
                .context("real admission capture")?;
            let admitted = lease.peer_credential.clone();
            let final_auth = self.credentials(c, peer)?.raw;
            let receiver = if event.core.kind == 3 {
                Some(auth(self.credentials(c, local)?.raw))
            } else {
                None
            };
            (Some(auth(admitted)), Some(auth(final_auth)), receiver)
        };
        Ok(store::L1Readable {
            author: *self.spec(c)?.peers[role as usize]
                .cert
                .device_id()
                .as_bytes(),
            source: event.raw()?,
            text,
            original: auth(original),
            admission,
            final_auth,
            receiver,
            receipt,
            queue,
        })
    }
    #[allow(clippy::too_many_arguments)] // Explicit native/event transcript inputs.
    pub(super) fn encrypt(
        &mut self,
        head: &Head,
        private: &mut PrivateState,
        kind: u64,
        request: [u8; 16],
        payload: Value,
        auth: Id,
        sequence: u64,
    ) -> Result<Event> {
        let guard = private.guard.as_ref().context("selected guard")?;
        let mut parents = Vec::new();
        if kind == 3 {
            if guard.own.next > 1 {
                parents.extend(guard.own.last);
            }
            if guard.peer.next > 1 {
                parents.extend(guard.peer.last);
            }
            parents.sort();
            parents.dedup();
        }
        let core = wire::Core {
            c: head.c,
            d: head.d.context("D")?,
            e: head.e.context("E")?,
            sid: head.sid.clone().context("SID")?,
            role: head.role,
            n: guard.own.next,
            previous: guard.own.last,
            sequence,
            kind,
            request,
            parents,
            expected: guard.expected_send()?,
            author_auth: auth,
        };
        let inner = Event::inner(&core, payload)?;
        self.stats.packet_attempts += 1;
        let packet = private
            .session
            .as_mut()
            .context("original session")?
            .encrypt(&inner)?;
        let header = packet.header()?;
        let mut event = Event {
            core,
            native_kind: u64::from(packet.kind),
            key: header.key,
            index: header.index,
            ciphertext: packet.bytes,
            signature: Vec::new(),
        };
        event.sign(self.device.identity())?;
        private
            .guard
            .as_mut()
            .context("guard")?
            .commit_send(&event, event.id()?)?;
        Ok(event)
    }
    #[allow(clippy::too_many_arguments)] // Closed atomic bootstrap transition.
    fn bootstrap_commit(
        &mut self,
        c: u8,
        binding: Id,
        kind: store::L1Bootstrap,
        mut head: Head,
        private: Option<PrivateState>,
        pair: Option<Vec<u8>>,
        controls: Vec<Vec<u8>>,
        rows: Vec<store::L1Readable>,
        ready: Vec<(u8, Vec<u8>)>,
    ) -> Result<()> {
        let encrypt = rows
            .iter()
            .filter(|r| r.author == *self.device.identity().device_id().as_bytes())
            .count() as u64;
        let decrypt = rows.len() as u64 - encrypt;
        let change = self.change(c, &mut head, private.as_ref())?;
        self.commit(
            c,
            binding,
            store::L1Operation::Bootstrap {
                kind,
                change,
                pair,
                controls,
                rows,
                ready,
            },
        )?;
        self.stats.committed_encrypt += encrypt;
        self.stats.committed_decrypt += decrypt;
        if let Some(private) = private {
            self.retain_native(c, &head, private);
        }
        self.wake.notify_waiters();
        Ok(())
    }
    pub fn ensure_offer(&mut self, c: u8, pair: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 0, "only A offers");
        let mut head = self.head(c)?;
        if let Some(id) = head.offer {
            return Ok(self.repo()?.signed_control(c, id)?);
        }
        let consent = self.control(c, store::L1Control::Consent)?;
        let transcript = handshake::verify_initial_pair(
            self.spec(c)?,
            pair,
            Some(wire::hash(handshake::CONSENT, &consent)),
        )?;
        let (private, _) = PrivateState::fresh(false)?;
        self.stats.account_constructors += 1;
        let refs = self.spec(c)?.cert_refs()?;
        let offer = wire::signed_control(
            1,
            a([
                b(head.c),
                b(refs[0]),
                b(refs[1]),
                b(transcript),
                b(admin::random::<32>()?),
                b(private.account.curve_key()),
                u(1),
                u(1),
            ]),
            self.device.identity(),
        )?;
        head.phase = 1;
        head.offer = Some(wire::hash(wire::CONTROL_ID, &offer));
        self.bootstrap_commit(
            c,
            binding,
            store::L1Bootstrap::Offer,
            head,
            Some(private),
            Some(pair.to_vec()),
            vec![offer.clone()],
            Vec::new(),
            Vec::new(),
        )?;
        Ok(offer)
    }
    pub fn grant(&mut self, c: u8, offer: &[u8], pair: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 1, "only B grants");
        let mut head = self.head(c)?;
        let offer_id = wire::hash(wire::CONTROL_ID, offer);
        let transcript = handshake::verify_initial_pair(self.spec(c)?, pair, None)?;
        let offer_body = wire::control(offer, 1, self.spec(c)?.peers[0].cert.device_id())?;
        let o = offer_body.fields(8)?;
        let refs = self.spec(c)?.cert_refs()?;
        ensure!(
            o[0].fixed::<32>()? == head.c
                && o[1].fixed::<32>()? == refs[0]
                && o[2].fixed::<32>()? == refs[1]
                && o[3].fixed::<32>()? == transcript
                && o[6].uint()? == 1
                && o[7].uint()? == 1,
            "Offer original scope/transcript"
        );
        if let Some(id) = head.offer {
            if id != offer_id {
                let old = self.repo()?.signed_control(c, id)?;
                self.signed_fork(c, "Offer", &old, offer)?;
            }
            return Ok(self
                .repo()?
                .signed_control(c, head.grant.context("Grant absent")?)?);
        }
        let (mut private, otk) = PrivateState::fresh(true)?;
        self.stats.account_constructors += 1;
        let otk = otk.context("reserved OTK")?;
        let grant = wire::signed_control(
            2,
            a([
                b(head.c),
                b(offer_id),
                b(transcript),
                b(admin::random::<32>()?),
                b(private.account.curve_key()),
                b(otk),
                u(1),
                u(1),
            ]),
            self.device.identity(),
        )?;
        let grant_id = wire::hash(wire::CONTROL_ID, &grant);
        let (d, e) = handshake::epoch(self.spec(c)?, offer, &grant)?;
        private.reservation = Some((offer_id, grant_id, otk, 0));
        head.phase = 2;
        head.offer = Some(offer_id);
        head.grant = Some(grant_id);
        head.d = Some(d);
        head.e = Some(e);
        self.bootstrap_commit(
            c,
            binding,
            store::L1Bootstrap::Grant,
            head,
            Some(private),
            Some(pair.to_vec()),
            vec![offer.to_vec(), grant.clone()],
            Vec::new(),
            Vec::new(),
        )?;
        Ok(grant)
    }
    pub fn init(&mut self, c: u8, grant: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 0, "only A opens outbound");
        let mut head = self.head(c)?;
        let grant_id = wire::hash(wire::CONTROL_ID, grant);
        let offer_id = head.offer.context("Offer missing")?;
        let offer = self.repo()?.signed_control(c, offer_id)?;
        let (d, e) = handshake::epoch(self.spec(c)?, &offer, grant)?;
        if let Some(id) = head.grant {
            if id != grant_id {
                let old = self.repo()?.signed_control(c, id)?;
                self.signed_fork(c, "Grant", &old, grant)?;
            }
            return Ok(self
                .event_at(c, 0, 0)?
                .context("durable A0 missing")?
                .raw()?);
        }
        let grant_body = wire::control(grant, 2, self.spec(c)?.peers[1].cert.device_id())?;
        let g = grant_body.fields(8)?;
        let mut private = self.native(c, &head)?;
        private.session = Some(private.account.outbound(g[4].fixed()?, g[5].fixed()?)?);
        self.stats.outbound_constructors += 1;
        private.guard = Some(Guard::new(0));
        head.phase = 3;
        head.grant = Some(grant_id);
        head.d = Some(d);
        head.e = Some(e);
        head.sid = Some(private.session.as_ref().context("outbound session")?.sid());
        let pair = self.control(c, store::L1Control::InitialPair)?;
        let transcript = handshake::verify_initial_pair(self.spec(c)?, &pair, None)?;
        let original = self.original_auth(c, 0)?;
        let keys = private.session.as_ref().context("session")?.keys();
        let sequence = self.repo()?.next_author_sequence()?;
        let request = offer_id[..16].try_into()?;
        let event = self.encrypt(
            &head,
            &mut private,
            1,
            request,
            a([
                b(offer_id),
                b(grant_id),
                b(transcript),
                a(keys.iter().map(b)),
            ]),
            wire::hash(b"", &original),
            sequence,
        )?;
        let row = self.readable_row(c, &event, None, None, original, None)?;
        let raw = event.raw()?;
        self.bootstrap_commit(
            c,
            binding,
            store::L1Bootstrap::Init,
            head,
            Some(private),
            None,
            vec![grant.to_vec()],
            vec![row],
            Vec::new(),
        )?;
        Ok(raw)
    }
    pub fn confirm(&mut self, c: u8, raw: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 1, "only B accepts prekey");
        let mut head = self.head(c)?;
        let event = Event::parse(raw, self.spec(c)?.peers[0].cert.device_id())?;
        self.check_scope(&head, &event, false)?;
        ensure!(
            event.core.kind == 1
                && event.core.n == 0
                && event.core.request.as_slice() == &head.offer.context("Offer")?[..16]
                && event.core.parents.is_empty(),
            "Init genesis shape"
        );
        if head.phase == 3 {
            let old = self.event_at(c, 0, 0)?.context("durable A0")?;
            if old.id()? != event.id()? {
                self.signed_fork(c, "Init", &old.raw()?, raw)?;
            }
            return Ok(self.event_at(c, 1, 0)?.context("durable B0")?.raw()?);
        }
        ensure!(head.phase == 2, "Grant required");
        let mut private = self.native(c, &head)?;
        let offer_id = head.offer.context("Offer")?;
        let grant_id = head.grant.context("Grant")?;
        let offer = self.repo()?.signed_control(c, offer_id)?;
        let offer_body = wire::control(&offer, 1, self.spec(c)?.peers[0].cert.device_id())?;
        let packet = Packet {
            kind: 0,
            bytes: event.ciphertext.clone(),
        };
        let keys = packet.header()?.prekey_keys.context("Init public tuple")?;
        ensure!(
            keys[0] == offer_body.fields(8)?[5].fixed::<32>()?
                && keys[2] == private.reservation.context("saved OTK")?.2
                && private.reservation.context("saved OTK")?.3 == 0,
            "Init original key/unused reservation"
        );
        let mut guard = Guard::new(1);
        guard.check_receive(&event)?;
        self.stats.packet_attempts += 1;
        let (session, plain) = private.account.inbound(keys[0], &packet)?;
        self.stats.inbound_constructors += 1;
        ensure!(
            session.sid() == event.core.sid && session.keys() == keys,
            "Init native SID/keys"
        );
        let inner = event.verify_inner(&plain)?;
        let f = inner.fields(4)?;
        ensure!(
            f[0].fixed::<32>()? == offer_id
                && f[1].fixed::<32>()? == grant_id
                && f[2] == offer_body.fields(8)?[3]
                && f[3] == a(keys.iter().map(b)),
            "Init inner transcript"
        );
        let original_a = self.original_auth(c, 0)?;
        ensure!(
            event.core.author_auth == wire::hash(b"", &original_a),
            "Init original auth"
        );
        guard.commit_receive(&event, event.id()?)?;
        private.session = Some(session);
        private.guard = Some(guard);
        private.reservation.as_mut().context("OTK")?.3 = 1;
        head.phase = 3;
        head.sid = Some(event.core.sid.clone());
        let row_a = self.readable_row(c, &event, None, None, original_a, None)?;
        let original_b = self.original_auth(c, 1)?;
        let sequence = self.repo()?.next_author_sequence()?;
        let b0 = self.encrypt(
            &head,
            &mut private,
            2,
            grant_id[..16].try_into()?,
            a([
                b(offer_id),
                b(grant_id),
                b(event.core.sid.as_bytes()),
                b(event.id()?),
            ]),
            wire::hash(b"", &original_b),
            sequence,
        )?;
        let row_b = self.readable_row(c, &b0, None, None, original_b, None)?;
        let ready = self.make_ready(&head, event.id()?, b0.id()?)?;
        let raw = b0.raw()?;
        self.bootstrap_commit(
            c,
            binding,
            store::L1Bootstrap::Confirm,
            head,
            Some(private),
            None,
            Vec::new(),
            vec![row_a, row_b],
            vec![(1, ready)],
        )?;
        Ok(raw)
    }
    pub fn receive_confirm(&mut self, c: u8, raw: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        ensure!(self.role(c)? == 0, "only A consumes Confirm");
        let head = self.head(c)?;
        let event = Event::parse(raw, self.spec(c)?.peers[1].cert.device_id())?;
        self.check_scope(&head, &event, true)?;
        ensure!(
            event.core.kind == 2
                && event.core.n == 0
                && event.core.request.as_slice() == &head.grant.context("Grant")?[..16]
                && event.core.parents.is_empty(),
            "Confirm genesis shape"
        );
        if let Some(old) = self.event_at(c, 1, 0)? {
            if old.id()? != event.id()? {
                self.signed_fork(c, "Confirm", &old.raw()?, raw)?;
            }
            return self.control(c, store::L1Control::Ready(0));
        }
        let mut private = self.native(c, &head)?;
        private
            .guard
            .as_ref()
            .context("guard")?
            .check_receive(&event)?;
        self.stats.packet_attempts += 1;
        let plain = private
            .session
            .as_mut()
            .context("session")?
            .decrypt(&Packet {
                kind: 1,
                bytes: event.ciphertext.clone(),
            })?;
        let inner = event.verify_inner(&plain)?;
        let f = inner.fields(4)?;
        let a0 = self.event_at(c, 0, 0)?.context("A0")?;
        ensure!(
            f[0].fixed::<32>()? == head.offer.context("Offer")?
                && f[1].fixed::<32>()? == head.grant.context("Grant")?
                && f[2].text()? == head.sid.as_deref().context("SID")?
                && f[3].fixed::<32>()? == a0.id()?,
            "Confirm inner transcript"
        );
        let original = self.original_auth(c, 1)?;
        ensure!(
            event.core.author_auth == wire::hash(b"", &original),
            "Confirm original auth"
        );
        private
            .guard
            .as_mut()
            .context("guard")?
            .commit_receive(&event, event.id()?)?;
        let row = self.readable_row(c, &event, None, None, original, None)?;
        let ready = self.make_ready(&head, a0.id()?, event.id()?)?;
        self.bootstrap_commit(
            c,
            binding,
            store::L1Bootstrap::ReceiveConfirm,
            head,
            Some(private),
            None,
            Vec::new(),
            vec![row],
            vec![(0, ready.clone())],
        )?;
        Ok(ready)
    }
    fn make_ready(&self, head: &Head, a0: Id, b0: Id) -> Result<Vec<u8>> {
        Ok(wire::signed_control(
            3,
            a([
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                b(head.sid.as_deref().context("SID")?.as_bytes()),
                b(a0),
                b(b0),
                u(head.role),
                u(1),
                u(1),
            ]),
            self.device.identity(),
        )?)
    }
    pub fn ready(&mut self, c: u8, raw: &[u8], binding: Id) -> Result<Vec<u8>> {
        self.check_lease(c, binding)?;
        let head = self.head(c)?;
        ensure!(head.phase == 3, "NeedHandshake");
        let peer = 1 - head.role;
        let body = wire::control(raw, 3, self.spec(c)?.peers[peer as usize].cert.device_id())?;
        let claimed = ready_claims(&head, peer, &body).context("Ready shape/scope refusal")?;
        let a0 = self.event_at(c, 0, 0)?.context("A0")?;
        let b0 = self.event_at(c, 1, 0)?.context("B0")?;
        let expected = a([
            b(head.d.context("D")?),
            b(head.e.context("E")?),
            b(head.sid.as_deref().context("SID")?.as_bytes()),
            b(a0.id()?),
            b(b0.id()?),
            u(peer),
            u(1),
            u(1),
        ]);
        if claimed != [a0.id()?, b0.id()?] {
            let prior = match self
                .repo()?
                .control(c, store::L1Control::Ready(peer as u8))?
            {
                Some(prior) => prior,
                None => {
                    if peer == 0 {
                        a0.raw()?
                    } else {
                        b0.raw()?
                    }
                }
            };
            self.signed_fork(c, "Ready genesis", &prior, raw)?;
        }
        ensure!(body == expected, "Ready bindings");
        if let Some(old) = self
            .repo()?
            .control(c, store::L1Control::Ready(peer as u8))?
        {
            if old != raw {
                self.signed_fork(c, "Ready", &old, raw)?;
            }
        } else {
            self.bootstrap_commit(
                c,
                binding,
                store::L1Bootstrap::Ready,
                head,
                None,
                None,
                Vec::new(),
                Vec::new(),
                vec![(peer as u8, raw.to_vec())],
            )?;
        }
        let own = self.role(c)?;
        self.control(c, store::L1Control::Ready(own as u8))
    }
    pub fn check_scope(&self, head: &Head, event: &Event, selected: bool) -> Result<()> {
        ensure!(
            event.core.c == head.c
                && head.d == Some(event.core.d)
                && head.e == Some(event.core.e)
                && (!selected || head.sid.as_ref() == Some(&event.core.sid)),
            "Event C/D/E/SID binding"
        );
        Ok(())
    }
    pub fn signed_fork(&mut self, c: u8, reason: &str, old: &[u8], new: &[u8]) -> Result<()> {
        ensure!(
            old.len() <= 32 * 1024 && new.len() <= 32 * 1024 && reason.len() <= 128,
            "fork proof bounds"
        );
        if self
            .repo()?
            .control(c, store::L1Control::BlockedFork)?
            .is_none()
        {
            let proof = wire::encode(&a([u(1), b(reason.as_bytes()), b(old), b(new)]), 64 * 1024)?;
            self.repo()?
                .commit(store::L1Operation::Block { context: c, proof }, None)?;
        }
        self.invalidate(c);
        anyhow::bail!("Blocked: signed fork {reason}")
    }
}

/// Shape/scope errors are ordinary refusals; only the returned same-scope
/// genesis claims may be compared with durable genesis and classified as a fork.
pub(super) fn ready_claims(head: &Head, peer: u64, body: &Value) -> Result<[Id; 2]> {
    let f = super::super::ready1::scoped_fields(head, peer, body)?;
    Ok([f[3].fixed::<32>()?, f[4].fixed::<32>()?])
}
