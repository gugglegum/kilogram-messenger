//! C-scoped imported/full-L1 proof reads. Two candidates, no Native or primary-table access.
use super::super::super::{history_cursor as cursor, history_types::Identity};
use super::*;

#[derive(Clone, Serialize)]
pub struct L1HistoryItem {
    pub id: String,
    pub role: u8,
    pub ordinal: u64,
    pub sequence: u64,
    pub request: String,
    pub text: String,
    pub proof_kind: &'static str,
    pub delivered: bool,
}
#[derive(Serialize)]
struct Page<'a> {
    version: u8,
    view: String,
    #[serde(rename = "F")]
    frontiers: [u64; 2],
    finished: bool,
    cursor: String,
    items: &'a [L1HistoryItem],
}
struct Candidate {
    id: Id,
    row: L1HistoryItem,
    parents: Vec<(u8, u64)>,
}

impl L1Repository {
    pub fn history(
        &mut self,
        c: u8,
        supplied: Option<&HistoryCursor>,
        limit: usize,
    ) -> Result<Vec<u8>> {
        self.occupied(c)?;
        if !(1..=32).contains(&limit) {
            return Err(Error::Bound("history page limit"));
        }
        if self.core.poisoned {
            return Err(Error::ReopenRequired);
        }
        let tx = self.core.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &self.core.crypto, self.core.root.clone());
        w.tree.enable_read_reuse();
        let identity = w.l1_history_view(c)?;
        let meta = w.l1_meta(c)?;
        let mut position = if let Some(supplied) = supplied {
            cursor::authenticate(&self.core.crypto, &identity, supplied)?
        } else {
            cursor::Position {
                frontiers: meta.next,
                next: [1, 1],
            }
        };
        for role in 0..2 {
            if position.next[role] == 0 || position.next[role] > position.frontiers[role] {
                return Err(Error::InvalidCursor);
            }
            if position.frontiers[role] > meta.next[role] {
                return Err(Error::SnapshotUnavailable);
            }
        }
        let mut candidates: [Option<Candidate>; 2] = [None, None];
        let mut items = Vec::with_capacity(limit);
        let view = super::super::source::hex(&identity.view_id()?);
        let initial_cursor = cursor::seal(&self.core.crypto, &identity, &position)?.to_base64();
        // Exact escaped header bytes; false is one byte larger than final true.
        let header = serde_json::to_vec(&Page {
            version: 3,
            view: view.clone(),
            frontiers: position.frontiers,
            finished: false,
            cursor: initial_cursor,
            items: &[],
        })
        .map_err(|_| Error::Bound("L1 history JSON"))?
        .len();
        let mut encoded = header;
        while items.len() < limit && position.next != position.frontiers {
            for (role, candidate) in candidates.iter_mut().enumerate() {
                if candidate.is_none() && position.next[role] < position.frontiers[role] {
                    *candidate = Some(w.l1_candidate(
                        &self.profile,
                        c,
                        role as u8,
                        position.next[role],
                        &identity,
                        position.frontiers,
                    )?);
                }
            }
            let role = (0..2)
                .filter(|r| {
                    candidates[*r].as_ref().is_some_and(|v| {
                        v.parents
                            .iter()
                            .all(|(role, n)| *n < position.next[usize::from(*role)])
                    })
                })
                .min_by_key(|r| candidates[*r].as_ref().map(|v| v.id))
                .ok_or(Error::Integrity("L1 history causal frontier"))?;
            let candidate = candidates[role]
                .as_ref()
                .ok_or(Error::Integrity("L1 history candidate"))?;
            let actual = serde_json::to_vec(&candidate.row)
                .map_err(|_| Error::Bound("L1 history row JSON"))?
                .len()
                + usize::from(!items.is_empty());
            if encoded + actual > 48 * 1024 - 64 {
                break;
            }
            // Cursor advances only after the actual escaped candidate fits.
            encoded += actual;
            items.push(
                candidates[role]
                    .take()
                    .ok_or(Error::Integrity("L1 candidate"))?
                    .row,
            );
            position.next[role] += 1;
        }
        let cursor = cursor::seal(&self.core.crypto, &identity, &position)?.to_base64();
        let raw = serde_json::to_vec(&Page {
            version: 3,
            view,
            frontiers: position.frontiers,
            finished: position.next == position.frontiers,
            cursor,
            items: &items,
        })
        .map_err(|_| Error::Bound("L1 history JSON"))?;
        if raw.len() > 48 * 1024 - 64 {
            return Err(Error::Bound("L1 history encoded response"));
        }
        Ok(raw)
    }
}

impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(super) fn l1_initialize_history(&mut self, p: &Profile, c: u8) -> Result<()> {
        let wrapper = self.l1_head(c)?;
        let head = codec::decode(wrapper.fields(9)?[4].bytes()?, 2048)?;
        let h = head.fields(13)?;
        let meta = self.l1_meta(c)?;
        if h[5].uint()? != 3
            || meta.next.contains(&0)
            || self.l1_control(p, c, L1Control::Ready(0))?.is_none()
            || self.l1_control(p, c, L1Control::Ready(1))?.is_none()
        {
            return Err(Error::HistoryUnavailable);
        }
        let sid = h[11]
            .optional()?
            .ok_or(Error::HistoryUnavailable)?
            .text()?
            .to_owned();
        let identity = Identity {
            incarnation: self.root.incarnation,
            init: HistoryInit {
                scope: HistoryScope {
                    context: p.contexts[usize::from(c)].0,
                    dialog: h[9].optional()?.ok_or(Error::HistoryUnavailable)?.fixed()?,
                    epoch: h[10]
                        .optional()?
                        .ok_or(Error::HistoryUnavailable)?
                        .fixed()?,
                    session_id: sid,
                },
                own_role: p.contexts[usize::from(c)].1,
            },
        };
        self.put(
            super::super::super::history::identity_key(c)?,
            &identity.encode()?,
            0,
            false,
        )
    }
    fn l1_history_view(&mut self, c: u8) -> Result<Identity> {
        let raw = self
            .get(super::super::super::history::identity_key(c)?)?
            .ok_or(Error::HistoryUnavailable)?;
        let identity = Identity::decode(&raw)?;
        if identity.incarnation != self.root.incarnation
            || identity.init.scope.context != self.root.contexts[usize::from(c)]
        {
            return Err(Error::Integrity("L1 history view scope"));
        }
        Ok(identity)
    }
    fn l1_candidate(
        &mut self,
        p: &Profile,
        c: u8,
        role: u8,
        n: u64,
        identity: &Identity,
        frontiers: [u64; 2],
    ) -> Result<Candidate> {
        let raw_id = self
            .get(super::super::super::history::lane(c, role, n)?)?
            .ok_or(Error::Integrity("history lane missing"))?;
        let id: Id = raw_id
            .as_slice()
            .try_into()
            .map_err(|_| Error::Integrity("history lane ID"))?;
        let source = self
            .get(Key::hash(c, Namespace::Event, id)?)?
            .ok_or(Error::Integrity("history source missing"))?;
        let e = super::super::copy::event_fields(&source)?;
        let value = codec::decode(&source, 16 * 1024)?;
        let core = value.fields(5)?[1].fields(14)?;
        let scope = &identity.init.scope;
        if e.id != id
            || e.c != scope.context
            || e.role != role
            || e.n != n
            || e.kind != 3
            || core[2].fixed::<32>()? != scope.dialog
            || core[3].fixed::<32>()? != scope.epoch
            || core[4].text()? != scope.session_id
        {
            return Err(Error::Integrity("history immutable source scope"));
        }
        let projection = ProjectionValue::decode(
            &self
                .get(Key::hash(c, Namespace::Projection, id)?)?
                .ok_or(Error::Integrity("history projection missing"))?,
        )?;
        let binding = codec::decode(&projection.binding, 2304)?;
        let f = binding.fields(5)?;
        if f[0].uint()? != 7
            || f[1].fixed::<32>()? != p.mid
            || f[2].fixed::<32>()? != p.source
            || f[4].fixed::<16>()? != e.request
        {
            return Err(Error::Integrity("history proof binding"));
        }
        let proof_kind = match &f[3] {
            Value::B(pointer) => {
                let expected = migration_key(p.mid, 5, &id)?;
                if pointer.as_slice() != expected.bytes {
                    return Err(Error::Integrity("snapshot membership key"));
                }
                let raw = self
                    .get(expected)?
                    .ok_or(Error::Integrity("snapshot membership missing"))?;
                let snapshot = codec::read_record(&raw, 5, 4096)?;
                let s = snapshot.fields(13)?;
                if s[0].uint()? != 7
                    || s[1].fixed::<32>()? != p.mid
                    || s[3].fixed::<32>()? != e.c
                    || s[4].uint()? != u64::from(role)
                    || s[5].uint()? != n
                    || s[6].fixed::<32>()? != id
                    || s[7].fixed::<16>()? != e.request
                {
                    return Err(Error::Integrity("snapshot exact source membership"));
                }
                let reference = self
                    .get(Key::hash(0, Namespace::Migration, s[2].fixed()?)?)?
                    .ok_or(Error::Integrity("snapshot source reference"))?;
                let reference = codec::read_record(&reference, 4, 1024)?;
                let r = reference.fields(9)?;
                if r[1].fixed::<32>()? != p.source
                    || r[4].uint()? != 11
                    || r[5].optional()?.map(Value::fixed::<32>).transpose()? != Some(e.c)
                    || r[7].fixed::<32>()? != hash(b"", &source)
                    || r[8].uint()? != source.len() as u64
                {
                    return Err(Error::Integrity("snapshot original source correspondence"));
                }
                "LegacySnapshot"
            }
            Value::A(_) => {
                let pointer = f[3].fields(2)?;
                let k = Key::hash(
                    c,
                    Namespace::Control,
                    hash(b"kg:od1:full-L1-proof/v7\0", &id),
                )?;
                if pointer[0].uint()? != 1 || pointer[1].fixed::<32>()? != k.bytes {
                    return Err(Error::Integrity("full L1 proof key"));
                }
                let proof = self
                    .get(k)?
                    .ok_or(Error::Integrity("full L1 proof missing"))?;
                let proof = codec::read_record(&proof, 41, 4096)?;
                let r = proof.fields(11)?;
                if r[0].uint()? != 7
                    || r[1].fixed::<32>()? != p.mid
                    || r[2].fixed::<32>()? != e.c
                    || r[3].uint()? != u64::from(role)
                    || r[4].uint()? != n
                    || r[5].fixed::<32>()? != id
                    || r[6].fixed::<32>()? != e.original
                {
                    return Err(Error::Integrity("full L1 exact source/capture"));
                }
                for field in std::iter::once(&r[6])
                    .chain(r[7..10].iter().filter_map(|v| v.optional().ok().flatten()))
                {
                    let auth = field.fixed::<32>()?;
                    let raw = self
                        .get(Key::hash(c, Namespace::Auth, auth)?)?
                        .ok_or(Error::Integrity("full L1 captured auth"))?;
                    if hash(b"", &raw) != auth {
                        return Err(Error::Integrity("full L1 captured auth hash"));
                    }
                }
                for field in &r[7..11] {
                    field.optional()?;
                }
                check_capture_shape(
                    role == identity.init.own_role,
                    true,
                    [
                        r[7].optional()?.is_some(),
                        r[8].optional()?.is_some(),
                        r[9].optional()?.is_some(),
                        r[10].optional()?.is_some(),
                    ],
                )?;
                if role != identity.init.own_role
                    && r[7..11]
                        .iter()
                        .any(|v| !v.optional().is_ok_and(|o| o.is_some()))
                {
                    return Err(Error::Integrity("full received L1 captures incomplete"));
                }
                if let Some(receipt) = r[10].optional()? {
                    let raw = self
                        .get(super::super::super::history::receipt(c, role, n)?)?
                        .ok_or(Error::Integrity("full L1 readable receipt"))?;
                    if receipt.fixed::<32>()? != hash(b"", &raw) {
                        return Err(Error::Integrity("full L1 actual receipt hash"));
                    }
                }
                "FullL1"
            }
            _ => return Err(Error::Integrity("unknown history proof variant")),
        };
        let receipt = self.get(super::super::super::history::receipt(c, role, n)?)?;
        if let Some(raw) = &receipt {
            let value = codec::decode(raw, 1024)?;
            let r = value.fields(2)?[0].fields(8)?;
            if r[1] != core[2]
                || r[2] != core[3]
                || r[3].uint()? != u64::from(role)
                || r[4].uint()? != n
                || r[5].fixed::<32>()? != id
            {
                return Err(Error::Integrity("history receipt delivery overlay"));
            }
        }
        let mut parents = Vec::with_capacity(2);
        for parent in core[11].list()? {
            let id = parent.fixed()?;
            let raw = self
                .get(Key::hash(c, Namespace::Event, id)?)?
                .ok_or(Error::Integrity("history parent missing"))?;
            let e = super::super::copy::event_fields(&raw)?;
            if e.c != scope.context || e.kind != 3 || e.n >= frontiers[usize::from(e.role)] {
                return Err(Error::Integrity("history parent outside snapshot"));
            }
            parents.push((e.role, e.n));
        }
        Ok(Candidate {
            id,
            row: L1HistoryItem {
                id: super::super::source::hex(&id),
                role,
                ordinal: n,
                sequence: core[8].uint()?,
                request: super::super::source::hex(&e.request),
                text: projection.text,
                proof_kind,
                delivered: receipt.is_some(),
            },
            parents,
        })
    }
}
