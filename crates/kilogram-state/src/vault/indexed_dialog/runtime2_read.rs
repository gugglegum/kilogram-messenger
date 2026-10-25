//! Bounded, authenticated owner reads. No generic writable handle escapes.
use super::*;

pub struct RuntimeQueueItem {
    pub sequence: u64,
    pub request: [u8; 16],
    pub text: String,
}
pub struct RuntimeSource {
    pub id: Id,
    pub role: u8,
    pub ordinal: u64,
    pub raw: Vec<u8>,
}
pub struct RuntimeFrontiers {
    pub next: [u64; 2],
    pub tails: [Option<Id>; 2],
}
pub struct RuntimeReceipt {
    pub ordinal: u64,
    pub id: Id,
    pub raw: Vec<u8>,
}
pub struct RuntimeRepairState {
    pub repair_free: u64,
    pub outstanding: u64,
    pub pending: u64,
    pub head: Option<u64>,
    pub grant: Option<(Id, u64, u64, [u8; 32])>,
}
#[derive(serde::Serialize)]
pub struct RuntimeCounts {
    pub sources: u64,
    pub queued: u64,
    pub debt: u64,
    pub pending: u64,
    pub receipts: [u64; 2],
}
macro_rules! read_work {
    ($this:ident, $w:ident, $body:block) => {{
        if $this.store.poisoned {
            return Err(Error::ReopenRequired);
        }
        let tx = $this.store.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut $w = Work::new(
            &nodes,
            &values,
            &$this.store.crypto,
            $this.store.root.clone(),
        );
        $w.tree.enable_read_reuse();
        $body
    }};
}
impl RuntimeRepository {
    pub fn counts(&self, c: u8) -> Result<RuntimeCounts> {
        if self.store.poisoned {
            return Err(Error::ReopenRequired);
        }
        let count = |ns| -> Result<u64> {
            let k = Key::ordinal(c, ns, 0)?;
            Ok(self.store.root.roots[k.slot()?].count)
        };
        Ok(RuntimeCounts {
            sources: self.store.root.roots[Key::hash(c, Namespace::Event, [0; 32])?.slot()?].count,
            queued: count(Namespace::Queue)?,
            debt: count(Namespace::Undelivered)?,
            pending: count(Namespace::Pending)?,
            receipts: [count(Namespace::ReceiptA)?, count(Namespace::ReceiptB)?],
        })
    }
    pub fn repair_state(&self, c: u8) -> Result<RuntimeRepairState> {
        if self.store.poisoned {
            return Err(Error::ReopenRequired);
        }
        Ok(self
            .store
            .root
            .ledger
            .get(usize::from(c))
            .ok_or(Error::Bound("context"))?
            .runtime_state())
    }
    pub fn installed(&mut self, c: u8) -> Result<bool> {
        Ok(self.store.get(key(c, b"spec")?)?.is_some())
    }
    pub fn native(&mut self, c: u8) -> Result<Option<Vec<u8>>> {
        let snapshot = self.snapshot(c)?;
        if snapshot.head.phase < 3
            && self.store.root.roots[Key::hash(c, Namespace::Native, [0; 32])?.slot()?].count != 0
        {
            return Err(Error::Integrity("pre-session authoritative Native"));
        }
        if snapshot.head.phase == 3 && snapshot.pre_native.is_some() {
            return Err(Error::Integrity("duplicate pre-session Native"));
        }
        let raw = if snapshot.head.phase < 3 {
            snapshot.pre_native
        } else {
            self.store
                .history_checkpoint(c, snapshot.root_generation)?
                .native
                .map(|r| r.blob)
        };
        if snapshot.head.native_hash != raw.as_ref().map(|v| *blake3::hash(v).as_bytes()) {
            return Err(Error::Integrity("runtime Native Head link"));
        }
        Ok(raw)
    }
    pub fn seen(&mut self, c: u8, role: u8, key: Id) -> Result<Option<HistorySeenRecord>> {
        self.store.history_seen(c, self.generation(), role, key)
    }
    pub fn request(
        &mut self,
        c: u8,
        request: [u8; 16],
    ) -> Result<Option<(HistoryRequestOutcome, String)>> {
        read_work!(self, w, { w.runtime_request(c, request) })
    }
    /// 0 queued, 1 materialized, 2 durably acknowledged by the opposite receiver.
    pub fn request_delivery(&mut self, c: u8, request: [u8; 16]) -> Result<Option<u8>> {
        let Some((outcome, _)) = self.request(c, request)? else {
            return Ok(None);
        };
        let Some(id) = outcome.event else {
            return Ok(Some(0));
        };
        let role = self.snapshot(c)?.head.role;
        let row = self
            .history_point(c, id)?
            .ok_or(Error::Integrity("request source/projection"))?;
        if row.role != role || row.request != request {
            return Err(Error::Integrity("request owner binding"));
        }
        read_work!(self, w, {
            let receipt = w.runtime_receipt(c, role, row.ordinal)?;
            let debt = w.get(Key::ordinal(c, Namespace::Undelivered, row.ordinal)?)?;
            if receipt.is_some() == debt.is_some() {
                return Err(Error::Integrity("request receipt/debt correspondence"));
            }
            Ok(Some(if receipt.is_some() { 2 } else { 1 }))
        })
    }
    pub fn queue(&mut self, c: u8, first: u64, limit: usize) -> Result<Vec<RuntimeQueueItem>> {
        let raw = self
            .store
            .range(Key::ordinal(c, Namespace::Queue, first)?, limit)?;
        let mut out = Vec::with_capacity(raw.len());
        for (key, raw) in raw {
            let sequence = key.ordinal_value()?;
            let (handle, body) = credits::history_ticket(&raw, 1)?;
            if handle[16..] != [0; 16] {
                return Err(Error::Integrity("queue request handle"));
            }
            let request = handle[..16]
                .try_into()
                .map_err(|_| Error::Integrity("queue request"))?;
            let (record, text) = self
                .request(c, request)?
                .ok_or(Error::Integrity("queue request missing"))?;
            if record.queue_sequence != sequence
                || record.event.is_some()
                || text.as_bytes() != body
            {
                return Err(Error::Integrity("queue request correspondence"));
            }
            out.push(RuntimeQueueItem {
                sequence,
                request,
                text,
            });
        }
        Ok(out)
    }
    pub fn frontiers(&mut self, c: u8) -> Result<RuntimeFrontiers> {
        read_work!(self, w, {
            let (next, tails) = w.runtime_frontiers(c)?;
            Ok(RuntimeFrontiers { next, tails })
        })
    }
    pub fn source(&mut self, c: u8, role: u8, n: u64) -> Result<Option<RuntimeSource>> {
        read_work!(self, w, {
            if w.get(history::lane(c, role, n)?)?.is_none() {
                return Ok(None);
            }
            let (e, raw) = w.runtime_event(c, role, n)?;
            Ok(Some(RuntimeSource {
                id: e.id,
                role,
                ordinal: n,
                raw,
            }))
        })
    }
    pub fn source_page(
        &mut self,
        c: u8,
        role: u8,
        first: u64,
        limit: usize,
    ) -> Result<Vec<RuntimeSource>> {
        if !(1..=32).contains(&limit) {
            return Err(Error::Bound("source page"));
        }
        let frontiers = self.frontiers(c)?;
        if role > 1 {
            return Err(Error::Bound("source role"));
        }
        let end = first
            .saturating_add(limit as u64)
            .min(frontiers.next[usize::from(role)]);
        let mut out = Vec::with_capacity(limit);
        for n in first..end {
            out.push(
                self.source(c, role, n)?
                    .ok_or(Error::Integrity("source page hole"))?,
            );
        }
        Ok(out)
    }
    pub fn debt_page(&mut self, c: u8, first: u64, limit: usize) -> Result<Vec<RuntimeSource>> {
        if !(1..=32).contains(&limit) {
            return Err(Error::Bound("debt page"));
        }
        let role = self.snapshot(c)?.head.role;
        let entries = self
            .store
            .range(Key::ordinal(c, Namespace::Undelivered, first)?, limit)?;
        let mut out = Vec::with_capacity(entries.len());
        for (key, raw) in entries {
            let n = key.ordinal_value()?;
            let (id, _) = credits::history_ticket(&raw, 3)?;
            let source = self
                .source(c, role, n)?
                .ok_or(Error::Integrity("debt source missing"))?;
            if source.id != id {
                return Err(Error::Integrity("debt source correspondence"));
            }
            out.push(source);
        }
        Ok(out)
    }
    pub fn pending(&mut self, c: u8, n: u64) -> Result<Option<PendingView>> {
        read_work!(self, w, {
            let Some(raw) = w.get(Key::ordinal(c, Namespace::Pending, n)?)? else {
                return Ok(None);
            };
            let pending = credits::pending_view(&raw)?;
            w.history2_check_retry(
                c,
                &HistoryProtocol2Operation::AcceptPending {
                    source: pending.body.clone(),
                    head: pending.head,
                    grant: None,
                    admission_author_ref: pending
                        .admission_auth
                        .ok_or(Error::Integrity("pending admission"))?,
                    auth: vec![],
                },
            )?;
            Ok(Some(pending))
        })
    }
    pub fn authority(&mut self, c: u8, id: Id) -> Result<Vec<u8>> {
        let raw = self
            .store
            .get(Key::hash(c, Namespace::Auth, id)?)?
            .ok_or(Error::Integrity("authority missing"))?;
        if history::protocol2::source::read_auth_bundle(&raw)?.id != id {
            return Err(Error::Integrity("authority reference"));
        }
        Ok(raw)
    }
    pub fn receipts(&mut self, c: u8, first: u64, limit: usize) -> Result<Vec<RuntimeReceipt>> {
        if first == 0 || !(1..=32).contains(&limit) {
            return Err(Error::Bound("receipt page"));
        }
        let role = 1 - self.snapshot(c)?.head.role;
        read_work!(self, w, {
            let (next, _) = w.runtime_frontiers(c)?;
            let mut out = Vec::with_capacity(limit);
            for n in first
                ..first
                    .saturating_add(limit as u64)
                    .min(next[usize::from(role)])
            {
                let raw = w
                    .runtime_receipt(c, role, n)?
                    .ok_or(Error::Integrity("readable receipt missing"))?;
                let id = history_types::hash_parts(b"kg:od2:receipt-id\0", &[&raw]);
                out.push(RuntimeReceipt {
                    ordinal: n,
                    id,
                    raw,
                });
            }
            Ok(out)
        })
    }
    pub fn receipt(&mut self, c: u8, role: u8, n: u64) -> Result<Option<Vec<u8>>> {
        read_work!(self, w, { w.runtime_receipt(c, role, n) })
    }
    pub fn history(
        &mut self,
        c: u8,
        supplied: Option<&HistoryCursor>,
        limit: usize,
    ) -> Result<HistoryPage> {
        self.store
            .history_read_bounded(c, supplied, limit, Some(48 * 1024 - 512))
    }
    pub fn history_point(&mut self, c: u8, event: Id) -> Result<Option<HistoryRow>> {
        self.store.history_point(c, event)
    }
}
