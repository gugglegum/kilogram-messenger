//! Closed lowering of one authenticated legacy entry. No arbitrary destination keys.
use super::super::*;
use super::{
    LegacyRead,
    codec::{self, Value, a, b, opt, u},
    records::*,
    source::{self, LegacyEntry},
};

/// Only the private owner obtains this text by authenticating/opening the original
/// self-HPKE projection. It is never an IPC input or a replacement source record.
pub struct MigrationProjection {
    pub text: String,
    pub request: [u8; 16],
}
pub(in crate::vault::indexed_dialog) struct CopyItem {
    pub entry: LegacyEntry,
    pub body: Zeroizing<Vec<u8>>,
    pub projection: Option<MigrationProjection>,
    pub snapshot: Option<Vec<u8>>,
    pub seen: Option<(u8, Id, u64, Id)>,
    pub author_context: Option<u8>,
    pub pending_state: Option<Vec<u8>>,
    pub paid_auth: Vec<Id>,
    pub paid_auth_sizes: Vec<(Id, u64)>,
}
pub(super) struct ProducedRow {
    pub key: Key,
    pub raw: Vec<u8>,
    pub reserved: u64,
}
#[derive(Default)]
pub(super) struct ProducedRows {
    pub rows: Vec<ProducedRow>,
    pub pending: Option<(u8, u64, bool)>,
}
impl ProducedRows {
    fn put(&mut self, key: Key, raw: &[u8], reserved: u64, _internal: bool) -> Result<()> {
        self.rows.push(ProducedRow {
            key,
            raw: raw.to_vec(),
            reserved,
        });
        Ok(())
    }
    fn migration_meta_put(
        &mut self,
        key: Key,
        raw: &[u8],
        capacity: usize,
        extra: u64,
    ) -> Result<()> {
        self.put(
            key,
            raw,
            (capacity
                .checked_sub(raw.len())
                .ok_or(Error::Bound("metadata capacity"))? as u64)
                .checked_add(extra)
                .ok_or(Error::Bound("metadata reservation"))?,
            true,
        )
    }
    pub fn generate(profile: &Profile, item: &CopyItem) -> Result<Self> {
        let mut rows = Self::default();
        rows.lower(profile, item)?;
        Ok(rows)
    }
}
pub(super) struct EventFields {
    pub id: Id,
    pub c: Id,
    pub role: u8,
    pub n: u64,
    pub kind: u8,
    pub request: [u8; 16],
    pub original: Id,
    pub key: Id,
    pub g: u64,
}
pub(super) fn raw_hash(domain: &[u8], raw: &[u8]) -> Id {
    let mut h = blake3::Hasher::new();
    h.update(domain);
    h.update(raw);
    *h.finalize().as_bytes()
}
pub(super) fn event_fields(raw: &[u8]) -> Result<EventFields> {
    let v = codec::decode(raw, 16 * 1024)?;
    let event = v.fields(5)?;
    let c = event[1].fields(14)?;
    let header = event[2].fields(3)?;
    let role = u8::try_from(c[5].uint()?).map_err(|_| Error::Integrity("legacy role"))?;
    let kind = u8::try_from(c[9].uint()?).map_err(|_| Error::Integrity("legacy kind"))?;
    if event[0].uint()? != 1
        || c[0].uint()? != 1
        || role > 1
        || c[6].uint()? >= 512
        || !(1..=3).contains(&kind)
    {
        return Err(Error::Integrity("legacy event structure"));
    }
    Ok(EventFields {
        id: raw_hash(b"kg:od1:event-id\0", raw),
        c: c[1].fixed()?,
        role,
        n: c[6].uint()?,
        kind,
        request: c[10].fixed()?,
        original: c[13].fixed()?,
        key: header[1].fixed()?,
        g: c[12].fields(4)?[0].uint()?,
    })
}
pub(super) fn context(profile: &Profile, c: Id) -> Result<u8> {
    profile
        .contexts
        .iter()
        .position(|(id, _)| *id == c)
        .map(|n| n as u8)
        .ok_or(Error::Integrity("source context owner"))
}
fn ordinal(path: &str) -> Result<u64> {
    path.rsplit('/')
        .next()
        .ok_or(Error::Integrity("ordinal path"))?
        .parse()
        .map_err(|_| Error::Integrity("ordinal path"))
}
fn lane_ordinal(path: &str) -> Result<(u8, u64)> {
    let mut parts = path.rsplit('/');
    let n = parts
        .next()
        .ok_or(Error::Integrity("ordinal"))?
        .parse()
        .map_err(|_| Error::Integrity("ordinal"))?;
    let lane = parts
        .next()
        .ok_or(Error::Integrity("lane"))?
        .parse::<u8>()
        .map_err(|_| Error::Integrity("lane"))?;
    if lane > 1 || n >= 512 {
        return Err(Error::Integrity("legacy lane bound"));
    }
    Ok((lane, n))
}
pub(super) fn old_key(c: u8, ns: Namespace, path: &str) -> Result<Key> {
    Key::hash(
        c,
        ns,
        codec::digest(b"kg:od2:legacy-path\0", &b(path.as_bytes()))?,
    )
}
pub(super) fn prepare_item(
    reader: &dyn LegacyRead,
    profile: &Profile,
    facts: &MigrationOriginFacts,
    entry: &LegacyEntry,
    projection: Option<MigrationProjection>,
) -> Result<CopyItem> {
    let body = reader.required(&entry.path)?;
    let mut item = CopyItem {
        entry: entry.clone(),
        body,
        projection,
        snapshot: None,
        seen: None,
        author_context: None,
        pending_state: None,
        paid_auth: Vec::new(),
        paid_auth_sizes: Vec::new(),
    };
    if entry.kind == 11 {
        let e = event_fields(&item.body)?;
        let c = context(profile, e.c)?;
        if entry.path.rsplit('/').next() != Some(source::hex(&e.id).as_str())
            || entry.context != Some(e.c)
        {
            return Err(Error::Integrity("EventId source membership"));
        }
        let head_entry = reader
            .entries()
            .iter()
            .find(|v| v.kind == 9 && v.context == Some(e.c))
            .ok_or(Error::Integrity("Head witness"))?;
        let native_entry = reader
            .entries()
            .iter()
            .find(|v| v.kind == 10 && v.context == Some(e.c))
            .ok_or(Error::Integrity("Native witness"))?;
        let original = migration_key(profile.mid, 4, &entry.record_key)?.bytes;
        let projection_entry = reader.entries().iter().find(|v| {
            v.kind == 12
                && v.context == Some(e.c)
                && v.path.rsplit('/').next() == Some(source::hex(&e.id).as_str())
        });
        let projection_ref = projection_entry
            .map(|v| migration_key(profile.mid, 4, &v.record_key).map(|k| k.bytes))
            .transpose()?;
        let local = profile.contexts[usize::from(c)].1;
        let receipt_path = format!(
            "runtime/od1/contexts/{}/receipts/{}/{:020}",
            source::hex(&e.c),
            e.role,
            e.n
        );
        let receipt = reader.get(&receipt_path)?;
        let (delivery, r) = if e.kind != 3 {
            if receipt.is_some() || projection_ref.is_some() {
                return Err(Error::Integrity("genesis message artifacts"));
            }
            (0, a([u(1)]))
        } else {
            if projection_ref.is_none() {
                return Err(Error::Integrity("readable projection witness"));
            }
            if e.role == local {
                let path = format!(
                    "runtime/od1/contexts/{}/outbox/{:020}",
                    source::hex(&e.c),
                    e.n
                );
                let raw = reader.required(&path)?;
                let v = codec::decode(&raw, 1024)?;
                let f = v.fields(3)?;
                if f[0].uint()? != 1 || f[1].fixed::<32>()? != e.id || f[2].uint()? > 1 {
                    return Err(Error::Integrity("outbox membership"));
                }
                if f[2].uint()? == 0 {
                    if receipt.is_some() {
                        return Err(Error::Integrity("legacy receipt/delivery contradiction"));
                    }
                    (1, a([u(1)]))
                } else {
                    (
                        2,
                        receipt_capture(receipt.as_ref().map(|r| r.as_slice()), &e)?,
                    )
                }
            } else {
                (
                    3,
                    receipt_capture(receipt.as_ref().map(|r| r.as_slice()), &e)?,
                )
            }
        };
        let snapshot = codec::record(
            5,
            &a([
                u(7),
                b(profile.mid),
                b(original),
                b(e.c),
                u(e.role.into()),
                u(e.n),
                b(e.id),
                b(e.request),
                b(migration_key(profile.mid, 4, &head_entry.record_key)?.bytes),
                b(migration_key(profile.mid, 4, &native_entry.record_key)?.bytes),
                opt(projection_ref.map(b)),
                a([
                    a([u(0), b(e.original)]),
                    if e.role == local {
                        a([u(1)])
                    } else {
                        a([u(2)])
                    },
                    a([u(2)]),
                    r,
                ]),
                u(delivery),
            ]),
            4096,
        )?;
        item.snapshot = Some(snapshot);
        let f = facts
            .contexts
            .get(usize::from(c))
            .ok_or(Error::Integrity("context origin facts"))?;
        item.seen = f
            .seen
            .iter()
            .find(|(role, key, _, first)| *role == e.role && *key == e.key && *first == e.id)
            .copied();
        if item.seen.is_some_and(|s| s.2 != e.g) {
            return Err(Error::Integrity("Seen generation source"));
        }
    } else if entry.kind == 14 {
        let c = entry.context.ok_or(Error::Integrity("pending context"))?;
        let n = ordinal(&entry.path)?;
        item.pending_state = Some(
            reader
                .required(&format!(
                    "runtime/od1/contexts/{}/pending-state/{n:020}",
                    source::hex(&c)
                ))?
                .to_vec(),
        );
        // Existing captured Auth bytes pay one temporary ticket, never all
        // tickets referencing a shared bundle. Lowest source ordinal owns it.
        let mut claimed = BTreeSet::new();
        for state in reader
            .entries()
            .iter()
            .filter(|e| e.kind == 15 && e.context == Some(c))
        {
            let current = ordinal(&state.path)?;
            if current > n {
                break;
            }
            let raw = reader.required(&state.path)?;
            let value = codec::decode(&raw, 2048)?;
            let f = value.fields(8)?;
            for field in [&f[4], &f[5]] {
                let id = field.fixed::<32>()?;
                if claimed.insert(id) && current == n {
                    item.paid_auth.push(id);
                }
            }
        }
        for id in &item.paid_auth {
            let path = format!("runtime/od1/auth/{}", source::hex(id));
            let auth = reader
                .entries()
                .iter()
                .find(|e| e.path == path && e.kind == 7)
                .ok_or(Error::Integrity("captured source Auth"))?;
            item.paid_auth_sizes.push((
                *id,
                auth.length
                    .checked_add(OVERHEAD)
                    .ok_or(Error::Bound("Auth charge"))?,
            ));
        }
    } else if entry.kind == 28 {
        let id: Id = item
            .body
            .as_slice()
            .try_into()
            .map_err(|_| Error::Integrity("author ID"))?;
        let mut found = None;
        for source in reader
            .entries()
            .iter()
            .filter(|e| e.kind == 11 || e.kind == 14)
        {
            let matches = if source.kind == 11 {
                source.path.rsplit('/').next() == Some(source::hex(&id).as_str())
            } else {
                event_fields(&reader.required(&source.path)?)?.id == id
            };
            if matches {
                let c = context(
                    profile,
                    source.context.ok_or(Error::Integrity("author context"))?,
                )?;
                if found.replace(c).is_some_and(|old| old != c) {
                    return Err(Error::Integrity("author sequence across contexts"));
                }
            }
        }
        item.author_context = Some(found.ok_or(Error::Integrity("orphan author row"))?);
    }
    Ok(item)
}
fn receipt_capture(raw: Option<&[u8]>, e: &EventFields) -> Result<Value> {
    let raw = raw.ok_or(Error::Integrity("mandatory actual legacy receipt"))?;
    let v = codec::decode(raw, 1024)?;
    let f = v.fields(2)?[0].fields(8)?;
    if f[0].uint()? != 1
        || f[3].uint()? != u64::from(e.role)
        || f[4].uint()? != e.n
        || f[5].fixed::<32>()? != e.id
    {
        return Err(Error::Integrity("receipt source binding"));
    }
    Ok(a([u(0), b(f[7].fixed::<32>()?)]))
}
impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(super) fn migration_ticket_check(&mut self, profile: &Profile, c: u8) -> Result<()> {
        let slot = usize::from(c) * 24 + 9;
        if self.root.roots[slot].count > 65 {
            return Err(Error::Integrity("legacy pending bound"));
        }
        let leaves = self
            .tree
            .range(slot as u8, self.root.roots[slot], [0; 32], 66)?;
        let mut ordinary = 0;
        let mut head = None;
        let mut paid_auth = BTreeSet::new();
        for leaf in leaves {
            let key = Key {
                context: c,
                namespace: Namespace::Pending,
                bytes: leaf.prefix,
            };
            let raw = self.get(key)?.ok_or(Error::Integrity("pending ticket"))?;
            let value = codec::read_record(&raw, 13, 32 * 1024)?;
            let f = value.fields(10)?;
            if f[0].uint()? != 7
                || f[1].fixed::<32>()? != profile.mid
                || f[2].fixed::<32>()? != profile.source
                || f[3].uint()? != u64::from(c)
                || f[4].uint()? != 2
                || f[5].uint()? != key.ordinal_value()?
            {
                return Err(Error::Integrity("legacy pending ticket scope"));
            }
            let body = f[7].fields(2)?;
            let event = event_fields(body[0].bytes()?)?;
            let state = codec::decode(body[1].bytes()?, 2048)?;
            let s = state.fields(8)?;
            if s[1].fixed::<32>()? != event.id
                || s[2].uint()? > 1
                || s[3].uint()? > 1
                || s[4].fixed::<32>()? != event.original
            {
                return Err(Error::Integrity("legacy pending exact captures"));
            }
            let refs = [s[4].fixed::<32>()?, s[5].fixed::<32>()?];
            let mut expected = vec![5];
            expected.extend(refs[0]);
            expected.extend(refs[1]);
            if self
                .get(Key::hash(c, Namespace::Admission, event.id)?)?
                .as_deref()
                != Some(expected.as_slice())
            {
                return Err(Error::Integrity("pending admission link"));
            }
            let mut paid = expected.len() as u64 + OVERHEAD;
            for auth in f[9].list()? {
                let id = auth.fixed::<32>()?;
                if !refs.contains(&id) || !paid_auth.insert(id) {
                    return Err(Error::Integrity("one owner for captured Auth credit"));
                }
                let auth = self
                    .get(Key::hash(c, Namespace::Auth, id)?)?
                    .ok_or(Error::Integrity("captured Auth"))?;
                paid = paid
                    .checked_add(auth.len() as u64 + OVERHEAD)
                    .ok_or(Error::Bound("capture paid"))?;
            }
            if paid != f[8].uint()? {
                return Err(Error::Integrity("derived capture-paid charge"));
            }
            let reserved = (FRAME_CREDIT + TEMP_CREDIT)
                .checked_sub(raw.len() as u64 + OVERHEAD)
                .and_then(|n| n.checked_sub(paid))
                .ok_or(Error::Integrity("temporary reservation"))?;
            self.migration_leaf_check(key, &raw, reserved)?;
            if s[2].uint()? == 1 {
                if head.replace(event.n).is_some() {
                    return Err(Error::Integrity("one legacy head"));
                }
            } else {
                ordinary += 1;
            }
        }
        let ledger = self.root.ledger[usize::from(c)].runtime_state();
        if ledger.pending != ordinary || ledger.head != head {
            return Err(Error::Integrity("pending/head ledger correspondence"));
        }
        Ok(())
    }
    pub(super) fn migration_copy_item(&mut self, profile: &Profile, item: &CopyItem) -> Result<()> {
        let rows = ProducedRows::generate(profile, item)?;
        if let Some(c) = item
            .entry
            .context
            .map(|id| context(profile, id))
            .transpose()?
        {
            for (id, charge) in &item.paid_auth_sizes {
                let actual = self
                    .get(Key::hash(c, Namespace::Auth, *id)?)?
                    .ok_or(Error::Integrity("captured Auth missing"))?;
                if actual.len() as u64 + OVERHEAD != *charge {
                    return Err(Error::Integrity("actual captured Auth charge"));
                }
            }
        }
        for row in rows.rows {
            self.put(row.key, &row.raw, row.reserved, false)?;
        }
        if let Some((c, n, head)) = rows.pending {
            self.root.ledger[usize::from(c)].migration_import_pending(n, head)?;
        }
        Ok(())
    }
}
impl ProducedRows {
    fn lower(&mut self, profile: &Profile, item: &CopyItem) -> Result<()> {
        let entry = &item.entry;
        if raw_hash(b"", &item.body) != entry.content_hash || item.body.len() as u64 != entry.length
        {
            return Err(Error::Integrity("source copy hash/length"));
        }
        let raw_ref = codec::record(4, &entry.reference(profile.source), 1024)?;
        self.put(
            migration_key(profile.mid, 4, &entry.record_key)?,
            &raw_ref,
            0,
            false,
        )?;
        let c = entry.context.map(|id| context(profile, id)).transpose()?;
        match entry.kind {
            11 => {
                let e = event_fields(&item.body)?;
                let c = c.ok_or(Error::Integrity("event context"))?;
                self.put(Key::hash(c, Namespace::Event, e.id)?, &item.body, 0, false)?;
                let snapshot = item
                    .snapshot
                    .as_ref()
                    .ok_or(Error::Integrity("migration-only snapshot"))?;
                let id = migration_key(profile.mid, 5, &e.id)?;
                self.put(id, snapshot, 0, false)?;
                if let Some((role, key, g, first)) = item.seen {
                    if role != e.role || key != e.key || g != e.g || first != e.id {
                        return Err(Error::Integrity("Seen source"));
                    }
                    let ns = if role == 0 {
                        Namespace::SeenA
                    } else {
                        Namespace::SeenB
                    };
                    let mut raw = g.to_be_bytes().to_vec();
                    raw.extend(first);
                    self.put(Key::hash(c, ns, key)?, &raw, 0, false)?;
                }
            }
            12 => {
                let c = c.ok_or(Error::Integrity("projection context"))?;
                let id = source::hex_id(
                    entry
                        .path
                        .rsplit('/')
                        .next()
                        .ok_or(Error::Integrity("projection ID"))?,
                )?;
                let p = item
                    .projection
                    .as_ref()
                    .ok_or(Error::Integrity("verified self-HPKE projection required"))?;
                let binding = codec::encode(
                    &a([
                        u(7),
                        b(profile.mid),
                        b(profile.source),
                        b(migration_key(profile.mid, 5, &id)?.bytes),
                        b(p.request),
                    ]),
                    2304,
                )?;
                self.put(
                    Key::hash(c, Namespace::Projection, id)?,
                    &ProjectionValue {
                        text: p.text.clone(),
                        binding,
                    }
                    .encode()?,
                    0,
                    false,
                )?;
            }
            10 => self.migration_meta_put(
                Key::hash(
                    c.ok_or(Error::Integrity("Native context"))?,
                    Namespace::Native,
                    [0; 32],
                )?,
                &item.body,
                128 * 1024,
                0,
            )?,
            16 => {
                let (role, n) = lane_ordinal(&entry.path)?;
                let ns = if role == 0 {
                    Namespace::LaneA
                } else {
                    Namespace::LaneB
                };
                self.put(
                    Key::ordinal(c.ok_or(Error::Integrity("lane context"))?, ns, n)?,
                    &item.body,
                    0,
                    false,
                )?;
            }
            17 => {
                let (role, n) = lane_ordinal(&entry.path)?;
                let ns = if role == 0 {
                    Namespace::ReceiptA
                } else {
                    Namespace::ReceiptB
                };
                self.put(
                    Key::ordinal(c.ok_or(Error::Integrity("receipt context"))?, ns, n)?,
                    &item.body,
                    0,
                    false,
                )?;
            }
            20 => {
                let c = c.ok_or(Error::Integrity("request context"))?;
                let text = entry
                    .path
                    .rsplit('/')
                    .next()
                    .ok_or(Error::Integrity("request ID"))?;
                if text.len() != 32 {
                    return Err(Error::Integrity("request width"));
                }
                let mut request = [0; 16];
                for (i, x) in request.iter_mut().enumerate() {
                    *x = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
                        .map_err(|_| Error::Integrity("request hex"))?;
                }
                self.put(Key::request(c, request)?, &item.body, 0, false)?;
            }
            7 => {
                let id = source::hex_id(
                    entry
                        .path
                        .rsplit('/')
                        .next()
                        .ok_or(Error::Integrity("auth ID"))?,
                )?;
                if raw_hash(b"", &item.body) != id {
                    return Err(Error::Integrity("auth content address"));
                }
                for c in 0..profile.contexts.len() {
                    self.put(
                        Key::hash(c as u8, Namespace::Auth, id)?,
                        &item.body,
                        0,
                        false,
                    )?;
                }
            }
            13 | 14 | 19 => {
                self.migration_ticket(profile, item, c.ok_or(Error::Integrity("ticket context"))?)?
            }
            28 => {
                let c = item
                    .author_context
                    .ok_or(Error::Integrity("author owner"))?;
                let fields = entry.path.split('/').collect::<Vec<_>>();
                let signer =
                    source::hex_id(fields.get(3).ok_or(Error::Integrity("author signer"))?)?;
                let slot = profile
                    .signers
                    .iter()
                    .position(|d| *d == signer)
                    .ok_or(Error::Integrity("unregistered signer"))?;
                let ns = [Namespace::Author0, Namespace::Author1, Namespace::Author2][slot];
                let mut raw = profile.contexts[usize::from(c)].0.to_vec();
                raw.extend_from_slice(&item.body);
                self.put(Key::ordinal(c, ns, ordinal(&entry.path)?)?, &raw, 0, false)?;
            }
            0..=6 | 8 | 9 | 15 | 18 | 21..=27 | 29..=31 => {
                if let Some(c) = c
                    && !matches!(entry.kind, 9 | 15)
                {
                    self.put(
                        old_key(c, Namespace::Control, &entry.path)?,
                        &item.body,
                        0,
                        false,
                    )?;
                }
            }
            _ => return Err(Error::Integrity("unsupported copy entry")),
        }
        Ok(())
    }
    fn migration_ticket(&mut self, profile: &Profile, item: &CopyItem, c: u8) -> Result<()> {
        let e = &item.entry;
        let mut admission_paid = 0u64;
        let (ns, order, stage, reserve, body) = match e.kind {
            13 => {
                let name = e
                    .path
                    .rsplit('/')
                    .next()
                    .ok_or(Error::Integrity("queue name"))?;
                let (sequence, request) = name
                    .split_once('-')
                    .ok_or(Error::Integrity("queue order"))?;
                let sequence = sequence
                    .parse::<u64>()
                    .map_err(|_| Error::Integrity("queue order"))?;
                let mut id = [0; 16];
                if request.len() != 32 {
                    return Err(Error::Integrity("queue request"));
                }
                for (i, x) in id.iter_mut().enumerate() {
                    *x = u8::from_str_radix(&request[2 * i..2 * i + 2], 16)
                        .map_err(|_| Error::Integrity("queue request"))?;
                }
                (
                    Namespace::Queue,
                    sequence,
                    1,
                    FRAME_CREDIT,
                    a([b(id), b(&item.body[..])]),
                )
            }
            14 => {
                let event = event_fields(&item.body)?;
                let state = item
                    .pending_state
                    .as_ref()
                    .ok_or(Error::Integrity("pending admission capture"))?;
                let v = codec::decode(state, 2048)?;
                let f = v.fields(8)?;
                let head = f[2].uint()? == 1;
                if f[0].uint()? != 1
                    || f[1].fixed::<32>()? != event.id
                    || f[2].uint()? > 1
                    || f[3].uint()? > 1
                    || f[4].fixed::<32>()? != event.original
                {
                    return Err(Error::Integrity("actual pending capture"));
                }
                let refs = [f[4].fixed::<32>()?, f[5].fixed::<32>()?];
                let mut admission = vec![5];
                admission.extend(refs[0]);
                admission.extend(refs[1]);
                self.put(
                    Key::hash(c, Namespace::Admission, event.id)?,
                    &admission,
                    0,
                    false,
                )?;
                admission_paid = admission.len() as u64 + OVERHEAD;
                let mut unique = BTreeSet::new();
                for id in &item.paid_auth {
                    if !refs.contains(id) || !unique.insert(id) {
                        return Err(Error::Integrity("pending capture ownership"));
                    }
                    let charge = item
                        .paid_auth_sizes
                        .iter()
                        .find(|(key, _)| key == id)
                        .ok_or(Error::Integrity("source Auth charge"))?
                        .1;
                    admission_paid = admission_paid
                        .checked_add(charge)
                        .ok_or(Error::Bound("pending capture charge"))?;
                }
                self.pending = Some((c, event.n, head));
                (
                    Namespace::Pending,
                    event.n,
                    2,
                    FRAME_CREDIT + TEMP_CREDIT,
                    a([b(&item.body[..]), b(state)]),
                )
            }
            19 => {
                let v = codec::decode(&item.body, 1024)?;
                let f = v.fields(3)?;
                if f[0].uint()? != 1 || f[2].uint()? > 1 {
                    return Err(Error::Integrity("legacy outbox"));
                }
                if f[2].uint()? == 1 {
                    return Ok(());
                }
                (
                    Namespace::Undelivered,
                    ordinal(&e.path)?,
                    3,
                    48 * 1024,
                    b(&item.body[..]),
                )
            }
            _ => return Err(Error::Integrity("ticket kind")),
        };
        let raw = codec::record(
            13,
            &a([
                u(7),
                b(profile.mid),
                b(profile.source),
                u(c.into()),
                u(stage),
                u(order),
                b(migration_key(profile.mid, 4, &e.record_key)?.bytes),
                body,
                u(admission_paid),
                a(item.paid_auth.iter().map(b)),
            ]),
            32 * 1024,
        )?;
        let reserved = if stage == 2 {
            reserve
                .checked_sub(raw.len() as u64 + OVERHEAD)
                .and_then(|n| n.checked_sub(admission_paid))
                .ok_or(Error::Bound("legacy pending credit"))?
        } else {
            reserve
        };
        self.put(Key::ordinal(c, ns, order)?, &raw, reserved, false)
    }
}
