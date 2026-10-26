use super::*;

impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(super) fn l1_auth(&mut self, c: u8, auth: &HistoryAuth) -> Result<u64> {
        auth.check_bound()?;
        if hash(b"", &auth.bytes) != auth.id {
            return Err(Error::Integrity("L1 auth content address"));
        }
        let k = Key::hash(c, Namespace::Auth, auth.id)?;
        let paid = if self.get(k)?.is_none() {
            auth.bytes.len() as u64 + OVERHEAD1
        } else {
            0
        };
        self.put(k, &auth.bytes, 0, false)?;
        Ok(paid)
    }
    pub(super) fn l1_leaf_reserved(&mut self, k: Key) -> Result<u64> {
        let slot = k.slot()?;
        let leaf = self
            .tree
            .find(slot as u8, self.root.roots[slot], k.bytes)?
            .ok_or(Error::Integrity("required L1 ticket leaf"))?;
        if let tree::Body::Leaf { reserved, .. } = leaf.body {
            Ok(reserved)
        } else {
            Err(Error::Integrity("L1 ticket leaf"))
        }
    }
    #[allow(clippy::too_many_arguments)] // Ticket fields remain explicit at each closed operation.
    pub(super) fn l1_ticket(
        &self,
        p: &Profile,
        c: u8,
        stage: u64,
        order: u64,
        body: Value,
        paid: u64,
        auth: Vec<Id>,
    ) -> Result<Vec<u8>> {
        // [] distinguishes a real post-Ready obligation from an imported V4Ref.
        codec::record(
            13,
            &a([
                u(7),
                b(p.mid),
                b(p.source),
                u(c.into()),
                u(stage),
                u(order),
                a([]),
                body,
                u(paid),
                a(auth.into_iter().map(b)),
            ]),
            32 * 1024,
        )
    }
    fn l1_author(&mut self, p: &Profile, c: u8, author: Id, sequence: u64, id: Id) -> Result<()> {
        let slot = p
            .signers
            .iter()
            .position(|d| *d == author)
            .ok_or(Error::OwnerMismatch)?;
        let ns = [Namespace::Author0, Namespace::Author1, Namespace::Author2][slot];
        let mut raw = p.contexts[usize::from(c)].0.to_vec();
        raw.extend(id);
        self.put(Key::ordinal(c, ns, sequence)?, &raw, 0, false)
    }
    fn l1_change_check(&mut self, p: &Profile, change: &L1NativeChange) -> Result<Value> {
        let c = change.context;
        if self.l1_control(p, c, L1Control::BlockedFork)?.is_some() {
            return Err(Error::PolicyRejected("signed fork blocked"));
        }
        let wrapper = self.l1_head(c)?;
        let old = own_head(wrapper.fields(9)?[4].bytes()?)?;
        let old_f = old.fields(13)?;
        let next = own_head(&change.head)?;
        let n = next.fields(13)?;
        if change.wall_time < self.l1_meta(c)?.wall_high_water {
            return Err(Error::PolicyRejected("clock rollback before Native commit"));
        }
        if hash(b"", wrapper.fields(9)?[4].bytes()?) != change.expected_head
            || self.l1_meta(c)?.policy_generation != change.policy_generation
            || n[0].uint()? != 1
            || n[1] != old_f[1]
            || n[2] != old_f[2]
            || n[3] != old_f[3]
            || n[4].uint()?
                != old_f[4]
                    .uint()?
                    .checked_add(1)
                    .ok_or(Error::Bound("Head generation"))?
            || n[5].uint()? < old_f[5].uint()?
            || n[5].uint()? > 3
            || n[6].uint()?
                != old_f[6]
                    .uint()?
                    .checked_add(u64::from(change.native.is_some()))
                    .ok_or(Error::Bound("Native generation"))?
        {
            return Err(Error::Conflict);
        }
        for i in [7, 8, 9, 10, 11] {
            if old_f[i].optional()?.is_some() && old_f[i] != n[i] {
                return Err(Error::Conflict);
            }
        }
        if let Some(blob) = &change.native {
            if n[12].optional()?.map(Value::fixed::<32>).transpose()?
                != Some(hash(b"kg:od1:native-state\0", blob))
            {
                return Err(Error::Integrity("Native after hash"));
            }
        } else if n[12] != old_f[12] {
            return Err(Error::Integrity("metadata must preserve Native"));
        }
        Ok(old)
    }
    fn l1_change_finish(&mut self, change: &L1NativeChange) -> Result<()> {
        let mut wrapper = self.l1_head(change.context)?.fields(9)?.to_vec();
        wrapper[4] = b(&change.head);
        let head = own_head(&change.head)?;
        wrapper[5] = head.fields(13)?[12].clone();
        let raw = codec::record(10, &a(wrapper), 8192)?;
        self.put(
            head_key(change.context)?,
            &raw,
            (8192 - raw.len()) as u64,
            true,
        )?;
        if let Some(native) = &change.native {
            self.put(
                Key::hash(change.context, Namespace::Native, [0; 32])?,
                native,
                (128 * 1024 - native.len()) as u64,
                true,
            )?;
        }
        let mut meta = self.l1_meta(change.context)?;
        meta.wall_high_water = meta.wall_high_water.max(change.wall_time);
        self.l1_meta_put(change.context, &meta)?;
        Ok(())
    }
    fn l1_event_scope(
        &mut self,
        p: &Profile,
        c: u8,
        raw: &[u8],
        head: &Value,
    ) -> Result<super::super::super::copy::EventFields> {
        let event = super::super::super::copy::event_fields(raw)?;
        let v = codec::decode(raw, 16 * 1024)?;
        let f = v.fields(5)?;
        let core = f[1].fields(14)?;
        let h = head.fields(13)?;
        if event.c != p.contexts[usize::from(c)].0
            || f[4].bytes()?.len() != 64
            || h[9].optional()?.map(Value::fixed::<32>).transpose()? != Some(core[2].fixed()?)
            || h[10].optional()?.map(Value::fixed::<32>).transpose()? != Some(core[3].fixed()?)
            || h[11].optional()?.map(Value::bytes).transpose()? != Some(core[4].bytes()?)
        {
            return Err(Error::Integrity("L1 Event/Head scope"));
        }
        Ok(event)
    }
    fn l1_readable(&mut self, p: &Profile, c: u8, row: L1Readable, head: &Value) -> Result<()> {
        let e = self.l1_event_scope(p, c, &row.source, head)?;
        let value = codec::decode(&row.source, 16 * 1024)?;
        let f = value.fields(5)?;
        let core = f[1].fields(14)?;
        let mut meta = self.l1_meta(c)?;
        let role = usize::from(e.role);
        let local = p.contexts[usize::from(c)].1;
        check_capture_shape(
            e.role == local,
            e.kind == 3,
            [
                row.admission.is_some(),
                row.final_auth.is_some(),
                row.receiver.is_some(),
                row.receipt.is_some(),
            ],
        )?;
        if e.n != meta.next[role]
            || core[7].optional()?.map(Value::fixed::<32>).transpose()? != meta.tails[role]
            || e.original != row.original.id
        {
            return Err(Error::Integrity("L1 readable exact frontier/O"));
        }
        for parent in core[11].list()? {
            let id = parent.fixed()?;
            let raw = self
                .get(Key::hash(c, Namespace::Event, id)?)?
                .ok_or(Error::Integrity("readable parent missing"))?;
            let parent = super::super::super::copy::event_fields(&raw)?;
            if parent.c != e.c
                || parent.kind != 3
                || parent.n >= meta.next[usize::from(parent.role)]
            {
                return Err(Error::Integrity("readable parent scope/frontier"));
            }
        }
        if e.role == local {
            if row.author != p.device
                || row.admission.is_some()
                || row.final_auth.is_none()
                || row.receiver.is_some()
                || row.receipt.is_some()
            {
                return Err(Error::Integrity("own L1 capture shape"));
            }
            let mut global = self.l1_global()?;
            if core[8].uint()? != global.next_author {
                return Err(Error::Conflict);
            }
            global.next_author = global
                .next_author
                .checked_add(1)
                .ok_or(Error::Bound("author sequence exhausted"))?;
            self.l1_global_put(&global)?;
        } else if row.author == p.device
            || row.admission.is_none()
            || row.final_auth.is_none()
            || e.kind == 3 && (row.receiver.is_none() || row.receipt.is_none())
        {
            return Err(Error::Integrity("peer L1 capture shape"));
        }
        self.l1_author(p, c, row.author, core[8].uint()?, e.id)?;
        self.l1_auth(c, &row.original)?;
        for capture in [&row.admission, &row.final_auth, &row.receiver]
            .into_iter()
            .flatten()
        {
            self.l1_auth(c, capture)?;
        }
        self.put(Key::hash(c, Namespace::Event, e.id)?, &row.source, 0, false)?;
        self.put(
            super::super::super::super::history::lane(c, e.role, e.n)?,
            &e.id,
            0,
            false,
        )?;
        let seen_ns = if e.role == 0 {
            Namespace::SeenA
        } else {
            Namespace::SeenB
        };
        let seen_key = Key::hash(c, seen_ns, e.key)?;
        let index = core[12].fields(4)?[1].uint()?;
        if let Some(old) = self.get(seen_key)? {
            if old.len() != 40 || old[..8] != e.g.to_be_bytes() || index == 0 && old[8..] != e.id {
                return Err(Error::Integrity("Seen1 correspondence"));
            }
        } else {
            if index != 0 {
                return Err(Error::Integrity("unseen nonfirst readable key"));
            }
            let mut raw = e.g.to_be_bytes().to_vec();
            raw.extend(e.id);
            self.put(seen_key, &raw, 0, false)?;
        }
        let proof_key = Key::hash(
            c,
            Namespace::Control,
            hash(b"kg:od1:full-L1-proof/v7\0", &e.id),
        )?;
        let proof = codec::record(
            41,
            &a([
                u(7),
                b(p.mid),
                b(e.c),
                u(e.role.into()),
                u(e.n),
                b(e.id),
                b(row.original.id),
                opt(row.admission.as_ref().map(|v| b(v.id))),
                opt(row.final_auth.as_ref().map(|v| b(v.id))),
                opt(row.receiver.as_ref().map(|v| b(v.id))),
                opt(row.receipt.as_ref().map(|v| b(hash(b"", v)))),
            ]),
            4096,
        )?;
        self.put(proof_key, &proof, 0, false)?;
        if e.kind == 3 {
            let text = row
                .text
                .ok_or(Error::Integrity("Text projection missing"))?;
            let binding = codec::encode(
                &a([
                    u(7),
                    b(p.mid),
                    b(p.source),
                    a([u(1), b(proof_key.bytes)]),
                    b(e.request),
                ]),
                2304,
            )?;
            self.put(
                Key::hash(c, Namespace::Projection, e.id)?,
                &ProjectionValue {
                    text: text.clone(),
                    binding,
                }
                .encode()?,
                0,
                false,
            )?;
            if e.role == local {
                if self.root.roots[usize::from(c) * 24 + Namespace::Undelivered as usize].count
                    >= L1_MAX_UNACKED
                {
                    return Err(Error::Quota);
                }
                let queue = row
                    .queue
                    .ok_or(Error::Integrity("own Text has no accepted queue"))?;
                let raw = self
                    .get(Key::ordinal(c, Namespace::Queue, queue)?)?
                    .ok_or(Error::Integrity("materialized queue missing"))?;
                let ticket = codec::read_record(&raw, 13, 32 * 1024)?;
                if ticket.fields(10)?[7].fields(2)?[0].fixed::<16>()? != e.request {
                    return Err(Error::Conflict);
                }
                self.remove(Key::ordinal(c, Namespace::Queue, queue)?)?;
                let debt = self.l1_ticket(
                    p,
                    c,
                    3,
                    e.n,
                    b(codec::encode(&a([u(1), b(e.id), u(0)]), 1024)?),
                    0,
                    Vec::new(),
                )?;
                self.put(
                    Key::ordinal(c, Namespace::Undelivered, e.n)?,
                    &debt,
                    48 * 1024,
                    false,
                )?;
                self.put(
                    Key::request(c, e.request)?,
                    &packed(&L1Request {
                        queue_sequence: Some(queue),
                        event: Some(e.id),
                        text_hash: hash(b"kg:od1:request-text\0", text.as_bytes()),
                    })?,
                    0,
                    true,
                )?;
            } else {
                let raw = self
                    .get(Key::ordinal(c, Namespace::Pending, e.n)?)?
                    .ok_or(Error::Integrity("readable pending missing"))?;
                let ticket = codec::read_record(&raw, 13, 32 * 1024)?;
                let state = codec::decode(ticket.fields(10)?[7].fields(2)?[1].bytes()?, 2048)?;
                let s = state.fields(8)?;
                if s[1].fixed::<32>()? != e.id
                    || s[3].uint()? != 0
                    || s[4].fixed::<32>()? != row.original.id
                    || s[5].fixed::<32>()?
                        != row
                            .admission
                            .as_ref()
                            .ok_or(Error::Integrity("saved admission"))?
                            .id
                {
                    return Err(Error::Integrity("pending actual O/A/quarantine"));
                }
                self.remove(Key::ordinal(c, Namespace::Pending, e.n)?)?;
                self.root.ledger[usize::from(c)].legacy1_retire_pending(e.n, s[2].uint()? == 1)?;
                let receipt = row
                    .receipt
                    .as_ref()
                    .ok_or(Error::Integrity("readable receipt missing"))?;
                self.l1_receipt(
                    c,
                    &e,
                    &row.source,
                    receipt,
                    row.receiver
                        .as_ref()
                        .ok_or(Error::Integrity("receiver capture missing"))?,
                )?;
            }
        } else if row.text.is_some() || row.queue.is_some() || row.receipt.is_some() {
            return Err(Error::Integrity("genesis capture/debt shape"));
        }
        meta.next[role] = meta.next[role]
            .checked_add(1)
            .ok_or(Error::Bound("L1 frontier"))?;
        meta.tails[role] = Some(e.id);
        self.l1_meta_put(c, &meta)?;
        Ok(())
    }
    pub(super) fn l1_native_rows(
        &mut self,
        p: &Profile,
        change: L1NativeChange,
        rows: Vec<L1Readable>,
        own: bool,
    ) -> Result<()> {
        if own
            && self.root.roots[usize::from(change.context) * 24 + Namespace::Undelivered as usize]
                .count
                .checked_add(rows.len() as u64)
                .is_none_or(|n| n > L1_MAX_UNACKED)
        {
            return Err(Error::Quota);
        }
        self.l1_change_check(p, &change)?;
        let c = change.context;
        let after = own_head(&change.head)?;
        if change.native.is_none()
            || after.fields(13)?[5].uint()? != 3
            || self.l1_control(p, c, L1Control::Ready(0))?.is_none()
            || self.l1_control(p, c, L1Control::Ready(1))?.is_none()
        {
            return Err(Error::PolicyRejected("NeedHandshake"));
        }
        for row in rows {
            let e = super::super::super::copy::event_fields(&row.source)?;
            if e.kind != 3 || (e.role == p.contexts[usize::from(c)].1) != own {
                return Err(Error::Integrity("L1 native purpose/role"));
            }
            self.l1_readable(p, c, row, &after)?;
        }
        self.l1_change_finish(&change)
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the closed Bootstrap operation, not a generic writer.
    pub(super) fn l1_bootstrap(
        &mut self,
        p: &Profile,
        kind: L1Bootstrap,
        change: L1NativeChange,
        pair: Option<Vec<u8>>,
        controls: Vec<Vec<u8>>,
        rows: Vec<L1Readable>,
        ready: Vec<(u8, Vec<u8>)>,
    ) -> Result<()> {
        let old = self.l1_change_check(p, &change)?;
        let before = old.fields(13)?;
        let next = own_head(&change.head)?;
        let after = next.fields(13)?;
        let c = change.context;
        let phase = (before[5].uint()?, after[5].uint()?, before[3].uint()?);
        let valid = match kind {
            L1Bootstrap::Offer => phase == (0, 1, 0) && rows.is_empty(),
            L1Bootstrap::Grant => phase == (0, 2, 1) && rows.is_empty(),
            L1Bootstrap::Init => phase == (1, 3, 0) && rows.len() == 1,
            L1Bootstrap::Confirm => phase == (2, 3, 1) && rows.len() == 2,
            L1Bootstrap::ReceiveConfirm => phase == (3, 3, 0) && rows.len() == 1,
            L1Bootstrap::Ready => {
                phase.0 == 3 && phase.1 == 3 && rows.is_empty() && change.native.is_none()
            }
        };
        if !valid || !matches!(kind, L1Bootstrap::Ready) && change.native.is_none() {
            return Err(Error::Integrity(
                "bootstrap legal phase/constructor boundary",
            ));
        }
        if let Some(pair) = pair {
            if let Some(old) = self.l1_control(p, c, L1Control::InitialPair)?
                && old != pair
            {
                return Err(Error::Conflict);
            }
            self.l1_current_put(
                c,
                L1Control::InitialPair.name()?.as_bytes(),
                &pair,
                192 * 1024,
            )?;
        }
        for control in controls {
            let id = hash(b"kg:od1:control-id\0", &control);
            self.put(Key::hash(c, Namespace::Control, id)?, &control, 0, false)?;
        }
        for row in rows {
            if super::super::super::copy::event_fields(&row.source)?.kind == 3 {
                return Err(Error::Integrity("bootstrap Text forbidden"));
            }
            self.l1_readable(p, c, row, &next)?;
        }
        for (role, raw) in ready {
            if let Some(old) = self.l1_control(p, c, L1Control::Ready(role))?
                && old != raw
            {
                return Err(Error::Conflict);
            }
            self.l1_current_put(c, L1Control::Ready(role).name()?.as_bytes(), &raw, 8192)?;
        }
        self.l1_change_finish(&change)?;
        let meta = self.l1_meta(c)?;
        if meta.next.iter().all(|n| *n > 0)
            && self.l1_control(p, c, L1Control::Ready(0))?.is_some()
            && self.l1_control(p, c, L1Control::Ready(1))?.is_some()
        {
            self.l1_initialize_history(p, c)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the closed Possess operation and its paid captures.
    pub(super) fn l1_pending(
        &mut self,
        p: &Profile,
        c: u8,
        author: Id,
        raw: Vec<u8>,
        original: HistoryAuth,
        admission: HistoryAuth,
        head: bool,
    ) -> Result<()> {
        let wrapper = self.l1_head(c)?;
        let inner = own_head(wrapper.fields(9)?[4].bytes()?)?;
        let e = self.l1_event_scope(p, c, &raw, &inner)?;
        let meta = self.l1_meta(c)?;
        if e.role == p.contexts[usize::from(c)].1
            || e.kind != 3
            || e.n < meta.next[usize::from(e.role)]
            || original.id != e.original
            || head && e.n != meta.next[usize::from(e.role)]
        {
            return Err(Error::Integrity("pending scope/frontier/head"));
        }
        let k = Key::ordinal(c, Namespace::Pending, e.n)?;
        if let Some(old) = self.get(k)? {
            let value = codec::read_record(&old, 13, 32 * 1024)?;
            if value.fields(10)?[7].fields(2)?[0].bytes()? != raw {
                return Err(Error::Conflict);
            }
            return Ok(());
        }
        let value = codec::decode(&raw, 16 * 1024)?;
        self.l1_author(
            p,
            c,
            author,
            value.fields(5)?[1].fields(14)?[8].uint()?,
            e.id,
        )?;
        let mut paid_auth = Vec::new();
        let mut paid = 65 + OVERHEAD1;
        for auth in [&original, &admission] {
            let charge = self.l1_auth(c, auth)?;
            if charge > 0 {
                paid_auth.push(auth.id);
                paid = paid
                    .checked_add(charge)
                    .ok_or(Error::Bound("capture charge"))?;
            }
        }
        let mut admission_row = vec![5];
        admission_row.extend(original.id);
        admission_row.extend(admission.id);
        self.put(
            Key::hash(c, Namespace::Admission, e.id)?,
            &admission_row,
            0,
            false,
        )?;
        let charge = raw.len() + original.bytes.len() + admission.bytes.len() + 1024;
        if charge > 64 * 1024 {
            return Err(Error::Bound("L1 admission item64KiB"));
        }
        let state = codec::encode(
            &a([
                u(1),
                b(e.id),
                u(u64::from(head)),
                u(0),
                b(original.id),
                b(admission.id),
                b([]),
                u(charge as u64),
            ]),
            2048,
        )?;
        let ticket = self.l1_ticket(p, c, 2, e.n, a([b(raw), b(state)]), paid, paid_auth)?;
        let reserved = (FRAME_CREDIT + TEMP_CREDIT)
            .checked_sub(ticket.len() as u64 + OVERHEAD1)
            .and_then(|n| n.checked_sub(paid))
            .ok_or(Error::Bound("L1 pending reservation"))?;
        self.put(k, &ticket, reserved, false)?;
        self.root.ledger[usize::from(c)].legacy1_admit_pending(e.n, head)?;
        Ok(())
    }
    fn l1_receipt(
        &mut self,
        c: u8,
        e: &super::super::super::copy::EventFields,
        source: &[u8],
        raw: &[u8],
        receiver: &HistoryAuth,
    ) -> Result<()> {
        let value = codec::decode(raw, 1024)?;
        let f = value.fields(2)?;
        let receipt = f[0].fields(8)?;
        let source = codec::decode(source, 16 * 1024)?;
        let core = source.fields(5)?[1].fields(14)?;
        if receipt[0].uint()? != 1
            || receipt[1] != core[2]
            || receipt[2] != core[3]
            || receipt[3].uint()? != u64::from(e.role)
            || receipt[4].uint()? != e.n
            || receipt[5].fixed::<32>()? != e.id
            || receipt[7].fixed::<32>()? != receiver.id
            || f[1].bytes()?.len() != 64
        {
            return Err(Error::Integrity("Receipt1 exact source/capture"));
        }
        self.l1_auth(c, receiver)?;
        self.put(
            super::super::super::super::history::receipt(c, e.role, e.n)?,
            raw,
            0,
            false,
        )
    }
    pub(super) fn l1_ack(
        &mut self,
        p: &Profile,
        c: u8,
        id: Id,
        receipt: Vec<u8>,
        receiver: HistoryAuth,
    ) -> Result<()> {
        let raw = self
            .get(Key::hash(c, Namespace::Event, id)?)?
            .ok_or(Error::Integrity("ACK unknown source"))?;
        let e = super::super::super::copy::event_fields(&raw)?;
        if e.kind != 3 || e.role != p.contexts[usize::from(c)].1 {
            return Err(Error::Integrity("ACK source kind/role"));
        }
        let receipt_key = super::super::super::super::history::receipt(c, e.role, e.n)?;
        if let Some(old) = self.get(receipt_key)? {
            if old != receipt {
                return Err(Error::Conflict);
            }
            return Ok(());
        }
        let debt_key = Key::ordinal(c, Namespace::Undelivered, e.n)?;
        let debt = self
            .get(debt_key)?
            .ok_or(Error::Integrity("exact ACK debt missing"))?;
        let ticket = codec::read_record(&debt, 13, 32 * 1024)?;
        let old = codec::decode(ticket.fields(10)?[7].bytes()?, 1024)?;
        if old.fields(3)?[1].fixed::<32>()? != id {
            return Err(Error::Integrity("ACK debt ID"));
        }
        self.l1_receipt(c, &e, &raw, &receipt, &receiver)?;
        self.remove(debt_key)
    }
}
