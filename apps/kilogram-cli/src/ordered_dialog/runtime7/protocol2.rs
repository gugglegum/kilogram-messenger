//! Closed existing/pre-Q wire2 dispatch. Fresh Runtime6 dispatch is unchanged.
use super::*;
use kilogram_protocol::ordered_dialog_v2 as v2;
use owner::{Lease1, Owner7};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

pub(super) struct DispatchExisting7 {
    pub frame: v2::ParsedFrame2,
    pub lease: Lease1,
    floor: u64,
    pub _refs: Option<window::RefPermit2>,
}
impl DispatchExisting7 {
    pub fn gate(&self) -> Result<()> {
        self.lease.gate()?;
        ensure!(now()? >= self.floor, "dispatch clock rollback");
        Ok(())
    }
}
#[derive(Clone)]
pub(super) struct Prefix7 {
    pub request: [u8; 16],
    pub start: u64,
    pub end: u64,
    pub previous: Option<Id>,
    pub grant: Option<Id>,
    pub head: bool,
}
pub(super) struct Outgoing7 {
    pub dispatch: DispatchExisting7,
    pub ids: Vec<Id>,
    pub next: u64,
}
type ReceiptBundle7 = (Vec<v2::TaggedItem>, BTreeMap<Id, Vec<u8>>, Vec<(u64, Id)>);
fn cursor(next: u64, last: Option<Id>) -> Value {
    a([u(next), opt(last.map(b))])
}
fn catalog(records: &BTreeMap<Id, Vec<u8>>) -> Value {
    a(records.iter().map(|(id, raw)| a([b(id), b(raw)])))
}
fn tagged(items: &[v2::TaggedItem]) -> Value {
    a(items
        .iter()
        .map(|item| a([u(item.format.into()), b(&item.raw)])))
}

impl Owner7 {
    pub(super) fn retry_outgoing2(
        &mut self,
        c: u8,
        binding: Id,
        normal_first: u64,
        received: u64,
        credit: Value,
        once: &verification::retry::Once,
    ) -> Result<(Option<Outgoing7>, Option<verification::retry::Ticket>)> {
        let plan = self.verification_plan(c)?;
        let request = plan.as_ref().and_then(|p| p.retry_live.as_ref());
        let ticket = once.choose(request, binding, |request| {
            let lease = self.check_existing_lease(c, binding)?;
            let context = self.spec(c)?.c;
            let role = self.role(c)? as u8;
            let active = self.repo()?.existing_state(c)?.mode == store::ExistingMode::Active;
            let debt = self.repo()?.debt_page(c, None, 1)?.into_iter().next();
            let old = super::super::test_support::load()?;
            let held = old.held(context, u64::from(role), request.from)
                || plan
                    .as_ref()
                    .is_some_and(|p| p.suspend_live || p.hold_ordinals.contains(&request.from));
            let gate = verification::retry::Gate {
                nonce: self.verification.retry_nonce(),
                binding,
                context,
                generation: lease.generation,
                role,
                active,
                received,
                normal_first,
                debt,
                held,
            };
            gate.ticket(request)
        })?;
        let first = ticket.as_ref().map_or(normal_first, |t| t.request.from);
        let delivery = self.outgoing2(c, binding, first, credit)?;
        if let Some(ticket) = &ticket {
            let sent = delivery
                .as_ref()
                .context("S54 retry cannot skip actual lowest debt")
                .with_context(|| "S54 retry ordinary credit/source unavailable")?;
            verification::retry::completed_ids(ticket, &sent.ids, sent.next)?;
            // outgoing2 already owns its ordinary reserve32 and verified these
            // immutable sources. Bind the actual first encoded item, without
            // retaining an unaccounted payload before that reservation.
            let first = sent.dispatch.frame.body().value().fields(7)?[6]
                .list()?
                .first()
                .context("S54 first actual Deliver item")?
                .fields(2)?;
            ensure!(first[0].uint()? == 1, "S54 retry original old source only");
            let item = v2::TaggedItem {
                format: 1,
                raw: first[1].bytes()?.to_vec(),
            };
            let source = self.verify_source2(c, &item, &BTreeMap::new())?;
            let facts = data2::facts(&source)?;
            ensure!(
                facts.id == ticket.first_id
                    && facts.meta.role == 1
                    && facts.meta.n == ticket.request.from
                    && facts.meta.kind == 3,
                "S54 retry actual immutable first old debt source"
            );
        }
        Ok((delivery, ticket))
    }
    pub(super) fn retry_complete2(
        &mut self,
        c: u8,
        binding: Id,
        ticket: &verification::retry::Ticket,
        ids: &[Id],
        next: u64,
    ) -> Result<()> {
        let lease = self.check_existing_lease(c, binding)?;
        ensure!(
            binding == ticket.binding
                && lease.generation == ticket.request.lease_generation
                && self.spec(c)?.c == ticket.context
                && self.role(c)? == 1
                && self.repo()?.existing_state(c)?.mode == store::ExistingMode::Active,
            "S54 Complete retains captured current lease/source scope"
        );
        self.verification.retry_completed(ticket, ids, next)
    }
    fn scope_values2(&mut self, c: u8) -> Result<Vec<Value>> {
        let s = self.scope2(c)?;
        Ok(vec![b(s.c()), b(s.d()), b(s.e()), b(s.sid().as_bytes())])
    }
    fn check_scope2(&mut self, c: u8, f: &[Value]) -> Result<()> {
        ensure!(
            f.len() >= 4 && f[..4] == self.scope_values2(c)?,
            "existing frame C/D/E/SID"
        );
        Ok(())
    }
    pub(super) fn credit2(&mut self, c: u8) -> Result<Value> {
        let state = self.repo()?.repair_state(c)?;
        let slots = 64 - state.pending;
        Ok(a([
            u(slots),
            u(slots * 64 * 1024),
            u(if state.head.is_none() { 64 * 1024 } else { 0 }),
            u(16),
            u(v2::MAX_WIRE as u64),
            u(state.repair_free),
            opt(state
                .grant
                .map(|(id, start, end, _)| a([b(id), u(start), u(end)]))),
        ]))
    }
    pub(super) fn head_body2(&mut self, c: u8) -> Result<Value> {
        self.current_existing(c)?;
        let state = self.repo()?.existing_state(c)?;
        let meta = self.repo()?.metadata(c)?;
        let role = self.role(c)? as usize;
        let next = meta.next[1 - role];
        let pending = self.repo()?.pending(c, next)?;
        let reason = if pending.as_ref().is_some_and(|p| p.quarantined) {
            6
        } else if self.repo()?.counts(c)?.pending > 0 && pending.is_none() {
            1
        } else {
            0
        };
        let mut f = self.scope_values2(c)?;
        f.extend([
            cursor(meta.next[role], meta.tails[role]),
            cursor(next, meta.tails[1 - role]),
            admission2::mode_value(&state),
            a([u(reason), opt((reason != 0).then_some(u(next)))]),
            self.credit2(c)?,
        ]);
        Ok(a(f))
    }
    pub(super) fn validate_head2(
        &mut self,
        c: u8,
        body: &Value,
        lease: Option<&Lease1>,
    ) -> Result<v2::ModeView2> {
        let f = body.fields(9)?;
        self.check_scope2(c, f)?;
        let view = v2::ModeView2::parse(&f[6])?;
        let state = self.repo()?.existing_state(c)?;
        let role = self.role(c)? as u8;
        match lease {
            Some(lease) => admission2::live_view(role, &state, lease, &view)?,
            None => admission2::fresh_pair(role, &state, &view)?,
        }
        let source = f[4].fields(2)?;
        let next = source[0].uint()?;
        let last = wire::optional_id(&source[1])?.context("source mandatory tail")?;
        ensure!(next > 0, "source frontier excludes zero");
        let peer = 1 - usize::from(role);
        if view.stage <= 4 {
            ensure!(next <= 512, "pre-active source bound");
            ensure!(
                state.cuts[peer].is_none_or(|cut| next <= cut),
                "earlier source hint exceeds known frozen cut"
            );
        }
        if matches!(view.stage, 2..=4) {
            ensure!(Some(next) == view.cuts[peer], "frozen sender source/cut");
        }
        if view.stage == 5 {
            ensure!(
                view.cuts[peer].is_some_and(|cut| next >= cut),
                "active source below cut"
            );
        }
        let meta = self.repo()?.metadata(c)?;
        if next == meta.peer_source_next {
            ensure!(
                Some(last) == meta.peer_source_last,
                "equal source watermark conflict"
            );
        }
        if next <= meta.next[peer] {
            let raw = self
                .repo()?
                .event_at(c, peer as u8, next - 1)?
                .context("source lower hint proof")?;
            let (format, _, _) = data2::declared(&raw)?;
            let event =
                self.verify_source2(c, &v2::TaggedItem { format, raw }, &BTreeMap::new())?;
            ensure!(
                data2::facts(&event)?.id == last,
                "source lower hint exact immutable ID"
            );
        }
        let received = f[5].fields(2)?;
        let rx = received[0].uint()?;
        let tail = wire::optional_id(&received[1])?.context("receive mandatory tail")?;
        ensure!(
            rx > 0 && rx <= meta.next[usize::from(role)],
            "receive cursor beyond actual own source"
        );
        let raw = self
            .repo()?
            .event_at(c, role, rx - 1)?
            .context("receive cursor immutable source")?;
        let (format, _, _) = data2::declared(&raw)?;
        let event = self.verify_source2(c, &v2::TaggedItem { format, raw }, &BTreeMap::new())?;
        ensure!(
            data2::facts(&event)?.id == tail,
            "receive cursor immutable source ID"
        );
        Ok(view)
    }
    pub(super) fn accept_head2(
        &mut self,
        c: u8,
        binding: Id,
        body: &Value,
        fresh: bool,
    ) -> Result<()> {
        let lease = self.check_existing_lease(c, binding)?;
        let view = self.validate_head2(c, body, if fresh { None } else { Some(&lease) })?;
        let f = body.fields(9)?;
        let source = f[4].fields(2)?;
        let next = source[0].uint()?;
        let last = wire::optional_id(&source[1])?;
        let state = self.repo()?.existing_state(c)?;
        if self.role(c)? == 1
            && state.mode == store::ExistingMode::Prepared
            && let Some(q) = view.q
        {
            self.control_gate2(c, binding, |controls, gate| {
                controls.peer_hint(c, q, view.cuts, view.stage, gate)
            })?;
        }
        if self.role(c)? == 1 && state.mode == store::ExistingMode::Active && view.stage == 5 {
            self.control_gate2(c, binding, |controls, gate| {
                controls.peer_active(c, view.q.context("peer active Q")?, gate)
            })?;
        }
        let current = self.leases[usize::from(c)]
            .as_mut()
            .context("existing lease")?;
        if view.stage > current.peer_stage {
            current.peer_stage = view.stage;
        }
        if let Some(q) = view.q {
            current.peer_q = Some(q);
        }
        for r in 0..2 {
            if let Some(cut) = view.cuts[r] {
                current.peer_cuts[r] = Some(cut);
            }
        }
        let meta = self.repo()?.metadata(c)?;
        if next > meta.peer_source_next {
            self.commit_data2(
                c,
                Some(binding),
                store::ExistingData7::ObserveHead { next, last },
            )?;
        }
        if self.verification.enabled() {
            let lease = self.check_existing_lease(c, binding)?;
            let context = self.spec(c)?.c;
            let pre_q = self.repo()?.existing_state(c)?.q.is_none() && view.q.is_none();
            self.verification
                .head(context, binding, lease.generation, pre_q);
        }
        Ok(())
    }
    pub(super) fn control_gate2<T>(
        &mut self,
        c: u8,
        binding: Id,
        action: impl FnOnce(
            &mut control::ExistingControls7<'_>,
            control::ControlFinalGate<'_>,
        ) -> Result<T>,
    ) -> Result<T> {
        let lease = self.check_existing_lease(c, binding)?;
        let pre_p = self.repo()?.existing_state(c)?.mode == store::ExistingMode::L1;
        let mut gate = || {
            lease.gate()?;
            if pre_p
                && lease
                    .opportunity_deadline
                    .is_none_or(|deadline| Instant::now() >= deadline)
            {
                return Err(db::Error::PolicyRejected("pre-P opportunity expired"));
            }
            Ok(())
        };
        self.existing(c, |controls| action(controls, &mut gate))
    }
    pub(super) fn dispatch_frame2(
        &mut self,
        c: u8,
        binding: Id,
        kind: v2::FrameKind,
        request: Option<[u8; 16]>,
        body: Value,
        refs: Option<window::RefPermit2>,
    ) -> Result<DispatchExisting7> {
        let lease = self.check_existing_lease(c, binding)?;
        let floor = now()?;
        let frame = self.signed2(kind, binding, request, body)?;
        Ok(DispatchExisting7 {
            frame,
            lease,
            floor,
            _refs: refs,
        })
    }
    pub(super) fn verified_frame2(
        &mut self,
        c: u8,
        binding: Id,
        frame: v2::ParsedFrame2,
    ) -> Result<v2::VerifiedFrame2> {
        self.check_existing_lease(c, binding)?;
        let peer = 1 - self.role(c)?;
        Ok(frame.verify(binding, self.peer2(c, peer)?.certificate().device_id())?)
    }
    pub(super) fn propose_frame2(&mut self, c: u8, binding: Id) -> Result<DispatchExisting7> {
        let p = self.control_gate2(c, binding, |controls, gate| controls.propose(c, gate))?;
        let state = self.repo()?.existing_state(c)?;
        let consent = self
            .repo()?
            .existing_control(c, state.selected_consent.context("selected consent")?)?;
        self.dispatch_frame2(
            c,
            binding,
            v2::FrameKind::Propose,
            None,
            a([b(p), b(consent)]),
            None,
        )
    }
    pub(super) fn select_frame2(
        &mut self,
        c: u8,
        binding: Id,
        accept: &[u8],
    ) -> Result<DispatchExisting7> {
        let k = self.control_gate2(c, binding, |controls, gate| {
            controls.select(c, accept, gate)
        })?;
        self.dispatch_frame2(c, binding, v2::FrameKind::Select, None, a([b(k)]), None)
    }
    pub(super) fn saved_select_frame2(&mut self, c: u8, binding: Id) -> Result<DispatchExisting7> {
        let id = self.repo()?.existing_state(c)?.select.context("saved K")?;
        let raw = self.repo()?.existing_control(c, id)?;
        self.dispatch_frame2(c, binding, v2::FrameKind::Select, None, a([b(raw)]), None)
    }
    fn source_item2(&mut self, c: u8, n: u64) -> Result<(v2::TaggedItem, Id, Vec<u8>)> {
        let role = self.role(c)? as u8;
        let raw = self
            .repo()?
            .event_at(c, role, n)?
            .context("immutable own source missing")?;
        let (format, _, reference) = data2::declared(&raw)?;
        let event = self.verify_source2(
            c,
            &v2::TaggedItem {
                format,
                raw: raw.clone(),
            },
            &BTreeMap::new(),
        )?;
        ensure!(
            data2::facts(&event)?.meta.kind == 3,
            "Text data source required"
        );
        let auth = self
            .repo()?
            .auth(c, reference)?
            .context("immutable original capture")?;
        Ok((v2::TaggedItem { format, raw }, reference, auth))
    }
    pub(super) fn prefix2(
        &mut self,
        c: u8,
        binding: Id,
    ) -> Result<Option<(DispatchExisting7, Prefix7)>> {
        self.check_existing_lease(c, binding)?;
        if self.verification_plan(c)?.is_some_and(|s| s.suspend_prefix) {
            return Ok(None);
        }
        let schedule = super::super::test_support::load()?;
        if schedule.applies(self.spec(c)?.c) && schedule.suspend_prefix_requests {
            return Ok(None);
        }
        let meta = self.repo()?.metadata(c)?;
        let role = self.role(c)? as usize;
        let start = meta.next[1 - role];
        if start >= meta.peer_source_next || self.repo()?.pending(c, start)?.is_some() {
            return Ok(None);
        }
        let repair = self.repo()?.repair_state(c)?;
        let ordinary = 64 - repair.pending;
        let head = ordinary == 0 && repair.head.is_none();
        if ordinary == 0 && !head {
            return Ok(None);
        }
        let count = if head { 1 } else { ordinary.min(16) };
        let end = meta
            .peer_source_next
            .min(start.checked_add(count).context("prefix bound")?);
        let selected = self.repo()?.existing_state(c)?.mode.selected();
        let grant = if selected {
            if let Some((id, ..)) = repair.grant {
                self.commit_data2(c, Some(binding), store::ExistingData7::CancelGrant { id })?;
            }
            let id = admin::random()?;
            self.commit_data2(
                c,
                Some(binding),
                store::ExistingData7::Grant { id, start, end },
            )?;
            Some(id)
        } else {
            None
        };
        let mut credit = self.credit2(c)?.fields(7)?.to_vec();
        credit[0] = u(if head { 0 } else { ordinary });
        credit[1] = u(if head { 0 } else { ordinary * 64 * 1024 });
        credit[3] = u(if head { 1 } else { 16 });
        credit[4] = u(if head { 68 * 1024 } else { v2::MAX_WIRE as u64 });
        let previous = meta.tails[1 - role];
        let mut f = self.scope_values2(c)?;
        f.extend([
            u((1 - role) as u64),
            u(start),
            u(end),
            opt(previous.map(b)),
            a(credit),
        ]);
        let frame = self.dispatch_frame2(c, binding, v2::FrameKind::PrefixReq, None, a(f), None)?;
        let plan = Prefix7 {
            request: frame.frame.request(),
            start,
            end,
            previous,
            grant,
            head,
        };
        Ok(Some((frame, plan)))
    }
    fn prefix_reply2(
        &mut self,
        c: u8,
        binding: Id,
        frame: &v2::ParsedFrame2,
    ) -> Result<DispatchExisting7> {
        let f = frame.body().value().fields(9)?;
        self.check_scope2(c, f)?;
        let role = self.role(c)? as usize;
        let meta = self.repo()?.metadata(c)?;
        let start = f[5].uint()?;
        let end = f[6].uint()?;
        let credit = f[8].fields(7)?;
        ensure!(
            f[4].uint()? == role as u64
                && start > 0
                && start < end
                && end <= meta.next[role]
                && end - start <= 16,
            "prefix request source range/count"
        );
        let previous = self
            .repo()?
            .event_at(c, role as u8, start - 1)?
            .context("prefix previous source")?;
        let (format, _, _) = data2::declared(&previous)?;
        let previous = self.verify_source2(
            c,
            &v2::TaggedItem {
                format,
                raw: previous,
            },
            &BTreeMap::new(),
        )?;
        ensure!(
            wire::optional_id(&f[7])? == Some(data2::facts(&previous)?.id),
            "prefix exact previous"
        );
        let count = if credit[0].uint()? == 0 {
            1
        } else {
            credit[0].uint()?.min(credit[3].uint()?).min(16)
        };
        let cap = usize::try_from(credit[4].uint()?)?.min(v2::MAX_WIRE);
        let refs = self.window.reserve(c, 32)?;
        let mut items = Vec::new();
        let mut records = BTreeMap::new();
        let mut selected = None;
        for n in start..end.min(start.checked_add(count).context("prefix count")?) {
            let (item, reference, auth) = self.source_item2(c, n)?;
            items.push(item);
            records.insert(reference, auth);
            let mut body = self.scope_values2(c)?;
            body.extend([
                u(role as u64),
                u(start),
                u(n + 1),
                cursor(meta.next[role], meta.tails[role]),
                catalog(&records),
                tagged(&items),
                u(u64::from(n + 1 < end)),
            ]);
            let reply = self.signed2(
                v2::FrameKind::PrefixResp,
                binding,
                Some(frame.request()),
                a(body),
            )?;
            let admission = items.iter().try_fold(0usize, |sum, item| {
                sum.checked_add(item.raw.len() + 1024)
                    .context("prefix admission sum")
            })? + records.values().map(Vec::len).sum::<usize>();
            if reply.raw().len() + 4 > cap
                || admission
                    > usize::try_from(if credit[0].uint()? == 0 {
                        credit[2].uint()?
                    } else {
                        credit[1].uint()?
                    })?
            {
                break;
            }
            selected = Some(reply);
        }
        Ok(DispatchExisting7 {
            frame: selected.context("encoded prefix head does not fit credits")?,
            lease: self.check_existing_lease(c, binding)?,
            floor: now()?,
            _refs: Some(refs),
        })
    }
    pub(super) fn accept_prefix2(
        &mut self,
        c: u8,
        binding: Id,
        frame: v2::ParsedFrame2,
        plan: &Prefix7,
    ) -> Result<Option<verification::PageNotice>> {
        let verified = self.verified_frame2(c, binding, frame)?;
        let frame = verified.frame();
        ensure!(
            frame.body().kind() == v2::FrameKind::PrefixResp && frame.request() == plan.request,
            "PrefixResp2 required"
        );
        let f = frame.body().value().fields(11)?;
        self.check_scope2(c, f)?;
        let peer = 1 - self.role(c)?;
        let start = f[5].uint()?;
        let end = f[6].uint()?;
        let source = f[7].fields(2)?;
        let items = v2::tagged_items(&f[9], v2::MAX_EVENT)?;
        let records = super::super::repair::auth_catalog(&f[8])?;
        ensure!(
            f[4].uint()? == peer
                && start == plan.start
                && end > start
                && end <= plan.end
                && end - start == items.len() as u64
                && items.len() <= 16
                && end <= source[0].uint()?
                && wire::optional_id(&source[1])?.is_some(),
            "PrefixResp2 exact span/source"
        );
        let mut previous = plan.previous;
        for (i, item) in items.iter().enumerate() {
            let event = self.verify_source2(c, item, &records)?;
            let facts = data2::facts(&event)?;
            ensure!(
                facts.meta.n == start + i as u64 && facts.meta.previous == previous,
                "PrefixResp2 exact adjacency/previous"
            );
            previous = Some(facts.id);
        }
        if end == source[0].uint()? {
            ensure!(
                previous == wire::optional_id(&source[1])?,
                "PrefixResp2 source tail"
            );
        }
        let meta = self.repo()?.metadata(c)?;
        let source_next = source[0].uint()?;
        let source_last = wire::optional_id(&source[1])?;
        ensure!(source_next >= end, "prefix source watermark");
        let last = source_last.context("prefix source mandatory tail")?;
        let state = self.repo()?.existing_state(c)?;
        if state.mode.selected() {
            ensure!(
                state.cuts[peer as usize].is_some_and(|cut| source_next >= cut),
                "selected prefix source below cut"
            );
        } else {
            let lease = self.check_existing_lease(c, binding)?;
            let cut = state.cuts[peer as usize].or(lease.peer_cuts[peer as usize]);
            ensure!(
                source_next <= 512 && cut.is_none_or(|cut| source_next == cut),
                "pre-Q prefix source/cut bound"
            );
        }
        ensure!(
            source_next != meta.peer_source_next || Some(last) == meta.peer_source_last,
            "prefix equal watermark tail conflict"
        );
        if source_next <= meta.next[peer as usize] {
            let raw = self
                .repo()?
                .event_at(c, peer as u8, source_next - 1)?
                .context("prefix lower cursor source")?;
            let (format, _, _) = data2::declared(&raw)?;
            let event =
                self.verify_source2(c, &v2::TaggedItem { format, raw }, &BTreeMap::new())?;
            ensure!(
                data2::facts(&event)?.id == last,
                "prefix lower cursor immutable tail"
            );
        }
        // A signed response may observe genuine concurrent materialization.
        // It advances the same monotonic watermark as Head; it never rolls back.
        if source_next > meta.peer_source_next {
            self.commit_data2(
                c,
                Some(binding),
                store::ExistingData7::ObserveHead {
                    next: source_next,
                    last: Some(last),
                },
            )?;
        }
        let mut new_sources = 0;
        if self.verification.enabled() {
            for n in start..end {
                if self.repo()?.event_at(c, peer as u8, n)?.is_none()
                    && self.repo()?.pending(c, n)?.is_none()
                {
                    new_sources += 1;
                }
            }
        }
        self.receive2(c, binding, items, records, plan.grant, plan.head)?;
        if let Some(id) = plan.grant {
            self.commit_data2(c, Some(binding), store::ExistingData7::CancelGrant { id })?;
        }
        if !self.verification.enabled() {
            return Ok(None);
        }
        let page = verification::Page {
            start,
            end,
            new_sources,
            head: plan.head,
            grant: plan.grant,
            generation: self.repo()?.generation(),
            operation: self.repo()?.last_commit()?,
        };
        let context = self.spec(c)?.c;
        let role = self.role(c)? as u8;
        self.verification.page(c, context, role, page)
    }
    pub(super) fn outgoing2(
        &mut self,
        c: u8,
        binding: Id,
        first: u64,
        credit: Value,
    ) -> Result<Option<Outgoing7>> {
        self.check_existing_lease(c, binding)?;
        let verification = self.verification_plan(c)?;
        if verification.as_ref().is_some_and(|s| s.suspend_live) {
            return Ok(None);
        }
        let meta = self.repo()?.metadata(c)?;
        let role = self.role(c)? as usize;
        let cr = credit.fields(7)?;
        let schedule = super::super::test_support::load()?;
        let context = self.spec(c)?.c;
        let batch = if schedule.applies(context) {
            schedule.direct_batch.unwrap_or(16)
        } else {
            16
        };
        let count = cr[0].uint()?.min(cr[3].uint()?).min(16);
        if count == 0 || first >= meta.next[role] {
            return Ok(None);
        }
        let end = meta.next[role].min(first.checked_add(count).context("live bound")?);
        let refs = self.window.reserve(c, 32)?;
        let mut items = Vec::new();
        let mut records = BTreeMap::new();
        let mut ids = Vec::new();
        let mut selected = None;
        for n in first..end {
            if items.len() >= batch {
                break;
            }
            if schedule.held(context, role as u64, n) {
                continue;
            }
            if verification
                .as_ref()
                .is_some_and(|s| s.hold_ordinals.contains(&n))
            {
                continue;
            }
            let (item, reference, auth) = self.source_item2(c, n)?;
            let event = self.verify_source2(c, &item, &BTreeMap::new())?;
            ids.push(data2::facts(&event)?.id);
            items.push(item);
            records.insert(reference, auth);
            let mut body = self.scope_values2(c)?;
            body.extend([u(role as u64), catalog(&records), tagged(&items)]);
            let reply = self.signed2(v2::FrameKind::Deliver, binding, None, a(body))?;
            let admission = items
                .iter()
                .map(|item| item.raw.len() + 1024)
                .sum::<usize>()
                + records.values().map(Vec::len).sum::<usize>();
            if reply.raw().len() + 4 > usize::try_from(cr[4].uint()?)?
                || admission > usize::try_from(cr[1].uint()?)?
            {
                ids.pop();
                break;
            }
            selected = Some((reply, n + 1, ids.clone()));
        }
        let Some((frame, next, ids)) = selected else {
            return Ok(None);
        };
        Ok(Some(Outgoing7 {
            dispatch: DispatchExisting7 {
                frame,
                lease: self.check_existing_lease(c, binding)?,
                floor: now()?,
                _refs: Some(refs),
            },
            ids,
            next,
        }))
    }
    fn receipt_bundle2(&mut self, c: u8, lane: u8, pairs: &[(u64, Id)]) -> Result<ReceiptBundle7> {
        let lease = self.leases[usize::from(c)]
            .as_ref()
            .context("existing lease")?;
        let peer_selected = lease.peer_stage >= 4;
        let mut items = Vec::new();
        let mut records = BTreeMap::new();
        let mut missing = Vec::new();
        for (n, id) in pairs {
            let source = self.repo()?.event_at(c, lane, *n)?;
            let Some(raw) = source else {
                missing.push((*n, *id));
                continue;
            };
            let (format, _, _) = data2::declared(&raw)?;
            let source =
                self.verify_source2(c, &v2::TaggedItem { format, raw }, &BTreeMap::new())?;
            ensure!(
                data2::facts(&source)?.id == *id,
                "ACK query exact immutable source"
            );
            let Some(raw) = self.repo()?.receipt(c, lane, *n)? else {
                missing.push((*n, *id));
                continue;
            };
            let value = wire::decode(&raw, 1024)?;
            let r = value.fields(2)?[0].list()?;
            let format = r[0].uint()? as u8;
            ensure!(matches!(format, 1 | 2), "saved receipt format");
            value.fields(2)?[0].fields(if format == 1 { 8 } else { 12 })?;
            if format == 2 && !peer_selected {
                missing.push((*n, *id));
                continue;
            }
            let reference = if format == 1 {
                r[7].fixed::<32>()?
            } else {
                r[11].fixed::<32>()?
            };
            let auth = self
                .repo()?
                .auth(c, reference)?
                .context("saved receiver capture")?;
            let receiver = v2::ParsedCredentialBundle::parse(&auth)?.verify_root()?;
            if self.repo()?.existing_state(c)?.mode.selected() {
                v2::TaggedItem {
                    format,
                    raw: raw.clone(),
                }
                .verify_receipt(&source, &self.selection2(c)?, &receiver)?;
            } else {
                let v2::MixedEvent::Legacy(source) = &source else {
                    anyhow::bail!("pre-Q receipt source")
                };
                v2::ParsedReceipt1::parse(&raw)?.verify_pre_q(
                    source,
                    &self.scope2(c)?,
                    &receiver,
                )?;
            }
            records.insert(reference, auth);
            items.push(v2::TaggedItem { format, raw });
        }
        Ok((items, records, missing))
    }
    fn ack_reply2(
        &mut self,
        c: u8,
        binding: Id,
        frame: &v2::ParsedFrame2,
    ) -> Result<DispatchExisting7> {
        let f = frame.body().value().fields(6)?;
        self.check_scope2(c, f)?;
        let lane = 1 - self.role(c)? as u8;
        ensure!(f[4].uint()? == u64::from(lane), "ACK query lane");
        let pairs = f[5]
            .list()?
            .iter()
            .map(|v| -> Result<(u64, Id)> {
                let p = v.fields(2)?;
                Ok((p[0].uint()?, p[1].fixed()?))
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(pairs.len() <= 16, "runtime ACK query count");
        let refs = self.window.reserve(c, 32)?;
        let (items, records, missing) = self.receipt_bundle2(c, lane, &pairs)?;
        let mut body = self.scope_values2(c)?;
        body.extend([
            u(lane.into()),
            catalog(&records),
            tagged(&items),
            a(missing.iter().map(|(n, id)| a([u(*n), b(id)]))),
        ]);
        self.dispatch_frame2(
            c,
            binding,
            v2::FrameKind::AckBatch,
            Some(frame.request()),
            a(body),
            Some(refs),
        )
    }
    pub(super) fn accept_acks2(
        &mut self,
        c: u8,
        binding: Id,
        frame: v2::ParsedFrame2,
    ) -> Result<Vec<Id>> {
        let verified = self.verified_frame2(c, binding, frame)?;
        let frame = verified.frame();
        ensure!(
            frame.body().kind() == v2::FrameKind::AckBatch,
            "AckBatch2 required"
        );
        let f = frame.body().value().fields(8)?;
        self.check_scope2(c, f)?;
        ensure!(f[4].uint()? == self.role(c)?, "ACK batch own lane");
        let catalog = super::super::repair::auth_catalog(&f[5])?;
        let items = v2::tagged_items(&f[6], 1024)?;
        ensure!(items.len() <= 16, "runtime ACK batch count");
        ensure!(
            self.repo()?.existing_state(c)?.mode.selected()
                || items.iter().all(|item| item.format == 1),
            "Receipt2 forbidden before local selection"
        );
        let used = items
            .iter()
            .map(|item| -> Result<Id> {
                let v = wire::decode(&item.raw, 1024)?;
                let r = v.fields(2)?[0].fields(if item.format == 1 { 8 } else { 12 })?;
                ensure!(
                    r[0].uint()? == u64::from(item.format),
                    "explicit receipt format/version"
                );
                Ok(if item.format == 1 {
                    r[7].fixed()?
                } else {
                    r[11].fixed()?
                })
            })
            .collect::<Result<BTreeSet<_>>>()?;
        ensure!(
            catalog.keys().copied().collect::<BTreeSet<_>>() == used,
            "ACK exact-used AuthRecords"
        );
        let _refs = self.window.reserve(c, 32)?;
        let mut ids = Vec::new();
        for item in items {
            ids.push(self.accept_receipt2(c, binding, item, &catalog)?);
        }
        Ok(ids)
    }
    pub(super) fn ack_query2(&mut self, c: u8, binding: Id) -> Result<Option<DispatchExisting7>> {
        let schedule = super::super::test_support::load()?;
        if schedule.applies(self.spec(c)?.c) && schedule.disable_ack {
            return Ok(None);
        }
        let debt = self.repo()?.debt_page(c, None, 16)?;
        if debt.is_empty() {
            return Ok(None);
        }
        let mut body = self.scope_values2(c)?;
        body.extend([
            u(self.role(c)?),
            a(debt.iter().map(|(n, id)| a([u(*n), b(id)]))),
        ]);
        self.dispatch_frame2(c, binding, v2::FrameKind::AckQuery, None, a(body), None)
            .map(Some)
    }
    pub(super) fn ack_push2(&mut self, c: u8, binding: Id) -> Result<Option<Outgoing7>> {
        let schedule = super::super::test_support::load()?;
        if schedule.applies(self.spec(c)?.c) && schedule.disable_ack {
            return Ok(None);
        }
        let meta = self.repo()?.metadata(c)?;
        let peer = 1 - self.role(c)? as u8;
        let end = meta.next[peer as usize].min(meta.ack_next.saturating_add(16));
        let mut pairs = Vec::new();
        for n in meta.ack_next..end {
            let raw = self
                .repo()?
                .event_at(c, peer, n)?
                .context("ACK readable source")?;
            let (format, _, _) = data2::declared(&raw)?;
            let source =
                self.verify_source2(c, &v2::TaggedItem { format, raw }, &BTreeMap::new())?;
            pairs.push((n, data2::facts(&source)?.id));
        }
        if pairs.is_empty() {
            return Ok(None);
        }
        let (items, records, missing) = self.receipt_bundle2(c, peer, &pairs)?;
        // Do not advance across a deferred Receipt2 for a Prepared peer.
        if !missing.is_empty() {
            return Ok(None);
        }
        let refs = self.window.reserve(c, 32)?;
        let mut body = self.scope_values2(c)?;
        body.extend([u(peer.into()), catalog(&records), tagged(&items), a([])]);
        let dispatch = self.dispatch_frame2(
            c,
            binding,
            v2::FrameKind::AckBatch,
            None,
            a(body),
            Some(refs),
        )?;
        Ok(Some(Outgoing7 {
            dispatch,
            ids: pairs.iter().map(|(_, id)| *id).collect(),
            next: end,
        }))
    }
    pub(super) fn complete2(
        &mut self,
        c: u8,
        binding: Id,
        request: [u8; 16],
        ids: Vec<Id>,
    ) -> Result<DispatchExisting7> {
        let mut body = self.scope_values2(c)?;
        body.extend([b(request), a(ids.iter().map(b))]);
        self.dispatch_frame2(
            c,
            binding,
            v2::FrameKind::Complete,
            Some(request),
            a(body),
            None,
        )
    }
    pub(super) fn check_complete2(
        &mut self,
        c: u8,
        binding: Id,
        frame: v2::ParsedFrame2,
        request: [u8; 16],
        ids: &[Id],
    ) -> Result<()> {
        let verified = self.verified_frame2(c, binding, frame)?;
        let frame = verified.frame();
        let f = frame.body().value().fields(6)?;
        ensure!(
            frame.body().kind() == v2::FrameKind::Complete
                && frame.request() == request
                && f[4].fixed::<16>()? == request,
            "Complete2 exact request"
        );
        self.check_scope2(c, f)?;
        let got = f[5]
            .list()?
            .iter()
            .map(Value::fixed::<32>)
            .collect::<wire::Result<Vec<_>>>()?;
        ensure!(got == ids, "Complete2 exact possession IDs");
        Ok(())
    }
    pub(super) fn dispatch2(
        &mut self,
        c: u8,
        binding: Id,
        frame: v2::ParsedFrame2,
    ) -> Result<DispatchExisting7> {
        let verified = self.verified_frame2(c, binding, frame)?;
        let frame = verified.frame();
        let request = frame.request();
        use v2::FrameKind as K;
        match frame.body().kind() {
            K::Head => {
                self.accept_head2(c, binding, frame.body().value(), false)?;
                let body = self.head_body2(c)?;
                self.dispatch_frame2(c, binding, K::Head, Some(request), body, None)
            }
            K::Resume => {
                let f = frame.body().value().fields(10)?;
                self.check_scope2(c, f)?;
                let scope = self.scope2(c)?;
                let mut ids = scope
                    .old_control_ids()
                    .into_iter()
                    .map(|id| opt(Some(b(id))))
                    .collect::<Vec<_>>();
                for role in 0..2 {
                    ids.push(opt(Some(b(wire::hash(
                        wire::CONTROL_ID,
                        &self.control(c, store::L1Control::Ready(role))?,
                    )))));
                }
                ensure!(
                    f[4] == a(ids),
                    "Resume2 exact original controls/genesis/Ready1"
                );
                let body = a(f[..4].iter().cloned().chain(f[5..].iter().cloned()));
                self.accept_head2(c, binding, &body, false)?;
                let body = self.head_body2(c)?;
                self.dispatch_frame2(c, binding, K::Head, Some(request), body, None)
            }
            K::OpenIntent => {
                self.control_gate2(c, binding, |controls, gate| {
                    controls.accept_peer_notification(c, &verified, gate)
                })?;
                self.stats.open_intent_received += 1;
                self.wake.notify_waiters();
                let body = self.head_body2(c)?;
                self.dispatch_frame2(c, binding, K::Head, Some(request), body, None)
            }
            K::Propose => {
                let f = frame.body().value().fields(2)?;
                let p = f[0].bytes()?.to_vec();
                let consent = f[1].bytes()?.to_vec();
                let accept = self.control_gate2(c, binding, |controls, gate| {
                    controls.prepare(c, &p, &consent, gate)
                })?;
                self.dispatch_frame2(c, binding, K::Accept, Some(request), a([b(accept)]), None)
            }
            K::Select => {
                let raw = frame.body().value().fields(1)?[0].bytes()?.to_vec();
                let committed = self.control_gate2(c, binding, |controls, gate| {
                    controls.commit_b(c, &raw, gate)
                })?;
                self.dispatch_frame2(
                    c,
                    binding,
                    K::Committed,
                    Some(request),
                    a([b(committed)]),
                    None,
                )
            }
            K::PrefixReq => self.prefix_reply2(c, binding, frame),
            K::Deliver => {
                let f = frame.body().value().fields(7)?;
                self.check_scope2(c, f)?;
                ensure!(f[4].uint()? == 1 - self.role(c)?, "Deliver2 peer lane");
                let records = super::super::repair::auth_catalog(&f[5])?;
                let items = v2::tagged_items(&f[6], v2::MAX_EVENT)?;
                let ids = self.receive2(c, binding, items, records, None, false)?;
                self.complete2(c, binding, request, ids)
            }
            K::EpochCipher => {
                let f = frame.body().value().fields(5)?;
                let scope = self.scope2(c)?;
                ensure!(
                    f[0].fixed::<32>()? == scope.c()
                        && f[1].fixed::<32>()? == scope.d()
                        && f[2].fixed::<32>()? == scope.e(),
                    "EpochCipher2 existing scope"
                );
                let item = v2::TaggedItem {
                    format: f[3].uint()?.try_into()?,
                    raw: f[4].bytes()?.to_vec(),
                };
                let (_, _, id) = data2::declared(&item.raw)?;
                let mut records = BTreeMap::new();
                records.insert(
                    id,
                    self.repo()?
                        .auth(c, id)?
                        .context("EpochCipher2 original capture")?,
                );
                let ids = self.receive2(c, binding, vec![item], records, None, false)?;
                self.complete2(c, binding, request, ids)
            }
            K::AckQuery => self.ack_reply2(c, binding, frame),
            K::AckBatch => {
                let ids = self.accept_acks2(c, binding, frame.clone())?;
                self.complete2(c, binding, request, ids)
            }
            _ => anyhow::bail!("unsupported existing frame direction/stage"),
        }
    }
}
