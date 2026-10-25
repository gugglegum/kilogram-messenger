use super::*;
use admission::Link2;
use iroh::endpoint::Connection;
use owner::{Owner2, Purpose2};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

pub(super) struct Dispatch2 {
    pub c: u8,
    pub frame: wire::ParsedFrame2,
    valid: Arc<AtomicBool>,
    deadline: Instant,
    wall_deadline: u64,
    high_water: Arc<AtomicU64>,
    route: kilogram_transport_iroh::RoutePolicy,
    _references: Option<window::RefPermit2>,
}
pub(super) struct PreparedBody2 {
    pub body: Value,
    pub references: window::RefPermit2,
}
pub(super) struct LiveWindow2 {
    pub bodies: Vec<Value>,
    pub ids: Vec<Id>,
    pub next: u64,
    pub _references: window::RefPermit2,
}
impl Dispatch2 {
    pub fn check(&self, connection: &Connection) -> Result<()> {
        ensure!(
            self.valid.load(Ordering::Acquire)
                && Instant::now() < self.deadline
                && connection.close_reason().is_none(),
            "dispatch permit invalid"
        );
        let wall = now()?;
        ensure!(
            wall < self.wall_deadline && wall >= self.high_water.load(Ordering::Acquire),
            "dispatch clock/permission expired"
        );
        let paths = connection.paths();
        let path = paths
            .iter()
            .find(|p| p.is_selected())
            .context("no selected route")?;
        ensure!(
            match self.route {
                kilogram_transport_iroh::RoutePolicy::DirectOnly => path.is_ip(),
                kilogram_transport_iroh::RoutePolicy::RelayOnly => path.is_relay(),
                _ => path.is_ip() || path.is_relay(),
            },
            "dispatch route changed"
        );
        Ok(())
    }
}
pub(super) fn scope(selected: &wire::StaticSelection2) -> Result<Vec<Value>> {
    Ok(vec![
        b(selected.c()),
        b(selected.d()),
        b(selected.e()),
        b(selected.sid().context("SID")?.as_bytes()),
    ])
}
fn cursor(next: u64, last: Option<Id>) -> Value {
    a([u(next), opt(last.map(b))])
}
pub(super) fn catalog(records: Vec<db::HistoryAuth>) -> Value {
    a(records.into_iter().map(|r| a([b(r.id), b(r.bytes)])))
}
pub(super) struct PrefixPlan2 {
    pub request: [u8; 16],
    pub lane: u8,
    pub start: u64,
    pub end: u64,
    pub head: bool,
    pub grant: Option<Id>,
}
type AuthenticatedItems2 = (Vec<Vec<u8>>, BTreeMap<Id, Vec<u8>>);
pub(super) struct AckProgress2 {
    pub first: u64,
    pub ids: Vec<Id>,
}
impl Owner2 {
    pub(super) fn live_window(&mut self, c: u8, first: u64) -> Result<Option<LiveWindow2>> {
        let _timing = timing::span(timing::Metric::LiveWindow);
        let (s, _) = self.require(c, true)?;
        let schedule = hooks::load()?;
        if schedule.applies(c) && schedule.suspend_live {
            return Ok(None);
        }
        let count = self.repository.counts(c)?.debt.min(64) as usize;
        if count == 0 {
            return Ok(None);
        }
        let references = self.window.reserve(c, count)?;
        let mut sources = {
            let _timing = timing::span(timing::Metric::DebtPage);
            self.repository.debt_page(c, first, count.min(32))?
        };
        if sources.len() == 32 && count > 32 {
            let next = sources
                .last()
                .context("debt tail")?
                .ordinal
                .checked_add(1)
                .context("debt cursor")?;
            let _timing = timing::span(timing::Metric::DebtPage);
            sources.extend(self.repository.debt_page(c, next, count - 32)?);
        }
        if sources.is_empty() {
            return Ok(None);
        }
        // Reservation may conservatively exceed the remaining suffix; it is never borrowed by another C.
        let next = sources
            .last()
            .context("debt tail")?
            .ordinal
            .checked_add(1)
            .context("debt cursor")?;
        let ids = sources.iter().map(|e| e.id).collect::<Vec<_>>();
        let schedule = hooks::load()?;
        sources.retain(|source| !schedule.held(c, s.head.role, source.ordinal));
        if sources.is_empty() {
            return Ok(None);
        }
        let selected = self.selected(&s)?;
        let mut bodies = Vec::new();
        for chunk in sources.chunks(16) {
            let mut records = BTreeMap::new();
            for source in chunk {
                let e = wire::ParsedEvent2::parse(&source.raw)?;
                let id = e.core().author_auth;
                records
                    .entry(id)
                    .or_insert(self.repository.authority(c, id)?);
            }
            let mut body = scope(&selected)?;
            body.extend([
                u(u64::from(s.head.role)),
                catalog(
                    records
                        .into_iter()
                        .map(|(id, bytes)| db::HistoryAuth { id, bytes })
                        .collect(),
                ),
                a(chunk.iter().map(|e| a([u(2), b(&e.raw)]))),
            ]);
            wire::encode(&a(body.clone()), wire::MAX_WIRE - 512)?;
            bodies.push(a(body));
        }
        Ok(Some(LiveWindow2 {
            bodies,
            ids,
            next,
            _references: references,
        }))
    }
    pub(super) fn delivery_items(&mut self, c: u8, body: &Value) -> Result<AuthenticatedItems2> {
        let _timing = timing::span(timing::Metric::DeliveryItems);
        let f = body.fields(7)?;
        self.check_scope(c, f)?;
        let s = self.snapshot(c)?;
        ensure!(
            f[4].uint()? == u64::from(1 - s.head.role),
            "Deliver peer lane"
        );
        let items = wire::tagged_items(&f[6], wire::MAX_EVENT)?;
        let mut refs = Vec::with_capacity(items.len());
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            ensure!(item.format == 2, "UpgradeRequired");
            let event = wire::ParsedEvent2::parse(&item.raw)?;
            ensure!(event.core().role == 1 - s.head.role, "Deliver event lane");
            refs.push(event.core().author_auth);
            out.push(item.raw);
        }
        let records = wire::auth_records_for(&f[5], &refs)?.into_iter().collect();
        Ok((out, records))
    }
    pub(super) fn slot(&mut self, id: Id) -> Result<u8> {
        let p = self.repository.profile()?;
        Ok(p.contexts
            .iter()
            .position(|c| *c == id)
            .context("unknown context")? as u8)
    }
    pub(super) fn verify_frame(&mut self, link: &Link2, frame: &wire::ParsedFrame2) -> Result<()> {
        let _timing = timing::span(timing::Metric::VerifyFrame);
        ensure!(
            link.valid.load(Ordering::Acquire) && Instant::now() < link.deadline,
            "stale link"
        );
        let (s, current) = self.require(link.c, true)?;
        ensure!(
            frame
                .body()
                .value()
                .list()?
                .first()
                .context("frame context")?
                .fixed::<32>()?
                == s.head.context,
            "frame context pin"
        );
        let lease = self.leases[usize::from(link.c)].as_ref().context("lease")?;
        ensure!(
            lease.generation == link.generation && lease.binding == link.binding,
            "connection generation changed"
        );
        frame.clone().verify(
            link.binding,
            current.peers[usize::from(1 - s.head.role)]
                .certificate()
                .device_id(),
        )?;
        Ok(())
    }
    pub(super) fn frame(
        &mut self,
        link: &Link2,
        kind: wire::FrameKind,
        body: Value,
        request: [u8; 16],
        references: Option<window::RefPermit2>,
    ) -> Result<Dispatch2> {
        let _timing = timing::span(timing::Metric::FrameSign);
        let (_, _) = self.require(link.c, true)?;
        let lease = self.leases[usize::from(link.c)].as_ref().context("lease")?;
        ensure!(
            lease.generation == link.generation && lease.binding == link.binding,
            "stale dispatch generation"
        );
        let body = wire::FrameBody2::parse(kind, &wire::encode(&body, wire::MAX_WIRE - 4)?)?;
        let frame = wire::ParsedFrame2::sign(body, link.binding, request, &self.device)?;
        let peer = wire::ParsedRuntimeLocator2::parse(
            self.repository
                .snapshot(link.c)?
                .spec
                .peer_locator
                .as_deref()
                .context("peer locator")?,
        )?;
        Ok(Dispatch2 {
            c: link.c,
            frame,
            valid: lease.valid.clone(),
            deadline: lease.deadline,
            wall_deadline: lease.wall_deadline,
            high_water: self.clock_high_water.clone(),
            route: self.route.connection_policy(&peer)?,
            _references: references,
        })
    }
    pub(super) fn check_scope(&mut self, c: u8, body: &[Value]) -> Result<()> {
        let s = self.snapshot(c)?;
        let selected = self.selected(&s)?;
        ensure!(
            body.len() >= 4
                && body[0].fixed::<32>()? == selected.c()
                && body[1].fixed::<32>()? == selected.d()
                && body[2].fixed::<32>()? == selected.e()
                && body[3].text()? == selected.sid().context("SID")?,
            "frame selected scope/SID"
        );
        Ok(())
    }
    pub(super) fn head_body(&mut self, c: u8) -> Result<Value> {
        let _timing = timing::span(timing::Metric::HeadBody);
        let (s, _) = self.require(c, true)?;
        let selected = self.selected(&s)?;
        let heads = self.repository.frontiers(c)?;
        let own = usize::from(s.head.role);
        let peer = 1 - own;
        let repair = self.repository.repair_state(c)?;
        let mut body = scope(&selected)?;
        body.extend([
            cursor(heads.next[own], heads.tails[own]),
            cursor(heads.next[peer], heads.tails[peer]),
            a([
                u(if s.head.active { 5 } else { 2 }),
                opt(s.head.mode_ref.map(b)),
                a([opt(Some(u(0))), opt(Some(u(0)))]),
            ]),
            a([u(u64::from(s.head.blocked.is_some())), a([])]),
            a([
                u(64 - repair.pending),
                u((64 - repair.pending) * 64 * 1024),
                u(if repair.head.is_none() { 64 * 1024 } else { 0 }),
                u(16),
                u(wire::MAX_WIRE as u64),
                u(self.leases[usize::from(c)]
                    .as_ref()
                    .context("lease")?
                    .wall_deadline),
                opt(repair.grant.map(|g| a([b(g.0), u(g.1), u(g.2)]))),
            ]),
        ]);
        Ok(a(body))
    }
    pub(super) fn accept_head(&mut self, c: u8, body: &Value) -> Result<(u64, Option<Id>, u64)> {
        let f = body.fields(9)?;
        self.check_scope(c, f)?;
        let (s, _) = self.require(c, true)?;
        wire::ModeView2::parse(&f[6])?.check_selected(self.selected(&s)?.proof())?;
        let source = f[4].fields(2)?;
        let next = source[0].uint()?;
        let last = wire::optional_id(&source[1])?;
        let readable = f[5].fields(2)?;
        let remote_next = readable[0].uint()?;
        ensure!(
            next > 0 && last.is_some() && remote_next > 0,
            "active cursor shape"
        );
        let own = self.repository.frontiers(c)?;
        let ownrole = usize::from(s.head.role);
        ensure!(
            remote_next <= own.next[ownrole],
            "peer readable beyond local source"
        );
        if let Some(id) = wire::optional_id(&readable[1])? {
            ensure!(
                self.repository
                    .source(c, s.head.role, remote_next - 1)?
                    .is_some_and(|e| e.id == id),
                "peer readable cursor ID"
            );
        }
        if next > s.head.peer_source_next {
            self.commit(
                &s,
                db::RuntimeOperation::PeerHead {
                    context: c,
                    fence: s.fence(now()?),
                    next,
                    last,
                },
                Purpose2::Peer,
                (0, 0),
            )?;
        } else if next == s.head.peer_source_next {
            ensure!(
                last == s.head.peer_source_last,
                "peer source cursor conflict"
            );
        }
        Ok((next, last, remote_next))
    }
    pub(super) fn grant_repair(&mut self, c: u8) -> Result<()> {
        let (s, _) = self.require(c, true)?;
        let frontiers = self.repository.frontiers(c)?;
        let start = frontiers.next[usize::from(1 - s.head.role)];
        let end = s.head.peer_source_next;
        if start >= end || self.repository.pending(c, start)?.is_some() {
            return Ok(());
        }
        let mut r = self.repository.repair_state(c)?;
        if let Some((id, _, old_end, _)) = r.grant {
            if start < old_end {
                return Ok(());
            }
            let (s, _) = self.require(c, true)?;
            self.commit(
                &s,
                db::RuntimeOperation::Repair {
                    context: c,
                    fence: s.fence(now()?),
                    repair: db::HistoryRepair::CancelGrant { id },
                },
                Purpose2::Peer,
                (0, 0),
            )?;
            return Ok(());
        }
        let end = end.min(start.checked_add(256).context("repair window")?);
        if end <= start {
            return Ok(());
        }
        let need = (end - start) * db::FRAME_CREDIT;
        if r.repair_free < need {
            let (s, _) = self.require(c, true)?;
            let bytes = (need - r.repair_free).min(db::REPAIR_POOL - r.repair_free - r.outstanding);
            ensure!(bytes > 0, "repair pool exhausted");
            self.commit(
                &s,
                db::RuntimeOperation::Repair {
                    context: c,
                    fence: s.fence(now()?),
                    repair: db::HistoryRepair::Refill { bytes },
                },
                Purpose2::Peer,
                (0, 0),
            )?;
            return Ok(());
        }
        let (s, _) = self.require(c, true)?;
        self.commit(
            &s,
            db::RuntimeOperation::Repair {
                context: c,
                fence: s.fence(now()?),
                repair: db::HistoryRepair::Grant {
                    id: shared_admin::random()?,
                    start,
                    end,
                },
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        r = self.repository.repair_state(c)?;
        ensure!(r.grant.is_some(), "repair grant missing");
        Ok(())
    }
    pub(super) fn prefix_request(
        &mut self,
        c: u8,
        request: [u8; 16],
    ) -> Result<Option<(Value, PrefixPlan2)>> {
        let _timing = timing::span(timing::Metric::PrefixRequest);
        let (s, _) = self.require(c, true)?;
        let selected = self.selected(&s)?;
        let heads = self.repository.frontiers(c)?;
        let schedule = hooks::load()?;
        if schedule.applies(c) && schedule.suspend_prefix {
            return Ok(None);
        }
        let lane = 1 - s.head.role;
        let start = heads.next[usize::from(lane)];
        if start >= s.head.peer_source_next {
            return Ok(None);
        }
        if self.repository.pending(c, start)?.is_some() {
            return Ok(None);
        }
        let repair = self.repository.repair_state(c)?;
        let Some((id, grant_start, grant_end, _)) = repair.grant else {
            return Ok(None);
        };
        if start >= grant_end {
            return Ok(None);
        }
        ensure!(grant_start <= start, "repair grant frontier");
        let head = repair.pending == 64;
        if head && repair.head.is_some() {
            return Ok(None);
        }
        let count = if head { 1 } else { 16.min(64 - repair.pending) };
        if count == 0 {
            return Ok(None);
        }
        let end = start
            .checked_add(count)
            .context("prefix end")?
            .min(s.head.peer_source_next)
            .min(grant_end);
        let credit = a([
            u(64 - repair.pending),
            u((64 - repair.pending) * 64 * 1024),
            u(if repair.head.is_none() { 64 * 1024 } else { 0 }),
            u(if head { 1 } else { 16 }),
            u(if head {
                68 * 1024
            } else {
                wire::MAX_WIRE as u64
            }),
            u(self.leases[usize::from(c)]
                .as_ref()
                .context("lease")?
                .wall_deadline),
            opt(Some(a([b(id), u(grant_start), u(grant_end)]))),
        ]);
        let mut body = scope(&selected)?;
        body.extend([
            u(u64::from(lane)),
            u(start),
            u(end),
            opt(heads.tails[usize::from(lane)].map(b)),
            credit,
        ]);
        Ok(Some((
            a(body),
            PrefixPlan2 {
                request,
                lane,
                start,
                end,
                head,
                grant: Some(id),
            },
        )))
    }
    pub(super) fn prefix_response(&mut self, c: u8, request: &Value) -> Result<PreparedBody2> {
        let _timing = timing::span(timing::Metric::PrefixResponse);
        let f = request.fields(9)?;
        self.check_scope(c, f)?;
        let (s, _) = self.require(c, true)?;
        let selected = self.selected(&s)?;
        ensure!(
            f[4].uint()? == u64::from(s.head.role),
            "prefix own source lane"
        );
        let start = f[5].uint()?;
        let end = f[6].uint()?;
        let credit = f[8].fields(7)?;
        ensure!(
            start > 0 && end > start && end - start <= 256 && credit[5].uint()? > now()?,
            "prefix range/deadline"
        );
        let heads = self.repository.frontiers(c)?;
        ensure!(
            start < heads.next[usize::from(s.head.role)],
            "prefix unavailable"
        );
        ensure!(
            wire::optional_id(&f[7])?
                == self
                    .repository
                    .source(c, s.head.role, start - 1)?
                    .map(|r| r.id),
            "prefix previous source"
        );
        let count = (end - start)
            .min(heads.next[usize::from(s.head.role)] - start)
            .min(credit[3].uint()?)
            .min(if credit[0].uint()? == 0 {
                1
            } else {
                credit[0].uint()?
            })
            .min(32) as usize;
        ensure!(count > 0, "prefix zero credit");
        let references = self.window.reserve(c, count)?;
        let sources = self.repository.source_page(c, s.head.role, start, count)?;
        ensure!(sources.len() == count, "source page cardinality");
        let mut records = BTreeMap::new();
        for source in &sources {
            let e = wire::ParsedEvent2::parse(&source.raw)?;
            let id = e.core().author_auth;
            records
                .entry(id)
                .or_insert(self.repository.authority(c, id)?);
        }
        let actual_end = start
            .checked_add(sources.len() as u64)
            .context("prefix actual end")?;
        let mut body = scope(&selected)?;
        body.extend([
            u(u64::from(s.head.role)),
            u(start),
            u(actual_end),
            cursor(
                heads.next[usize::from(s.head.role)],
                heads.tails[usize::from(s.head.role)],
            ),
            catalog(
                records
                    .into_iter()
                    .map(|(id, bytes)| db::HistoryAuth { id, bytes })
                    .collect(),
            ),
            a(sources.into_iter().map(|e| a([u(2), b(e.raw)]))),
            u(u64::from(actual_end < heads.next[usize::from(s.head.role)])),
        ]);
        let result = a(body);
        ensure!(
            wire::encode(&result, wire::MAX_WIRE - 512)?.len() + 512 <= credit[4].uint()? as usize,
            "prefix complete wire credit"
        );
        Ok(PreparedBody2 {
            body: result,
            references,
        })
    }
    pub(super) fn prefix_items(
        &mut self,
        c: u8,
        body: &Value,
        plan: &PrefixPlan2,
    ) -> Result<AuthenticatedItems2> {
        let f = body.fields(11)?;
        self.check_scope(c, f)?;
        ensure!(
            f[4].uint()? == u64::from(plan.lane)
                && f[5].uint()? == plan.start
                && f[6].uint()? >= plan.start
                && f[6].uint()? <= plan.end,
            "prefix response correlation"
        );
        let items = wire::tagged_items(&f[9], wire::MAX_EVENT)?;
        ensure!(
            items.len() as u64 == f[6].uint()? - plan.start && (!plan.head || items.len() <= 1),
            "prefix response count"
        );
        let mut ids = Vec::with_capacity(items.len());
        let mut out = Vec::with_capacity(items.len());
        for (offset, item) in items.into_iter().enumerate() {
            ensure!(item.format == 2, "UpgradeRequired");
            let e = wire::ParsedEvent2::parse(&item.raw)?;
            ensure!(
                e.core().role == plan.lane && e.core().n == plan.start + offset as u64,
                "prefix sequential slots"
            );
            ids.push(e.core().author_auth);
            out.push(item.raw);
        }
        let records = wire::auth_records_for(&f[8], &ids)?.into_iter().collect();
        Ok((out, records))
    }
    pub(super) fn complete_body(&mut self, c: u8, request: [u8; 16], ids: &[Id]) -> Result<Value> {
        let s = self.snapshot(c)?;
        let mut body = scope(&self.selected(&s)?)?;
        body.extend([b(request), a(ids.iter().map(b))]);
        Ok(a(body))
    }
    pub(super) fn ack_body(
        &mut self,
        c: u8,
        query: Option<&Value>,
    ) -> Result<(Value, Option<AckProgress2>)> {
        let _timing = timing::span(timing::Metric::AckBody);
        let (s, _) = self.require(c, true)?;
        let selected = self.selected(&s)?;
        let lane = 1 - s.head.role;
        let mut receipts = Vec::new();
        let mut missing = Vec::new();
        let mut progress = None;
        if let Some(query) = query {
            let f = query.fields(6)?;
            self.check_scope(c, f)?;
            ensure!(f[4].uint()? == u64::from(lane), "ACK query peer lane");
            let frontier = self.repository.frontiers(c)?.next[usize::from(lane)];
            for slot in f[5].list()? {
                let p = slot.fields(2)?;
                let n = p[0].uint()?;
                let id = p[1].fixed::<32>()?;
                if n < frontier {
                    let source = self
                        .repository
                        .source(c, lane, n)?
                        .context("receipt source")?;
                    ensure!(source.id == id, "ACK query source ID");
                    receipts.extend(self.repository.receipts(c, n, 1)?);
                } else {
                    missing.push(a([u(n), b(id)]));
                }
            }
        } else {
            receipts = self.repository.receipts(c, s.head.ack_next, 16)?;
            if !receipts.is_empty() {
                progress = Some(AckProgress2 {
                    first: s.head.ack_next,
                    ids: receipts.iter().map(|r| r.id).collect(),
                });
            }
        }
        let mut records = BTreeMap::new();
        for r in &receipts {
            let parsed = wire::ParsedReceipt2::parse(&r.raw)?;
            let id = parsed.fields().receiver_auth;
            records
                .entry(id)
                .or_insert(self.repository.authority(c, id)?);
        }
        let mut body = scope(&selected)?;
        body.extend([
            u(u64::from(lane)),
            catalog(
                records
                    .into_iter()
                    .map(|(id, bytes)| db::HistoryAuth { id, bytes })
                    .collect(),
            ),
            a(receipts.into_iter().map(|r| a([u(2), b(r.raw)]))),
            a(missing),
        ]);
        Ok((a(body), progress))
    }
    pub(super) fn ack_items(&mut self, c: u8, body: &Value) -> Result<AuthenticatedItems2> {
        let _timing = timing::span(timing::Metric::AckItems);
        let f = body.fields(8)?;
        self.check_scope(c, f)?;
        let s = self.snapshot(c)?;
        ensure!(f[4].uint()? == u64::from(s.head.role), "ACK lane");
        let items = wire::tagged_items(&f[6], wire::MAX_RECEIPT)?;
        let mut refs = Vec::with_capacity(items.len());
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            ensure!(item.format == 2, "UpgradeRequired");
            refs.push(
                wire::ParsedReceipt2::parse(&item.raw)?
                    .fields()
                    .receiver_auth,
            );
            out.push(item.raw);
        }
        let records = wire::auth_records_for(&f[5], &refs)?.into_iter().collect();
        Ok((out, records))
    }
    pub(super) fn ack_progress(&mut self, c: u8, progress: AckProgress2) -> Result<()> {
        let _timing = timing::span(timing::Metric::AckProgress);
        let (s, _) = self.require(c, true)?;
        self.commit(
            &s,
            db::RuntimeOperation::AckProgress {
                context: c,
                fence: s.fence(now()?),
                first: progress.first,
                ids: progress.ids,
            },
            Purpose2::Peer,
            (0, 0),
        )?;
        Ok(())
    }
    pub(super) fn ack_query(&mut self, c: u8) -> Result<Option<PreparedBody2>> {
        let _timing = timing::span(timing::Metric::AckQuery);
        let (s, _) = self.require(c, true)?;
        let selected = self.selected(&s)?;
        let count = self.repository.counts(c)?.debt.min(16) as usize;
        if count == 0 {
            return Ok(None);
        }
        let references = self.window.reserve(c, count)?;
        let debt = self.repository.debt_page(c, 1, count)?;
        ensure!(debt.len() == count, "debt page cardinality");
        let mut body = scope(&selected)?;
        body.extend([
            u(u64::from(s.head.role)),
            a(debt.iter().map(|r| a([u(r.ordinal), b(r.id)]))),
        ]);
        Ok(Some(PreparedBody2 {
            body: a(body),
            references,
        }))
    }
}
