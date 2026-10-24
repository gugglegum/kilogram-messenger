//! Causal history over immutable authenticated rows. No wire/native runtime adapter.
use super::history_cursor::{self as cursor, Position};
use super::history_types::*;
use super::*;

#[cfg(test)]
#[path = "history_scale.rs"]
mod scale;
#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
#[path = "history_writer.rs"]
mod writer;

#[derive(Clone, Debug)]
pub struct HistoryPage {
    pub view: Id,
    pub frontiers: [u64; 2],
    pub finished: bool,
    pub cursor: HistoryCursor,
    pub items: Vec<HistoryRow>,
    pub stats: HistoryReadStats,
}

pub(super) fn identity_key(c: u8) -> Result<Key> {
    Key::hash(
        c,
        Namespace::Control,
        *blake3::hash(b"kg:history2:scope-key\0").as_bytes(),
    )
}
pub(super) fn heads_key(c: u8) -> Result<Key> {
    Key::hash(
        c,
        Namespace::Current,
        *blake3::hash(b"kg:history2:heads-key\0").as_bytes(),
    )
}
pub(super) fn lane(c: u8, role: u8, n: u64) -> Result<Key> {
    let namespace = match role {
        0 => Namespace::LaneA,
        1 => Namespace::LaneB,
        _ => return Err(Error::Integrity("history lane role")),
    };
    Key::ordinal(c, namespace, n)
}
pub(super) fn receipt(c: u8, role: u8, n: u64) -> Result<Key> {
    let namespace = match role {
        0 => Namespace::ReceiptA,
        1 => Namespace::ReceiptB,
        _ => return Err(Error::Integrity("history receipt role")),
    };
    Key::ordinal(c, namespace, n)
}
fn id_value(raw: &[u8]) -> Result<Id> {
    raw.try_into()
        .map_err(|_| Error::Integrity("history Lane ID"))
}
pub(super) struct Candidate {
    pub row: HistoryRow,
    pub binding: Binding,
}
fn choose_candidate(candidates: &[Option<Candidate>; 2], next: [u64; 2]) -> Result<usize> {
    (0..2)
        .filter(|&role| {
            candidates[role].as_ref().is_some_and(|c| {
                c.binding
                    .parents
                    .iter()
                    .all(|p| p.ordinal < next[usize::from(p.role)])
            })
        })
        .min_by_key(|&role| candidates[role].as_ref().map(|c| c.row.id))
        .ok_or(Error::Integrity("history causal frontier blocked"))
}

impl IndexedDialogVault {
    pub fn initialize_history(
        &mut self,
        context: u8,
        expected: u64,
        commit_id: Id,
        init: HistoryInit,
    ) -> Result<CommitReport> {
        self.commit_history(context, expected, commit_id, HistoryOperation::Init(init))
    }
    pub fn commit_history(
        &mut self,
        context: u8,
        expected: u64,
        commit_id: Id,
        operation: HistoryOperation,
    ) -> Result<CommitReport> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        if context > 1 {
            return Err(Error::Bound("history context"));
        }
        operation.check_bound()?;
        let view = if let HistoryOperation::Init(init) = &operation {
            Identity {
                incarnation: self.root.incarnation,
                init: init.clone(),
            }
            .view_id()?
        } else {
            let tx = self.vault.database.begin_read().map_err(db)?;
            let nodes = tx.open_table(NODES).map_err(db)?;
            let values = tx.open_table(VALUES).map_err(db)?;
            Work::new(&nodes, &values, &self.crypto, self.root.clone())
                .history_view(context)?
                .1
                .view
        };
        // Bound typed inputs before serialization. No caller-supplied digest is accepted.
        let raw = Zeroizing::new(
            postcard::to_allocvec(&(context, view, expected, &operation))
                .map_err(|_| Error::Bound("history commit codec"))?,
        );
        if raw.len() > MAX_PLAN {
            return Err(Error::Bound("history input plan"));
        }
        let digest = self.crypto.hash(0, b"history-commit-request/v1", &raw);
        let input_buffer_bound = raw.len() * 3 + 16384;
        self.commit_with(
            expected,
            commit_id,
            digest,
            CommitInput::History {
                context,
                operation: Box::new(operation),
            },
            input_buffer_bound,
        )
    }
    pub fn history_first(&mut self, context: u8, limit: usize) -> Result<HistoryPage> {
        self.history_read(context, None, limit)
    }
    pub fn history_page(
        &mut self,
        context: u8,
        cursor: &HistoryCursor,
        limit: usize,
    ) -> Result<HistoryPage> {
        self.history_read(context, Some(cursor), limit)
    }
    fn history_read(
        &self,
        context: u8,
        supplied: Option<&HistoryCursor>,
        limit: usize,
    ) -> Result<HistoryPage> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        if !(1..=32).contains(&limit) {
            return Err(Error::Bound("history page limit"));
        }
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        let (identity, heads) = w.history_view(context)?;
        let mut position = if let Some(supplied) = supplied {
            cursor::authenticate(&self.crypto, &identity, supplied)?
        } else {
            Position {
                frontiers: heads.frontiers,
                next: [1, 1],
            }
        };
        for role in 0..2 {
            if position.next[role] == 0 || position.next[role] > position.frontiers[role] {
                return Err(Error::InvalidCursor);
            }
            if position.frontiers[role] > heads.frontiers[role] {
                return Err(Error::SnapshotUnavailable);
            }
        }
        let mut candidates: [Option<Candidate>; 2] = [None, None];
        let mut stats = HistoryReadStats::default();
        for (role, candidate) in candidates.iter_mut().enumerate() {
            if position.next[role] < position.frontiers[role] {
                *candidate = Some(w.history_candidate(
                    context,
                    role as u8,
                    position.next[role],
                    &heads.view,
                    position.frontiers,
                )?);
                stats.candidate_loads += 1;
            }
        }
        stats.max_candidates = candidates.iter().flatten().count();
        let mut items = Vec::with_capacity(limit);
        while items.len() < limit && position.next != position.frontiers {
            let selected = choose_candidate(&candidates, position.next)?;
            let candidate = candidates[selected]
                .take()
                .ok_or(Error::Integrity("history candidate missing"))?;
            items.push(candidate.row);
            position.next[selected] = position.next[selected]
                .checked_add(1)
                .ok_or(Error::Bound("history ordinal overflow"))?;
            if position.next[selected] < position.frontiers[selected] {
                candidates[selected] = Some(w.history_candidate(
                    context,
                    selected as u8,
                    position.next[selected],
                    &heads.view,
                    position.frontiers,
                )?);
                stats.candidate_loads += 1;
            }
            stats.max_candidates = stats
                .max_candidates
                .max(candidates.iter().flatten().count());
        }
        stats.point_gets = w.history_gets;
        stats.nodes_read = w.tree.reads;
        stats.values_read = w.values_read;
        stats.returned_payload_bytes = items.iter().map(|r| r.text.len()).sum();
        stats.app_buffer_bound_bytes =
            (w.history_buffer_peak + 1024 * 1024 + stats.returned_payload_bytes + 16384) as u64;
        if stats.max_candidates > 2 || stats.point_gets > 8 * (limit as u64 + 2) + 2 {
            return Err(Error::Bound("history read work"));
        }
        let page = HistoryPage {
            view: heads.view,
            frontiers: position.frontiers,
            finished: position.next == position.frontiers,
            cursor: cursor::seal(&self.crypto, &identity, &position)?,
            items,
            stats,
        };
        // No advanced cursor escapes before both actual response encodings fit.
        page.encode_cbor()?;
        page.encode_json()?;
        Ok(page)
    }
    pub fn history_point(&mut self, context: u8, event: Id) -> Result<Option<HistoryRow>> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        let (_, heads) = w.history_view(context)?;
        let Some((binding, _)) = w.history_projection(context, event, &heads.view)? else {
            if w.hget(Key::hash(context, Namespace::Event, event)?)?
                .is_some()
            {
                return Err(Error::Integrity("history orphan source"));
            }
            return Ok(None);
        };
        let candidate = w.history_candidate(
            context,
            binding.role,
            binding.ordinal,
            &heads.view,
            heads.frontiers,
        )?;
        if candidate.row.id != event {
            return Err(Error::Integrity("history point Lane mismatch"));
        }
        Ok((binding.kind == 3).then_some(candidate.row))
    }
    pub fn history_request(
        &mut self,
        context: u8,
        request: [u8; 16],
    ) -> Result<Option<HistoryRequestOutcome>> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        let (_, heads) = w.history_view(context)?;
        let Some((record, _)) = w.history_request_record(context, request, &heads.view)? else {
            return Ok(None);
        };
        Ok(Some(HistoryRequestOutcome {
            queue_sequence: record.queue,
            event: record.event,
        }))
    }
}

impl<
    'a,
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
> Work<'a, N, V>
{
    fn hget(&mut self, key: Key) -> Result<Option<Vec<u8>>> {
        self.history_gets += 1;
        let value = self.get(key)?;
        if self.changed.is_empty() && self.tree.changed.is_empty() {
            self.history_buffer_peak = self.history_buffer_peak.max(self.buffer_bound_bytes());
        }
        Ok(value)
    }
    fn hrequired(&mut self, key: Key) -> Result<Vec<u8>> {
        self.hget(key)?
            .ok_or(Error::Integrity("history expected record missing"))
    }
    fn history_view(&mut self, c: u8) -> Result<(Identity, Heads)> {
        let a = self.hget(identity_key(c)?)?;
        let b = self.hget(heads_key(c)?)?;
        let (a, b) = match (a, b) {
            (Some(a), Some(b)) => (a, b),
            (None, None) => return Err(Error::HistoryUnavailable),
            _ => return Err(Error::Integrity("partial history view")),
        };
        let identity = Identity::decode(&a)?;
        let heads = Heads::decode(&b)?;
        if identity.incarnation != self.root.incarnation
            || identity.init.scope.context != self.root.contexts[usize::from(c)]
            || identity.view_id()? != heads.view
            || self.root.roots[identity_key(c)?.slot()?].count != 1
            || self.root.roots[heads_key(c)?.slot()?].count != 1
        {
            return Err(Error::Integrity("history persisted scope/view"));
        }
        for role in 0..2 {
            let count = self.root.roots[lane(c, role as u8, 0)?.slot()?].count;
            if match heads.tails[role] {
                None => heads.frontiers[role] != 1 || count != 0,
                Some(_) => count != heads.frontiers[role],
            } {
                return Err(Error::Integrity("history heads/count"));
            }
        }
        if (heads.tails[1].is_some() && heads.tails[0].is_none())
            || (heads.frontiers.iter().any(|n| *n > 1) && heads.tails.contains(&None))
        {
            return Err(Error::Integrity("history genesis shape"));
        }
        Ok((identity, heads))
    }
    fn history_projection(
        &mut self,
        c: u8,
        id: Id,
        view: &Id,
    ) -> Result<Option<(Binding, String)>> {
        let Some(raw) = self.hget(Key::hash(c, Namespace::Projection, id)?)? else {
            return Ok(None);
        };
        let projection = ProjectionValue::decode(&raw)?;
        let binding = Binding::decode(&projection.binding)?;
        if binding.view != *view || (binding.kind != 3 && !projection.text.is_empty()) {
            return Err(Error::Integrity("history projection view/kind"));
        }
        Ok(Some((binding, projection.text)))
    }
    fn history_candidate(
        &mut self,
        c: u8,
        role: u8,
        n: u64,
        view: &Id,
        frontiers: [u64; 2],
    ) -> Result<Candidate> {
        // A page never accumulates proof paths for all candidates in its output.
        self.tree.clear_read_proofs();
        if n >= frontiers[usize::from(role)] && n != 0 {
            return Err(Error::Integrity("history candidate frontier"));
        }
        let id = id_value(&self.hrequired(lane(c, role, n)?)?)?;
        let (binding, text) = self
            .history_projection(c, id, view)?
            .ok_or(Error::Integrity("history projection missing"))?;
        if binding.role != role || binding.ordinal != n {
            return Err(Error::Integrity("history candidate slot"));
        }
        let source = self.hrequired(Key::hash(c, Namespace::Event, id)?)?;
        binding.verify_source(id, &source)?;
        if n > 0 && binding.previous != Some(id_value(&self.hrequired(lane(c, role, n - 1)?)?)?) {
            return Err(Error::Integrity("history previous Lane"));
        }
        for p in &binding.parents {
            if p.ordinal >= frontiers[usize::from(p.role)] || p.event == id {
                return Err(Error::Integrity("history parent frontier"));
            }
            let (parent, _) = self
                .history_projection(c, p.event, view)?
                .ok_or(Error::Integrity("history parent missing"))?;
            if parent.kind != 3
                || parent.role != p.role
                || parent.ordinal != p.ordinal
                || id_value(&self.hrequired(lane(c, p.role, p.ordinal)?)?)? != p.event
            {
                return Err(Error::Integrity("history parent mapping"));
            }
        }
        let row = HistoryRow {
            id,
            role,
            ordinal: n,
            sequence: binding.sequence,
            request: binding.request,
            text,
        };
        Ok(Candidate { row, binding })
    }
    fn history_request_record(
        &mut self,
        c: u8,
        request: [u8; 16],
        view: &Id,
    ) -> Result<Option<(RequestRecord, String)>> {
        let Some(raw) = self.hget(Key::request(c, request)?)? else {
            return Ok(None);
        };
        let record = RequestRecord::decode(&raw)?;
        if record.request != request {
            return Err(Error::Integrity("history request key"));
        }
        let text = if let Some(event) = record.event {
            let (binding, text) = self
                .history_projection(c, event, view)?
                .ok_or(Error::Integrity("history request Event"))?;
            let identity = Identity::decode(&self.hrequired(identity_key(c)?)?)?;
            if binding.kind != 3
                || binding.request != request
                || binding.role != identity.init.own_role
                || id_value(&self.hrequired(lane(c, binding.role, binding.ordinal)?)?)? != event
            {
                return Err(Error::Integrity("history materialized request"));
            }
            binding.verify_source(
                event,
                &self.hrequired(Key::hash(c, Namespace::Event, event)?)?,
            )?;
            text
        } else {
            let raw = self.hrequired(Key::ordinal(c, Namespace::Queue, record.queue)?)?;
            let (id, body) = credits::history_ticket(&raw, 1)?;
            let mut handle = [0; 32];
            handle[..16].copy_from_slice(&request);
            if id != handle {
                return Err(Error::Integrity("history Queue request"));
            }
            String::from_utf8(body).map_err(|_| Error::Integrity("history Queue text"))?
        };
        if hash_parts(b"kg:history2:request-body\0", &[text.as_bytes()]) != record.body_hash {
            return Err(Error::Integrity("history request body"));
        }
        Ok(Some((record, text)))
    }
}
