//! Exact wire1 frames over typed indexed source/receipt reads.
use super::*;
use owner::Owner7;
use std::collections::BTreeMap;
pub(super) struct Dispatch7 {
    pub frame: Frame,
    pub refs: Option<window::RefPermit2>,
}
fn catalog(records: &BTreeMap<Id, Vec<u8>>) -> Value {
    a(records.iter().map(|(id, raw)| a([b(id), b(raw)])))
}

impl Owner7 {
    pub fn resume_body(&mut self, c: u8) -> Result<Value> {
        ensure!(self.active(c)?, "NeedHandshake");
        let head = self.head(c)?;
        let meta = self.repo()?.metadata(c)?;
        let repair = self.repo()?.repair_state(c)?;
        let own = head.role as usize;
        let peer = 1 - own;
        let next = meta.next[peer];
        let pending = self.repo()?.pending(c, next)?;
        let reason = if pending.as_ref().is_some_and(|p| p.quarantined) {
            6
        } else if self.repo()?.counts(c)?.pending > 0 && pending.is_none() {
            1
        } else {
            0
        };
        let mut ids = vec![opt(head.offer.map(b)), opt(head.grant.map(b))];
        for role in 0..2 {
            ids.push(opt(self
                .event_at(c, role, 0)?
                .map(|e| e.id())
                .transpose()?
                .map(b)));
        }
        for role in 0..2 {
            ids.push(opt(self
                .repo()?
                .control(c, store::L1Control::Ready(role))?
                .map(|r| b(wire::hash(wire::CONTROL_ID, &r)))));
        }
        let cursor = |r: usize| a([u(meta.next[r]), opt(meta.tails[r].map(b))]);
        let available = 64 - repair.pending;
        let credit = a([
            u(available),
            u(available * 64 * 1024),
            u(if repair.head.is_none() { 64 * 1024 } else { 0 }),
            u(32),
            u(wire::MAX_WIRE as u64),
        ]);
        Ok(a([
            b(head.c),
            b(head.d.context("D")?),
            b(head.e.context("E")?),
            b(head.sid.as_deref().context("SID")?.as_bytes()),
            a(ids),
            cursor(own),
            cursor(peer),
            a([
                u(reason),
                opt(if reason == 0 { None } else { Some(u(next)) }),
            ]),
            credit,
        ]))
    }
    pub fn observe_hint(&mut self, c: u8, frame: &Frame) -> Result<()> {
        self.check_lease(c, frame.binding)?;
        let head = self.head(c)?;
        let f = if frame.kind == 9 {
            frame.body.fields(9)?
        } else {
            ensure!(frame.kind == 17, "hint kind");
            frame.body.fields(7)?
        };
        ensure!(
            f[0].fixed::<32>()? == head.c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e,
            "Head/Resume C/D/E"
        );
        let (source, receive, status, credit) = if frame.kind == 9 {
            ensure!(
                f[3].text()? == head.sid.as_deref().context("SID")? && f[4].list()?.len() == 6,
                "Resume session/handshake IDs"
            );
            (&f[5], &f[6], &f[7], &f[8])
        } else {
            (&f[3], &f[4], &f[5], &f[6])
        };
        let source = source.fields(2)?;
        let receive = receive.fields(2)?;
        let next = source[0].uint()?;
        let last = wire::optional_id(&source[1])?;
        let received = receive[0].uint()?;
        let recv_last = wire::optional_id(&receive[1])?;
        let meta = self.repo()?.metadata(c)?;
        ensure!(
            (1..=512).contains(&next)
                && last.is_some()
                && (1..=meta.next[head.role as usize]).contains(&received)
                && recv_last.is_some(),
            "source/receive cursor bounds"
        );
        ensure!(
            self.event_at(c, head.role, received - 1)?
                .context("hint own immutable predecessor")?
                .id()?
                == recv_last.context("receive last")?,
            "receive cursor exact local ID"
        );
        let status = status.fields(2)?;
        let credit = credit.fields(5)?;
        ensure!(
            status[0].uint()? <= 7
                && credit[0].uint()? <= 64
                && credit[1].uint()? <= 64 * 64 * 1024
                && credit[2].uint()? <= 64 * 1024
                && credit[3].uint()? <= 32
                && credit[4].uint()? <= wire::MAX_WIRE as u64,
            "hint status/credit limits1"
        );
        self.commit(
            c,
            frame.binding,
            store::L1Operation::ObserveHead {
                context: c,
                next,
                last,
            },
        )?;
        Ok(())
    }
    pub fn prefix_request(&mut self, c: u8, binding: Id) -> Result<Option<Frame>> {
        self.check_lease(c, binding)?;
        let head = self.head(c)?;
        let schedule = super::super::test_support::load()?;
        if schedule.applies(head.c) && schedule.suspend_prefix_requests {
            return Ok(None);
        }
        let meta = self.repo()?.metadata(c)?;
        let peer = 1 - head.role as usize;
        let next = meta.next[peer];
        if next >= meta.peer_source_next || self.repo()?.pending(c, next)?.is_some() {
            return Ok(None);
        }
        let ledger = self.repo()?.repair_state(c)?;
        let ordinary = 64 - ledger.pending;
        let head_only = ordinary == 0;
        ensure!(!head_only || ledger.head.is_none(), "blocked reserved head");
        let max = if head_only { 1 } else { 32 };
        let body = a([
            b(head.c),
            b(head.d.context("D")?),
            b(head.e.context("E")?),
            u(peer as u64),
            u(next),
            u(meta.peer_source_next.min(next + max)),
            opt(meta.tails[peer].map(b)),
            u(max),
            u(if head_only {
                68 * 1024
            } else {
                wire::MAX_WIRE as u64
            }),
            u(ordinary),
            u(ordinary * 64 * 1024),
            u(if ledger.head.is_none() { 64 * 1024 } else { 0 }),
        ]);
        Ok(Some(self.signed(11, binding, body)?))
    }
    pub fn prefix_response(&mut self, c: u8, frame: &Frame) -> Result<Dispatch7> {
        self.check_lease(c, frame.binding)?;
        let head = self.head(c)?;
        let meta = self.repo()?.metadata(c)?;
        let f = frame.body.fields(12)?;
        ensure!(
            f[0].fixed::<32>()? == head.c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == head.role,
            "MissingPrefix scope/lane"
        );
        let start = f[4].uint()?;
        let end = f[5].uint()?;
        let max = f[7].uint()?;
        let cap = usize::try_from(f[8].uint()?)?;
        ensure!(
            start > 0
                && start < end
                && end <= meta.next[head.role as usize]
                && end <= 512
                && (1..=32).contains(&max)
                && (1024..=wire::MAX_WIRE).contains(&cap),
            "MissingPrefix range/cap"
        );
        ensure!(
            self.event_at(c, head.role, start - 1)?
                .context("prefix predecessor")?
                .id()?
                == wire::optional_id(&f[6])?.context("prefix predecessor required")?,
            "prefix predecessor mismatch"
        );
        ensure!(
            f[9].uint()? <= 64 && f[10].uint()? <= 64 * 64 * 1024 && f[11].uint()? <= 64 * 1024,
            "MissingPrefix credit limits1"
        );
        if f[9].uint()? == 0 || f[10].uint()? < 64 * 1024 {
            ensure!(
                max == 1 && end == start + 1 && f[11].uint()? >= 64 * 1024,
                "head-only reserve"
            );
        }
        let permit = self
            .window
            .reserve(c, usize::try_from((end - start).min(max))?)?;
        let mut events = Vec::new();
        let mut auth = BTreeMap::new();
        let mut selected = None;
        for n in start..end.min(start + max) {
            let event = self
                .event_at(c, head.role, n)?
                .context("immutable source missing")?;
            let reference = event.core.author_auth;
            if let std::collections::btree_map::Entry::Vacant(entry) = auth.entry(reference) {
                entry.insert(
                    self.repo()?
                        .auth(c, reference)?
                        .context("source O missing")?,
                );
            }
            events.push(b(event.raw()?));
            let actual_end = start + events.len() as u64;
            let body = a([
                b(head.c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                u(head.role),
                u(start),
                u(actual_end),
                a([
                    u(meta.next[head.role as usize]),
                    opt(meta.tails[head.role as usize].map(b)),
                ]),
                catalog(&auth),
                a(events.clone()),
                u(u64::from(actual_end < end)),
            ]);
            let reply = Frame::signed(
                12,
                frame.binding,
                frame.request,
                body,
                self.device.identity(),
            )?;
            if reply.raw()?.len() + 4 > cap {
                break;
            }
            selected = Some(reply);
        }
        Ok(Dispatch7 {
            frame: selected.context("full encoded prefix head does not fit")?,
            refs: Some(permit),
        })
    }
    pub fn accept_data_frame(&mut self, c: u8, frame: &Frame) -> Result<Vec<Id>> {
        self.check_lease(c, frame.binding)?;
        let head = self.head(c)?;
        let f = if frame.kind == 10 {
            frame.body.fields(6)?
        } else {
            ensure!(frame.kind == 12, "data kind");
            frame.body.fields(10)?
        };
        ensure!(
            f[0].fixed::<32>()? == head.c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == 1 - head.role,
            "data C/D/E/lane"
        );
        let (records, items) = if frame.kind == 10 {
            (&f[4], &f[5])
        } else {
            (&f[7], &f[8])
        };
        let records = super::super::repair::auth_catalog(records)?;
        let events = items
            .list()?
            .iter()
            .map(|v| v.bytes().map(ToOwned::to_owned))
            .collect::<wire::Result<Vec<_>>>()?;
        ensure!(!events.is_empty() && events.len() <= 32, "data batch count");
        if frame.kind == 12 {
            let start = f[4].uint()?;
            let end = f[5].uint()?;
            let source = f[6].fields(2)?;
            let source_next = source[0].uint()?;
            let source_last =
                wire::optional_id(&source[1])?.context("Prefix source tail required")?;
            ensure!(
                start > 0
                    && end > start
                    && end <= source_next
                    && source_next <= 512
                    && end - start == events.len() as u64
                    && f[9].uint()? <= 1,
                "PrefixBatch span"
            );
            let mut previous = None;
            for (i, raw) in events.iter().enumerate() {
                let event = Event::parse(
                    raw,
                    self.spec(c)?.peers[1 - head.role as usize].cert.device_id(),
                )?;
                ensure!(
                    event.core.n == start + i as u64 && (i == 0 || event.core.previous == previous),
                    "PrefixBatch adjacency"
                );
                previous = Some(event.id()?);
            }
            if end == source_next {
                ensure!(previous == Some(source_last), "Prefix exact source tail");
            }
        }
        let used = events
            .iter()
            .map(|raw| -> Result<Id> {
                let value = wire::decode(raw, wire::MAX_EVENT)?;
                Ok(wire::Core::parse(&value.fields(5)?[1])?.author_auth)
            })
            .collect::<Result<std::collections::BTreeSet<_>>>()?;
        ensure!(
            records.keys().all(|id| used.contains(id)),
            "unused auth catalog row"
        );
        self.receive_events(c, events, records, frame.binding)
    }
    pub fn ack_query(&mut self, c: u8, binding: Id) -> Result<Option<Frame>> {
        self.check_lease(c, binding)?;
        let head = self.head(c)?;
        let schedule = super::super::test_support::load()?;
        let applies = schedule.applies(head.c);
        if applies && schedule.disable_ack {
            return Ok(None);
        }
        let limit = if applies {
            schedule.ack_query_batch.unwrap_or(32)
        } else {
            32
        };
        let items = self.repo()?.debt_page(c, None, limit)?;
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.signed(
            16,
            binding,
            a([
                b(head.c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                u(head.role),
                a(items.into_iter().map(|(n, id)| a([u(n), b(id)]))),
            ]),
        )?))
    }
    pub fn ack_response(&mut self, c: u8, frame: &Frame) -> Result<Dispatch7> {
        self.check_lease(c, frame.binding)?;
        let head = self.head(c)?;
        let f = frame.body.fields(5)?;
        ensure!(
            f[0].fixed::<32>()? == head.c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == 1 - head.role
                && f[4].list()?.len() <= 32,
            "ACK query scope/count"
        );
        let permit = if f[4].list()?.is_empty() {
            None
        } else {
            Some(self.window.reserve(c, f[4].list()?.len())?)
        };
        let lane = f[3].uint()?;
        let mut receipts = Vec::new();
        let mut absent = Vec::new();
        let mut auth = BTreeMap::new();
        for pair in f[4].list()? {
            let p = pair.fields(2)?;
            let n = p[0].uint()?;
            let id = p[1].fixed::<32>()?;
            ensure!((1..512).contains(&n), "ACK ordinal");
            if let Some(raw) = self.repo()?.receipt(c, lane as u8, n)? {
                let value = wire::decode(&raw, 1024)?;
                let r = value.fields(2)?[0].fields(8)?;
                ensure!(r[5].fixed::<32>()? == id, "ACK slot exact ID");
                let reference = r[7].fixed()?;
                if let std::collections::btree_map::Entry::Vacant(entry) = auth.entry(reference) {
                    entry.insert(
                        self.repo()?
                            .auth(c, reference)?
                            .context("receipt R missing")?,
                    );
                }
                receipts.push(b(raw));
            } else {
                absent.push(pair.clone());
            }
        }
        Ok(Dispatch7 {
            frame: Frame::signed(
                15,
                frame.binding,
                frame.request,
                a([
                    b(head.c),
                    b(head.d.context("D")?),
                    b(head.e.context("E")?),
                    u(lane),
                    catalog(&auth),
                    a(receipts),
                    a(absent),
                ]),
                self.device.identity(),
            )?,
            refs: permit,
        })
    }
    pub fn accept_ack_frame(&mut self, c: u8, frame: &Frame) -> Result<()> {
        self.check_lease(c, frame.binding)?;
        let head = self.head(c)?;
        let f = frame.body.fields(7)?;
        ensure!(
            f[0].fixed::<32>()? == head.c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == head.role
                && f[5].list()?.len() <= 32
                && f[6].list()?.len() <= 32,
            "AckBatch scope/count"
        );
        let auth = super::super::repair::auth_catalog(&f[4])?;
        for receipt in f[5].list()? {
            let raw = receipt.bytes()?;
            let value = wire::decode(raw, 1024)?;
            let reference = value.fields(2)?[0].fields(8)?[7].fixed()?;
            self.accept_receipt(
                c,
                raw.to_vec(),
                auth.get(&reference)
                    .context("ACK R catalog missing")?
                    .clone(),
                frame.binding,
            )?;
        }
        Ok(())
    }
    pub fn outgoing(
        &mut self,
        c: u8,
        binding: Id,
        after: Option<u64>,
    ) -> Result<Option<Dispatch7>> {
        self.check_lease(c, binding)?;
        let head = self.head(c)?;
        let schedule = super::super::test_support::load()?;
        let items = self
            .repo()?
            .debt_page(c, after, 32)?
            .into_iter()
            .filter(|(n, _)| !schedule.held(head.c, head.role, *n))
            .take(schedule.direct_batch.unwrap_or(8))
            .collect::<Vec<_>>();
        if items.is_empty() {
            return Ok(None);
        }
        if schedule.trace_rpc && schedule.applies(head.c) {
            eprintln!(
                "l1_outgoing c={c} after={after:?} ordinals={:?}",
                items.iter().map(|(n, _)| *n).collect::<Vec<_>>()
            );
        }
        let permit = self.window.reserve(c, items.len())?;
        let mut events = Vec::new();
        let mut auth = BTreeMap::new();
        for (n, id) in items {
            let event = self.event_at(c, head.role, n)?.context("outbox source")?;
            ensure!(event.id()? == id, "outbox ID");
            let reference = event.core.author_auth;
            if let std::collections::btree_map::Entry::Vacant(entry) = auth.entry(reference) {
                entry.insert(self.repo()?.auth(c, reference)?.context("outbox O")?);
            }
            events.push(b(event.raw()?));
        }
        Ok(Some(Dispatch7 {
            frame: super::super::test_support::delivery(
                self.signed(
                    10,
                    binding,
                    a([
                        b(head.c),
                        b(head.d.context("D")?),
                        b(head.e.context("E")?),
                        u(head.role),
                        catalog(&auth),
                        a(events),
                    ]),
                )?,
                self.device.identity(),
            )?,
            refs: Some(permit),
        }))
    }
    pub fn dispatch(&mut self, c: u8, frame: Frame) -> Result<Option<Dispatch7>> {
        self.check_lease(c, frame.binding)?;
        let role = 1 - self.role(c)?;
        frame.verify(
            frame.binding,
            self.spec(c)?.peers[role as usize].cert.device_id(),
        )?;
        let head = self.head(c)?;
        let reply = match frame.kind {
            4 => {
                ensure!(
                    frame.body.fields(6)?[0].fixed::<32>()? == head.c,
                    "OpenIntent C"
                );
                None
            }
            5 => {
                let f = frame.body.fields(3)?;
                ensure!(f[0].fixed::<32>()? == head.c, "Offer C");
                let raw = self.grant(c, f[1].bytes()?, f[2].bytes()?, frame.binding)?;
                Some((
                    6,
                    a([
                        b(head.c),
                        b(wire::hash(wire::CONTROL_ID, f[1].bytes()?)),
                        b(raw),
                    ]),
                ))
            }
            7 => {
                let f = frame.body.fields(4)?;
                ensure!(
                    f[0].fixed::<32>()? == head.c
                        && Some(f[1].fixed::<32>()?) == head.d
                        && Some(f[2].fixed::<32>()?) == head.e,
                    "Init scope"
                );
                let raw = self.confirm(c, f[3].bytes()?, frame.binding)?;
                let h = self.head(c)?;
                Some((
                    7,
                    a([b(h.c), b(h.d.context("D")?), b(h.e.context("E")?), b(raw)]),
                ))
            }
            8 => {
                let f = frame.body.fields(4)?;
                ensure!(
                    f[0].fixed::<32>()? == head.c
                        && Some(f[1].fixed::<32>()?) == head.d
                        && Some(f[2].fixed::<32>()?) == head.e,
                    "Ready scope"
                );
                let raw = self.ready(c, f[3].bytes()?, frame.binding)?;
                Some((
                    8,
                    a([
                        b(head.c),
                        b(head.d.context("D")?),
                        b(head.e.context("E")?),
                        b(raw),
                    ]),
                ))
            }
            9 => {
                self.observe_hint(c, &frame)?;
                Some((9, self.resume_body(c)?))
            }
            10 => {
                let ids = self.accept_data_frame(c, &frame)?;
                Some((
                    13,
                    a([
                        b(head.c),
                        b(head.d.context("D")?),
                        b(head.e.context("E")?),
                        b(frame.request),
                        a(ids.into_iter().map(b)),
                    ]),
                ))
            }
            11 => return Ok(Some(self.prefix_response(c, &frame)?)),
            15 => {
                self.accept_ack_frame(c, &frame)?;
                None
            }
            16 => return Ok(Some(self.ack_response(c, &frame)?)),
            17 => {
                self.observe_hint(c, &frame)?;
                None
            }
            13 | 14 => None,
            _ => anyhow::bail!("unsupported admitted wire1 kind"),
        };
        reply
            .map(|(kind, body)| {
                Ok(Dispatch7 {
                    frame: Frame::signed(
                        kind,
                        frame.binding,
                        frame.request,
                        body,
                        self.device.identity(),
                    )?,
                    refs: None,
                })
            })
            .transpose()
    }
}
