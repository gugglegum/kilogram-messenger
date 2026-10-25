use super::*;
use kilogram_ratchet::ordered_v2::{self as guard, Seen, SeenObservation};
use native::Native2;
use owner::{Owner2, Purpose2};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;

pub(super) struct SendInput2<'a> {
    pub credential: &'a wire::RootVerifiedCredential,
    pub sequence: u64,
    pub request: [u8; 16],
    pub text: String,
}
/// At most four authenticated trial results. Discarded on every batch exit;
/// these observations never become an accepted source/checkpoint before commit.
pub(super) struct TrialView2 {
    pub context: u8,
    pub seen: BTreeMap<(u8, Id), Seen>,
    parents: BTreeSet<Id>,
}
impl TrialView2 {
    fn new(context: u8) -> Self {
        Self {
            context,
            seen: BTreeMap::new(),
            parents: BTreeSet::new(),
        }
    }
}
impl Owner2 {
    fn trial_parent(&mut self, c: u8, id: Id) -> Result<bool> {
        let _timing = timing::span(timing::Metric::ParentRead);
        if let Some(view) = &self.trial_view {
            ensure!(view.context == c, "trial parent context");
            if view.parents.contains(&id) {
                return Ok(true);
            }
        }
        Ok(self.repository.history_point(c, id)?.is_some())
    }
    fn record_trial(&mut self, event: &wire::ParsedEvent2, seen: SeenObservation) -> Result<()> {
        if let Some(view) = &mut self.trial_view {
            ensure!(view.parents.len() < 4, "native trial batch cap");
            let meta = event.meta();
            let observed = match seen {
                SeenObservation::Present(seen) => seen,
                SeenObservation::Absent => Seen {
                    generation: meta.expected.g,
                    first: event.id(),
                },
            };
            view.seen.insert((meta.role, meta.key), observed);
            view.parents.insert(event.id());
        }
        Ok(())
    }
    fn trial_number(&self) -> u8 {
        self.trial_view
            .as_ref()
            .map_or(1, |view| view.parents.len() as u8 + 1)
    }
    pub(super) fn trial_send(
        &mut self,
        c: u8,
        s: &db::RuntimeSnapshot,
        private: &mut Native2,
        selected: &wire::StaticSelection2,
        chosen: &db::RuntimeSelection,
        input: SendInput2<'_>,
    ) -> Result<(wire::ParsedEvent2, db::HistoryReadable2)> {
        let _timing = timing::span(timing::Metric::TrialSend);
        let SendInput2 {
            credential,
            sequence,
            mut request,
            text,
        } = input;
        let g = private.guard()?;
        let n = g.own().next;
        let kind = if n == 0 { s.head.role + 1 } else { 3 };
        if n == 0 {
            let id = if s.head.role == 0 {
                selected.proof().proof().offer_id()
            } else {
                selected.proof().proof().grant_id()
            }
            .context("selected control")?;
            request.copy_from_slice(&id[..16]);
        }
        let mut parents = Vec::with_capacity(2);
        if n > 1 {
            parents.extend(g.own().last);
        }
        if n > 0 && g.peer().next > 1 {
            parents.extend(g.peer().last);
        }
        parents.sort();
        parents.dedup();
        let core = wire::Core2 {
            c: s.head.context,
            d: selected.d(),
            e: selected.e(),
            sid: selected.sid().context("SID")?.to_owned(),
            role: s.head.role,
            n,
            previous: g.own().last,
            sequence,
            kind,
            request,
            parents,
            expected: g.expected_send()?,
            author_auth: credential.reference(),
            q: selected.proof().q(),
        };
        let offer = wire::ParsedControl2::parse(&chosen.offer)?;
        let grant = wire::ParsedControl2::parse(&chosen.grant)?;
        let payload = match kind {
            1 => a([
                b(offer.id()),
                b(grant.id()),
                offer.body().fields(8)?[3].clone(),
                a(private
                    .session
                    .as_ref()
                    .context("session")?
                    .keys()
                    .iter()
                    .map(b)),
            ]),
            2 => a([
                b(offer.id()),
                b(grant.id()),
                b(core.sid.as_bytes()),
                b(g.peer().first.context("A0 first")?),
            ]),
            _ => a([b(text.as_bytes())]),
        };
        let plain = Zeroizing::new(wire::inner2(&core, payload)?);
        let expected = guard::EventMeta {
            role: core.role,
            n,
            previous: core.previous,
            sequence,
            kind,
            native_kind: u8::from(kind != 1),
            key: [0; 32],
            index: core.expected.index,
            expected: core.expected.clone(),
        };
        let step = g.preflight_send(&expected, s.root_generation, s.head.native_generation)?;
        if !g.rotate_due() && n > 0 {
            let own = g.own();
            let mut meta = expected.clone();
            meta.key = own.key.context("own key")?;
            ensure!(
                self.seen(c, &meta)?
                    == SeenObservation::Present(Seen {
                        generation: own.g,
                        first: own.first.context("own first")?
                    }),
                "own continuation Seen"
            );
        }
        self.stats.packet_attempts += 1;
        eprintln!(
            "od2_native_attempt pid={} attempt={} context={c} role={} n={n} encrypt=1",
            std::process::id(),
            self.stats.packet_attempts,
            s.head.role
        );
        let packet = {
            let _timing = timing::span(timing::Metric::NativeEncrypt);
            private
                .session
                .as_mut()
                .context("session")?
                .encrypt(&plain)?
        };
        hooks::after_native_trial(c, self.trial_number())?;
        let event = wire::sign_event(core, &packet, &self.device)?;
        eprintln!(
            "od2_packet_header context={c} encrypt=1 n={} g={} index={} id={}",
            n,
            event.core().expected.g,
            event.meta().index,
            hex(&event.id())
        );
        event
            .clone()
            .verify(selected, credential)?
            .verify_inner(&plain)?;
        let meta = event.meta();
        let seen = self.seen(c, &meta)?;
        private.guard_mut()?.apply(step, &meta, event.id(), seen)?;
        self.record_trial(&event, seen)?;
        Ok((
            event.clone(),
            db::HistoryReadable2 {
                source: event.raw().to_vec(),
                text,
                auth: auth(&[credential]),
                readable_author_ref: credential.reference(),
                receiver_receipt: None,
            },
        ))
    }
    pub(super) fn trial_receive(
        &mut self,
        c: u8,
        s: &db::RuntimeSnapshot,
        private: &mut Native2,
        selected: &wire::StaticSelection2,
        chosen: &db::RuntimeSelection,
        verified: &wire::VerifiedEvent2,
    ) -> Result<String> {
        let _timing = timing::span(timing::Metric::TrialReceive);
        let event = verified.event();
        let meta = event.meta();
        for parent in &event.core().parents {
            ensure!(self.trial_parent(c, *parent)?, "NeedParent");
        }
        let seen = self.seen(c, &meta)?;
        let step = private.guard()?.preflight_receive(
            &meta,
            seen,
            s.root_generation,
            s.head.native_generation,
        )?;
        self.stats.packet_attempts += 1;
        eprintln!(
            "od2_native_attempt pid={} attempt={} context={c} role={} n={} decrypt=1",
            std::process::id(),
            self.stats.packet_attempts,
            meta.role,
            meta.n
        );
        let plain = Zeroizing::new({
            let _timing = timing::span(timing::Metric::NativeDecrypt);
            if let Some(session) = &mut private.session {
                session.decrypt(&event.packet())?
            } else {
                ensure!(
                    s.head.phase == 2 && s.head.role == 1 && meta.n == 0 && meta.kind == 1,
                    "inbound constructor phase"
                );
                let packet = event.packet();
                let keys = packet.header()?.prekey_keys.context("Init prekey")?;
                let r = private.reservation.context("reserved OTK")?;
                ensure!(r.2 == keys[2] && r.3 == 0, "OTK selection");
                let (session, plain) = private.account.inbound(keys[0], &packet)?;
                eprintln!("od2_session_constructor context={c} role=1");
                ensure!(
                    session.keys() == keys && session.sid() == event.core().sid,
                    "native constructor keys/SID"
                );
                private.session = Some(session);
                plain
            }
        });
        hooks::after_native_trial(c, self.trial_number())?;
        let payload = verified.verify_inner(&plain)?;
        let text = if meta.kind == 3 {
            payload.value().fields(1)?[0].text()?.to_owned()
        } else {
            let p = payload.value().fields(4)?;
            let offer = wire::ParsedControl2::parse(&chosen.offer)?;
            let grant = wire::ParsedControl2::parse(&chosen.grant)?;
            ensure!(
                p[0].fixed::<32>()? == offer.id() && p[1].fixed::<32>()? == grant.id(),
                "genesis selection payload"
            );
            if meta.kind == 1 {
                ensure!(
                    p[2] == offer.body().fields(8)?[3]
                        && p[3]
                            == a(private
                                .session
                                .as_ref()
                                .context("session")?
                                .keys()
                                .iter()
                                .map(b)),
                    "Init transcript/keys"
                );
            } else {
                ensure!(
                    p[2].text()? == selected.sid().context("SID")?
                        && p[3].fixed::<32>()? == private.guard()?.own().first.context("own A0")?,
                    "Confirm A0/SID"
                );
            }
            String::new()
        };
        private.guard_mut()?.apply(step, &meta, event.id(), seen)?;
        self.record_trial(event, seen)?;
        Ok(text)
    }
    pub fn queue(
        &mut self,
        c: u8,
        request: [u8; 16],
        text: String,
    ) -> Result<db::HistoryRequestOutcome> {
        let _timing = timing::span(timing::Metric::QueueOperation);
        ensure!(text.len() <= wire::MAX_TEXT, "Text cap");
        ensure!(self.repository.installed(c)?, "NeedHandshake");
        let s = self.repository.snapshot(c)?;
        if s.head.phase == 3
            && let Some((outcome, old)) = self.repository.request(c, request)?
        {
            ensure!(old == text, "RequestIdConflict");
            return Ok(outcome);
        }
        ensure!(self.enabled.contains(&c), "context is paused");
        ensure!(s.head.phase == 3, "NeedHandshake");
        self.current(&s, now()?)?;
        self.commit(
            &s,
            db::RuntimeOperation::Queue {
                context: c,
                fence: s.fence(now()?),
                request,
                text,
            },
            Purpose2::Local,
            (0, 0),
        )?;
        Ok(self
            .repository
            .request(c, request)?
            .context("accepted request")?
            .0)
    }
    pub(super) fn materialize(&mut self, c: u8) -> Result<bool> {
        let _timing = timing::span(timing::Metric::MaterializeTurn);
        ensure!(c < 2, "context slot");
        // Root counters are authenticated, generation-coherent and checked for
        // poison. An empty turn has no trial/dispatch needing policy authority.
        if self.repository.counts(c)?.queued == 0 {
            return Ok(false);
        }
        let _active_timing = timing::span(timing::Metric::MaterializeActive);
        let (s, current) = self.require(c, false)?;
        if !s.head.active || s.head.blocked.is_some() {
            return Ok(false);
        }
        let count = self.repository.counts(c)?.queued.min(4) as usize;
        if count == 0 {
            return Ok(false);
        }
        let Ok(_permit) = self.window.reserve(c, count) else {
            return Ok(false);
        };
        let items = self.repository.queue(c, 0, count)?;
        ensure!(items.len() == count, "queue batch cardinality");
        let selected = self.selected(&s)?;
        let mut trial = self.native(c, &s)?;
        ensure!(self.trial_view.is_none(), "nested native batch");
        self.trial_view = Some(TrialView2::new(c));
        self.stats.ram_refs[usize::from(c)] = count;
        self.stats.max_ram_refs[usize::from(c)] =
            self.stats.max_ram_refs[usize::from(c)].max(count);
        let result = (|| -> Result<bool> {
            let mut rows = Vec::with_capacity(count);
            let mut ids = Vec::with_capacity(count);
            for (offset, item) in items.into_iter().enumerate() {
                let (event, row) = self.trial_send(
                    c,
                    &s,
                    &mut trial,
                    &selected,
                    &s.selection,
                    SendInput2 {
                        credential: &current.peers[usize::from(s.head.role)],
                        sequence: s
                            .profile
                            .next_author_sequence
                            .checked_add(offset as u64)
                            .context("author sequence")?,
                        request: item.request,
                        text: item.text,
                    },
                )?;
                ids.push(event.id());
                rows.push(row);
            }
            let mut target = s.head.clone();
            target.native_generation = target
                .native_generation
                .checked_add(1)
                .context("native generation")?;
            let after = trial.seal(&s.profile, &target, &self.encryption)?;
            self.stats.max_native_bytes = self.stats.max_native_bytes.max(after.len());
            let hash = wire::hash(b"", &after);
            self.commit(
                &s,
                db::RuntimeOperation::Materialize {
                    context: c,
                    fence: s.fence(now()?),
                    rows,
                    native: db::HistoryNativeTransition {
                        before_hash: s.head.native_hash,
                        after_blob: after,
                        frame_ids: ids,
                    },
                },
                Purpose2::Local,
                (count as u64, 0),
            )?;
            self.cache_native(c, hash, trial)?;
            self.stats.materialized += count as u64;
            Ok(true)
        })();
        self.trial_view = None;
        self.stats.ram_refs[usize::from(c)] = 0;
        result
    }
    pub(super) fn credential(
        &mut self,
        c: u8,
        id: Id,
        supplied: &BTreeMap<Id, Vec<u8>>,
    ) -> Result<wire::RootVerifiedCredential> {
        let _timing = timing::span(timing::Metric::Credential);
        if let Some((_, credential)) = self.credential_cache.iter().find(|(old, _)| *old == id) {
            ensure!(
                supplied
                    .get(&id)
                    .is_none_or(|raw| raw.as_slice() == credential.raw()),
                "authority exact bytes"
            );
            return Ok(credential.clone());
        }
        let raw = if let Some(raw) = supplied.get(&id) {
            raw.clone()
        } else {
            self.repository.authority(c, id)?
        };
        let parsed = wire::ParsedCredentialBundle::parse(&raw)?;
        ensure!(parsed.reference() == id, "authority hash");
        let credential = parsed.verify_root()?;
        if self.credential_cache.len() == 8 {
            self.credential_cache.pop_front();
        }
        self.credential_cache.push_back((id, credential.clone()));
        Ok(credential)
    }
    pub(super) fn ingest(
        &mut self,
        c: u8,
        raw: Vec<u8>,
        supplied: &BTreeMap<Id, Vec<u8>>,
        head: bool,
        grant: Option<Id>,
    ) -> Result<Id> {
        let _timing = timing::span(timing::Metric::IngestOperation);
        let (s, current) = self.require(c, true)?;
        ensure!(
            s.head.active && s.head.blocked.is_none(),
            "NeedHandshake/Blocked"
        );
        let selected = self.selected(&s)?;
        let event = wire::ParsedEvent2::parse(&raw)?;
        ensure!(
            event.core().role == 1 - s.head.role && event.core().kind == 3 && event.core().n > 0,
            "peer Text role"
        );
        let old = if let Some(old) = self
            .repository
            .source(c, event.core().role, event.core().n)?
        {
            Some(old.raw)
        } else {
            self.repository.pending(c, event.core().n)?.map(|p| p.body)
        };
        if old.as_ref() == Some(&raw) {
            return Ok(event.id());
        }
        let original = self.credential(c, event.core().author_auth, supplied)?;
        event.clone().verify(&selected, &original)?;
        if let Some(old) = old {
            self.block(
                c,
                &s,
                "signed source fork",
                vec![old, raw, original.raw().to_vec()],
            )?;
            anyhow::bail!("signed source fork");
        }
        let peer = &current.peers[usize::from(1 - s.head.role)];
        self.commit(
            &s,
            db::RuntimeOperation::AcceptPending {
                context: c,
                fence: s.fence(now()?),
                source: raw,
                head,
                grant,
                admission_author_ref: peer.reference(),
                auth: auth(&[&original, peer]),
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        let pending = self.repository.repair_state(c)?;
        self.stats.max_pending[usize::from(c)] =
            self.stats.max_pending[usize::from(c)].max(pending.pending);
        if head {
            self.stats.head_repairs[usize::from(c)] += 1;
            eprintln!(
                "od2_funded_head context={c} ordinary_pending={} ordinal={} id={}",
                pending.pending,
                event.core().n,
                hex(&event.id())
            );
        }
        Ok(event.id())
    }
    pub(super) fn drain(&mut self, c: u8) -> Result<bool> {
        let _timing = timing::span(timing::Metric::DrainTurn);
        ensure!(c < 2, "context slot");
        let credits = self.repository.credits()?;
        if credits.pending[usize::from(c)] == 0 && !credits.head_in_use[usize::from(c)] {
            return Ok(false);
        }
        let _active_timing = timing::span(timing::Metric::DrainActive);
        let (s, current) = self.require(c, true)?;
        let schedule = hooks::load()?;
        if !s.head.active
            || s.head.blocked.is_some()
            || (schedule.applies(c) && schedule.suspend_drain)
        {
            return Ok(false);
        }
        let first = self.repository.frontiers(c)?.next[usize::from(1 - s.head.role)];
        if self.repository.pending(c, first)?.is_none() {
            return Ok(false);
        }
        let mut private = self.native(c, &s)?;
        ensure!(
            private.guard()?.peer().next == first,
            "Native/readable frontier mismatch"
        );
        let selected = self.selected(&s)?;
        ensure!(self.trial_view.is_none(), "nested native batch");
        self.trial_view = Some(TrialView2::new(c));
        let result = (|| -> Result<bool> {
            let own = &current.peers[usize::from(s.head.role)];
            let peer = &current.peers[usize::from(1 - s.head.role)];
            let mut rows = Vec::with_capacity(4);
            let mut events = Vec::with_capacity(4);
            for offset in 0..4 {
                let n = first.checked_add(offset).context("pending ordinal")?;
                let Some(pending) = self.repository.pending(c, n)? else {
                    break;
                };
                let event = wire::ParsedEvent2::parse(&pending.body)?;
                let original = self.credential(c, event.core().author_auth, &BTreeMap::new())?;
                let verified = event.clone().verify(&selected, &original)?;
                let mut missing = false;
                for id in &event.core().parents {
                    if !self.trial_parent(c, *id)? {
                        missing = true;
                        break;
                    }
                }
                if missing {
                    break;
                }
                let text = match self.trial_receive(
                    c,
                    &s,
                    &mut private,
                    &selected,
                    &s.selection,
                    &verified,
                ) {
                    Ok(text) => text,
                    Err(error) => {
                        if error.downcast_ref::<db::Error>().is_none()
                            && !matches!(
                                error.downcast_ref::<kilogram_ratchet::ordered::NativeFailure>(),
                                Some(kilogram_ratchet::ordered::NativeFailure::Operational(_))
                            )
                        {
                            self.block(
                                c,
                                &s,
                                "invalid signed pending",
                                vec![pending.body.clone(), original.raw().to_vec()],
                            )?;
                        }
                        return Err(error);
                    }
                };
                let receipt = wire::ParsedReceipt2::sign(
                    wire::ReceiptFields2 {
                        c: s.head.context,
                        d: selected.d(),
                        e: selected.e(),
                        sid: selected.sid().context("SID")?.to_owned(),
                        q: selected.proof().q(),
                        format: 2,
                        lane: event.core().role,
                        n,
                        event: event.id(),
                        receiver_cert: own.cert_ref(),
                        receiver_auth: own.reference(),
                    },
                    &self.device,
                )?;
                receipt.clone().verify(&verified, &selected, own)?;
                rows.push(db::HistoryReadable2 {
                    source: pending.body,
                    text,
                    auth: auth(&[own, peer]),
                    readable_author_ref: peer.reference(),
                    receiver_receipt: Some(receipt.raw().to_vec()),
                });
                events.push((event, private.guard()?.clone()));
            }
            if rows.is_empty() {
                return Ok(false);
            }
            let count = rows.len();
            let mut target = s.head.clone();
            target.native_generation = target
                .native_generation
                .checked_add(1)
                .context("native generation")?;
            let after = private.seal(&s.profile, &target, &self.encryption)?;
            self.stats.max_native_bytes = self.stats.max_native_bytes.max(after.len());
            let hash = wire::hash(b"", &after);
            self.commit(
                &s,
                db::RuntimeOperation::Drain {
                    context: c,
                    fence: s.fence(now()?),
                    rows,
                    native: db::HistoryNativeTransition {
                        before_hash: s.head.native_hash,
                        after_blob: after,
                        frame_ids: events.iter().map(|(event, _)| event.id()).collect(),
                    },
                },
                Purpose2::Peer,
                (0, count as u64),
            )?;
            self.cache_native(c, hash, private)?;
            for (event, guard) in events {
                eprintln!(
                    "od2_readable_commit context={c} role={} n={} g={} index={} id={} own_g={} own_index={} own_first={} rotate_due={}",
                    event.core().role,
                    event.core().n,
                    event.meta().expected.g,
                    event.meta().index,
                    hex(&event.id()),
                    guard.own().g,
                    guard.own().index,
                    guard.own().first.map(|id| hex(&id)).unwrap_or_default(),
                    guard.rotate_due()
                );
            }
            self.stats.readable += count as u64;
            Ok(true)
        })();
        self.trial_view = None;
        result
    }
    fn block(
        &mut self,
        c: u8,
        s: &db::RuntimeSnapshot,
        reason: &str,
        evidence: Vec<Vec<u8>>,
    ) -> Result<()> {
        self.commit(
            s,
            db::RuntimeOperation::Block {
                context: c,
                fence: s.fence(now()?),
                reason: reason.into(),
                evidence,
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        if let Some(lease) = self.leases[usize::from(c)].take() {
            lease.invalidate();
        }
        Ok(())
    }
    pub(super) fn acknowledge(
        &mut self,
        c: u8,
        raw: Vec<u8>,
        supplied: &BTreeMap<Id, Vec<u8>>,
    ) -> Result<()> {
        let _timing = timing::span(timing::Metric::AcknowledgeOperation);
        let (s, _) = self.require(c, true)?;
        let selected = self.selected(&s)?;
        let receipt = wire::ParsedReceipt2::parse(&raw)?;
        let r = receipt.fields();
        ensure!(r.lane == s.head.role, "receipt own lane");
        if self.repository.receipt(c, r.lane, r.n)?.as_ref() == Some(&raw) {
            return Ok(());
        }
        let source = self
            .repository
            .source(c, r.lane, r.n)?
            .context("receipt source")?;
        let event = wire::ParsedEvent2::parse(&source.raw)?;
        let author = self.credential(c, event.core().author_auth, &BTreeMap::new())?;
        let verified = event.verify(&selected, &author)?;
        let receiver = self.credential(c, r.receiver_auth, supplied)?;
        receipt.verify(&verified, &selected, &receiver)?;
        self.commit(
            &s,
            db::RuntimeOperation::Acknowledge {
                context: c,
                fence: s.fence(now()?),
                raw,
                auth: auth(&[&receiver]),
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        Ok(())
    }
}
