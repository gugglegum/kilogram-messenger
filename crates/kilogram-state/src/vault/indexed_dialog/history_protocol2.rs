//! Closed protocol2 correspondence. Cryptography/native truth/current policy stay with the owner.
use super::*;
use serde::{Deserialize, Serialize};
#[path = "history_source_v2.rs"]
mod source;
use source::{
    EventFields, FreshFields, read_auth_bundle, read_event2, read_fresh_binding2, read_receipt2,
};
#[cfg(test)]
#[path = "history_protocol2_storage_tests.rs"]
mod storage_tests;
#[cfg(test)]
#[path = "history_protocol2_tests.rs"]
mod tests;

#[derive(Clone, Serialize)]
pub struct HistoryProtocol2Binding {
    pub proof: Vec<u8>,
    pub author_slots: [u8; 2],
}
#[derive(Clone, Serialize)]
pub struct HistoryReadable2 {
    pub source: Vec<u8>,
    pub text: String,
    pub auth: Vec<HistoryAuth>,
    pub readable_author_ref: Id,
    pub receiver_receipt: Option<Vec<u8>>,
}
#[derive(Clone, Serialize)]
pub enum HistoryProtocol2Operation {
    Init {
        init: HistoryInit,
        binding: HistoryProtocol2Binding,
    },
    Genesis {
        row: HistoryReadable2,
        native: HistoryNativeTransition,
    },
    TextBatch {
        rows: Vec<HistoryReadable2>,
        native: HistoryNativeTransition,
    },
    AcceptPending {
        source: Vec<u8>,
        head: bool,
        grant: Option<Id>,
        admission_author_ref: Id,
        auth: Vec<HistoryAuth>,
    },
    Acknowledge {
        receipt_format: u8,
        receipt_raw: Vec<u8>,
        auth: Vec<HistoryAuth>,
    },
}
impl HistoryProtocol2Operation {
    pub(crate) fn check_bound(&self) -> Result<()> {
        fn auth(xs: &[HistoryAuth], cap: usize) -> Result<()> {
            if xs.len() > cap {
                return Err(Error::Bound("protocol2 auth count"));
            }
            for x in xs {
                x.check_bound()?;
            }
            Ok(())
        }
        fn row(r: &HistoryReadable2) -> Result<()> {
            if r.source.len() > 16 * 1024
                || r.text.len() > 4096
                || r.receiver_receipt.as_ref().is_some_and(|b| b.len() > 1024)
            {
                return Err(Error::Bound("protocol2 row"));
            }
            auth(&r.auth, 2)
        }
        match self {
            Self::Init { init, binding } => {
                validate_scope(&init.scope)?;
                if init.own_role > 1
                    || binding.proof.len() > 16 * 1024
                    || binding.author_slots.iter().any(|s| *s > 2)
                {
                    return Err(Error::Bound("protocol2 init"));
                }
            }
            Self::Genesis { row: r, native } => {
                row(r)?;
                native.check_bound()?;
            }
            Self::TextBatch { rows, native } => {
                if rows.is_empty() || rows.len() > 4 {
                    return Err(Error::Bound("protocol2 batch"));
                }
                for r in rows {
                    row(r)?;
                }
                native.check_bound()?;
            }
            Self::AcceptPending { source, .. } if source.len() > 16 * 1024 => {
                return Err(Error::Bound("protocol2 pending"));
            }
            Self::AcceptPending { auth: xs, .. } => auth(xs, 2)?,
            Self::Acknowledge {
                receipt_raw,
                auth: xs,
                ..
            } => {
                if receipt_raw.len() > 1024 {
                    return Err(Error::Bound("protocol2 receipt"));
                }
                auth(xs, 1)?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct HistorySeenRecord {
    pub generation: u64,
    pub first_event: Id,
}
pub struct HistoryNativeRecord {
    pub hash: Id,
    pub blob: Vec<u8>,
}
pub struct HistoryCheckpointRead {
    pub view: Id,
    pub native: Option<HistoryNativeRecord>,
}
#[derive(Serialize, Deserialize)]
struct StoredBinding {
    version: u8,
    c: Id,
    d: Id,
    e: Id,
    sid: String,
    role: u8,
    proof: Vec<u8>,
    slots: [u8; 2],
}
struct RowsInput2 {
    rows: Vec<HistoryReadable2>,
    native: HistoryNativeTransition,
    genesis: bool,
}
impl StoredBinding {
    fn new(init: HistoryInit, binding: HistoryProtocol2Binding) -> Self {
        let s = init.scope;
        Self {
            version: 2,
            c: s.context,
            d: s.dialog,
            e: s.epoch,
            sid: s.session_id,
            role: init.own_role,
            proof: binding.proof,
            slots: binding.author_slots,
        }
    }
    fn init(&self) -> HistoryInit {
        HistoryInit {
            scope: HistoryScope {
                context: self.c,
                dialog: self.d,
                epoch: self.e,
                session_id: self.sid.clone(),
            },
            own_role: self.role,
        }
    }
    fn fields(&self) -> Result<FreshFields> {
        let f = read_fresh_binding2(&self.proof)?;
        if self.version != 2
            || self.role > 1
            || self.slots.iter().any(|s| *s > 2)
            || self.sid.len() != 43
            || !self.sid.is_ascii()
            || (f.c, f.d, f.e) != (self.c, self.d, self.e)
        {
            return Err(Error::Integrity("protocol2 stored binding"));
        }
        Ok(f)
    }
    fn encode(&self) -> Result<Vec<u8>> {
        self.fields()?;
        postcard::to_allocvec(self).map_err(|_| Error::Bound("protocol2 binding codec"))
    }
}
fn binding_key(c: u8) -> Result<Key> {
    Key::hash(
        c,
        Namespace::Control,
        hash_parts(b"kg:history2:protocol2-binding\0", &[]),
    )
}
fn final_key(c: u8, view: Id, event: Id) -> Result<Key> {
    Key::hash(
        c,
        Namespace::Control,
        hash_parts(b"kg:history2:readable-authority\0", &[&view, &event]),
    )
}
fn wire_receipt_key(c: u8, view: Id, role: u8, n: u64) -> Result<Key> {
    Key::hash(
        c,
        Namespace::Control,
        hash_parts(
            b"kg:history2:receipt-wire\0",
            &[&view, &[role], &n.to_be_bytes()],
        ),
    )
}
fn seen_key(c: u8, role: u8, key: Id) -> Result<Key> {
    Key::hash(
        c,
        match role {
            0 => Namespace::SeenA,
            1 => Namespace::SeenB,
            _ => return Err(Error::Bound("seen role")),
        },
        key,
    )
}
fn f_value(id: Id) -> Vec<u8> {
    let mut v = vec![1];
    v.extend(id);
    v
}
fn f_id(raw: &[u8]) -> Result<Id> {
    if raw.len() != 33 || raw[0] != 1 {
        return Err(Error::Integrity("F link"));
    }
    raw[1..].try_into().map_err(|_| Error::Integrity("F ref"))
}
fn receipt_value(raw: &[u8]) -> Vec<u8> {
    let mut v = vec![2];
    v.extend(raw);
    v
}
fn receipt_raw(value: &[u8]) -> Result<&[u8]> {
    if value.first() != Some(&2) || value.len() > 1025 {
        return Err(Error::Integrity("receipt link"));
    }
    Ok(&value[1..])
}
fn inputs(xs: &[HistoryAuth]) -> Result<BTreeMap<Id, Vec<u8>>> {
    let mut map = BTreeMap::new();
    let mut prev = None;
    for x in xs {
        let f = read_auth_bundle(&x.bytes)?;
        if x.id != f.id || prev.is_some_and(|p| p >= x.id) {
            return Err(Error::Bound("auth hash/order/duplicate"));
        }
        prev = Some(x.id);
        map.insert(x.id, x.bytes.clone());
    }
    Ok(map)
}
fn used(map: &BTreeMap<Id, Vec<u8>>, required: &[Id]) -> Result<()> {
    if map.keys().any(|k| !required.contains(k)) {
        return Err(Error::Bound("UnusedAuth"));
    }
    Ok(())
}

impl IndexedDialogVault {
    pub fn initialize_history_protocol2(
        &mut self,
        c: u8,
        expected: u64,
        id: Id,
        init: HistoryInit,
        binding: HistoryProtocol2Binding,
    ) -> Result<CommitReport> {
        self.commit_history_protocol2(
            c,
            expected,
            id,
            HistoryProtocol2Operation::Init { init, binding },
        )
    }
    pub fn commit_history_protocol2(
        &mut self,
        c: u8,
        expected: u64,
        id: Id,
        op: HistoryProtocol2Operation,
    ) -> Result<CommitReport> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        if c > 1 {
            return Err(Error::Bound("context"));
        }
        op.check_bound()?;
        let view = if let HistoryProtocol2Operation::Init { init, .. } = &op {
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
                .history_view(c)?
                .1
                .view
        };
        let raw = Zeroizing::new(
            postcard::to_allocvec(&(c, view, expected, &op))
                .map_err(|_| Error::Bound("protocol2 request"))?,
        );
        if raw.len() > MAX_PLAN {
            return Err(Error::Bound("protocol2 input plan"));
        }
        let digest = self.crypto.hash(0, b"protocol2-history-request/v1", &raw);
        let bound = raw.len() * 3 + 16384;
        self.commit_with(
            expected,
            id,
            digest,
            CommitInput::Protocol2 {
                context: c,
                operation: Box::new(op),
            },
            bound,
        )
    }
    pub fn history_checkpoint(&mut self, c: u8, expected: u64) -> Result<HistoryCheckpointRead> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        if c > 1 {
            return Err(Error::Bound("context"));
        }
        if expected != self.root.generation {
            return Err(Error::StaleGeneration);
        }
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        w.tree.enable_read_reuse();
        w.proto_required(c)?;
        let (identity, heads) = w.history_view(c)?;
        w.history_coherent(c, &identity, &heads)?;
        let blob = w.hget(Key::hash(c, Namespace::Native, [0; 32])?)?;
        if blob.is_none() && heads.tails.iter().any(Option::is_some) {
            return Err(Error::Integrity("missing Native"));
        }
        Ok(HistoryCheckpointRead {
            view: heads.view,
            native: blob.map(|blob| HistoryNativeRecord {
                hash: *blake3::hash(&blob).as_bytes(),
                blob,
            }),
        })
    }
    pub fn history_seen(
        &mut self,
        c: u8,
        expected: u64,
        role: u8,
        key: Id,
    ) -> Result<Option<HistorySeenRecord>> {
        if self.poisoned {
            return Err(Error::ReopenRequired);
        }
        if c > 1 {
            return Err(Error::Bound("context"));
        }
        if expected != self.root.generation {
            return Err(Error::StaleGeneration);
        }
        let tx = self.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.crypto, self.root.clone());
        w.tree.enable_read_reuse();
        w.proto_required(c)?;
        let (_, heads) = w.history_view(c)?;
        let Some(v) = w.hget(seen_key(c, role, key)?)? else {
            return Ok(None);
        };
        let record = seen_record(&v)?;
        let first = w.proto_source(c, record.first_event, &heads)?;
        if first.id != record.first_event
            || first.role != role
            || first.key != key
            || first.g != record.generation
            || first.index != 0
        {
            return Err(Error::Integrity("seen first source"));
        }
        Ok(Some(record))
    }
}
fn seen_record(raw: &[u8]) -> Result<HistorySeenRecord> {
    if raw.len() != 40 {
        return Err(Error::Integrity("seen width"));
    }
    Ok(HistorySeenRecord {
        generation: u64::from_be_bytes(
            raw[..8]
                .try_into()
                .map_err(|_| Error::Integrity("seen generation"))?,
        ),
        first_event: raw[8..]
            .try_into()
            .map_err(|_| Error::Integrity("seen first"))?,
    })
}

impl<
    'a,
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
> Work<'a, N, V>
{
    fn proto_binding(&mut self, c: u8) -> Result<Option<(StoredBinding, FreshFields)>> {
        let Some(raw) = self.hget(binding_key(c)?)? else {
            return Ok(None);
        };
        if raw.len() > 18 * 1024 {
            return Err(Error::Integrity("binding cap"));
        }
        let (binding, tail): (StoredBinding, _) =
            postcard::take_from_bytes(&raw).map_err(|_| Error::Integrity("binding codec"))?;
        if !tail.is_empty() || binding.c != self.root.contexts[usize::from(c)] {
            return Err(Error::Integrity("binding context/trailing"));
        }
        let fields = binding.fields()?;
        Ok(Some((binding, fields)))
    }
    fn proto_required(&mut self, c: u8) -> Result<(StoredBinding, FreshFields)> {
        self.proto_binding(c)?
            .ok_or(Error::Integrity("protocol2 binding absent"))
    }
    fn proto_source(&mut self, c: u8, id: Id, heads: &Heads) -> Result<EventFields> {
        let e = read_event2(&self.hrequired(Key::hash(c, Namespace::Event, id)?)?)?;
        let candidate = self.history_candidate(c, e.role, e.n, &heads.view, heads.frontiers)?;
        if candidate.row.id != id {
            return Err(Error::Integrity("protocol2 source Lane correspondence"));
        }
        Ok(e)
    }
    fn proto_first(
        &mut self,
        c: u8,
        heads: &Heads,
        seen: &HistorySeenRecord,
        lane: &EventFields,
        staged: &BTreeMap<Id, EventFields>,
    ) -> Result<Id> {
        let first = if let Some(first) = staged.get(&seen.first_event) {
            first.clone()
        } else {
            self.proto_source(c, seen.first_event, heads)?
        };
        if seen.generation != lane.g
            || first.id != seen.first_event
            || first.g != lane.g
            || first.key != lane.key
            || first.role != lane.role
            || first.scope != lane.scope
            || first.q != lane.q
            || first.index != 0
        {
            return Err(Error::Integrity("seen continuity"));
        }
        Ok(seen.first_event)
    }
    // Used only after the authenticated Identity was decoded at this Work root.
    // No cache survives staged mutations or another snapshot.
    pub(crate) fn history2_bound_view(&self, c: u8) -> Result<bool> {
        Ok(self.root.roots[identity_key(c)?.slot()?].count > 1)
    }
    pub(crate) fn history2_control_count(&mut self, c: u8, init: &HistoryInit) -> Result<u64> {
        if !self.history2_bound_view(c)? {
            return Ok(1);
        }
        let (binding, _) = self.proto_required(c)?;
        if binding.init() != *init {
            return Err(Error::Integrity("protocol2 identity/binding"));
        }
        // Identity + static proof, one F link per source, one wire link per receipt marker.
        let mut count = 2u64;
        for role in 0..=1 {
            count = count
                .checked_add(self.root.roots[lane(c, role, 0)?.slot()?].count)
                .ok_or(Error::Integrity("control source count"))?;
            count = count
                .checked_add(self.root.roots[receipt(c, role, 0)?.slot()?].count)
                .ok_or(Error::Integrity("control receipt count"))?;
        }
        Ok(count)
    }
    pub(crate) fn history2_guard_old(&mut self, c: u8, op: &HistoryOperation) -> Result<()> {
        if self.proto_binding(c)?.is_some()
            && !matches!(
                op,
                HistoryOperation::QueueAccept { .. } | HistoryOperation::RepairCredit(_)
            )
        {
            return Err(Error::Bound("protocol2 managed writer required"));
        }
        Ok(())
    }
    pub(crate) fn history2_check_retry(
        &mut self,
        c: u8,
        op: &HistoryProtocol2Operation,
    ) -> Result<()> {
        if matches!(op, HistoryProtocol2Operation::Init { .. }) {
            if self.hget(identity_key(c)?)?.is_some() {
                self.proto_required(c)?;
                let (i, h) = self.history_view(c)?;
                self.history_coherent(c, &i, &h)?;
            }
            Ok(())
        } else {
            let (binding, proof) = self.proto_required(c)?;
            let (identity, heads) = self.history_view(c)?;
            self.history_coherent(c, &identity, &heads)?;
            match op {
                HistoryProtocol2Operation::Genesis { row, native } => {
                    self.history2_retry_rows(c, &heads, std::slice::from_ref(row), native)
                }
                HistoryProtocol2Operation::TextBatch { rows, native } => {
                    self.history2_retry_rows(c, &heads, rows, native)
                }
                HistoryProtocol2Operation::Acknowledge {
                    receipt_format,
                    receipt_raw: raw,
                    auth,
                } => {
                    if *receipt_format != 2 {
                        return Err(Error::Conflict);
                    }
                    let receipt = read_receipt2(raw)?;
                    let source = self.proto_source(c, receipt.event, &heads)?;
                    let map = inputs(auth)?;
                    used(&map, &[receipt.auth])?;
                    self.check_receipt(c, raw, &source, &binding, &proof, &map)?;
                    let stored =
                        self.hrequired(wire_receipt_key(c, heads.view, receipt.role, receipt.n)?)?;
                    if receipt_raw(&stored)? != raw {
                        return Err(Error::Integrity("protocol2 retry receipt"));
                    }
                    Ok(())
                }
                HistoryProtocol2Operation::AcceptPending {
                    source,
                    head,
                    admission_author_ref,
                    auth,
                    ..
                } => {
                    let e = read_event2(source)?;
                    self.check_scope(&e, &binding, &proof)?;
                    let map = inputs(auth)?;
                    used(&map, &[e.author, *admission_author_ref])?;
                    for id in [e.author, *admission_author_ref] {
                        self.resolve_auth(
                            c,
                            id,
                            proof.certs[usize::from(e.role)],
                            &map,
                            &BTreeMap::new(),
                        )?;
                    }
                    let raw = self.hrequired(Key::ordinal(c, Namespace::Pending, e.n)?)?;
                    let pending = credits::pending_view(&raw)?;
                    if pending.event != e.id
                        || pending.body != *source
                        || pending.head != *head
                        || credits::pending_captures(&raw)?
                            != Some([e.author, *admission_author_ref])
                    {
                        return Err(Error::Integrity("protocol2 retry pending"));
                    }
                    Ok(())
                }
                HistoryProtocol2Operation::Init { .. } => Err(Error::Bound("nested Init2 retry")),
            }
        }
    }
    fn history2_retry_rows(
        &mut self,
        c: u8,
        heads: &Heads,
        rows: &[HistoryReadable2],
        native: &HistoryNativeTransition,
    ) -> Result<()> {
        for row in rows {
            let e = read_event2(&row.source)?;
            self.proto_source(c, e.id, heads)?;
            let raw = self.hrequired(Key::hash(c, Namespace::Event, e.id)?)?;
            let projection = ProjectionValue::decode(&self.hrequired(Key::hash(
                c,
                Namespace::Projection,
                e.id,
            )?)?)?;
            let final_ref = f_id(&self.hrequired(final_key(c, heads.view, e.id)?)?)?;
            if raw != row.source
                || projection.text != row.text
                || final_ref != row.readable_author_ref
            {
                return Err(Error::Integrity("protocol2 retry source/projection/F"));
            }
            if let Some(raw) = &row.receiver_receipt {
                let stored = self.hrequired(wire_receipt_key(c, heads.view, e.role, e.n)?)?;
                if receipt_raw(&stored)? != raw {
                    return Err(Error::Integrity("protocol2 retry row receipt"));
                }
            }
        }
        if self.hrequired(Key::hash(c, Namespace::Native, [0; 32])?)? != native.after_blob {
            return Err(Error::Integrity("protocol2 retry Native"));
        }
        Ok(())
    }
    fn resolve_auth(
        &mut self,
        c: u8,
        id: Id,
        cert: Id,
        map: &BTreeMap<Id, Vec<u8>>,
        staged: &BTreeMap<Id, Vec<u8>>,
    ) -> Result<HistoryAuth> {
        let stored = self.hget(Key::hash(c, Namespace::Auth, id)?)?;
        let provided = map.get(&id).or_else(|| staged.get(&id));
        if stored.as_ref().zip(provided).is_some_and(|(a, b)| a != b) {
            return Err(Error::Conflict);
        }
        let bytes = provided
            .cloned()
            .or(stored)
            .ok_or(Error::Integrity("MissingReferencedAuth"))?;
        let f = read_auth_bundle(&bytes)?;
        if f.id != id || f.cert != cert {
            return Err(Error::Integrity("auth hash/certificate/role"));
        }
        Ok(HistoryAuth { id, bytes })
    }
    fn check_scope(&self, e: &EventFields, b: &StoredBinding, p: &FreshFields) -> Result<()> {
        if e.scope != b.init().scope || e.q != p.q {
            return Err(Error::Conflict);
        }
        if e.n == 0 {
            let id = if e.role == 0 { p.offer } else { p.grant };
            if e.request.as_slice() != &id[..16] {
                return Err(Error::Integrity("genesis request/R4"));
            }
        }
        Ok(())
    }
    fn check_receipt(
        &mut self,
        c: u8,
        raw: &[u8],
        e: &EventFields,
        b: &StoredBinding,
        p: &FreshFields,
        map: &BTreeMap<Id, Vec<u8>>,
    ) -> Result<HistoryAuth> {
        let r = read_receipt2(raw)?;
        if r.scope != e.scope
            || r.q != e.q
            || r.format != 2
            || r.role != e.role
            || r.n != e.n
            || r.event != e.id
            || r.cert != p.certs[usize::from(1 - e.role)]
            || r.scope != b.init().scope
        {
            return Err(Error::Conflict);
        }
        self.resolve_auth(c, r.auth, r.cert, map, &BTreeMap::new())
    }
    pub(crate) fn history2_source_check(
        &mut self,
        c: u8,
        id: Id,
        binding: &Binding,
        raw: &[u8],
    ) -> Result<()> {
        if !self.history2_bound_view(c)? {
            return Ok(());
        }
        let (b, p) = self.proto_required(c)?;
        let e = read_event2(raw)?;
        self.check_scope(&e, &b, &p)?;
        let mut expected = self.normalized(c, &e, binding.view, binding.parents.clone())?;
        expected.core_hash = hash_parts(
            b"kg:history2:core\0",
            &[&[2], core_span(raw, e.core_offset, e.core_len)?],
        );
        expected.source_hash = hash_parts(b"kg:history2:source\0", &[&[2], raw]);
        if e.id != id || expected.encode()? != binding.encode()? {
            return Err(Error::Integrity(
                "protocol2 source/projection correspondence",
            ));
        }
        let empty = BTreeMap::new();
        self.resolve_auth(c, e.author, p.certs[usize::from(e.role)], &empty, &empty)?;
        let f = f_id(&self.hrequired(final_key(c, binding.view, id)?)?)?;
        self.resolve_auth(c, f, p.certs[usize::from(e.role)], &empty, &empty)?;
        let marker = self.hget(receipt(c, e.role, e.n)?)?;
        let wire = self.hget(wire_receipt_key(c, binding.view, e.role, e.n)?)?;
        if e.kind == 3 && e.role != b.role {
            let captured = self.hrequired(Key::hash(c, Namespace::Admission, id)?)?;
            if captured.len() != 65 || captured[0] != 5 || captured[1..33] != e.author {
                return Err(Error::Integrity("admission evidence"));
            }
            let admission: Id = captured[33..]
                .try_into()
                .map_err(|_| Error::Integrity("admission ref"))?;
            self.resolve_auth(c, admission, p.certs[usize::from(e.role)], &empty, &empty)?;
            if marker.is_none() || wire.is_none() {
                return Err(Error::Integrity("peer receipt evidence"));
            }
        }
        if marker.is_some() != wire.is_some() {
            return Err(Error::Integrity("receipt marker/wire"));
        }
        if let Some(w) = wire {
            if e.kind != 3 || marker.as_deref() != Some(&id[..]) {
                return Err(Error::Integrity("receipt slot"));
            }
            self.check_receipt(c, receipt_raw(&w)?, &e, &b, &p, &empty)?;
        }
        Ok(())
    }
    fn normalized(
        &self,
        _c: u8,
        e: &EventFields,
        view: Id,
        parents: Vec<HistoryParent>,
    ) -> Result<Binding> {
        if parents.iter().map(|p| p.event).collect::<Vec<_>>() != e.parents {
            return Err(Error::Integrity("source parents"));
        }
        Ok(Binding {
            format: 2,
            view,
            kind: e.kind,
            role: e.role,
            ordinal: e.n,
            sequence: e.sequence,
            request: if e.n == 0 { [0; 16] } else { e.request },
            previous: e.previous,
            core_offset: e.core_offset,
            core_len: e.core_len,
            core_hash: [0; 32],
            source_hash: [0; 32],
            parents,
        })
    }
    fn proto_rows(
        &mut self,
        c: u8,
        identity: &Identity,
        heads: Heads,
        b: &StoredBinding,
        fresh: &FreshFields,
        input: RowsInput2,
    ) -> Result<Plan> {
        let RowsInput2 {
            rows,
            native,
            genesis,
        } = input;
        let mut prepared = vec![];
        let mut links = vec![];
        let mut staged_auth = BTreeMap::new();
        let mut staged_events = BTreeMap::<Id, EventFields>::new();
        let mut staged_seen = BTreeMap::<(u8, Id), HistorySeenRecord>::new();
        let mut last: [Option<EventFields>; 2] = [None, None];
        for (role, entry) in last.iter_mut().enumerate() {
            if let Some(id) = heads.tails[role] {
                let source = self.proto_source(c, id, &heads)?;
                if source.role != role as u8
                    || source.n.checked_add(1) != Some(heads.frontiers[role])
                {
                    return Err(Error::Integrity("protocol2 tail/head correspondence"));
                }
                *entry = Some(source);
            }
        }
        for row in rows {
            let e = read_event2(&row.source)?;
            self.check_scope(&e, b, fresh)?;
            if genesis != (e.n == 0) {
                return Err(Error::Conflict);
            }
            let role = usize::from(e.role);
            let map = inputs(&row.auth)?;
            let mut authority = vec![];
            if e.kind == 3 && e.role != b.role {
                let pending = self.hrequired(Key::ordinal(c, Namespace::Pending, e.n)?)?;
                if credits::pending_view(&pending)?.body != row.source {
                    return Err(Error::Conflict);
                }
                let captures = credits::pending_captures(&pending)?
                    .ok_or(Error::Integrity("mandatory captures"))?;
                if captures[0] != e.author {
                    return Err(Error::Conflict);
                }
                for auth in captures {
                    self.resolve_auth(
                        c,
                        auth,
                        fresh.certs[role],
                        &BTreeMap::new(),
                        &BTreeMap::new(),
                    )?;
                }
            } else {
                authority.push(self.resolve_auth(
                    c,
                    e.author,
                    fresh.certs[role],
                    &map,
                    &staged_auth,
                )?);
            }
            authority.push(self.resolve_auth(
                c,
                row.readable_author_ref,
                fresh.certs[role],
                &map,
                &staged_auth,
            )?);
            let mut needed = vec![row.readable_author_ref];
            if e.kind == 3 && e.role != b.role {
                let raw = row
                    .receiver_receipt
                    .as_deref()
                    .ok_or(Error::Integrity("receipt required"))?;
                let r = self.check_receipt(c, raw, &e, b, fresh, &map)?;
                needed.push(r.id);
                authority.push(r);
            } else {
                needed.push(e.author);
                if row.receiver_receipt.is_some() {
                    return Err(Error::Conflict);
                }
            }
            used(&map, &needed)?;
            for a in &authority {
                staged_auth.insert(a.id, a.bytes.clone());
            }
            authority.sort_by_key(|a| a.id);
            authority.dedup_by_key(|a| a.id);
            let prior = last[role].as_ref();
            if prior.is_some_and(|old| {
                e.previous != Some(old.id)
                    || e.n != old.n.checked_add(1).unwrap_or(0)
                    || e.sequence <= old.sequence
            }) || prior.is_none() && e.n != 0
            {
                return Err(Error::Conflict);
            }
            let new = prior.is_none_or(|old| old.key != e.key);
            if new {
                let g = match prior {
                    None => 0,
                    Some(old) => old
                        .g
                        .checked_add(1)
                        .ok_or(Error::Bound("generation overflow"))?,
                };
                if e.g != g || e.index != 0 {
                    return Err(Error::Conflict);
                }
                let expected_tail = prior.map(|old| source::Tail {
                    g: old.g,
                    key: old.key,
                    index: old.index,
                    last: old.id,
                });
                if e.tail != expected_tail {
                    return Err(Error::Conflict);
                }
                let parent = if e.n == 0 && e.role == 0 {
                    None
                } else {
                    let other = last[1 - role].as_ref().ok_or(Error::Conflict)?;
                    let seen = if let Some(s) = staged_seen.get(&(1 - e.role, other.key)) {
                        s.clone()
                    } else {
                        seen_record(&self.hrequired(seen_key(c, 1 - e.role, other.key)?)?)?
                    };
                    Some(self.proto_first(c, &heads, &seen, other, &staged_events)?)
                };
                if e.parent != parent
                    || staged_seen.contains_key(&(e.role, e.key))
                    || self.hget(seen_key(c, e.role, e.key)?)?.is_some()
                {
                    return Err(Error::Conflict);
                }
                staged_seen.insert(
                    (e.role, e.key),
                    HistorySeenRecord {
                        generation: e.g,
                        first_event: e.id,
                    },
                );
            } else {
                let old = prior.ok_or(Error::Conflict)?;
                if e.g != old.g
                    || Some(e.index) != old.index.checked_add(1)
                    || e.parent != old.parent
                    || e.tail.is_some()
                {
                    return Err(Error::Conflict);
                }
                let seen = if let Some(s) = staged_seen.get(&(e.role, e.key)) {
                    s.clone()
                } else {
                    seen_record(&self.hrequired(seen_key(c, e.role, e.key)?)?)?
                };
                self.proto_first(c, &heads, &seen, &e, &staged_events)?;
            }
            e.index
                .checked_add(1)
                .ok_or(Error::Bound("native post index"))?;
            e.sequence
                .checked_add(1)
                .ok_or(Error::Bound("author overflow"))?;
            let mut parents = vec![];
            for parent in &e.parents {
                let (pr, pn) = if let Some(p) = staged_events.get(parent) {
                    (p.role, p.n)
                } else {
                    let (p, _) = self
                        .history_projection(c, *parent, &heads.view)?
                        .ok_or(Error::Conflict)?;
                    if p.kind != 3 {
                        return Err(Error::Conflict);
                    }
                    (p.role, p.ordinal)
                };
                parents.push(HistoryParent {
                    event: *parent,
                    role: pr,
                    ordinal: pn,
                });
            }
            let extras = HistoryExtras {
                auth: authority,
                seen: if new {
                    Some(HistorySeen {
                        key: e.key,
                        generation: e.g,
                    })
                } else {
                    None
                },
                author_slot: Some(b.slots[role]),
            };
            prepared.push(PreparedReadable {
                scope: e.scope.clone(),
                format: 2,
                source: row.source,
                core_offset: e.core_offset,
                core_len: e.core_len,
                kind: e.kind,
                role: e.role,
                ordinal: e.n,
                sequence: e.sequence,
                request: if e.n == 0 { [0; 16] } else { e.request },
                previous: e.previous,
                parents,
                text: row.text,
                extras,
            });
            links.push((
                e.id,
                e.role,
                e.n,
                row.readable_author_ref,
                row.receiver_receipt,
            ));
            last[role] = Some(e.clone());
            staged_events.insert(e.id, e);
        }
        let mut p = self.history_rows(c, identity, heads, prepared, native, genesis)?;
        p.protocol2 = true;
        for (id, role, n, final_ref, receipt_raw) in links {
            let mut added = vec![];
            let fk = final_key(c, identity.view_id()?, id)?;
            p.put(fk, f_value(final_ref))?;
            added.push(fk);
            if let Some(raw) = receipt_raw {
                let rk = wire_receipt_key(c, identity.view_id()?, role, n)?;
                p.put(rk, receipt_value(&raw))?;
                added.push(rk);
            }
            for r in &mut p.rows {
                if r.event == id {
                    r.refs.extend(&added);
                    r.refs.sort();
                    r.refs.dedup();
                }
            }
            for a in &mut p.credits {
                match a {
                    CreditAction::Materialize { event, records, .. }
                    | CreditAction::Readable { event, records, .. }
                        if *event == id =>
                    {
                        records.extend(&added);
                        records.sort();
                        records.dedup();
                    }
                    _ => {}
                }
            }
        }
        Ok(p)
    }
    pub(super) fn history2_extra_check(&mut self, p: &Plan) -> Result<()> {
        if !p.protocol2 {
            return Ok(());
        }
        let c = p.context;
        if p.purpose == Purpose::Init {
            if !p.puts.contains_key(&binding_key(c)?) {
                return Err(Error::Bound("missing protocol2 binding"));
            }
            return Ok(());
        }
        let (b, _) = self.proto_required(c)?;
        let view = Identity {
            incarnation: self.root.incarnation,
            init: b.init(),
        }
        .view_id()?;
        if p.purpose == Purpose::Pending && p.credits.len() != 2 {
            return Err(Error::Bound("mandatory pending capture"));
        }
        for row in &p.rows {
            let fk = final_key(c, view, row.event)?;
            if !p.puts.contains_key(&fk) || !row.refs.contains(&fk) {
                return Err(Error::Bound("missing F row link"));
            }
            if row.role != b.role && row.ordinal > 0 {
                let rk = wire_receipt_key(c, view, row.role, row.ordinal)?;
                if !p.puts.contains_key(&rk) || !row.refs.contains(&rk) {
                    return Err(Error::Bound("missing receipt row link"));
                }
            }
        }
        if p.purpose == Purpose::Ack {
            let Some(CreditAction::Acknowledge {
                ordinal, records, ..
            }) = p.credits.first()
            else {
                return Err(Error::Conflict);
            };
            let key = wire_receipt_key(c, view, b.role, *ordinal)?;
            if !p.puts.contains_key(&key) || !records.contains(&key) {
                return Err(Error::Bound("missing ACK wire evidence"));
            }
        }
        Ok(())
    }
    pub(super) fn history2_allowed_control(
        &mut self,
        p: &Plan,
        key: Key,
        value: &[u8],
    ) -> Result<bool> {
        if !p.protocol2 {
            return Ok(false);
        }
        let c = p.context;
        if p.purpose == Purpose::Init {
            if key != binding_key(c)? {
                return Ok(false);
            }
            let (b, tail): (StoredBinding, _) =
                postcard::take_from_bytes(value).map_err(|_| Error::Integrity("binding codec"))?;
            b.fields()?;
            if !tail.is_empty() || b.c != self.root.contexts[usize::from(c)] {
                return Ok(false);
            }
            let identity = Identity {
                incarnation: self.root.incarnation,
                init: b.init(),
            };
            return Ok(p
                .puts
                .get(&identity_key(c)?)
                .is_some_and(|x| identity.encode().is_ok_and(|bytes| bytes == *x)));
        }
        let (b, fresh) = self.proto_required(c)?;
        let view = Identity {
            incarnation: self.root.incarnation,
            init: b.init(),
        }
        .view_id()?;
        if matches!(p.purpose, Purpose::Genesis | Purpose::Text) {
            for row in &p.rows {
                if key == final_key(c, view, row.event)? {
                    let id = f_id(value)?;
                    let auth_key = Key::hash(c, Namespace::Auth, id)?;
                    let raw = p
                        .puts
                        .get(&auth_key)
                        .cloned()
                        .or(self.hget(auth_key)?)
                        .ok_or(Error::Integrity("F missing"))?;
                    let auth = read_auth_bundle(&raw)?;
                    return Ok(auth.id == id && auth.cert == fresh.certs[usize::from(row.role)]);
                }
                if key == wire_receipt_key(c, view, row.role, row.ordinal)?
                    && row.role != b.role
                    && row.ordinal > 0
                {
                    let raw = p
                        .puts
                        .get(&Key::hash(c, Namespace::Event, row.event)?)
                        .ok_or(Error::Conflict)?;
                    let e = read_event2(raw)?;
                    let r = read_receipt2(receipt_raw(value)?)?;
                    let auth_key = Key::hash(c, Namespace::Auth, r.auth)?;
                    let auth = p
                        .puts
                        .get(&auth_key)
                        .cloned()
                        .or(self.hget(auth_key)?)
                        .ok_or(Error::Integrity("R missing"))?;
                    let af = read_auth_bundle(&auth)?;
                    return Ok(r.scope == e.scope
                        && r.q == e.q
                        && r.format == 2
                        && r.role == e.role
                        && r.n == e.n
                        && r.event == e.id
                        && r.cert == fresh.certs[usize::from(1 - e.role)]
                        && af.id == r.auth
                        && af.cert == r.cert);
                }
            }
        }
        if p.purpose == Purpose::Ack {
            let raw = receipt_raw(value)?;
            let r = read_receipt2(raw)?;
            if key != wire_receipt_key(c, view, b.role, r.n)? || r.role != b.role {
                return Ok(false);
            }
            let e = read_event2(&self.hrequired(Key::hash(c, Namespace::Event, r.event)?)?)?;
            let mut map = BTreeMap::new();
            if let Some(bytes) = p.puts.get(&Key::hash(c, Namespace::Auth, r.auth)?) {
                map.insert(r.auth, bytes.clone());
            }
            self.check_receipt(c, raw, &e, &b, &fresh, &map)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub(crate) fn history2_apply(&mut self, c: u8, op: HistoryProtocol2Operation) -> Result<()> {
        if let HistoryProtocol2Operation::Init { init, binding } = op {
            let stored = StoredBinding::new(init, binding);
            let raw = stored.encode()?;
            if let Some((old, _)) = self.proto_binding(c)? {
                if old.encode()? != raw {
                    return Err(Error::Conflict);
                }
                let (i, h) = self.history_view(c)?;
                return self.history_coherent(c, &i, &h);
            }
            self.history_fresh(c)?;
            if stored.c != self.root.contexts[usize::from(c)] {
                return Err(Error::Conflict);
            }
            let identity = Identity {
                incarnation: self.root.incarnation,
                init: stored.init(),
            };
            let mut p = Plan::new(c, Purpose::Init);
            p.protocol2 = true;
            p.put(identity_key(c)?, identity.encode()?)?;
            p.put(
                heads_key(c)?,
                Heads {
                    view: identity.view_id()?,
                    frontiers: [1, 1],
                    tails: [None, None],
                }
                .encode(),
            )?;
            p.put(binding_key(c)?, raw)?;
            return self.history_execute(p);
        }
        let (b, fresh) = self.proto_required(c)?;
        let (identity, heads) = self.history_view(c)?;
        match op {
            HistoryProtocol2Operation::Genesis { row, native } => {
                let plan = self.proto_rows(
                    c,
                    &identity,
                    heads,
                    &b,
                    &fresh,
                    RowsInput2 {
                        rows: vec![row],
                        native,
                        genesis: true,
                    },
                )?;
                self.history_execute(plan)
            }
            HistoryProtocol2Operation::TextBatch { rows, native } => {
                let plan = self.proto_rows(
                    c,
                    &identity,
                    heads,
                    &b,
                    &fresh,
                    RowsInput2 {
                        rows,
                        native,
                        genesis: false,
                    },
                )?;
                self.history_execute(plan)
            }
            HistoryProtocol2Operation::AcceptPending {
                source,
                head,
                grant,
                admission_author_ref,
                auth,
            } => {
                let e = read_event2(&source)?;
                self.check_scope(&e, &b, &fresh)?;
                if e.kind != 3 || e.role == b.role {
                    return Err(Error::Conflict);
                }
                let map = inputs(&auth)?;
                used(&map, &[e.author, admission_author_ref])?;
                let empty = BTreeMap::new();
                let original =
                    self.resolve_auth(c, e.author, fresh.certs[usize::from(e.role)], &map, &empty)?;
                let admission = self.resolve_auth(
                    c,
                    admission_author_ref,
                    fresh.certs[usize::from(e.role)],
                    &map,
                    &empty,
                )?;
                if let Some(old) = self.hget(Key::ordinal(c, Namespace::Pending, e.n)?)? {
                    let pending = credits::pending_view(&old)?;
                    if pending.event != e.id
                        || pending.body != source
                        || credits::pending_captures(&old)?
                            != Some([e.author, admission_author_ref])
                    {
                        return Err(Error::Conflict);
                    }
                    return Ok(());
                }
                if e.n < heads.frontiers[usize::from(e.role)] {
                    let old =
                        self.history_candidate(c, e.role, e.n, &heads.view, heads.frontiers)?;
                    let cap = self.hrequired(Key::hash(c, Namespace::Admission, e.id)?)?;
                    if old.row.id != e.id
                        || cap
                            != [vec![5], e.author.to_vec(), admission_author_ref.to_vec()].concat()
                    {
                        return Err(Error::Conflict);
                    }
                    return Ok(());
                }
                if head && e.n != heads.frontiers[usize::from(e.role)] {
                    return Err(Error::Conflict);
                }
                let mut p = Plan::new(c, Purpose::Pending);
                p.protocol2 = true;
                add_auth(&mut p, &original)?;
                add_auth(&mut p, &admission)?;
                p.credits.push(CreditAction::Pending {
                    context: c,
                    ordinal: e.n,
                    event: e.id,
                    body: source,
                    head,
                    grant,
                });
                p.credits.push(CreditAction::CapturePending {
                    context: c,
                    ordinal: e.n,
                    event: e.id,
                    original_auth: e.author,
                    admission_auth: admission_author_ref,
                });
                self.history_execute(p)
            }
            HistoryProtocol2Operation::Acknowledge {
                receipt_format,
                receipt_raw: raw,
                auth,
            } => {
                if receipt_format != 2 {
                    return Err(Error::Conflict);
                }
                let r = read_receipt2(&raw)?;
                let map = inputs(&auth)?;
                used(&map, &[r.auth])?;
                if r.role != b.role {
                    return Err(Error::Conflict);
                }
                let candidate =
                    self.history_candidate(c, b.role, r.n, &heads.view, heads.frontiers)?;
                let source = self.hrequired(Key::hash(c, Namespace::Event, candidate.row.id)?)?;
                let e = read_event2(&source)?;
                let receiver = self.check_receipt(c, &raw, &e, &b, &fresh, &map)?;
                let wire_key = wire_receipt_key(c, heads.view, r.role, r.n)?;
                if let Some(old) = self.hget(wire_key)? {
                    if receipt_raw(&old)? != raw {
                        return Err(Error::Conflict);
                    }
                    return Ok(());
                }
                let debt = self.hrequired(Key::ordinal(c, Namespace::Undelivered, r.n)?)?;
                if credits::history_ticket(&debt, 3)?.0 != e.id {
                    return Err(Error::Conflict);
                }
                let mut p = Plan::new(c, Purpose::Ack);
                p.protocol2 = true;
                let marker = receipt(c, r.role, r.n)?;
                p.put(marker, e.id.to_vec())?;
                p.put(wire_key, receipt_value(&raw))?;
                let auth_key = add_auth(&mut p, &receiver)?;
                p.credits.push(CreditAction::Acknowledge {
                    context: c,
                    ordinal: r.n,
                    event: e.id,
                    records: vec![marker, wire_key, auth_key],
                });
                self.history_execute(p)
            }
            HistoryProtocol2Operation::Init { .. } => Err(Error::Bound("nested Init2")),
        }
    }
}
