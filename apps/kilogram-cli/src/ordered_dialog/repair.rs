//! Receiver-driven prefix/receipt repair; cursors never pretend to be possession IDs.
use super::{
    admin,
    engine::{Engine, prefix},
    ingress::pending_state,
    outbox::receipt_path,
};
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Event, Frame, Id, Value, a, b, opt, u};
use std::collections::BTreeMap;

pub fn auth_catalog(value: &Value) -> Result<BTreeMap<Id, Vec<u8>>> {
    let rows = value.list()?;
    ensure!(rows.len() <= 32, "auth catalog count");
    let mut records = BTreeMap::new();
    let mut previous = None;
    for row in rows {
        let f = row.fields(2)?;
        let id = f[0].fixed()?;
        let raw = f[1].bytes()?;
        ensure!(
            previous.is_none_or(|p| p < id)
                && wire::hash(b"", raw) == id
                && raw.len() <= wire::MAX_AUTH,
            "auth catalog order/hash/bytes"
        );
        super::handshake::Credentials::parse(raw)?;
        records.insert(id, raw.to_vec());
        previous = Some(id);
    }
    Ok(records)
}
fn catalog_value(records: &BTreeMap<Id, Vec<u8>>) -> Value {
    a(records.iter().map(|(id, raw)| a([b(id), b(raw)])))
}

impl Engine {
    /// Intent is committed with readability. Attempted precedes the socket write;
    /// Sent follows it and says nothing about remote receipt or delivery.
    pub fn ack_intents(&self, c: Id, binding: Id) -> Result<Option<(Frame, Vec<u64>)>> {
        self.check_lease(c, binding)?;
        let lane = 1 - self.role(c)?;
        let mut items = Vec::new();
        let mut ordinals = Vec::new();
        for path in self
            .store
            .paths(&format!("{}ack-intent/{lane}/", prefix(c)))
        {
            let value = wire::decode(&admin::required(&self.store, &path)?, 1024)?;
            let fields = value.fields(3)?;
            ensure!(
                fields[0].uint()? == 1 && fields[2].uint()? <= 2,
                "ACK intent checkpoint"
            );
            if fields[2].uint()? == 2 {
                continue;
            }
            let n = path
                .rsplit('/')
                .next()
                .context("ACK intent ordinal")?
                .parse::<u64>()?;
            let raw = admin::required(&self.store, &receipt_path(c, lane, n))?;
            ensure!(
                wire::hash(wire::RECEIPT_ID, &raw) == fields[1].fixed::<32>()?,
                "ACK intent receipt hash"
            );
            let r = wire::decode(&raw, 1024)?;
            let id = r.fields(2)?[0].fields(8)?[5].fixed::<32>()?;
            items.push(a([u(n), b(id)]));
            ordinals.push(n);
            if ordinals.len() == 32 {
                break;
            }
        }
        if items.is_empty() {
            return Ok(None);
        }
        let head = self.head(c)?;
        let query = Frame::signed(
            16,
            binding,
            admin::random::<16>()?,
            a([
                b(c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                u(lane),
                a(items),
            ]),
            self.device.identity(),
        )?;
        Ok(Some((self.ack_response(c, &query)?, ordinals)))
    }
    pub fn mark_ack_intents(
        &mut self,
        c: Id,
        binding: Id,
        ordinals: &[u64],
        status: u64,
    ) -> Result<()> {
        self.check_lease(c, binding)?;
        ensure!(
            !ordinals.is_empty() && ordinals.len() <= 32 && (status == 1 || status == 2),
            "ACK intent transition"
        );
        let lane = 1 - self.role(c)?;
        let mut changes = super::engine::Changes::default();
        for n in ordinals {
            let path = super::ingress::ack_intent(c, lane, *n);
            let old = wire::decode(&admin::required(&self.store, &path)?, 1024)?;
            let f = old.fields(3)?;
            ensure!(
                f[0].uint()? == 1 && f[2].uint()? <= status,
                "ACK intent rollback"
            );
            changes.put(
                path,
                wire::encode(&a([u(1), f[1].clone(), u(status)]), 1024)?,
            );
        }
        let mut head = self.head(c)?;
        head.bump(false)?;
        self.check_lease(c, binding)?;
        self.save(&mut head, None, changes)
    }
    pub fn resume_body(&self, c: Id) -> Result<Value> {
        let head = self.head(c)?;
        let native = self.native(&head)?;
        let guard = native.guard.as_ref().context("resume guard")?;
        let (slots, bytes, head_used) = self.pending_stats(c)?;
        let next = guard.peer.next;
        let quarantine = match self.store.get(&pending_state(c, next))? {
            Some(raw) => wire::decode(&raw, 2048)?.fields(8)?[3].uint()? == 1,
            None => false,
        };
        let future = !self
            .store
            .paths(&format!("{}pending/", prefix(c)))
            .is_empty();
        let reason = if quarantine {
            6
        } else if future
            && self
                .store
                .get(&super::engine::pending_path(c, next))?
                .is_none()
        {
            1
        } else {
            0
        };
        let cursor = |n, last: Option<Id>| a([u(n), opt(last.map(b))]);
        let mut ids = vec![opt(head.offer.map(b)), opt(head.grant.map(b))];
        for role in 0..=1 {
            ids.push(opt(self
                .event_at(c, role, 0)?
                .map(|e| e.id())
                .transpose()?
                .map(b)));
        }
        for role in 0..=1 {
            ids.push(opt(self
                .store
                .get(&super::engine::ready_path(c, role))?
                .map(|r| b(wire::hash(wire::CONTROL_ID, &r)))));
        }
        let credit = a([
            u((64 - slots) as u64),
            u(((64 * 64 * 1024 - bytes).min((64 - slots) * 64 * 1024)) as u64),
            u(if head_used { 0 } else { 64 * 1024 }),
            u(32),
            u(wire::MAX_WIRE as u64),
        ]);
        Ok(a([
            b(c),
            b(head.d.context("D")?),
            b(head.e.context("E")?),
            b(head.sid.as_deref().context("SID")?.as_bytes()),
            a(ids),
            cursor(guard.own.next, guard.own.last),
            cursor(guard.peer.next, guard.peer.last),
            a([
                u(reason),
                opt(if reason == 0 { None } else { Some(u(next)) }),
            ]),
            credit,
        ]))
    }
    pub fn prefix_request(&self, c: Id, peer_head: u64) -> Result<Option<Value>> {
        let head = self.head(c)?;
        let native = self.native(&head)?;
        let guard = native.guard.as_ref().context("prefix guard")?;
        let next = guard.peer.next;
        if next >= peer_head
            || self
                .store
                .get(&super::engine::pending_path(c, next))?
                .is_some()
        {
            return Ok(None);
        }
        let (slots, bytes, head_used) = self.pending_stats(c)?;
        let ordinary_bytes = (64 * 64 * 1024 - bytes).min((64 - slots) * 64 * 1024);
        let head_only = slots == 64 || ordinary_bytes < 64 * 1024;
        ensure!(!head_only || !head_used, "blocked head reserve");
        let max = if head_only { 1 } else { 32 };
        let end = peer_head.min(next + max);
        Ok(Some(a([
            b(c),
            b(head.d.context("D")?),
            b(head.e.context("E")?),
            u(1 - head.role),
            u(next),
            u(end),
            opt(guard.peer.last.map(b)),
            u(max),
            u(if head_only {
                68 * 1024
            } else {
                wire::MAX_WIRE as u64
            }),
            u((64 - slots) as u64),
            u(ordinary_bytes as u64),
            u(if head_used { 0 } else { 64 * 1024 }),
        ])))
    }
    fn event_catalog(&self, events: &[Event]) -> Result<BTreeMap<Id, Vec<u8>>> {
        let mut records = BTreeMap::new();
        for event in events {
            records
                .entry(event.core.author_auth)
                .or_insert(admin::required(
                    &self.store,
                    &admin::auth_path(event.core.author_auth),
                )?);
        }
        Ok(records)
    }
    pub fn deliver_frame(
        &self,
        c: Id,
        events: &[Event],
        binding: Id,
        request: [u8; 16],
    ) -> Result<Frame> {
        self.check_lease(c, binding)?;
        ensure!(!events.is_empty() && events.len() <= 32, "deliver count");
        let head = self.head(c)?;
        let records = self.event_catalog(events)?;
        let raw = events
            .iter()
            .map(|e| e.raw().map(b))
            .collect::<wire::Result<Vec<_>>>()?;
        Ok(Frame::signed(
            10,
            binding,
            request,
            a([
                b(c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                u(head.role),
                catalog_value(&records),
                a(raw),
            ]),
            self.device.identity(),
        )?)
    }
    pub fn prefix_response(&self, c: Id, request: &Frame) -> Result<Frame> {
        self.check_lease(c, request.binding)?;
        let head = self.head(c)?;
        let f = request.body.fields(12)?;
        ensure!(
            f[0].fixed::<32>()? == c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == head.role,
            "prefix request domain/lane"
        );
        let start = f[4].uint()?;
        let end = f[5].uint()?;
        let max = f[7].uint()?;
        let byte_cap = usize::try_from(f[8].uint()?)?;
        let native = self.native(&head)?;
        let guard = native.guard.as_ref().context("source guard")?;
        ensure!(
            start > 0
                && start < end
                && end <= guard.own.next
                && end <= 512
                && max > 0
                && max <= 32
                && (1024..=wire::MAX_WIRE).contains(&byte_cap),
            "MissingPrefix/range/credit"
        );
        let prev = self
            .event_at(c, head.role, start - 1)?
            .context("MissingPrefix predecessor")?
            .id()?;
        ensure!(
            wire::optional_id(&f[6])? == Some(prev),
            "prefix predecessor mismatch"
        );
        let head_only = f[9].uint()? == 0 || f[10].uint()? < 64 * 1024;
        if head_only {
            ensure!(
                max == 1 && end == start + 1 && f[11].uint()? >= 64 * 1024,
                "head-only credit"
            );
        }
        ensure!(
            f[9].uint()? <= 64 && f[10].uint()? <= 64 * 64 * 1024 && f[11].uint()? <= 64 * 1024,
            "credit cap"
        );
        let mut events = Vec::new();
        let mut selected = None;
        for n in start..end.min(start + max) {
            events.push(
                self.event_at(c, head.role, n)?
                    .context("MissingPrefix committed record")?,
            );
            let records = self.event_catalog(&events)?;
            let raw = events
                .iter()
                .map(|e| e.raw().map(b))
                .collect::<wire::Result<Vec<_>>>()?;
            let actual_end = start + events.len() as u64;
            let body = a([
                b(c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                u(head.role),
                u(start),
                u(actual_end),
                a([u(guard.own.next), opt(guard.own.last.map(b))]),
                catalog_value(&records),
                a(raw),
                u(u64::from(actual_end < end)),
            ]);
            let frame = Frame::signed(
                12,
                request.binding,
                request.request,
                body,
                self.device.identity(),
            );
            match frame {
                Ok(frame) if frame.raw()?.len() + 4 <= byte_cap => selected = Some(frame),
                _ => break,
            }
        }
        selected.context("StorageBlocked: full encoded head frame does not fit")
    }
    pub fn accept_data_frame(&mut self, c: Id, frame: &Frame) -> Result<super::ingress::Batch> {
        let head = self.head(c)?;
        let f = frame.body.list()?;
        ensure!(
            (frame.kind == 10 || frame.kind == 12)
                && f[0].fixed::<32>()? == c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == 1 - head.role,
            "data frame scope/lane"
        );
        let (auth, items) = if frame.kind == 10 {
            (&f[4], &f[5])
        } else {
            (&f[7], &f[8])
        };
        let records = auth_catalog(auth)?;
        let events = items
            .list()?
            .iter()
            .map(|v| v.bytes().map(ToOwned::to_owned))
            .collect::<wire::Result<Vec<_>>>()?;
        if frame.kind == 12 {
            let start = f[4].uint()?;
            let end = f[5].uint()?;
            ensure!(
                end > start && end - start == events.len() as u64 && f[9].uint()? <= 1,
                "prefix response span"
            );
            let mut previous = None;
            for (i, raw) in events.iter().enumerate() {
                let event = Event::parse(
                    raw,
                    self.spec(c)?.peers[1 - head.role as usize].cert.device_id(),
                )?;
                ensure!(
                    event.core.n == start + i as u64 && (i == 0 || event.core.previous == previous),
                    "prefix response adjacency"
                );
                previous = Some(event.id()?);
            }
        }
        let used: std::collections::BTreeSet<_> = events
            .iter()
            .map(|raw| -> Result<Id> {
                let v = wire::decode(raw, wire::MAX_EVENT)?;
                Ok(wire::Core::parse(&v.fields(5)?[1])?.author_auth)
            })
            .collect::<Result<_>>()?;
        ensure!(
            records.keys().all(|id| used.contains(id)),
            "unused auth record"
        );
        self.receive_batch(c, &events, &records, frame.binding)
    }
    pub fn ack_query(
        &self,
        c: Id,
        items: &[(u64, Id)],
        binding: Id,
        request: [u8; 16],
    ) -> Result<Frame> {
        let head = self.head(c)?;
        ensure!(!items.is_empty() && items.len() <= 32, "ACK query count");
        Ok(Frame::signed(
            16,
            binding,
            request,
            a([
                b(c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                u(head.role),
                a(items.iter().map(|(n, id)| a([u(*n), b(id)]))),
            ]),
            self.device.identity(),
        )?)
    }
    pub fn ack_response(&self, c: Id, frame: &Frame) -> Result<Frame> {
        self.check_lease(c, frame.binding)?;
        let head = self.head(c)?;
        let f = frame.body.fields(5)?;
        ensure!(
            f[0].fixed::<32>()? == c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == 1 - head.role
                && f[4].list()?.len() <= 32,
            "ACK query scope"
        );
        let lane = f[3].uint()?;
        let mut receipts = Vec::new();
        let mut absent = Vec::new();
        let mut auth = BTreeMap::new();
        for pair in f[4].list()? {
            let p = pair.fields(2)?;
            let n = p[0].uint()?;
            let id = p[1].fixed::<32>()?;
            if let Some(raw) = self.store.get(&receipt_path(c, lane, n))? {
                let r = wire::decode(&raw, 1024)?;
                let r = r.fields(2)?[0].fields(8)?;
                ensure!(r[5].fixed::<32>()? == id, "ACK query slot fork");
                let reference = r[7].fixed()?;
                auth.entry(reference)
                    .or_insert(admin::required(&self.store, &admin::auth_path(reference))?);
                receipts.push(b(raw));
            } else {
                absent.push(pair.clone());
            }
        }
        Ok(Frame::signed(
            15,
            frame.binding,
            frame.request,
            a([
                b(c),
                b(head.d.context("D")?),
                b(head.e.context("E")?),
                u(lane),
                catalog_value(&auth),
                a(receipts),
                a(absent),
            ]),
            self.device.identity(),
        )?)
    }
    pub fn accept_ack_frame(&mut self, c: Id, frame: &Frame) -> Result<usize> {
        let head = self.head(c)?;
        let f = frame.body.fields(7)?;
        ensure!(
            f[0].fixed::<32>()? == c
                && Some(f[1].fixed::<32>()?) == head.d
                && Some(f[2].fixed::<32>()?) == head.e
                && f[3].uint()? == head.role,
            "ACK batch scope"
        );
        let auth = auth_catalog(&f[4])?;
        ensure!(
            f[5].list()?.len() <= 32 && f[6].list()?.len() <= 32,
            "ACK batch count"
        );
        let mut count = 0;
        for receipt in f[5].list()? {
            let raw = receipt.bytes()?;
            let r = wire::decode(raw, 1024)?;
            let reference = r.fields(2)?[0].fields(8)?[7].fixed()?;
            let credential = auth.get(&reference).context("ACK authority missing")?;
            if self.accept_receipt(c, raw, credential, frame.binding)? {
                count += 1
            }
        }
        Ok(count)
    }
}
