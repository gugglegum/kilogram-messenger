//! Serialized, at most four-packet trials. Accepted queue and pending stay durable.
use super::*;
use kilogram_crypto::SealedMessage;
use kilogram_ratchet::ordered::{NativeFailure, Packet};
use owner::Owner7;
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;
const LOCAL: &[u8] = b"kg:od1:local-projection\0";

impl Owner7 {
    fn open_queue(&self, c: Id, item: &store::L1Queue) -> Result<String> {
        let value = wire::decode(&item.sealed, 8192)?;
        let f = value.fields(3)?;
        ensure!(f[0].uint()? == 1, "local queue version");
        let raw = Zeroizing::new(self.device.encryption().open(
            &SealedMessage {
                encapsulated_key: f[1].fixed::<32>()?.to_vec(),
                ciphertext: f[2].bytes()?.to_vec(),
            },
            LOCAL,
            &wire::encode(&a([u(1), b(c), b(item.request), u(0)]), 1024)?,
        )?);
        let value = Zeroizing::new(wire::decode(&raw, 8192)?);
        let f = value.fields(4)?;
        ensure!(
            f[0].uint()? == 1
                && f[1].fixed::<32>()? == c
                && f[2].fixed::<16>()? == item.request
                && f[3].bytes()?.len() <= 4096,
            "queue original scope/body"
        );
        Ok(f[3].text()?.to_owned())
    }
    pub fn queue(&mut self, c: u8, request: [u8; 16], text: String) -> Result<store::L1Request> {
        let expires = self.current(c)?;
        ensure!(text.len() <= 4096, "Text body max4096 UTF8 bytes");
        let text_hash = wire::hash(b"kg:od1:request-text\0", text.as_bytes());
        if let Some(prior) = self.repo()?.request(c, request)? {
            if let Some(event) = prior.event {
                ensure!(
                    self.repo()?.projection(c, event)?.text == text,
                    "RequestIdConflict"
                );
            } else {
                let item = self
                    .repo()?
                    .queue_at(c, prior.queue_sequence.context("queued request sequence")?)?;
                ensure!(
                    self.open_queue(self.spec(c)?.c, &item)? == text,
                    "RequestIdConflict"
                );
            }
            return Ok(prior);
        }
        let role = self.role(c)? as usize;
        let meta = self.repo()?.metadata(c)?;
        let queued = self.repo()?.counts(c)?.queued;
        ensure!(
            meta.next[role].max(1) + queued < 512,
            "UpgradeRequired: send lane lifetime capacity"
        );
        ensure!(queued < 128, "StorageBlocked: unmaterialized queue");
        let context = self.spec(c)?.c;
        let plain = Zeroizing::new(wire::encode(
            &a([u(1), b(context), b(request), b(text.as_bytes())]),
            8192,
        )?);
        let sealed = self.device.encryption().public_key().seal(
            &plain,
            LOCAL,
            &wire::encode(&a([u(1), b(context), b(request), u(0)]), 1024)?,
        )?;
        let sealed = wire::encode(
            &a([u(1), b(sealed.encapsulated_key), b(sealed.ciphertext)]),
            8192,
        )?;
        let floor = now()?;
        let mut gate = || {
            if now().map_or(true, |wall| wall < floor || wall >= expires) {
                return Err(db::Error::PolicyRejected(
                    "queue current time/permission expired",
                ));
            }
            Ok(())
        };
        self.repo()?.commit(
            store::L1Operation::Queue {
                context: c,
                request,
                text_hash,
                sealed,
                now: floor,
            },
            Some(&mut gate),
        )?;
        self.wake.notify_waiters();
        self.repo()?
            .request(c, request)?
            .context("accepted request result")
    }
    pub fn materialize(&mut self, c: u8) -> Result<bool> {
        let counts = self.repo()?.counts(c)?;
        let available = materialize_limit(counts.queued, counts.debt)?;
        let schedule = super::super::test_support::load()?;
        if schedule.applies(self.spec(c)?.c) && schedule.suspend_materialization {
            return Ok(false);
        }
        if available == 0 || !self.active(c)? {
            return Ok(false);
        }
        let binding = self.leases[usize::from(c)]
            .as_ref()
            .context("AuthBlocked: materialize lease absent")?
            .binding;
        self.check_lease(c, binding)?;
        let items = self.repo()?.queue_page(c, None, available)?;
        if items.is_empty() {
            return Ok(false);
        }
        let _refs = self.window.reserve(c, items.len() * 2)?;
        let mut head = self.head(c)?;
        let mut private = self.native(c, &head)?;
        let mut rows = Vec::with_capacity(items.len());
        let mut sequence = self.repo()?.next_author_sequence()?;
        let original = self.credentials(c, head.role)?.raw;
        for item in items {
            self.check_lease(c, binding)?;
            let text = self.open_queue(head.c, &item)?;
            let event = self.encrypt(
                &head,
                &mut private,
                3,
                item.request,
                a([b(text.as_bytes())]),
                wire::hash(b"", &original),
                sequence,
            )?;
            rows.push(self.readable_row(
                c,
                &event,
                Some(text),
                Some(item.sequence),
                original.clone(),
                None,
            )?);
            sequence = sequence
                .checked_add(1)
                .context("author sequence exhausted")?;
        }
        let count = rows.len() as u64;
        let change = self.change(c, &mut head, Some(&private))?;
        self.commit(c, binding, store::L1Operation::Materialize { change, rows })?;
        self.retain_native(c, &head, private);
        self.stats.committed_encrypt += count;
        self.stats.materialized += count;
        self.wake.notify_waiters();
        Ok(true)
    }
    pub fn receive_events(
        &mut self,
        c: u8,
        events: Vec<Vec<u8>>,
        catalog: BTreeMap<Id, Vec<u8>>,
        binding: Id,
    ) -> Result<Vec<Id>> {
        self.check_lease(c, binding)?;
        ensure!(self.active(c)?, "NeedHandshake");
        ensure!(
            !events.is_empty() && events.len() <= 32 && catalog.len() <= 32,
            "wire1 batch cap"
        );
        let _refs = self.window.reserve(c, events.len())?;
        for (id, raw) in &catalog {
            ensure!(wire::hash(b"", raw) == *id, "auth catalog hash");
            Credentials::parse(raw)?;
        }
        let head = self.head(c)?;
        let peer = 1 - head.role;
        let cert = self.spec(c)?.peers[peer as usize].cert.clone();
        let mut accepted = Vec::new();
        for raw in events {
            let lease = self.check_lease(c, binding)?;
            let event = Event::parse(&raw, cert.device_id())?;
            self.check_scope(&head, &event, true)?;
            ensure!(
                event.core.role == peer && event.core.kind == 3,
                "data role/kind"
            );
            let id = event.id()?;
            if let Some(old) = self.event_at(c, peer, event.core.n)? {
                if old.id()? != id {
                    self.signed_fork(c, "ordinal", &old.raw()?, &raw)?;
                }
                accepted.push(id);
                continue;
            }
            if let Some(old) = self.repo()?.pending(c, event.core.n)? {
                if old.raw != raw {
                    self.signed_fork(c, "pending ordinal", &old.raw, &raw)?;
                }
                accepted.push(id);
                continue;
            }
            let original = match catalog.get(&event.core.author_auth) {
                Some(raw) => raw.clone(),
                None => self
                    .repo()?
                    .auth(c, event.core.author_auth)?
                    .context("original credential missing")?,
            };
            let author = Credentials::parse(&original)?;
            ensure!(
                author.reference() == event.core.author_auth
                    && author.cert_ref()? == self.spec(c)?.peers[peer as usize].cert_ref()?,
                "original credential/certificate binding"
            );
            if let Some((context, old)) = self
                .repo()?
                .authored(*cert.device_id().as_bytes(), event.core.sequence)?
                && old != id
            {
                let old_c = self.slot(context)?;
                let old = self.source_for_proof(old_c, old)?;
                self.signed_fork(c, "author sequence", &old, &raw)?;
            }
            let meta = self.repo()?.metadata(c)?;
            ensure!(
                event.core.n >= meta.next[peer as usize],
                "pending cannot precede readable frontier"
            );
            let ledger = self.repo()?.repair_state(c)?;
            let head_slot = event.core.n == meta.next[peer as usize] && ledger.head.is_none();
            ensure!(
                head_slot || ledger.pending < 64,
                "StorageBlocked: ordinary pending/head reserve"
            );
            let admission = lease.peer_credential;
            let op = store::L1Operation::Pending {
                context: c,
                author: *cert.device_id().as_bytes(),
                raw,
                original: db::HistoryAuth {
                    id: wire::hash(b"", &original),
                    bytes: original,
                },
                admission: db::HistoryAuth {
                    id: wire::hash(b"", &admission),
                    bytes: admission,
                },
                head: head_slot,
            };
            self.commit(c, binding, op)?;
            accepted.push(id);
        }
        self.wake.notify_waiters();
        Ok(accepted)
    }
    pub fn source_for_proof(&mut self, c: u8, id: Id) -> Result<Vec<u8>> {
        if let Some(raw) = self.repo()?.event(c, id)? {
            return Ok(raw);
        }
        // Only a conflict-proof lookup may inspect the bounded <=65 pending set.
        let mut after = None;
        loop {
            let page = self.repo()?.pending_page(c, after, 32)?;
            if page.is_empty() {
                break;
            }
            for item in &page {
                if wire::hash(wire::EVENT_ID, &item.raw) == id {
                    return Ok(item.raw.clone());
                }
            }
            after = page.last().map(|p| p.ordinal);
            if page.len() < 32 {
                break;
            }
        }
        anyhow::bail!("IntegrityBlocked: authored proof source missing")
    }
    pub fn make_receipt(&mut self, c: u8, event: &Event) -> Result<(Vec<u8>, Vec<u8>)> {
        ensure!(event.core.kind == 3, "genesis has no Text ACK");
        let role = self.role(c)?;
        let receiver = self.credentials(c, role)?;
        let body = a([
            u(1),
            b(event.core.d),
            b(event.core.e),
            u(event.core.role),
            u(event.core.n),
            b(event.id()?),
            b(receiver.cert_ref()?),
            b(receiver.reference()),
        ]);
        let signature =
            self.device
                .identity()
                .sign(&wire::signing_bytes(wire::RECEIPT_SIGN, &body, 1024)?);
        Ok((wire::encode(&a([body, b(signature)]), 1024)?, receiver.raw))
    }
    pub fn drain(&mut self, c: u8) -> Result<bool> {
        if self.repo()?.counts(c)?.pending == 0 || !self.active(c)? {
            return Ok(false);
        }
        let binding = self.leases[usize::from(c)]
            .as_ref()
            .context("AuthBlocked: drain lease absent")?
            .binding;
        self.check_lease(c, binding)?;
        // A gap or a quarantined head has no Native work. Check the committed
        // frontier before restoring the session or taking the drain refs.
        let meta = self.repo()?.metadata(c)?;
        let peer = 1 - self.role(c)?;
        let Some(first) = self.repo()?.pending(c, meta.next[peer as usize])? else {
            return Ok(false);
        };
        if first.quarantined {
            return Ok(false);
        }
        let _refs = self.window.reserve(c, 8)?;
        let mut head = self.head(c)?;
        let mut working = self.native(c, &head)?;
        let mut rows = Vec::new();
        let mut known = BTreeSet::new();
        let mut quarantine = None;
        for _ in 0..4 {
            let next = working.guard.as_ref().context("guard")?.peer.next;
            let Some(pending) = self.repo()?.pending(c, next)? else {
                break;
            };
            if pending.quarantined {
                break;
            }
            let event = Event::parse(
                &pending.raw,
                self.spec(c)?.peers[1 - head.role as usize].cert.device_id(),
            )?;
            self.check_scope(&head, &event, true)?;
            let mut parents_valid = true;
            for id in &event.core.parents {
                if !known.contains(id) {
                    if let Some(raw) = self.repo()?.event(c, *id)? {
                        let value = wire::decode(&raw, wire::MAX_EVENT)?;
                        let core = wire::Core::parse(&value.fields(5)?[1])?;
                        parents_valid &=
                            core.kind == 3 && core.c == head.c && core.e == event.core.e;
                    } else {
                        parents_valid = false;
                    }
                }
            }
            let mut trial = working.fork();
            let guard_result = trial.guard.as_ref().context("guard")?.check_receive(&event);
            if !parents_valid
                || matches!(guard_result, Err(super::super::ratchet::Failure::Input(_)))
            {
                quarantine = Some(next);
                break;
            }
            guard_result?;
            self.check_lease(c, binding)?;
            self.stats.packet_attempts += 1;
            let plain = match trial
                .session
                .as_mut()
                .context("original session")?
                .decrypt(&Packet {
                    kind: 1,
                    bytes: event.ciphertext.clone(),
                }) {
                Ok(plain) => plain,
                Err(NativeFailure::Input(_)) => {
                    self.stats.aborted_packet_calls += 1;
                    quarantine = Some(next);
                    break;
                }
                Err(error) => return Err(error.into()),
            };
            let text = (|| -> Result<String> {
                let payload = event.verify_inner(&plain)?;
                let text = payload.fields(1)?[0].text()?;
                ensure!(text.len() <= 4096, "Text body cap");
                Ok(text.to_owned())
            })();
            let text = match text {
                Ok(text) => text,
                Err(_) => {
                    self.stats.aborted_packet_calls += 1;
                    quarantine = Some(next);
                    break;
                }
            };
            trial
                .guard
                .as_mut()
                .context("guard")?
                .commit_receive(&event, event.id()?)?;
            let original = self
                .repo()?
                .auth(c, pending.original)?
                .context("pending original capture")?;
            let admission = self
                .repo()?
                .auth(c, pending.admission)?
                .context("pending admission capture")?;
            ensure!(
                wire::hash(b"", &original) == event.core.author_auth,
                "pending original bytes"
            );
            let (receipt, _) = self.make_receipt(c, &event)?;
            let mut row =
                self.readable_row(c, &event, Some(text), None, original, Some(receipt))?;
            row.admission = Some(db::HistoryAuth {
                id: pending.admission,
                bytes: admission,
            });
            rows.push(row);
            known.insert(event.id()?);
            working = trial;
        }
        let worked = !rows.is_empty();
        if worked {
            let count = rows.len() as u64;
            let change = self.change(c, &mut head, Some(&working))?;
            self.commit(c, binding, store::L1Operation::Drain { change, rows })?;
            self.retain_native(c, &head, working);
            self.stats.committed_decrypt += count;
            self.stats.readable += count;
        }
        if let Some(ordinal) = quarantine {
            self.commit(
                c,
                binding,
                store::L1Operation::Quarantine {
                    context: c,
                    ordinal,
                },
            )?;
        }
        if worked {
            self.wake.notify_waiters();
        }
        Ok(worked)
    }
    pub fn accept_receipt(
        &mut self,
        c: u8,
        raw: Vec<u8>,
        auth_raw: Vec<u8>,
        binding: Id,
    ) -> Result<()> {
        self.check_lease(c, binding)?;
        let head = self.head(c)?;
        let peer = 1 - head.role;
        let value = wire::decode(&raw, 1024)?;
        let f = value.fields(2)?;
        let r = f[0].fields(8)?;
        self.spec(c)?.peers[peer as usize].cert.device_id().verify(
            &wire::signing_bytes(wire::RECEIPT_SIGN, &f[0], 1024)?,
            f[1].bytes()?,
        )?;
        let receiver = Credentials::parse(&auth_raw)?;
        ensure!(
            receiver.cert_ref()? == self.spec(c)?.peers[peer as usize].cert_ref()?
                && receiver.reference() == r[7].fixed::<32>()?
                && r[0].uint()? == 1
                && Some(r[1].fixed::<32>()?) == head.d
                && Some(r[2].fixed::<32>()?) == head.e
                && r[3].uint()? == head.role
                && r[4].uint()? > 0
                && r[6].fixed::<32>()? == receiver.cert_ref()?,
            "Receipt1 exact signer/domain/lane"
        );
        let event = self
            .event_at(c, head.role, r[4].uint()?)?
            .context("receipt unknown source")?;
        ensure!(
            event.id()? == r[5].fixed::<32>()? && event.core.kind == 3,
            "receipt exact Text source"
        );
        self.commit(
            c,
            binding,
            store::L1Operation::Acknowledge {
                context: c,
                event: event.id()?,
                receipt: raw,
                receiver: db::HistoryAuth {
                    id: receiver.reference(),
                    bytes: auth_raw,
                },
            },
        )?;
        if super::super::test_support::load()?.applies(head.c) {
            super::super::test_support::checkpoint("l1-after-receipt-commit")?;
        }
        Ok(())
    }
}

pub(super) fn materialize_limit(queued: u64, debt: u64) -> Result<usize> {
    ensure!(
        debt <= store::L1_MAX_UNACKED,
        "IntegrityBlocked: L1 undelivered bound"
    );
    Ok(queued.min(4).min(store::L1_MAX_UNACKED - debt) as usize)
}
