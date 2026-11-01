//! Mixed data uses the original Native session and the selected Guard2. Wire
//! format chooses verification once; an error never retries a different format.
use super::*;
use kilogram_protocol::ordered_dialog_v2 as v2;
use kilogram_ratchet::ordered_v2::{self as g2, Seen, SeenObservation};
use native::{MixedNative7, Native7};
use owner::Owner7;
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;

pub(super) struct Facts7 {
    pub format: u8,
    pub id: Id,
    pub meta: g2::EventMeta,
    pub author: Id,
    pub parents: Vec<Id>,
}
pub(super) fn declared(raw: &[u8]) -> Result<(u8, u8, Id)> {
    let value = wire::decode(raw, v2::MAX_EVENT)?;
    let f = value.fields(5)?;
    match f[0].uint()? {
        1 => {
            let core = wire::Core::parse(&f[1])?;
            Ok((1, core.role.try_into()?, core.author_auth))
        }
        2 => {
            let e = v2::ParsedEvent2::parse(raw)?;
            Ok((2, e.core().role, e.core().author_auth))
        }
        _ => anyhow::bail!("unsupported source format"),
    }
}
pub(super) fn facts(event: &v2::MixedEvent) -> Result<Facts7> {
    Ok(match event {
        v2::MixedEvent::Legacy(v) => {
            let e = v.event();
            Facts7 {
                format: 1,
                id: e.id()?,
                meta: super::super::storage_migration::legacy_event_meta(e)?,
                author: e.core.author_auth,
                parents: e.core.parents.clone(),
            }
        }
        v2::MixedEvent::Current(v) => {
            let e = v.event();
            Facts7 {
                format: 2,
                id: e.id(),
                meta: e.meta(),
                author: e.core().author_auth,
                parents: e.core().parents.clone(),
            }
        }
    })
}
fn observation(raw: Option<Vec<u8>>) -> Result<SeenObservation> {
    match raw {
        None => Ok(SeenObservation::Absent),
        Some(raw) => {
            ensure!(raw.len() == 40, "mixed Seen value");
            Ok(SeenObservation::Present(Seen {
                generation: u64::from_be_bytes(raw[..8].try_into()?),
                first: raw[8..].try_into()?,
            }))
        }
    }
}
fn auth(bytes: Vec<u8>) -> db::HistoryAuth {
    db::HistoryAuth {
        id: wire::hash(b"", &bytes),
        bytes,
    }
}

impl Owner7 {
    pub(super) fn mixed_native(&mut self, c: u8, head: &Head) -> Result<MixedNative7> {
        let state = self.repo()?.existing_state(c)?;
        ensure!(state.mode.selected(), "Mixed7 requires local selection");
        let hash = head.blob.context("Mixed7 absent")?;
        if let Some((saved, Native7::Mixed(native))) = &self.native_cache[usize::from(c)]
            && *saved == hash
        {
            return Ok(native.fork());
        }
        self.selection2(c)?;
        let snapshot = self.repo()?.snapshot(c)?;
        let native = MixedNative7::open(
            &self.mixed_identity,
            head,
            state.q.context("Q")?,
            self.device.encryption(),
            snapshot
                .native
                .as_deref()
                .context("Mixed7 current native")?,
        )?;
        let role = head.role as usize;
        for (r, lane) in [(role, native.guard.own()), (1 - role, native.guard.peer())] {
            ensure!(
                lane.next == snapshot.meta.next[r] && lane.last == snapshot.meta.tails[r],
                "Mixed7/frontier correspondence"
            );
            if let Some(key) = lane.key {
                ensure!(
                    observation(self.repo()?.seen(c, r as u8, key)?)?
                        == SeenObservation::Present(Seen {
                            generation: lane.g,
                            first: lane.first.context("Mixed7 first")?
                        }),
                    "Mixed7 current Seen correspondence"
                );
            }
        }
        let trial = native.fork();
        self.native_cache[usize::from(c)] = Some((hash, Native7::Mixed(native)));
        Ok(trial)
    }
    fn mixed_change(
        &mut self,
        c: u8,
        head: &mut Head,
        native: &MixedNative7,
    ) -> Result<store::L1NativeChange> {
        let before = self.repo()?.head(c)?;
        let q = self.repo()?.existing_state(c)?.q.context("Q")?;
        head.bump(true)?;
        let raw = native.seal(&self.mixed_identity, head, q, self.device.encryption())?;
        self.stats.max_native_bytes = self.stats.max_native_bytes.max(raw.len());
        Ok(store::L1NativeChange {
            context: c,
            expected_head: wire::hash(b"", &before),
            policy_generation: self.repo()?.metadata(c)?.policy_generation,
            wall_time: now()?,
            head: head.encode()?,
            native: Some(raw),
        })
    }
    fn retain_mixed(&mut self, c: u8, head: &Head, native: MixedNative7) {
        if let Some(hash) = head.blob {
            self.native_cache[usize::from(c)] = Some((hash, Native7::Mixed(native)));
        }
    }
    pub(super) fn commit_data2(
        &mut self,
        c: u8,
        binding: Option<Id>,
        data: store::ExistingData7,
    ) -> Result<db::CommitReport> {
        let mode = self.repo()?.existing_state(c)?.mode;
        let materialize = matches!(&data, store::ExistingData7::Materialize { .. });
        ensure!(
            !materialize || mode == store::ExistingMode::Active,
            "existing local materialization requires Active"
        );
        ensure!(
            binding.is_some()
                || materialize
                || matches!(
                    &data,
                    store::ExistingData7::Queue { .. } | store::ExistingData7::Block { .. }
                ),
            "existing data requires actual lease"
        );
        let expires = self.current_existing(c)?;
        let floor = now()?;
        let lease = binding
            .map(|id| self.check_existing_lease(c, id))
            .transpose()?;
        let pre_p = self.repo()?.existing_state(c)?.mode == store::ExistingMode::L1;
        let native = matches!(
            &data,
            store::ExistingData7::Materialize { .. } | store::ExistingData7::Readable { .. }
        );
        let op = store::ExistingOperation7 {
            context: c,
            expected_root: self.repo()?.generation(),
            expected_head: wire::hash(b"", &self.repo()?.head(c)?),
            policy_generation: self.repo()?.metadata(c)?.policy_generation,
            wall_time: floor,
            action: store::ExistingAction7::Data(data),
        };
        let mut gate = || {
            if let Some(lease) = &lease {
                lease.gate()?;
                if pre_p
                    && lease
                        .opportunity_deadline
                        .is_none_or(|deadline| std::time::Instant::now() >= deadline)
                {
                    return Err(db::Error::PolicyRejected("pre-P opportunity expired"));
                }
            }
            if now().map_or(true, |n| n < floor || n >= expires) {
                return Err(db::Error::PolicyRejected("mixed final permission/clock"));
            }
            Ok(())
        };
        if native {
            super::super::test_support::checkpoint("s53-data-before-native-commit")?;
        }
        let result = self.repo()?.commit_existing(op, Some(&mut gate));
        if self.repo()?.requires_reopen() {
            for slot in 0..self.specs.len() {
                self.invalidate(slot as u8);
            }
            if native {
                self.stats.ambiguous_packet_calls += 1;
            }
        }
        let report = result?;
        self.sync_high_water()?;
        if native {
            super::super::test_support::checkpoint("s53-data-after-native-commit")?;
        }
        self.stats.max_commit_app_bytes = self
            .stats
            .max_commit_app_bytes
            .max(report.app_buffer_bound_bytes);
        Ok(report)
    }
    pub(super) fn verify_source2(
        &mut self,
        c: u8,
        item: &v2::TaggedItem,
        catalog: &BTreeMap<Id, Vec<u8>>,
    ) -> Result<v2::MixedEvent> {
        let (format, role, reference) = declared(&item.raw)?;
        ensure!(
            item.format == format && role < 2,
            "explicit source format/role"
        );
        let raw = match catalog.get(&reference) {
            Some(raw) => raw.clone(),
            None => self
                .repo()?
                .auth(c, reference)?
                .context("exact historical author absent")?,
        };
        let author = v2::ParsedCredentialBundle::parse(&raw)?.verify_root()?;
        ensure!(
            author.reference() == reference,
            "historical author reference"
        );
        let state = self.repo()?.existing_state(c)?;
        if state.mode.selected() {
            Ok(item.verify_event(&self.selection2(c)?, &author)?)
        } else {
            ensure!(format == 1, "Event2 forbidden before local selection");
            let verified = self.scope2(c)?.verify_event(&item.raw, &author)?;
            ensure!(
                state.cuts[role as usize].is_none_or(|cut| verified.event().core.n < cut),
                "legacy source at/above known cut"
            );
            Ok(v2::MixedEvent::Legacy(verified))
        }
    }
    pub(super) fn receive2(
        &mut self,
        c: u8,
        binding: Id,
        items: Vec<v2::TaggedItem>,
        catalog: BTreeMap<Id, Vec<u8>>,
        grant: Option<Id>,
        head_grant: bool,
    ) -> Result<Vec<Id>> {
        self.check_existing_lease(c, binding)?;
        ensure!(
            !items.is_empty() && items.len() <= 16,
            "existing runtime item count"
        );
        let selected = self.repo()?.existing_state(c)?.mode.selected();
        // Reject the whole batch before any possession or pending write.
        ensure!(
            selected || items.iter().all(|item| item.format == 1),
            "Event2 forbidden before local selection"
        );
        let peer = 1 - self.role(c)?;
        let mut verified = Vec::with_capacity(items.len());
        for item in &items {
            let event = self.verify_source2(c, item, &catalog)?;
            let f = facts(&event)?;
            ensure!(
                u64::from(f.meta.role) == peer && f.meta.kind == 3,
                "existing data peer/Text role"
            );
            verified.push(f);
        }
        let used = verified.iter().map(|f| f.author).collect::<BTreeSet<_>>();
        ensure!(
            catalog.keys().copied().collect::<BTreeSet<_>>() == used,
            "exact-used AuthRecords required"
        );
        let _refs = self.window.reserve(c, items.len() * 2)?;
        let mut ids = Vec::with_capacity(items.len());
        for (item, f) in items.into_iter().zip(verified) {
            let lease = self.check_existing_lease(c, binding)?;
            if let Some(old) = self.repo()?.event_at(c, peer as u8, f.meta.n)? {
                if old != item.raw {
                    self.signed_fork(c, "ordinal", &old, &item.raw)?;
                }
                ids.push(f.id);
                continue;
            }
            if let Some(old) = self.repo()?.pending(c, f.meta.n)? {
                if old.raw != item.raw {
                    self.signed_fork(c, "pending ordinal", &old.raw, &item.raw)?;
                }
                ids.push(f.id);
                continue;
            }
            let signer = *self.spec(c)?.peers[peer as usize]
                .cert
                .device_id()
                .as_bytes();
            if let Some((context, old)) = self.repo()?.authored(signer, f.meta.sequence)?
                && old != f.id
            {
                let old_c = self.slot(context)?;
                let old = self.source_for_proof(old_c, old)?;
                self.signed_fork(c, "author sequence", &old, &item.raw)?;
            }
            let meta = self.repo()?.metadata(c)?;
            let ledger = self.repo()?.repair_state(c)?;
            ensure!(
                f.meta.n >= meta.next[peer as usize],
                "pending cannot precede readable frontier"
            );
            let head = if selected {
                head_grant && f.meta.n == meta.next[peer as usize] && ledger.head.is_none()
            } else {
                f.meta.n == meta.next[peer as usize] && ledger.head.is_none()
            };
            ensure!(head || ledger.pending < 64, "StorageBlocked: pending/head");
            let report = self.commit_data2(
                c,
                Some(binding),
                store::ExistingData7::Pending {
                    author: signer,
                    raw: item.raw,
                    original: auth(catalog.get(&f.author).context("author catalog")?.clone()),
                    admission: auth(lease.peer_credential),
                    head,
                    grant,
                },
            )?;
            if self.verification.enabled() {
                let context = self.spec(c)?.c;
                self.verification
                    .pending(context, peer as u8, f.meta.n, f.id, report);
            }
            ids.push(f.id);
        }
        self.wake.notify_waiters();
        Ok(ids)
    }
    pub(super) fn materialize2(&mut self, c: u8) -> Result<bool> {
        if self.repo()?.existing_state(c)?.mode != store::ExistingMode::Active {
            return Ok(false);
        }
        let counts = self.repo()?.counts(c)?;
        let limit = usize::try_from(counts.queued.min(4))?;
        if limit == 0 {
            return Ok(false);
        }
        let schedule = super::super::test_support::load()?;
        if schedule.applies(self.spec(c)?.c) && schedule.suspend_materialization {
            return Ok(false);
        }
        self.current_existing(c)?;
        let items = self.repo()?.queue_page(c, None, limit)?;
        if items.is_empty() {
            return Ok(false);
        }
        let _refs = self.window.reserve(c, items.len() * 2)?;
        let selected = self.selection2(c)?;
        let author = self.peer2(c, self.role(c)?)?;
        let mut head = self.head(c)?;
        let mut native = self.mixed_native(c, &head)?;
        let mut sequence = self.repo()?.next_author_sequence()?;
        let mut seen = BTreeMap::new();
        let mut rows = Vec::with_capacity(items.len());
        for item in items {
            self.current_existing(c)?;
            let text = self.open_queue(head.c, &item)?;
            let g = &native.guard;
            let own = g.own();
            let n = own.next;
            let expected = g.expected_send()?;
            let mut parents = Vec::with_capacity(2);
            if n > 1 {
                parents.extend(own.last);
            }
            if g.peer().next > 1 {
                parents.extend(g.peer().last);
            }
            parents.sort();
            parents.dedup();
            let core = v2::Core2 {
                c: head.c,
                d: selected.d(),
                e: selected.e(),
                sid: selected.sid().context("SID")?.to_owned(),
                role: head.role as u8,
                n,
                previous: own.last,
                sequence,
                kind: 3,
                request: item.request,
                parents,
                expected: expected.clone(),
                author_auth: author.reference(),
                q: selected.proof().q(),
            };
            let pre = g2::EventMeta {
                role: core.role,
                n,
                previous: core.previous,
                sequence,
                kind: 3,
                native_kind: 1,
                key: [0; 32],
                index: expected.index,
                expected,
            };
            let step = g.preflight_send(&pre, self.repo()?.generation(), head.native_generation)?;
            if !g.rotate_due() {
                let key = own.key.context("own mixed key")?;
                let observed = self.seen2(c, core.role, key, &seen)?;
                ensure!(
                    observed
                        == SeenObservation::Present(Seen {
                            generation: own.g,
                            first: own.first.context("own first")?
                        }),
                    "own continuation Seen"
                );
            }
            let plain = Zeroizing::new(v2::inner2(&core, a([b(text.as_bytes())]))?);
            self.stats.packet_attempts += 1;
            let packet = native.session.encrypt(&plain)?;
            let event = v2::sign_event(core, &packet, self.device.identity())?;
            event
                .clone()
                .verify(&selected, &author)?
                .verify_inner(&plain)?;
            let meta = event.meta();
            let observed = self.seen2(c, meta.role, meta.key, &seen)?;
            native.guard.apply(step, &meta, event.id(), observed)?;
            seen.entry((meta.role, meta.key)).or_insert(match observed {
                SeenObservation::Present(seen) => seen,
                SeenObservation::Absent => Seen {
                    generation: meta.expected.g,
                    first: event.id(),
                },
            });
            rows.push(store::L1Readable {
                author: *self.device.identity().device_id().as_bytes(),
                source: event.raw().to_vec(),
                text: Some(text),
                original: auth(author.raw().to_vec()),
                admission: None,
                final_auth: Some(auth(author.raw().to_vec())),
                receiver: None,
                receipt: None,
                queue: Some(item.sequence),
            });
            sequence = g2::successor(sequence, g2::Counter::Author)?;
        }
        let count = rows.len() as u64;
        let change = self.mixed_change(c, &mut head, &native)?;
        let report =
            self.commit_data2(c, None, store::ExistingData7::Materialize { change, rows })?;
        self.retain_mixed(c, &head, native);
        self.stats.materialized += count;
        self.stats.committed_encrypt += count;
        self.observe_native(c, report)?;
        self.wake.notify_waiters();
        Ok(true)
    }
    fn seen2(
        &mut self,
        c: u8,
        role: u8,
        key: Id,
        trial: &BTreeMap<(u8, Id), Seen>,
    ) -> Result<SeenObservation> {
        if let Some(seen) = trial.get(&(role, key)) {
            return Ok(SeenObservation::Present(*seen));
        }
        observation(self.repo()?.seen(c, role, key)?)
    }
    fn receipt2(&mut self, c: u8, source: &v2::MixedEvent) -> Result<Vec<u8>> {
        let f = facts(source)?;
        let selected = self.selection2(c)?;
        if let Some(old) = self.repo()?.receipt(c, f.meta.role, f.meta.n)? {
            return Ok(old);
        }
        let receiver = self.peer2(c, self.role(c)?)?;
        let receipt = v2::ParsedReceipt2::sign(
            v2::ReceiptFields2 {
                c: selected.c(),
                d: selected.d(),
                e: selected.e(),
                sid: selected.sid().context("SID")?.to_owned(),
                q: selected.proof().q(),
                format: f.format,
                lane: f.meta.role,
                n: f.meta.n,
                event: f.id,
                receiver_cert: receiver.cert_ref(),
                receiver_auth: receiver.reference(),
            },
            self.device.identity(),
        )?;
        receipt.clone().verify_mixed(source, &selected, &receiver)?;
        Ok(receipt.raw().to_vec())
    }
    pub(super) fn drain2(&mut self, c: u8) -> Result<bool> {
        if !self.repo()?.existing_state(c)?.mode.selected() || self.repo()?.counts(c)?.pending == 0
        {
            return Ok(false);
        }
        let binding = self.leases[usize::from(c)]
            .as_ref()
            .context("NeedExistingAdmission")?
            .binding;
        self.check_existing_lease(c, binding)?;
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
        let mut working = self.mixed_native(c, &head)?;
        let mut rows = Vec::new();
        let mut known = BTreeSet::new();
        let mut seen = BTreeMap::new();
        let mut quarantine = None;
        for _ in 0..4 {
            let next = working.guard.peer().next;
            let Some(pending) = self.repo()?.pending(c, next)? else {
                break;
            };
            if pending.quarantined {
                break;
            }
            let (format, _, _) = declared(&pending.raw)?;
            let event = self.verify_source2(
                c,
                &v2::TaggedItem {
                    format,
                    raw: pending.raw.clone(),
                },
                &BTreeMap::new(),
            )?;
            let f = facts(&event)?;
            let mut parents = true;
            for parent in &f.parents {
                if !known.contains(parent) {
                    if let Some(raw) = self.repo()?.event(c, *parent)? {
                        let (format, _, _) = declared(&raw)?;
                        let p = self.verify_source2(
                            c,
                            &v2::TaggedItem { format, raw },
                            &BTreeMap::new(),
                        )?;
                        parents &= facts(&p)?.meta.kind == 3;
                    } else {
                        parents = false;
                    }
                }
            }
            let observed = self.seen2(c, f.meta.role, f.meta.key, &seen)?;
            let pre = working.guard.preflight_receive(
                &f.meta,
                observed,
                self.repo()?.generation(),
                head.native_generation,
            );
            if !parents || matches!(&pre, Err(g2::Error::Input(_))) {
                quarantine = Some(next);
                break;
            }
            let step = pre?;
            self.check_existing_lease(c, binding)?;
            let mut trial = working.fork();
            self.stats.packet_attempts += 1;
            let packet = match &event {
                v2::MixedEvent::Legacy(e) => kilogram_ratchet::ordered::Packet {
                    kind: e.event().native_kind as u8,
                    bytes: e.event().ciphertext.clone(),
                },
                v2::MixedEvent::Current(e) => e.event().packet(),
            };
            let plain = match trial.session.decrypt(&packet) {
                Ok(raw) => Zeroizing::new(raw),
                Err(kilogram_ratchet::ordered::NativeFailure::Input(_)) => {
                    self.stats.aborted_packet_calls += 1;
                    quarantine = Some(next);
                    break;
                }
                Err(error) => return Err(error.into()),
            };
            let text = (|| -> Result<String> {
                let payload = match &event {
                    v2::MixedEvent::Legacy(e) => e.event().verify_inner(&plain)?,
                    v2::MixedEvent::Current(e) => e.verify_inner(&plain)?.value().clone(),
                };
                let text = payload.fields(1)?[0].text()?;
                ensure!(text.len() <= 4096, "mixed Text cap");
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
            trial.guard.apply(step, &f.meta, f.id, observed)?;
            let original = self
                .repo()?
                .auth(c, pending.original)?
                .context("pending original")?;
            let admission = self
                .repo()?
                .auth(c, pending.admission)?
                .context("pending admission")?;
            ensure!(
                wire::hash(b"", &original) == f.author,
                "pending exact original"
            );
            let receipt = self.receipt2(c, &event)?;
            rows.push(store::L1Readable {
                author: *self.spec(c)?.peers[peer as usize]
                    .cert
                    .device_id()
                    .as_bytes(),
                source: pending.raw,
                text: Some(text),
                original: auth(original),
                admission: Some(auth(admission)),
                final_auth: Some(auth(self.peer2(c, peer)?.raw().to_vec())),
                receiver: Some(auth(self.peer2(c, 1 - peer)?.raw().to_vec())),
                receipt: Some(receipt),
                queue: None,
            });
            seen.entry((f.meta.role, f.meta.key))
                .or_insert(match observed {
                    SeenObservation::Present(seen) => seen,
                    SeenObservation::Absent => Seen {
                        generation: f.meta.expected.g,
                        first: f.id,
                    },
                });
            known.insert(f.id);
            working = trial;
        }
        let worked = !rows.is_empty();
        if worked {
            let count = rows.len() as u64;
            let change = self.mixed_change(c, &mut head, &working)?;
            let report = self.commit_data2(
                c,
                Some(binding),
                store::ExistingData7::Readable { change, rows },
            )?;
            self.retain_mixed(c, &head, working);
            self.stats.committed_decrypt += count;
            self.stats.readable += count;
            self.observe_native(c, report)?;
        }
        if let Some(ordinal) = quarantine {
            self.commit_data2(
                c,
                Some(binding),
                store::ExistingData7::Quarantine { ordinal },
            )?;
        }
        if worked {
            self.wake.notify_waiters();
        }
        Ok(worked)
    }
    pub(super) fn receipt_source2(&mut self, c: u8, role: u8, n: u64) -> Result<v2::MixedEvent> {
        let raw = self
            .repo()?
            .event_at(c, role, n)?
            .context("receipt unknown immutable source")?;
        let (format, _, _) = declared(&raw)?;
        self.verify_source2(c, &v2::TaggedItem { format, raw }, &BTreeMap::new())
    }
    pub(super) fn accept_receipt2(
        &mut self,
        c: u8,
        binding: Id,
        item: v2::TaggedItem,
        catalog: &BTreeMap<Id, Vec<u8>>,
    ) -> Result<Id> {
        self.check_existing_lease(c, binding)?;
        let role = self.role(c)? as u8;
        let value = wire::decode(&item.raw, 1024)?;
        let r = value.fields(2)?[0].list()?;
        let (n, reference) = match item.format {
            1 => {
                value.fields(2)?[0].fields(8)?;
                (r[4].uint()?, r[7].fixed::<32>()?)
            }
            2 => {
                value.fields(2)?[0].fields(12)?;
                (r[8].uint()?, r[11].fixed::<32>()?)
            }
            _ => anyhow::bail!("receipt format"),
        };
        let auth_raw = catalog
            .get(&reference)
            .context("receipt exact AuthRecords")?
            .clone();
        let receiver = v2::ParsedCredentialBundle::parse(&auth_raw)?.verify_root()?;
        ensure!(
            receiver.reference() == reference,
            "receipt receiver reference"
        );
        let source = self.receipt_source2(c, role, n)?;
        let f = facts(&source)?;
        let state = self.repo()?.existing_state(c)?;
        if state.mode.selected() {
            item.verify_receipt(&source, &self.selection2(c)?, &receiver)?;
        } else {
            ensure!(item.format == 1, "Receipt2 forbidden before selection");
            let v2::MixedEvent::Legacy(source) = &source else {
                anyhow::bail!("pre-Q source format")
            };
            v2::ParsedReceipt1::parse(&item.raw)?.verify_pre_q(
                source,
                &self.scope2(c)?,
                &receiver,
            )?;
        }
        self.commit_data2(
            c,
            Some(binding),
            store::ExistingData7::Acknowledge {
                event: f.id,
                receipt: item.raw,
                receiver: auth(auth_raw),
            },
        )?;
        Ok(f.id)
    }
}
