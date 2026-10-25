use super::*;
use serde::Serialize;
const MIN_DEBT: u64 = 48 * 1024;

#[derive(Clone, Debug, Serialize)]
pub enum CreditAction {
    Queue {
        context: u8,
        sequence: u64,
        id: Id,
        body: Vec<u8>,
    },
    Grant {
        context: u8,
        id: Id,
        start: u64,
        end: u64,
    },
    CancelGrant {
        context: u8,
        id: Id,
    },
    Refill {
        context: u8,
        bytes: u64,
    },
    Pending {
        context: u8,
        ordinal: u64,
        event: Id,
        body: Vec<u8>,
        head: bool,
        grant: Option<Id>,
    },
    Materialize {
        context: u8,
        queue: u64,
        request: Id,
        ordinal: u64,
        event: Id,
        records: Vec<Key>,
    },
    Readable {
        context: u8,
        ordinal: u64,
        event: Id,
        records: Vec<Key>,
    },
    Acknowledge {
        context: u8,
        ordinal: u64,
        event: Id,
        records: Vec<Key>,
    },
    CapturePending {
        context: u8,
        ordinal: u64,
        event: Id,
        original_auth: Id,
        admission_auth: Id,
    },
}
impl CreditAction {
    pub(super) fn input_bound(&self) -> Result<usize> {
        let size = match self {
            Self::Queue { body, .. } => {
                if body.len() > 4096 {
                    return Err(Error::Bound("queue input"));
                }
                body.len()
            }
            Self::Pending { body, .. } => {
                if body.len() > 16 * 1024 {
                    return Err(Error::Bound("pending input"));
                }
                body.len()
            }
            Self::Materialize { records, .. }
            | Self::Readable { records, .. }
            | Self::Acknowledge { records, .. } => {
                if records.len() > 24 {
                    return Err(Error::Bound("credit refs"));
                }
                records.len() * 40
            }
            _ => 0,
        };
        Ok(size + 512)
    }
}
#[derive(Clone, Eq, PartialEq)]
struct Grant {
    id: Id,
    start: u64,
    end: u64,
    available: [u8; 32],
}
impl Grant {
    fn count(&self) -> u64 {
        self.available
            .iter()
            .map(|v| u64::from(v.count_ones()))
            .sum()
    }
    fn bit(&self, n: u64) -> Result<(usize, u8)> {
        if n < self.start || n >= self.end {
            return Err(Error::Conflict);
        }
        let i = (n - self.start) as usize;
        Ok((i / 8, 1 << (i % 8)))
    }
}
#[derive(Clone, Eq, PartialEq)]
pub(super) struct Ledger {
    pub quota: u64,
    repair_free: u64,
    outstanding: u64,
    pending: u64,
    head: Option<u64>,
    grant: Option<Grant>,
}
impl Ledger {
    pub(super) fn migration_unfunded(quota: u64) -> Self {
        Self {
            quota,
            repair_free: 0,
            outstanding: 0,
            pending: 0,
            head: None,
            grant: None,
        }
    }
    pub(super) fn migration_ready(
        &mut self,
        ordinary: u64,
        head: Option<u64>,
        fund: bool,
    ) -> Result<()> {
        if self.repair_free != 0
            || self.outstanding != 0
            || self.grant.is_some()
            || self.pending != ordinary
            || self.head != head
            || ordinary > 64
        {
            return Err(Error::Integrity("migration one-time funding"));
        }
        if fund {
            self.repair_free = REPAIR_POOL;
        }
        Ok(())
    }
    pub(super) fn migration_import_pending(&mut self, ordinal: u64, head: bool) -> Result<()> {
        if ordinal >= 512 || self.repair_free != 0 || self.outstanding != 0 || self.grant.is_some()
        {
            return Err(Error::Integrity("legacy pending import"));
        }
        if head {
            if self.head.replace(ordinal).is_some() {
                return Err(Error::Conflict);
            }
        } else {
            self.pending = self
                .pending
                .checked_add(1)
                .filter(|n| *n <= 64)
                .ok_or(Error::Quota)?;
        }
        Ok(())
    }
    pub(super) fn runtime_state(&self) -> RuntimeRepairState {
        RuntimeRepairState {
            repair_free: self.repair_free,
            outstanding: self.outstanding,
            pending: self.pending,
            head: self.head,
            grant: self
                .grant
                .as_ref()
                .map(|g| (g.id, g.start, g.end, g.available)),
        }
    }
    pub(super) fn is_pristine(&self) -> bool {
        self.repair_free == REPAIR_POOL
            && self.outstanding == 0
            && self.pending == 0
            && self.head.is_none()
            && self.grant.is_none()
    }
    pub fn new(quota: u64) -> Result<Self> {
        let ledger = Self {
            quota,
            repair_free: REPAIR_POOL,
            outstanding: 0,
            pending: 0,
            head: None,
            grant: None,
        };
        ledger.check(0, 0)?;
        Ok(ledger)
    }
    fn grant_bytes(&self) -> u64 {
        self.grant.as_ref().map_or(0, |g| g.count() * FRAME_CREDIT)
    }
    pub fn check(&self, used: u64, reserved: u64) -> Result<()> {
        let protected = self
            .repair_free
            .checked_add(self.grant_bytes())
            .and_then(|v| v.checked_add(if self.head.is_none() { TEMP_CREDIT } else { 0 }))
            .ok_or(Error::Bound("protected credits"))?;
        let total = used
            .checked_add(reserved)
            .and_then(|v| v.checked_add(protected))
            .ok_or(Error::Bound("quota sum"))?;
        let pool = self
            .repair_free
            .checked_add(self.grant_bytes())
            .and_then(|v| v.checked_add(self.outstanding))
            .ok_or(Error::Bound("pool"))?;
        if total > self.quota || pool > REPAIR_POOL || self.pending > 64 {
            return Err(Error::Quota);
        }
        Ok(())
    }
    pub fn encode(&self, out: &mut Vec<u8>) {
        for n in [self.quota, self.repair_free, self.outstanding, self.pending] {
            out.extend(n.to_be_bytes());
        }
        out.push(u8::from(self.head.is_some()));
        out.extend(self.head.unwrap_or(0).to_be_bytes());
        out.push(u8::from(self.grant.is_some()));
        if let Some(g) = &self.grant {
            out.extend(g.id);
            out.extend(g.start.to_be_bytes());
            out.extend(g.end.to_be_bytes());
            out.extend(g.available);
        }
    }
    pub fn decode(r: &mut Reader<'_>) -> Result<Self> {
        let quota = r.u64()?;
        let repair_free = r.u64()?;
        let outstanding = r.u64()?;
        let pending = r.u64()?;
        let head_flag = r.byte()?;
        let h = r.u64()?;
        let head = match head_flag {
            0 if h == 0 => None,
            1 => Some(h),
            _ => return Err(Error::Integrity("head flag")),
        };
        let grant = match r.byte()? {
            0 => None,
            1 => {
                let id = r.id()?;
                let start = r.u64()?;
                let end = r.u64()?;
                let available = r.id()?;
                if end.checked_sub(start).is_none_or(|v| v == 0 || v > 256) {
                    return Err(Error::Integrity("grant bounds"));
                }
                for i in (end - start) as usize..256 {
                    if available[i / 8] & (1 << (i % 8)) != 0 {
                        return Err(Error::Integrity("grant bitmap"));
                    }
                }
                Some(Grant {
                    id,
                    start,
                    end,
                    available,
                })
            }
            _ => return Err(Error::Integrity("grant flag")),
        };
        Ok(Self {
            quota,
            repair_free,
            outstanding,
            pending,
            head,
            grant,
        })
    }
}
#[derive(Serialize)]
pub struct CreditReport {
    pub used: [u64; 2],
    pub reserved: [u64; 2],
    pub repair_free: [u64; 2],
    pub grant_unissued: [u64; 2],
    pub pending: [u64; 2],
    pub head_in_use: [bool; 2],
}
pub(super) fn report(root: &Root) -> Result<CreditReport> {
    let (used, reserved) = root.totals()?;
    Ok(CreditReport {
        used,
        reserved,
        repair_free: [root.ledger[0].repair_free, root.ledger[1].repair_free],
        grant_unissued: [root.ledger[0].grant_bytes(), root.ledger[1].grant_bytes()],
        pending: [root.ledger[0].pending, root.ledger[1].pending],
        head_in_use: [root.ledger[0].head.is_some(), root.ledger[1].head.is_some()],
    })
}
struct Ticket {
    stage: u8,
    id: Id,
    repair: bool,
    head: bool,
    body: Vec<u8>,
    admission_paid: u64,
    captures: Option<[Id; 2]>,
}
/// Owner-visible transport bytes and public credential references, never device/root secrets.
pub struct PendingView {
    pub event: Id,
    pub body: Vec<u8>,
    pub head: bool,
    pub original_auth: Option<Id>,
    pub admission_auth: Option<Id>,
}
pub(super) fn pending_view(raw: &[u8]) -> Result<PendingView> {
    let t = Ticket::decode(raw)?;
    if t.stage != 2 {
        return Err(Error::Integrity("pending phase"));
    }
    Ok(PendingView {
        event: t.id,
        body: t.body,
        head: t.head,
        original_auth: t.captures.map(|v| v[0]),
        admission_auth: t.captures.map(|v| v[1]),
    })
}
pub(super) fn history_ticket(raw: &[u8], stage: u8) -> Result<(Id, Vec<u8>)> {
    let t = Ticket::decode(raw)?;
    if t.stage != stage {
        return Err(Error::Integrity("history ticket phase"));
    }
    Ok((t.id, t.body))
}
pub(super) fn pending_captures(raw: &[u8]) -> Result<Option<[Id; 2]>> {
    let ticket = Ticket::decode(raw)?;
    if ticket.stage != 2 {
        return Err(Error::Integrity("pending capture stage"));
    }
    Ok(ticket.captures)
}
impl Ticket {
    fn encode(&self) -> Vec<u8> {
        let mut out = vec![5, self.stage, u8::from(self.repair), u8::from(self.head)];
        out.extend(self.id);
        out.extend(self.admission_paid.to_be_bytes());
        out.push(u8::from(self.captures.is_some()));
        if let Some(refs) = self.captures {
            out.extend(refs[0]);
            out.extend(refs[1]);
        }
        out.extend((self.body.len() as u64).to_be_bytes());
        out.extend(&self.body);
        out
    }
    fn decode(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw);
        if r.byte()? != 5 {
            return Err(Error::Integrity("ticket version"));
        }
        let stage = r.byte()?;
        let repair = r.byte()?;
        let head = r.byte()?;
        let id = r.id()?;
        let admission_paid = r.u64()?;
        let captures = match r.byte()? {
            0 => None,
            1 => Some([r.id()?, r.id()?]),
            _ => return Err(Error::Integrity("capture flag")),
        };
        let len = r.u64()?;
        if !(1..=3).contains(&stage) || repair > 1 || head > 1 || len > 16 * 1024 {
            return Err(Error::Integrity("ticket bound"));
        }
        let body = r.take(len as usize)?.to_vec();
        r.end()?;
        if admission_paid > TEMP_CREDIT
            || captures.is_none() && admission_paid != 0
            || stage != 2 && (repair != 0 || head != 0 || captures.is_some() || admission_paid != 0)
            || head != 0 && repair == 0
            || stage == 2 && charge(raw) + admission_paid > TEMP_CREDIT
        {
            return Err(Error::Integrity("ticket conservation/phase"));
        }
        Ok(Self {
            stage,
            id,
            repair: repair != 0,
            head: head != 0,
            body,
            admission_paid,
            captures,
        })
    }
}
fn key(c: u8, ns: Namespace, n: u64) -> Result<Key> {
    Key::ordinal(c, ns, n)
}
fn charge(bytes: &[u8]) -> u64 {
    bytes.len() as u64 + 41 + 32 + 2 * NODE_CAP as u64
}
fn ticket<
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
>(
    w: &mut Work<'_, N, V>,
    key: Key,
    stage: u8,
    id: Id,
) -> Result<Ticket> {
    let t = Ticket::decode(&w.get(key)?.ok_or(Error::Conflict)?)?;
    if t.stage != stage || t.id != id {
        return Err(Error::Conflict);
    }
    Ok(t)
}
fn cost<
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
>(
    w: &mut Work<'_, N, V>,
    c: u8,
    event: Id,
    records: &[Key],
    receipt: bool,
) -> Result<u64> {
    if records.is_empty() || records.len() > 24 {
        return Err(Error::Bound("retention evidence"));
    }
    let mut total = 0u64;
    let mut event_seen = false;
    let mut projection = false;
    let mut receipt_seen = false;
    let mut distinct = BTreeSet::new();
    for k in records {
        if k.context != c
            || !distinct.insert(*k)
            || matches!(
                k.namespace,
                Namespace::Credit | Namespace::Queue | Namespace::Pending | Namespace::Undelivered
            )
        {
            return Err(Error::Bound("retention key"));
        }
        if w.get(*k)?.is_none() {
            return Err(Error::Integrity("retention missing"));
        }
        if k.namespace == Namespace::Event && k.bytes == event {
            event_seen = true;
        }
        if k.namespace == Namespace::Projection && k.bytes == event {
            projection = true;
        }
        if matches!(k.namespace, Namespace::ReceiptA | Namespace::ReceiptB) {
            receipt_seen = true;
        }
        if w.spent.insert(*k) {
            total = total
                .checked_add(w.new_charge.get(k).copied().unwrap_or(0))
                .ok_or(Error::Bound("retention charges"))?;
        }
    }
    if receipt {
        if !receipt_seen {
            return Err(Error::Bound("receipt evidence required"));
        }
    } else if !event_seen || !projection {
        return Err(Error::Bound("source+projection required"));
    }
    w.settled[usize::from(c)] = true;
    Ok(total)
}
pub(super) fn apply<
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
>(
    w: &mut Work<'_, N, V>,
    action: CreditAction,
) -> Result<()> {
    match action {
        CreditAction::Queue {
            context: c,
            sequence,
            id,
            body,
        } => {
            if body.len() > 4096 || std::str::from_utf8(&body).is_err() {
                return Err(Error::Bound("queue body"));
            }
            let k = key(c, Namespace::Queue, sequence)?;
            let t = Ticket {
                stage: 1,
                admission_paid: 0,
                captures: None,
                id,
                repair: false,
                head: false,
                body,
            };
            let raw = t.encode();
            if let Some(old) = w.get(k)? {
                if old == raw {
                    return Ok(());
                }
                return Err(Error::Conflict);
            }
            w.put(k, &raw, FRAME_CREDIT, true)?;
        }
        CreditAction::Grant {
            context: c,
            id,
            start,
            end,
        } => {
            if c > 1 || end.checked_sub(start).is_none_or(|v| v == 0 || v > 256) {
                return Err(Error::Bound("grant window"));
            }
            let index = usize::from(c);
            if let Some(old) = &w.root.ledger[index].grant {
                if old.id == id && old.start == start && old.end == end {
                    return Ok(());
                }
                return Err(Error::Conflict);
            }
            let mut g = Grant {
                id,
                start,
                end,
                available: [0; 32],
            };
            for n in start..end {
                if w.get(key(c, Namespace::Pending, n)?)?.is_none() {
                    let (i, b) = g.bit(n)?;
                    g.available[i] |= b;
                }
            }
            let need = g.count() * FRAME_CREDIT;
            w.root.ledger[index].repair_free = w.root.ledger[index]
                .repair_free
                .checked_sub(need)
                .ok_or(Error::Quota)?;
            w.root.ledger[index].grant = Some(g);
        }
        CreditAction::CancelGrant { context: c, id } => {
            let l = w
                .root
                .ledger
                .get_mut(usize::from(c))
                .ok_or(Error::Bound("context"))?;
            let g = l.grant.as_ref().ok_or(Error::Conflict)?;
            if g.id != id {
                return Err(Error::Conflict);
            }
            l.repair_free = l
                .repair_free
                .checked_add(g.count() * FRAME_CREDIT)
                .ok_or(Error::Bound("cancel"))?;
            l.grant = None;
        }
        CreditAction::Refill { context: c, bytes } => {
            let l = w
                .root
                .ledger
                .get_mut(usize::from(c))
                .ok_or(Error::Bound("context"))?;
            l.repair_free = l
                .repair_free
                .checked_add(bytes)
                .ok_or(Error::Bound("refill"))?;
        }
        CreditAction::Pending {
            context: c,
            ordinal,
            event,
            body,
            head,
            grant,
        } => {
            if c > 1 || body.len() > 16 * 1024 || head && grant.is_none() {
                return Err(Error::Bound("pending"));
            }
            let k = key(c, Namespace::Pending, ordinal)?;
            let t = Ticket {
                stage: 2,
                admission_paid: 0,
                captures: None,
                id: event,
                repair: grant.is_some(),
                head,
                body,
            };
            let raw = t.encode();
            if let Some(old) = w.get(k)? {
                let previous = Ticket::decode(&old)?;
                if previous.stage == 2 && previous.id == t.id && previous.body == t.body {
                    return Ok(());
                }
                return Err(Error::Conflict);
            }
            let l = &mut w.root.ledger[usize::from(c)];
            if head {
                if l.head.is_some() {
                    return Err(Error::Quota);
                }
                l.head = Some(ordinal);
            } else {
                l.pending = l
                    .pending
                    .checked_add(1)
                    .ok_or(Error::Bound("pending count"))?;
                if l.pending > 64 {
                    return Err(Error::Quota);
                }
            }
            if let Some(id) = grant {
                let g = l.grant.as_mut().ok_or(Error::Conflict)?;
                if g.id != id {
                    return Err(Error::Conflict);
                }
                let (i, b) = g.bit(ordinal)?;
                if g.available[i] & b == 0 {
                    return Err(Error::Conflict);
                }
                g.available[i] &= !b;
                l.outstanding = l
                    .outstanding
                    .checked_add(FRAME_CREDIT)
                    .ok_or(Error::Bound("repair outstanding"))?;
            }
            let reserved = (FRAME_CREDIT + TEMP_CREDIT)
                .checked_sub(charge(&raw))
                .ok_or(Error::Bound("temporary charge"))?;
            w.put(k, &raw, reserved, true)?;
        }
        CreditAction::Materialize {
            context: c,
            queue,
            request,
            ordinal,
            event,
            records,
        } => {
            let old = key(c, Namespace::Queue, queue)?;
            let queued = ticket(w, old, 1, request)?;
            let bytes = w
                .get(Key::hash(c, Namespace::Projection, event)?)?
                .ok_or(Error::Integrity("materialized projection missing"))?;
            if ProjectionValue::decode(&bytes)?.text.as_bytes() != queued.body {
                return Err(Error::Conflict);
            }
            let cost = cost(w, c, event, &records, false)?;
            let body = postcard::to_allocvec(&records).map_err(|_| Error::Bound("source refs"))?;
            let raw = Ticket {
                stage: 3,
                admission_paid: 0,
                captures: None,
                id: event,
                repair: false,
                head: false,
                body,
            }
            .encode();
            let reserved = FRAME_CREDIT
                .checked_sub(cost)
                .and_then(|v| v.checked_sub(charge(&raw)))
                .ok_or(Error::Quota)?;
            if reserved < MIN_DEBT {
                return Err(Error::Quota);
            }
            let next = key(c, Namespace::Undelivered, ordinal)?;
            if w.get(next)?.is_some() {
                return Err(Error::Conflict);
            }
            w.put(next, &raw, reserved, true)?;
            w.remove(old)?;
        }
        CreditAction::Readable {
            context: c,
            ordinal,
            event,
            mut records,
        } => {
            let old = key(c, Namespace::Pending, ordinal)?;
            let t = ticket(w, old, 2, event)?;
            if !records.iter().any(|k| {
                k.context == c
                    && matches!(k.namespace, Namespace::ReceiptA | Namespace::ReceiptB)
                    && k.ordinal_value().is_ok_and(|n| n == ordinal)
            }) {
                return Err(Error::Bound("readable receipt ordinal"));
            }
            if w.get(Key::hash(c, Namespace::Event, event)?)?.as_deref() != Some(t.body.as_slice())
            {
                return Err(Error::Conflict);
            }
            if let Some(captures) = t.captures {
                let k = Key::hash(c, Namespace::Admission, event)?;
                let mut value = vec![5];
                value.extend(captures[0]);
                value.extend(captures[1]);
                w.put(k, &value, 0, false)?;
                if !records.contains(&k) {
                    records.push(k);
                }
            }
            let spent = cost(w, c, event, &records, false)?
                .checked_add(t.admission_paid)
                .ok_or(Error::Bound("readable retained charge"))?;
            if spent > FRAME_CREDIT {
                return Err(Error::Quota);
            }
            let l = &mut w.root.ledger[usize::from(c)];
            if t.head {
                if l.head != Some(ordinal) {
                    return Err(Error::Integrity("head ownership"));
                }
                l.head = None;
            } else {
                l.pending = l
                    .pending
                    .checked_sub(1)
                    .ok_or(Error::Integrity("pending underflow"))?;
            }
            if t.repair {
                l.outstanding = l
                    .outstanding
                    .checked_sub(FRAME_CREDIT)
                    .ok_or(Error::Integrity("outstanding underflow"))?;
                l.repair_free = l
                    .repair_free
                    .checked_add(FRAME_CREDIT - spent)
                    .ok_or(Error::Bound("repair remainder"))?;
            }
            w.remove(old)?;
        }
        CreditAction::Acknowledge {
            context: c,
            ordinal,
            event,
            records,
        } => {
            let old = key(c, Namespace::Undelivered, ordinal)?;
            ticket(w, old, 3, event)?;
            if !records.iter().any(|k| {
                matches!(k.namespace, Namespace::ReceiptA | Namespace::ReceiptB)
                    && k.ordinal_value().is_ok_and(|n| n == ordinal)
            }) {
                return Err(Error::Bound("receipt ordinal"));
            }
            let cost = cost(w, c, event, &records, true)?;
            let s = old.slot()?;
            let leaf = w
                .tree
                .find(s as u8, w.root.roots[s], old.bytes)?
                .ok_or(Error::Integrity("debt"))?;
            let tree::Body::Leaf { reserved, .. } = leaf.body else {
                return Err(Error::Integrity("debt leaf"));
            };
            if cost > reserved {
                return Err(Error::Quota);
            }
            w.remove(old)?;
        }
        CreditAction::CapturePending {
            context: c,
            ordinal,
            event,
            original_auth,
            admission_auth,
        } => {
            let k = key(c, Namespace::Pending, ordinal)?;
            let mut pending = ticket(w, k, 2, event)?;
            let captures = [original_auth, admission_auth];
            if let Some(old) = pending.captures {
                if old == captures {
                    return Ok(());
                }
                return Err(Error::Conflict);
            }
            let old_raw = pending.encode();
            let mut paid = 0u64;
            for id in captures {
                let auth = Key::hash(c, Namespace::Auth, id)?;
                if w.get(auth)?.is_none() {
                    return Err(Error::Integrity("admission snapshot missing"));
                }
                if w.spent.insert(auth) {
                    paid = paid
                        .checked_add(w.new_charge.get(&auth).copied().unwrap_or(0))
                        .ok_or(Error::Bound("capture charge"))?;
                }
            }
            pending.captures = Some(captures);
            pending.admission_paid = paid;
            let raw = pending.encode();
            if charge(&raw)
                .checked_add(paid)
                .is_none_or(|v| v > TEMP_CREDIT)
            {
                return Err(Error::Bound("full pending slot"));
            }
            let s = k.slot()?;
            let leaf = w
                .tree
                .find(s as u8, w.root.roots[s], k.bytes)?
                .ok_or(Error::Integrity("pending slot"))?;
            let tree::Body::Leaf { reserved, .. } = leaf.body else {
                return Err(Error::Integrity("pending leaf"));
            };
            let consumed = paid
                .checked_add(charge(&raw) - charge(&old_raw))
                .ok_or(Error::Bound("pending capture cost"))?;
            let reserved = reserved.checked_sub(consumed).ok_or(Error::Quota)?;
            w.settled[usize::from(c)] = true;
            w.put(k, &raw, reserved, true)?;
        }
    }
    Ok(())
}
