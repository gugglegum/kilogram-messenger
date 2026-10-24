//! Private lowering; the public API cannot construct this plan or its credit refs.
use super::*;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Purpose {
    Init,
    Genesis,
    Text,
    Queue,
    Pending,
    Capture,
    Repair,
    Ack,
}
#[derive(Clone)]
enum Origin {
    Genesis,
    Local { queue: u64, handle: Id },
    Peer,
}
#[derive(Clone)]
struct RowClaim {
    event: Id,
    role: u8,
    ordinal: u64,
    origin: Origin,
    refs: Vec<Key>,
}
#[derive(Clone)]
struct Plan {
    context: u8,
    purpose: Purpose,
    puts: BTreeMap<Key, Vec<u8>>,
    allowed: BTreeSet<Key>,
    rows: Vec<RowClaim>,
    credits: Vec<CreditAction>,
}
impl Plan {
    fn new(context: u8, purpose: Purpose) -> Self {
        Self {
            context,
            purpose,
            puts: BTreeMap::new(),
            allowed: BTreeSet::new(),
            rows: Vec::new(),
            credits: Vec::new(),
        }
    }
    fn put(&mut self, key: Key, value: Vec<u8>) -> Result<()> {
        if self.puts.get(&key).is_some_and(|old| *old != value) {
            return Err(Error::Conflict);
        }
        self.allowed.insert(key);
        self.puts.insert(key, value);
        Ok(())
    }
}
fn credit_context(action: &CreditAction) -> u8 {
    match action {
        CreditAction::Queue { context, .. }
        | CreditAction::Grant { context, .. }
        | CreditAction::CancelGrant { context, .. }
        | CreditAction::Refill { context, .. }
        | CreditAction::Pending { context, .. }
        | CreditAction::Materialize { context, .. }
        | CreditAction::Readable { context, .. }
        | CreditAction::Acknowledge { context, .. }
        | CreditAction::CapturePending { context, .. } => *context,
    }
}
fn refs(action: &CreditAction) -> &[Key] {
    match action {
        CreditAction::Materialize { records, .. }
        | CreditAction::Readable { records, .. }
        | CreditAction::Acknowledge { records, .. } => records,
        _ => &[],
    }
}
fn add_auth(plan: &mut Plan, auth: &HistoryAuth) -> Result<Key> {
    auth.check_bound()?;
    let key = Key::hash(plan.context, Namespace::Auth, auth.id)?;
    plan.put(key, auth.bytes.clone())?;
    Ok(key)
}
fn add_capture(plan: &mut Plan, ordinal: u64, event: Id, capture: HistoryCapture) -> Result<()> {
    add_auth(plan, &capture.original)?;
    add_auth(plan, &capture.admission)?;
    plan.credits.push(CreditAction::CapturePending {
        context: plan.context,
        ordinal,
        event,
        original_auth: capture.original.id,
        admission_auth: capture.admission.id,
    });
    Ok(())
}

impl<
    'a,
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
> Work<'a, N, V>
{
    #[cfg(test)]
    pub(crate) fn history_fixture_check(&mut self, c: u8) -> Result<()> {
        self.history_view(c).map(|_| ())
    }
    #[cfg(test)]
    pub(crate) fn history_fixture_apply(
        &mut self,
        c: u8,
        rows: Vec<PreparedReadable>,
    ) -> Result<()> {
        if rows.is_empty() || rows.len() > 128 {
            return Err(Error::Bound("history fixture batch"));
        }
        for row in rows {
            row.check_bound()?;
            let (identity, _) = self.history_view(c)?;
            if row.role != identity.init.own_role {
                self.history_apply(
                    c,
                    HistoryOperation::AcceptPending(HistoryPending {
                        scope: row.scope.clone(),
                        format: row.format,
                        ordinal: row.ordinal,
                        source: row.source.clone(),
                        head: false,
                        grant: None,
                        capture: None,
                    }),
                )?;
            }
            let event = row.event_id()?;
            let old = self.hrequired(Key::hash(c, Namespace::Native, [0; 32])?)?;
            let mut after_blob = vec![0x4e; 256];
            after_blob[..32].copy_from_slice(&event);
            self.history_apply(
                c,
                HistoryOperation::TextBatch {
                    rows: vec![row],
                    native: HistoryNativeTransition {
                        before_hash: Some(*blake3::hash(&old).as_bytes()),
                        after_blob,
                        frame_ids: vec![event],
                    },
                },
            )?;
            let buffers = self.buffer_bound_bytes();
            self.history_buffer_peak = self.history_buffer_peak.max(buffers);
            if buffers + 3 * MAX_PLAN > 192 * 1024 * 1024 {
                return Err(Error::Bound("history fixture live buffers"));
            }
        }
        Ok(())
    }
    pub(crate) fn history_guard_generic(
        &mut self,
        mutations: &[Mutation],
        actions: &[CreditAction],
    ) -> Result<()> {
        let mut keys = Vec::new();
        let mut touched = BTreeSet::new();
        for m in mutations {
            let key = match m {
                Mutation::Put { key, .. } | Mutation::Retire { key, .. } => *key,
            };
            if key == identity_key(key.context)? || key == heads_key(key.context)? {
                return Err(Error::Bound("reserved history key"));
            }
            keys.push(key);
        }
        for action in actions {
            let c = credit_context(action);
            if c > 1 {
                return Err(Error::Bound("credit context"));
            }
            touched.insert(c);
            keys.extend(refs(action));
            if let CreditAction::CapturePending {
                original_auth,
                admission_auth,
                ..
            } = action
            {
                keys.extend([
                    Key::hash(c, Namespace::Auth, *original_auth)?,
                    Key::hash(c, Namespace::Auth, *admission_auth)?,
                ]);
            }
        }
        for key in keys {
            let slot = key.slot()?;
            touched.insert(key.context);
            if key.namespace.global()
                && let Some(leaf) = self
                    .tree
                    .find(slot as u8, self.root.roots[slot], key.bytes)?
            {
                let tree::Body::Leaf { owner, .. } = leaf.body else {
                    return Err(Error::Integrity("Author leaf"));
                };
                if owner > 1 {
                    return Err(Error::Integrity("Author owner"));
                }
                self.hrequired(key)?;
                // Raw S1 cross-owner reads retain their semantics; fencing includes
                // the authenticated owner even when key.context names another C.
                touched.insert(owner);
            }
        }
        for c in touched {
            if self.hget(identity_key(c)?)?.is_some() || self.hget(heads_key(c)?)?.is_some() {
                return Err(Error::Bound("managed history writer required"));
            }
        }
        Ok(())
    }
    pub(crate) fn history_check_retry(
        &mut self,
        c: u8,
        operation: &HistoryOperation,
    ) -> Result<()> {
        if matches!(operation, HistoryOperation::Init(_)) {
            match self.history_view(c) {
                Ok((identity, heads)) => self.history_coherent(c, &identity, &heads),
                Err(Error::HistoryUnavailable) => Ok(()),
                Err(e) => Err(e),
            }
        } else {
            self.history_view(c).map(|_| ())
        }
    }
    fn history_coherent(&mut self, c: u8, _identity: &Identity, heads: &Heads) -> Result<()> {
        let mut total = 0u64;
        for role in 0..2 {
            let key = lane(c, role as u8, 0)?;
            let slot = key.slot()?;
            total = total
                .checked_add(self.root.roots[slot].count)
                .ok_or(Error::Integrity("history count overflow"))?;
            if let Some(tail) = heads.tails[role] {
                let genesis =
                    self.history_candidate(c, role as u8, 0, &heads.view, heads.frontiers)?;
                if genesis.binding.kind != role as u8 + 1 {
                    return Err(Error::Integrity("history genesis kind"));
                }
                let n = heads.frontiers[role] - 1;
                let last = if n == 0 {
                    genesis
                } else {
                    self.history_candidate(c, role as u8, n, &heads.view, heads.frontiers)?
                };
                if last.row.id != tail {
                    return Err(Error::Integrity("history tail source"));
                }
                let suffix = lane(c, role as u8, heads.frontiers[role])?;
                if !self
                    .tree
                    .range(slot as u8, self.root.roots[slot], suffix.bytes, 1)?
                    .is_empty()
                {
                    return Err(Error::Integrity("history Lane suffix"));
                }
            }
        }
        for ns in [Namespace::Event, Namespace::Projection] {
            if self.root.roots[Key::hash(c, ns, [0; 32])?.slot()?].count != total {
                return Err(Error::Integrity("history aggregate count"));
            }
        }
        let native_key = Key::hash(c, Namespace::Native, [0; 32])?;
        let native = self.hget(native_key)?;
        let expected = u64::from(heads.tails[0].is_some());
        if self.root.roots[native_key.slot()?].count != expected
            || match native {
                None => expected != 0,
                Some(raw) => expected != 1 || raw.is_empty() || raw.len() > 128 * 1024,
            }
        {
            return Err(Error::Integrity("history Native shape"));
        }
        Ok(())
    }
    fn history_fresh(&self, c: u8) -> Result<()> {
        let c = usize::from(c);
        let (used, reserved) = self.root.totals()?;
        if self.root.roots[c * 24..c * 24 + 24]
            .iter()
            .any(|s| *s != Summary::default())
            || self.root.roots[48..51]
                .iter()
                .any(|s| s.used[c] != 0 || s.reserved[c] != 0)
            || self.root.roots[51..]
                .iter()
                .any(|s| *s != Summary::default())
            || !self.root.ledger[c].is_pristine()
            || used[c] != 0
            || reserved[c] != 0
        {
            return Err(Error::ExistingStateRequiresMigration);
        }
        Ok(())
    }
    pub(crate) fn history_apply(&mut self, c: u8, operation: HistoryOperation) -> Result<()> {
        if let HistoryOperation::Init(init) = operation {
            if init.scope.context != self.root.contexts[usize::from(c)] {
                return Err(Error::Conflict);
            }
            let identity = Identity {
                incarnation: self.root.incarnation,
                init,
            };
            match self.history_view(c) {
                Ok((stored, heads)) => {
                    self.history_coherent(c, &stored, &heads)?;
                    return if identity == stored {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                Err(Error::HistoryUnavailable) => self.history_fresh(c)?,
                Err(e) => return Err(e),
            }
            let heads = Heads {
                view: identity.view_id()?,
                frontiers: [1, 1],
                tails: [None, None],
            };
            let mut plan = Plan::new(c, Purpose::Init);
            plan.put(identity_key(c)?, identity.encode()?)?;
            plan.put(heads_key(c)?, heads.encode())?;
            return self.history_execute(plan);
        }
        let (identity, heads) = self.history_view(c)?;
        let mut plan = match operation {
            HistoryOperation::Genesis { row, native } => {
                self.history_rows(c, &identity, heads, vec![row], native, true)?
            }
            HistoryOperation::TextBatch { rows, native } => {
                self.history_rows(c, &identity, heads, rows, native, false)?
            }
            HistoryOperation::QueueAccept { request, text } => {
                if let Some((_, old_text)) = self.history_request_record(c, request, &heads.view)? {
                    return if old_text == text {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                let mut p = Plan::new(c, Purpose::Queue);
                let q = self.root.generation;
                if self.hget(Key::ordinal(c, Namespace::Queue, q)?)?.is_some() {
                    return Err(Error::Conflict);
                }
                let r = RequestRecord {
                    queue: q,
                    request,
                    body_hash: hash_parts(b"kg:history2:request-body\0", &[text.as_bytes()]),
                    event: None,
                };
                p.put(Key::request(c, request)?, r.encode())?;
                let mut handle = [0; 32];
                handle[..16].copy_from_slice(&request);
                p.credits.push(CreditAction::Queue {
                    context: c,
                    sequence: q,
                    id: handle,
                    body: text.into_bytes(),
                });
                p
            }
            HistoryOperation::AcceptPending(pending) => {
                if pending.scope != identity.init.scope {
                    return Err(Error::Conflict);
                }
                let id = source_id(pending.format, &pending.source)?;
                let peer = 1 - identity.init.own_role;
                if pending.ordinal < heads.frontiers[usize::from(peer)] {
                    let old = self.history_candidate(
                        c,
                        peer,
                        pending.ordinal,
                        &heads.view,
                        heads.frontiers,
                    )?;
                    let source = self.hrequired(Key::hash(c, Namespace::Event, old.row.id)?)?;
                    return if old.row.id == id && source == pending.source {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                if pending.head && pending.ordinal != heads.frontiers[usize::from(peer)] {
                    return Err(Error::Conflict);
                }
                let mut p = Plan::new(c, Purpose::Pending);
                p.credits.push(CreditAction::Pending {
                    context: c,
                    ordinal: pending.ordinal,
                    event: id,
                    body: pending.source,
                    head: pending.head,
                    grant: pending.grant,
                });
                if let Some(capture) = pending.capture {
                    add_capture(&mut p, pending.ordinal, id, capture)?;
                }
                p
            }
            HistoryOperation::CapturePending {
                ordinal,
                event,
                capture,
            } => {
                let pending = credits::pending_view(&self.hrequired(Key::ordinal(
                    c,
                    Namespace::Pending,
                    ordinal,
                )?)?)?;
                if pending.event != event
                    || ordinal < heads.frontiers[usize::from(1 - identity.init.own_role)]
                {
                    return Err(Error::Conflict);
                }
                let mut p = Plan::new(c, Purpose::Capture);
                add_capture(&mut p, ordinal, event, capture)?;
                p
            }
            HistoryOperation::RepairCredit(repair) => {
                let mut p = Plan::new(c, Purpose::Repair);
                let action = match repair {
                    HistoryRepair::Grant { id, start, end } => {
                        if start < heads.frontiers[usize::from(1 - identity.init.own_role)] {
                            return Err(Error::Conflict);
                        }
                        CreditAction::Grant {
                            context: c,
                            id,
                            start,
                            end,
                        }
                    }
                    HistoryRepair::CancelGrant { id } => {
                        CreditAction::CancelGrant { context: c, id }
                    }
                    HistoryRepair::Refill { bytes } => CreditAction::Refill { context: c, bytes },
                };
                p.credits.push(action);
                p
            }
            HistoryOperation::Acknowledge {
                ordinal,
                event,
                auth,
            } => {
                let candidate = self.history_candidate(
                    c,
                    identity.init.own_role,
                    ordinal,
                    &heads.view,
                    heads.frontiers,
                )?;
                if ordinal == 0 || candidate.row.id != event {
                    return Err(Error::Conflict);
                }
                let receipt_key = receipt(c, identity.init.own_role, ordinal)?;
                let Some(debt) = self.hget(Key::ordinal(c, Namespace::Undelivered, ordinal)?)?
                else {
                    return if self.hrequired(receipt_key)? == event {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                };
                if credits::history_ticket(&debt, 3)?.0 != event {
                    return Err(Error::Conflict);
                }
                let mut p = Plan::new(c, Purpose::Ack);
                p.put(receipt_key, event.to_vec())?;
                let mut records = vec![receipt_key];
                for a in auth {
                    records.push(add_auth(&mut p, &a)?);
                }
                records.sort();
                records.dedup();
                p.credits.push(CreditAction::Acknowledge {
                    context: c,
                    ordinal,
                    event,
                    records,
                });
                p
            }
            HistoryOperation::Init(_) => return Err(Error::Bound("nested history init")),
        };
        // Always canonicalize refs before the private correspondence check.
        for claim in &mut plan.rows {
            claim.refs.sort();
            claim.refs.dedup();
        }
        self.history_execute(plan)
    }
    fn history_rows(
        &mut self,
        c: u8,
        identity: &Identity,
        mut heads: Heads,
        rows: Vec<PreparedReadable>,
        native: HistoryNativeTransition,
        genesis: bool,
    ) -> Result<Plan> {
        let mut p = Plan::new(
            c,
            if genesis {
                Purpose::Genesis
            } else {
                Purpose::Text
            },
        );
        let mut staged = BTreeMap::<Id, Binding>::new();
        let mut frame_ids = Vec::new();
        let mut queue_start = 0;
        for row in rows {
            if row.scope != identity.init.scope {
                return Err(Error::Conflict);
            }
            let id = row.event_id()?;
            let binding = Binding {
                format: row.format,
                view: heads.view,
                kind: row.kind,
                role: row.role,
                ordinal: row.ordinal,
                sequence: row.sequence,
                request: row.request,
                previous: row.previous,
                core_offset: row.core_offset,
                core_len: row.core_len,
                core_hash: hash_parts(
                    b"kg:history2:core\0",
                    &[
                        &[row.format],
                        core_span(&row.source, row.core_offset, row.core_len)?,
                    ],
                ),
                source_hash: hash_parts(b"kg:history2:source\0", &[&[row.format], &row.source]),
                parents: row.parents,
            };
            binding.check()?;
            let role = usize::from(binding.role);
            if genesis {
                if binding.kind == 3
                    || !row.text.is_empty()
                    || heads.tails[role].is_some()
                    || (role == 1 && heads.tails[0].is_none())
                    || (role == 0 && heads.tails[1].is_some())
                {
                    return Err(Error::Conflict);
                }
            } else if binding.kind != 3
                || heads.tails.contains(&None)
                || binding.ordinal != heads.frontiers[role]
                || binding.previous != heads.tails[role]
            {
                return Err(Error::Conflict);
            }
            for parent in &binding.parents {
                let parent_binding = if let Some(b) = staged.get(&parent.event) {
                    b.clone()
                } else {
                    let (b, _) = self
                        .history_projection(c, parent.event, &heads.view)?
                        .ok_or(Error::Integrity("history writer parent"))?;
                    if id_value(&self.hrequired(lane(c, parent.role, parent.ordinal)?)?)?
                        != parent.event
                    {
                        return Err(Error::Conflict);
                    }
                    b
                };
                if parent.event == id
                    || parent_binding.kind != 3
                    || parent_binding.role != parent.role
                    || parent_binding.ordinal != parent.ordinal
                    || parent.ordinal >= heads.frontiers[usize::from(parent.role)]
                {
                    return Err(Error::Conflict);
                }
            }
            let event_key = Key::hash(c, Namespace::Event, id)?;
            let projection_key = Key::hash(c, Namespace::Projection, id)?;
            let lane_key = lane(c, binding.role, binding.ordinal)?;
            let mut records = vec![
                event_key,
                projection_key,
                lane_key,
                heads_key(c)?,
                Key::hash(c, Namespace::Native, [0; 32])?,
            ];
            let origin = if genesis {
                Origin::Genesis
            } else if binding.role == identity.init.own_role {
                let (mut request, text) = self
                    .history_request_record(c, binding.request, &heads.view)?
                    .ok_or(Error::Conflict)?;
                if request.event.is_some() || text != row.text {
                    return Err(Error::Conflict);
                }
                let first = Key::ordinal(c, Namespace::Queue, queue_start)?;
                let slot = first.slot()?;
                let minimum = self
                    .tree
                    .range(slot as u8, self.root.roots[slot], first.bytes, 1)?;
                if minimum.first().map(|n| n.prefix)
                    != Some(Key::ordinal(c, Namespace::Queue, request.queue)?.bytes)
                {
                    return Err(Error::Conflict);
                }
                queue_start = request
                    .queue
                    .checked_add(1)
                    .ok_or(Error::Bound("history FIFO overflow"))?;
                request.event = Some(id);
                let request_key = Key::request(c, binding.request)?;
                records.push(request_key);
                p.put(request_key, request.encode())?;
                let mut handle = [0; 32];
                handle[..16].copy_from_slice(&binding.request);
                Origin::Local {
                    queue: request.queue,
                    handle,
                }
            } else {
                let pending = credits::pending_view(&self.hrequired(Key::ordinal(
                    c,
                    Namespace::Pending,
                    binding.ordinal,
                )?)?)?;
                if pending.event != id || pending.body != row.source {
                    return Err(Error::Conflict);
                }
                let receipt_key = receipt(c, binding.role, binding.ordinal)?;
                records.push(receipt_key);
                p.put(receipt_key, id.to_vec())?;
                Origin::Peer
            };
            p.put(event_key, row.source)?;
            p.put(
                projection_key,
                ProjectionValue {
                    text: row.text,
                    binding: binding.encode()?,
                }
                .encode()?,
            )?;
            p.put(lane_key, id.to_vec())?;
            for auth in &row.extras.auth {
                records.push(add_auth(&mut p, auth)?);
            }
            if let Some(seen) = row.extras.seen {
                let key = Key::hash(
                    c,
                    if role == 0 {
                        Namespace::SeenA
                    } else {
                        Namespace::SeenB
                    },
                    seen.key,
                )?;
                let mut value = seen.generation.to_be_bytes().to_vec();
                value.extend(id);
                p.put(key, value)?;
                records.push(key);
            }
            if let Some(slot) = row.extras.author_slot {
                let ns = match slot {
                    0 => Namespace::Author0,
                    1 => Namespace::Author1,
                    2 => Namespace::Author2,
                    _ => return Err(Error::Bound("history Author slot")),
                };
                let key = Key::ordinal(c, ns, binding.sequence)?;
                let mut value = identity.init.scope.context.to_vec();
                value.extend(id);
                p.put(key, value)?;
                records.push(key);
            }
            records.sort();
            records.dedup();
            match origin {
                Origin::Genesis => (),
                Origin::Local { queue, handle } => p.credits.push(CreditAction::Materialize {
                    context: c,
                    queue,
                    request: handle,
                    ordinal: binding.ordinal,
                    event: id,
                    records: records.clone(),
                }),
                Origin::Peer => p.credits.push(CreditAction::Readable {
                    context: c,
                    ordinal: binding.ordinal,
                    event: id,
                    records: records.clone(),
                }),
            }
            p.rows.push(RowClaim {
                event: id,
                role: binding.role,
                ordinal: binding.ordinal,
                origin,
                refs: records,
            });
            heads.tails[role] = Some(id);
            if !genesis {
                heads.frontiers[role] = heads.frontiers[role]
                    .checked_add(1)
                    .ok_or(Error::Bound("history frontier overflow"))?;
            }
            staged.insert(id, binding);
            frame_ids.push(id);
        }
        if frame_ids != native.frame_ids {
            return Err(Error::Conflict);
        }
        let key = Key::hash(c, Namespace::Native, [0; 32])?;
        let old = self.hget(key)?;
        if old.as_ref().map(|bytes| *blake3::hash(bytes).as_bytes()) != native.before_hash {
            return Err(Error::Conflict);
        }
        if old.is_none() && (!genesis || p.rows.first().is_none_or(|r| r.role != 0)) {
            return Err(Error::Conflict);
        }
        p.put(key, native.after_blob)?;
        p.put(heads_key(c)?, heads.encode())?;
        Ok(p)
    }
    fn history_validate_plan(&mut self, p: &Plan) -> Result<()> {
        let c = p.context;
        if p.puts.keys().copied().collect::<BTreeSet<_>>() != p.allowed {
            return Err(Error::Bound("history extra effect"));
        }
        let expected_actions = match p.purpose {
            Purpose::Init | Purpose::Genesis => p.credits.is_empty(),
            Purpose::Text => {
                !p.rows.is_empty() && p.rows.len() <= 4 && p.credits.len() == p.rows.len()
            }
            Purpose::Pending => p.credits.len() == 1 || p.credits.len() == 2,
            _ => p.credits.len() == 1,
        };
        if !expected_actions {
            return Err(Error::Bound("history transition cardinality"));
        }
        if p.purpose == Purpose::Pending {
            let Some(CreditAction::Pending { ordinal, event, .. }) = p.credits.first() else {
                return Err(Error::Conflict);
            };
            if p.credits.get(1).is_some_and(|a|!matches!(a,CreditAction::CapturePending {ordinal:n,event:id,..} if n==ordinal && id==event)) {
                return Err(Error::Conflict);
            }
        }
        for (key, value) in &p.puts {
            if key.context != c {
                return Err(Error::Bound("history cross-context effect"));
            }
            let slot = key.slot()?;
            if key.namespace.global() {
                if value.len() != 64 || value[..32] != self.root.contexts[usize::from(c)] {
                    return Err(Error::Conflict);
                }
                if let Some(leaf) = self
                    .tree
                    .find(slot as u8, self.root.roots[slot], key.bytes)?
                    && !matches!(leaf.body,tree::Body::Leaf {owner,..} if owner == c)
                {
                    return Err(Error::Conflict);
                }
            }
            let allowed = match p.purpose {
                Purpose::Init => *key == identity_key(c)? || *key == heads_key(c)?,
                Purpose::Queue => key.namespace == Namespace::Request,
                Purpose::Pending | Purpose::Capture => key.namespace == Namespace::Auth,
                Purpose::Repair => false,
                Purpose::Ack => matches!(
                    key.namespace,
                    Namespace::ReceiptA | Namespace::ReceiptB | Namespace::Auth
                ),
                Purpose::Genesis | Purpose::Text => match key.namespace {
                    Namespace::Event | Namespace::Projection => {
                        p.rows.iter().any(|r| r.event == key.bytes)
                    }
                    Namespace::LaneA | Namespace::LaneB => p
                        .rows
                        .iter()
                        .any(|r| lane(c, r.role, r.ordinal).is_ok_and(|k| k == *key)),
                    Namespace::Current => *key == heads_key(c)?,
                    Namespace::Native => key.bytes == [0; 32],
                    Namespace::Auth
                    | Namespace::SeenA
                    | Namespace::SeenB
                    | Namespace::Author0
                    | Namespace::Author1
                    | Namespace::Author2
                    | Namespace::Request
                    | Namespace::ReceiptA
                    | Namespace::ReceiptB => p.rows.iter().any(|r| r.refs.contains(key)),
                    _ => false,
                },
            };
            if !allowed {
                return Err(Error::Bound("history effect purpose/correspondence"));
            }
        }
        let mut slots = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut consumed = BTreeSet::new();
        let mut matched = BTreeSet::new();
        for row in &p.rows {
            if !slots.insert((row.role, row.ordinal)) || !ids.insert(row.event) {
                return Err(Error::Conflict);
            }
            for key in [
                Key::hash(c, Namespace::Event, row.event)?,
                Key::hash(c, Namespace::Projection, row.event)?,
                lane(c, row.role, row.ordinal)?,
            ] {
                if !p.puts.contains_key(&key) || self.hget(key)?.is_some() {
                    return Err(Error::Conflict);
                }
            }
            let projection = ProjectionValue::decode(
                p.puts
                    .get(&Key::hash(c, Namespace::Projection, row.event)?)
                    .ok_or(Error::Conflict)?,
            )?;
            let binding = Binding::decode(&projection.binding)?;
            let (identity, heads) = self.history_view(c)?;
            if binding.view != heads.view
                || binding.role != row.role
                || binding.ordinal != row.ordinal
            {
                return Err(Error::Conflict);
            }
            let source = p
                .puts
                .get(&Key::hash(c, Namespace::Event, row.event)?)
                .ok_or(Error::Conflict)?;
            binding.verify_source(row.event, source)?;
            match row.origin {
                Origin::Genesis => {
                    if binding.kind == 3 || !projection.text.is_empty() {
                        return Err(Error::Conflict);
                    }
                }
                Origin::Local { queue, handle } => {
                    let (ticket, body) = credits::history_ticket(
                        &self.hrequired(Key::ordinal(c, Namespace::Queue, queue)?)?,
                        1,
                    )?;
                    let mut expected_handle = [0; 32];
                    expected_handle[..16].copy_from_slice(&binding.request);
                    let request = RequestRecord::decode(
                        p.puts
                            .get(&Key::request(c, binding.request)?)
                            .ok_or(Error::Conflict)?,
                    )?;
                    if binding.kind != 3
                        || row.role != identity.init.own_role
                        || ticket != handle
                        || handle != expected_handle
                        || body != projection.text.as_bytes()
                        || request.queue != queue
                        || request.request != binding.request
                        || request.event != Some(row.event)
                        || request.body_hash != hash_parts(b"kg:history2:request-body\0", &[&body])
                    {
                        return Err(Error::Conflict);
                    }
                }
                Origin::Peer => {
                    let pending = credits::pending_view(&self.hrequired(Key::ordinal(
                        c,
                        Namespace::Pending,
                        row.ordinal,
                    )?)?)?;
                    let receipt = receipt(c, row.role, row.ordinal)?;
                    if binding.kind != 3
                        || row.role == identity.init.own_role
                        || pending.event != row.event
                        || pending.body != *source
                        || p.puts.get(&receipt).is_none_or(|v| *v != row.event)
                    {
                        return Err(Error::Conflict);
                    }
                }
            }
        }
        for action in &p.credits {
            if credit_context(action) != c
                || refs(action)
                    .iter()
                    .any(|key| key.context != c || !p.allowed.contains(key))
            {
                return Err(Error::Bound("history cross-context/extra credit ref"));
            }
            let consumption = match action {
                CreditAction::Materialize {
                    queue,
                    request,
                    ordinal,
                    event,
                    records,
                    ..
                } => {
                    let row = p.rows.iter().find(|r| r.event == *event && r.ordinal == *ordinal
                        && matches!(r.origin,Origin::Local {queue:q,handle:h} if q == *queue && h == *request)
                        && r.refs == *records).ok_or(Error::Conflict)?;
                    Some((0, *queue, row.event))
                }
                CreditAction::Readable {
                    ordinal,
                    event,
                    records,
                    ..
                } => {
                    let row = p
                        .rows
                        .iter()
                        .find(|r| {
                            r.event == *event
                                && r.ordinal == *ordinal
                                && matches!(r.origin, Origin::Peer)
                                && r.refs == *records
                        })
                        .ok_or(Error::Conflict)?;
                    Some((1, *ordinal, row.event))
                }
                _ => None,
            };
            if let Some((kind, key, event)) = consumption
                && (p.purpose != Purpose::Text
                    || !consumed.insert((kind, key))
                    || !matched.insert(event))
            {
                return Err(Error::Conflict);
            }
            let allowed = match p.purpose {
                Purpose::Init | Purpose::Genesis => false,
                Purpose::Text => matches!(
                    action,
                    CreditAction::Materialize { .. } | CreditAction::Readable { .. }
                ),
                Purpose::Queue => matches!(action, CreditAction::Queue { .. }),
                Purpose::Pending => matches!(
                    action,
                    CreditAction::Pending { .. } | CreditAction::CapturePending { .. }
                ),
                Purpose::Capture => matches!(action, CreditAction::CapturePending { .. }),
                Purpose::Repair => matches!(
                    action,
                    CreditAction::Grant { .. }
                        | CreditAction::CancelGrant { .. }
                        | CreditAction::Refill { .. }
                ),
                Purpose::Ack => matches!(action, CreditAction::Acknowledge { .. }),
            };
            if !allowed {
                return Err(Error::Bound("history credit purpose"));
            }
        }
        let expected: BTreeSet<_> = p
            .rows
            .iter()
            .filter(|r| !matches!(r.origin, Origin::Genesis))
            .map(|r| r.event)
            .collect();
        if matched != expected {
            return Err(Error::Conflict);
        }
        let native_key = Key::hash(c, Namespace::Native, [0; 32])?;
        let has_source = matches!(p.purpose, Purpose::Text | Purpose::Genesis);
        if p.puts.contains_key(&native_key) != has_source
            || has_source && p.rows.is_empty()
            || p.purpose == Purpose::Text && p.rows.iter().any(|r| r.ordinal == 0)
            || p.purpose == Purpose::Genesis && (p.rows.len() != 1 || p.rows[0].ordinal != 0)
            || !has_source && !p.rows.is_empty()
        {
            return Err(Error::Bound("history Native/source purpose"));
        }
        Ok(())
    }
    fn history_execute(&mut self, p: Plan) -> Result<()> {
        self.history_validate_plan(&p)?;
        for (key, value) in p.puts {
            self.put(key, &value, 0, false)?;
        }
        for action in p.credits {
            credits::apply(self, action)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests as f;
    use super::*;

    #[test]
    fn history_private_lowering_rejects_empty_extra_double_cross_context_and_old_event_consumes()
    -> f::TestResult {
        let (_dir, mut store) = f::fixture()?;
        let genesis = f::setup(&mut store, 0)?;
        let old = f::prepared(0, 0, 1, Some(genesis[0]), [1; 16], "hello", vec![])?;
        f::append(&mut store, 0, old.clone())?;
        f::apply(
            &mut store,
            0,
            HistoryOperation::QueueAccept {
                request: [2; 16],
                text: "hello".into(),
            },
        )?;
        let row = f::prepared(0, 0, 2, Some(old.event_id()?), [2; 16], "hello", vec![])?;
        let native = f::native(&mut store, 0, std::slice::from_ref(&row))?;
        let tx = store.vault.database.begin_read()?;
        let nodes = tx.open_table(NODES)?;
        let values = tx.open_table(VALUES)?;
        let mut w = Work::new(&nodes, &values, &store.crypto, store.root.clone());
        let (identity, heads) = w.history_view(0)?;
        let valid = w.history_rows(0, &identity, heads, vec![row], native, false)?;
        w.history_validate_plan(&valid)?;
        let before = w.root.encode().to_vec();
        for case in 0..9 {
            let mut p = valid.clone();
            match case {
                0 => p.rows.clear(),
                1 => p.credits.push(p.credits[0].clone()),
                2 => {
                    if let CreditAction::Materialize { request, .. } = &mut p.credits[0] {
                        request[0] ^= 1;
                    }
                }
                3 => {
                    if let CreditAction::Materialize { context, .. } = &mut p.credits[0] {
                        *context = 1;
                    }
                }
                4 => {
                    if let CreditAction::Materialize { ordinal, .. } = &mut p.credits[0] {
                        *ordinal += 1;
                    }
                }
                5 => {
                    if let CreditAction::Materialize { records, .. } = &mut p.credits[0] {
                        records.push(Key::hash(1, Namespace::Auth, f::id(9))?);
                    }
                }
                6 => {
                    p.puts
                        .insert(Key::hash(1, Namespace::Current, f::id(9))?, vec![1]);
                }
                7 => p.rows.push(p.rows[0].clone()),
                _ => {
                    p.credits.clear();
                }
            }
            assert!(w.history_validate_plan(&p).is_err(), "case{case}");
            assert_eq!(w.root.encode().to_vec(), before);
            assert!(w.changed.is_empty() && w.tree.changed.is_empty());
        }
        // Exact review1 trace, structurally unrepresentable through HistoryOperation.
        let mut trace = Plan::new(0, Purpose::Text);
        let q = match valid.rows[0].origin {
            Origin::Local { queue, .. } => queue,
            _ => return Err("local fixture".into()),
        };
        let mut request = [0; 32];
        request[..16].copy_from_slice(&[2; 16]);
        trace.credits.push(CreditAction::Materialize {
            context: 0,
            queue: q,
            request,
            ordinal: 2,
            event: old.event_id()?,
            records: vec![
                Key::hash(0, Namespace::Event, old.event_id()?)?,
                Key::hash(0, Namespace::Projection, old.event_id()?)?,
            ],
        });
        assert!(w.history_validate_plan(&trace).is_err());
        assert_eq!(w.root.encode().to_vec(), before);
        // An existing identical source cannot become a new row in another purpose.
        let mut old_plan = valid.clone();
        old_plan.rows[0].event = old.event_id()?;
        assert!(w.history_validate_plan(&old_plan).is_err());
        Ok(())
    }

    #[test]
    fn history_private_partial_foreign_malformed_heads_and_tails_never_initialize() -> f::TestResult
    {
        let (_dir, mut store) = f::fixture()?;
        let genesis = f::setup(&mut store, 0)?;
        f::append(
            &mut store,
            0,
            f::prepared(0, 0, 1, Some(genesis[0]), [1; 16], "body", vec![])?,
        )?;
        let tx = store.vault.database.begin_read()?;
        let nodes = tx.open_table(NODES)?;
        let values = tx.open_table(VALUES)?;
        for case in 0..8 {
            let mut w = Work::new(&nodes, &values, &store.crypto, store.root.clone());
            let (mut identity, mut heads) = w.history_view(0)?;
            match case {
                0 => w.remove(identity_key(0)?)?,
                1 => w.remove(heads_key(0)?)?,
                2 => {
                    heads.view[0] ^= 1;
                    w.put(heads_key(0)?, &heads.encode(), 0, false)?;
                }
                3 => w.put(heads_key(0)?, b"bad codec", 0, false)?,
                4 => {
                    heads.tails[0] = Some(f::id(999));
                    w.put(heads_key(0)?, &heads.encode(), 0, false)?;
                }
                5 => {
                    identity.incarnation[0] ^= 1;
                    w.put(identity_key(0)?, &identity.encode()?, 0, true)?;
                }
                6 => {
                    w.root.roots[lane(0, 0, 0)?.slot()?].count += 1;
                }
                _ => {
                    heads.tails[0] = None;
                    heads.frontiers[0] = 1;
                    w.root.roots[lane(0, 0, 0)?.slot()?] = Summary::default();
                    w.put(heads_key(0)?, &heads.encode(), 0, false)?;
                }
            }
            let before = w.root.encode().to_vec();
            let changes = w.changed.clone();
            let changed_nodes = w.tree.changed.clone();
            assert!(
                w.history_apply(0, HistoryOperation::Init(f::init(0)))
                    .is_err(),
                "case{case}"
            );
            assert_eq!(w.root.encode().to_vec(), before);
            assert_eq!(w.changed, changes);
            assert_eq!(w.tree.changed, changed_nodes);
        }
        Ok(())
    }
}
