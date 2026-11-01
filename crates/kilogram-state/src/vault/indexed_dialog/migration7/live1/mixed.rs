//! Existing-dialog data purposes. They share paid rows with L1, but cannot
//! initialize, bootstrap, select a mode, alter cuts or replace a Native pointer.
use super::*;

#[derive(Clone, Serialize)]
pub enum ExistingData7 {
    Queue {
        request: [u8; 16],
        text_hash: Id,
        sealed: Vec<u8>,
        now: u64,
    },
    Materialize {
        change: L1NativeChange,
        rows: Vec<L1Readable>,
    },
    Pending {
        author: Id,
        raw: Vec<u8>,
        original: HistoryAuth,
        admission: HistoryAuth,
        head: bool,
        grant: Option<Id>,
    },
    Readable {
        change: L1NativeChange,
        rows: Vec<L1Readable>,
    },
    Acknowledge {
        event: Id,
        receipt: Vec<u8>,
        receiver: HistoryAuth,
    },
    Quarantine {
        ordinal: u64,
    },
    AckProgress {
        next: u64,
    },
    ObserveHead {
        next: u64,
        last: Option<Id>,
    },
    Grant {
        id: Id,
        start: u64,
        end: u64,
    },
    CancelGrant {
        id: Id,
    },
    Block {
        proof: Vec<u8>,
    },
}
impl ExistingData7 {
    fn lower(self, c: u8) -> Option<L1Operation> {
        Some(match self {
            Self::Queue {
                request,
                text_hash,
                sealed,
                now,
            } => L1Operation::Queue {
                context: c,
                request,
                text_hash,
                sealed,
                now,
            },
            Self::Materialize { change, rows } => L1Operation::Materialize { change, rows },
            Self::Pending {
                author,
                raw,
                original,
                admission,
                head,
                ..
            } => L1Operation::Pending {
                context: c,
                author,
                raw,
                original,
                admission,
                head,
            },
            Self::Readable { change, rows } => L1Operation::Drain { change, rows },
            Self::Acknowledge {
                event,
                receipt,
                receiver,
            } => L1Operation::Acknowledge {
                context: c,
                event,
                receipt,
                receiver,
            },
            Self::Quarantine { ordinal } => L1Operation::Quarantine {
                context: c,
                ordinal,
            },
            Self::AckProgress { next } => L1Operation::AckProgress { context: c, next },
            Self::ObserveHead { next, last } => L1Operation::ObserveHead {
                context: c,
                next,
                last,
            },
            Self::Block { proof } => L1Operation::Block { context: c, proof },
            Self::Grant { .. } | Self::CancelGrant { .. } => return None,
        })
    }
    pub(super) fn bound(&self, c: u8) -> Result<()> {
        if let Some(operation) = self.clone().lower(c) {
            operation.bound()?;
            if operation.context() != Some(c) {
                return Err(Error::OwnerMismatch);
            }
        }
        Ok(())
    }
}

pub(super) fn data_core7(event: &Value) -> Result<&[Value]> {
    let f = event.fields(5)?;
    let version = f[0].uint()?;
    let core = f[1].fields(match version {
        1 => 14,
        2 => 15,
        _ => return Err(Error::Integrity("data format")),
    })?;
    if core[0].uint()? != version {
        return Err(Error::Integrity("data/core version"));
    }
    Ok(core)
}

/// Structural projection only. Cryptographic and Native verification stays in
/// the sole owner; the V4 importer continues using its format1-only parser.
pub(super) fn data_event7(raw: &[u8]) -> Result<super::super::copy::EventFields> {
    let value = codec::decode(raw, 16 * 1024)?;
    let core = data_core7(&value)?;
    if value.fields(5)?[0].uint()? == 1 {
        return super::super::copy::event_fields(raw);
    }
    let header = value.fields(5)?[2].fields(3)?;
    let role = u8::try_from(core[5].uint()?).map_err(|_| Error::Integrity("data role"))?;
    if role > 1
        || core[9].uint()? != 3
        || core[6].uint()? == 0
        || core[11].list()?.len() > 2
        || value.fields(5)?[4].bytes()?.len() != 64
    {
        return Err(Error::Integrity("existing Event2 structure"));
    }
    core[14].fixed::<32>()?;
    Ok(super::super::copy::EventFields {
        format: 2,
        id: hash(b"kg:od2:event-id\0", raw),
        c: core[1].fixed()?,
        role,
        n: core[6].uint()?,
        kind: 3,
        request: core[10].fixed()?,
        original: core[13].fixed()?,
        key: header[1].fixed()?,
        g: core[12].fields(4)?[0].uint()?,
    })
}

pub(super) fn receipt7(raw: &[u8], source: &[u8], q: Option<Id>, certs: [Id; 2]) -> Result<Id> {
    let event = data_event7(source)?;
    let value = codec::decode(source, 16 * 1024)?;
    let core = data_core7(&value)?;
    let value = codec::decode(raw, 1024)?;
    let f = value.fields(2)?;
    if f[1].bytes()?.len() != 64 {
        return Err(Error::Integrity("receipt signature size"));
    }
    let r = f[0].list()?;
    match r
        .first()
        .ok_or(Error::Integrity("receipt version"))?
        .uint()?
    {
        1 => {
            f[0].fields(8)?;
            if event.format != 1
                || r[1] != core[2]
                || r[2] != core[3]
                || r[3].uint()? != u64::from(event.role)
                || r[4].uint()? != event.n
                || r[5].fixed::<32>()? != event.id
                || r[6].fixed::<32>()? != certs[1 - usize::from(event.role)]
            {
                return Err(Error::Integrity("Receipt1 exact source"));
            }
            r[7].fixed()
        }
        2 => {
            f[0].fields(12)?;
            if r[1].fixed::<32>()? != event.c
                || r[2] != core[2]
                || r[3] != core[3]
                || r[4] != core[4]
                || Some(r[5].fixed::<32>()?) != q
                || r[6].uint()? != u64::from(event.format)
                || r[7].uint()? != u64::from(event.role)
                || r[8].uint()? != event.n
                || r[9].fixed::<32>()? != event.id
                || r[10].fixed::<32>()? != certs[1 - usize::from(event.role)]
            {
                return Err(Error::Integrity("Receipt2 exact source/Q/format"));
            }
            r[11].fixed()
        }
        _ => Err(Error::Integrity("receipt version")),
    }
}

impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(in crate::vault::indexed_dialog::migration7) fn current_pending_check7(
        &mut self,
        p: &Profile,
        c: u8,
    ) -> Result<()> {
        // Ready can precede the first L1 owner initialization. That state is
        // still exactly the V4 importer domain; mixed data cannot exist there.
        let mode = self.existing_state7(c)?;
        if self.get(key(c, b"meta")?)?.is_none() {
            if mode.mode != ExistingMode::L1 {
                return Err(Error::Integrity(
                    "mixed pending before owner initialization",
                ));
            }
            return self.migration_ticket_check(p, c);
        }
        let meta = self.l1_meta(c)?;
        let certs = self.l1_cert_refs(p, c)?;
        let wrapper = self.data_head7(c)?;
        let head = super::ops::own_head(wrapper.fields(9)?[4].bytes()?)?;
        let slot = usize::from(c) * 24 + Namespace::Pending as usize;
        if self.root.roots[slot].count > 65 {
            return Err(Error::Integrity("current pending bound"));
        }
        let leaves = self
            .tree
            .range(slot as u8, self.root.roots[slot], [0; 32], 66)?;
        let mut ordinary = 0;
        let mut head_ordinal = None;
        let mut repair = 0_u64;
        let mut paid_auth = std::collections::BTreeSet::new();
        for leaf in leaves {
            let pending_key = Key {
                context: c,
                namespace: Namespace::Pending,
                bytes: leaf.prefix,
            };
            let raw = self
                .get(pending_key)?
                .ok_or(Error::Integrity("current pending ticket"))?;
            let ticket = codec::read_record(&raw, 13, 32 * 1024)?;
            let f = ticket.fields(10)?;
            let n = pending_key.ordinal_value()?;
            if f[0].uint()? != 7
                || f[1].fixed::<32>()? != p.mid
                || f[2].fixed::<32>()? != p.source
                || f[3].uint()? != u64::from(c)
                || f[4].uint()? != 2
                || f[5].uint()? != n
            {
                return Err(Error::Integrity("current pending exact ticket scope"));
            }
            let body = f[7].fields(2)?;
            let source = body[0].bytes()?;
            let event = self.l1_event_scope(p, c, source, &head)?;
            let state = codec::decode(body[1].bytes()?, 2048)?;
            let s = state.fields(8)?;
            let version = s[0].uint()?;
            if !matches!(version, 1 | 2)
                || version == 2 && !mode.mode.selected()
                || event.format == 2 && version != 2
                || event.role == p.contexts[usize::from(c)].1
                || event.kind != 3
                || event.n != n
                || n < meta.next[usize::from(event.role)]
                || s[1].fixed::<32>()? != event.id
                || s[2].uint()? > 1
                || s[3].uint()? > 1
                || s[4].fixed::<32>()? != event.original
            {
                return Err(Error::Integrity("current pending source/mode/captures"));
            }
            let grant = s[6].bytes()?;
            if !grant.is_empty() && (version != 2 || grant.len() != 32) {
                return Err(Error::Integrity("pending repair origin"));
            }
            if version == 1 && !grant.is_empty() {
                return Err(Error::Integrity("legacy pending repair origin"));
            }
            if !grant.is_empty() {
                repair = repair
                    .checked_add(FRAME_CREDIT)
                    .ok_or(Error::Bound("pending funded sum"))?;
            }
            if let Value::B(origin) = &f[6] {
                if origin.len() != 32 || event.format != 1 || version != 1 {
                    return Err(Error::Integrity("V4 pending origin format"));
                }
                let origin_key = Key {
                    context: 0,
                    namespace: Namespace::Migration,
                    bytes: f[6].fixed()?,
                };
                let record = self
                    .get(origin_key)?
                    .ok_or(Error::Integrity("pending V4 origin"))?;
                let reference = codec::read_record(&record, 4, 1024)?;
                let r = reference.fields(9)?;
                if r[0].uint()? != 1
                    || r[1].fixed::<32>()? != p.source
                    || r[4].uint()? != 14
                    || r[5].optional()?.map(Value::fixed::<32>).transpose()? != Some(event.c)
                    || r[7].fixed::<32>()? != hash(b"", source)
                    || r[8].uint()? != source.len() as u64
                    || migration_key(p.mid, 4, &r[2].fixed::<32>()?)? != origin_key
                {
                    return Err(Error::Integrity("pending exact V4 source origin"));
                }
                r[3].text()?;
                r[6].fixed::<32>()?;
            } else if !f[6].list()?.is_empty() {
                return Err(Error::Integrity("current pending origin shape"));
            }
            let refs = [s[4].fixed::<32>()?, s[5].fixed::<32>()?];
            let mut admission = vec![5];
            admission.extend(refs[0]);
            admission.extend(refs[1]);
            if self
                .get(Key::hash(c, Namespace::Admission, event.id)?)?
                .as_deref()
                != Some(admission.as_slice())
            {
                return Err(Error::Integrity("pending exact admission link"));
            }
            let mut charge = source.len() + 1024;
            for id in refs {
                let auth = self
                    .get(Key::hash(c, Namespace::Auth, id)?)?
                    .ok_or(Error::Integrity("pending captured Auth"))?;
                if hash(b"", &auth) != id || auth.len() > 16 * 1024 {
                    return Err(Error::Integrity("pending captured Auth address/bound"));
                }
                let credential = codec::decode(&auth, 16 * 1024)?;
                let fields = credential.fields(3)?;
                if fields[0].uint()? != 1
                    || hash(b"", fields[1].bytes()?) != certs[usize::from(event.role)]
                {
                    return Err(Error::Integrity("pending O/A original peer certificate"));
                }
                charge = charge
                    .checked_add(auth.len())
                    .ok_or(Error::Bound("pending admission charge"))?;
            }
            if charge > 64 * 1024 || s[7].uint()? != charge as u64 {
                return Err(Error::Integrity("pending captured admission size"));
            }
            let mut paid = admission.len() as u64 + OVERHEAD;
            for item in f[9].list()? {
                let id = item.fixed::<32>()?;
                if !refs.contains(&id) || !paid_auth.insert(id) {
                    return Err(Error::Integrity("pending Auth paid owner"));
                }
                let auth = self
                    .get(Key::hash(c, Namespace::Auth, id)?)?
                    .ok_or(Error::Integrity("pending paid Auth"))?;
                paid = paid
                    .checked_add(auth.len() as u64 + OVERHEAD)
                    .ok_or(Error::Bound("pending paid captures"))?;
            }
            if paid != f[8].uint()? {
                return Err(Error::Integrity("pending paid amount"));
            }
            let reserved = (FRAME_CREDIT + TEMP_CREDIT)
                .checked_sub(raw.len() as u64 + OVERHEAD)
                .and_then(|value| value.checked_sub(paid))
                .ok_or(Error::Integrity("pending reservation"))?;
            self.migration_leaf_check(pending_key, &raw, reserved)?;
            if s[2].uint()? == 1 {
                if head_ordinal.replace(n).is_some()
                    || n != meta.next[usize::from(event.role)]
                    || version == 2 && grant.is_empty()
                {
                    return Err(Error::Integrity("pending exact funded head"));
                }
            } else {
                ordinary += 1;
            }
        }
        let ledger = self.root.ledger[usize::from(c)].runtime_state();
        if ordinary > 64
            || ordinary != ledger.pending
            || head_ordinal != ledger.head
            || repair != ledger.outstanding
        {
            return Err(Error::Integrity(
                "current pending/repair ledger correspondence",
            ));
        }
        Ok(())
    }
    pub(super) fn data_head7(&mut self, c: u8) -> Result<Value> {
        let raw = self
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("data Head7 absent"))?;
        let value = codec::read_record(&raw, 10, 8192)?;
        self.existing_head_check7(c, &value)?;
        Ok(value)
    }
    pub(super) fn existing_data7(&mut self, p: &Profile, c: u8, data: ExistingData7) -> Result<()> {
        data.bound(c)?;
        let mode = self.existing_state7(c)?.mode;
        if matches!(&data, ExistingData7::Materialize { .. }) && mode != ExistingMode::Active {
            return Err(Error::PolicyRejected(
                "existing materialization requires Active",
            ));
        }
        match data {
            ExistingData7::Pending {
                author,
                raw,
                original,
                admission,
                head,
                grant,
            } => {
                if !mode.selected() && grant.is_some() {
                    return Err(Error::PolicyRejected("pre-Q grant"));
                }
                self.l1_pending(p, c, author, raw, original, admission, head, grant)
            }
            ExistingData7::Grant { id, start, end } => {
                let meta = self.l1_meta(c)?;
                if !mode.selected()
                    || start != meta.next[1 - usize::from(p.contexts[usize::from(c)].1)]
                {
                    return Err(Error::PolicyRejected("selected repair grant frontier"));
                }
                super::super::super::credits::apply(
                    self,
                    CreditAction::Grant {
                        context: c,
                        id,
                        start,
                        end,
                    },
                )
            }
            ExistingData7::CancelGrant { id } => {
                if !mode.selected() {
                    return Err(Error::PolicyRejected("pre-Q cancel grant"));
                }
                super::super::super::credits::apply(
                    self,
                    CreditAction::CancelGrant { context: c, id },
                )
            }
            other => self.l1_apply(
                other
                    .lower(c)
                    .ok_or(Error::Integrity("closed existing data purpose"))?,
            ),
        }
    }
}
