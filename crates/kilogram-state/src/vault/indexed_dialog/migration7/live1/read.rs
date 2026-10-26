use super::*;

impl L1Repository {
    /// Read one authenticated primary witness through its immutable V4Ref. No index scan.
    fn original_point(&mut self, path: &str, kind: u8, cap: usize) -> Result<Option<Vec<u8>>> {
        let address = self.core.vault.record_key(path);
        let Some(raw) = self
            .core
            .get(migration_key(self.profile.mid, 4, &address)?)?
        else {
            return Ok(None);
        };
        let value = codec::read_record(&raw, 4, 1024)?;
        let f = value.fields(9)?;
        if f[0].uint()? != 1
            || f[1].fixed::<32>()? != self.profile.source
            || f[2].fixed::<32>()? != address
            || f[3].text()? != path
            || f[4].uint()? != u64::from(kind)
            || f[8].uint()? > cap as u64
        {
            return Err(Error::Integrity("cold primary witness scope/cap"));
        }
        let tx = self.core.vault.database.begin_read().map_err(db)?;
        let table = tx.open_table(crate::vault::RECORD_TABLE).map_err(db)?;
        let cipher = table
            .get(address.as_slice())
            .map_err(db)?
            .ok_or(Error::Integrity("cold primary witness missing"))?;
        if cipher.value().len() > cap + 512 || hash(b"", cipher.value()) != f[6].fixed::<32>()? {
            return Err(Error::Integrity("cold primary cipher changed"));
        }
        let record = self.core.vault.decrypt_record(cipher.value(), &address)?;
        if record.relative_path != path
            || record.content.len() as u64 != f[8].uint()?
            || hash(b"", &record.content) != f[7].fixed::<32>()?
        {
            return Err(Error::Integrity("cold primary content changed"));
        }
        Ok(Some(record.content))
    }
    pub(super) fn occupied(&self, c: u8) -> Result<Id> {
        self.profile
            .contexts
            .get(usize::from(c))
            .map(|p| p.0)
            .ok_or(Error::Bound("unoccupied L1 slot"))
    }
    pub fn owner_material(&mut self) -> Result<L1OwnerMaterial> {
        Ok(L1OwnerMaterial {
            account: self.core.root.account,
            nonce: self.core.root.claim,
            incarnation: self.core.root.incarnation,
            signing: self.core.root.signing.clone(),
            encryption: self.core.root.encryption.clone(),
            certificate: self
                .original_point("device-certificate.cert", 4, 16 * 1024)?
                .ok_or(Error::Integrity("saved certificate"))?,
        })
    }
    pub fn initialized(&mut self) -> Result<bool> {
        Ok(self.core.get(key(0, b"global")?)?.is_some())
    }
    pub fn initialize(
        &mut self,
        accounts: Vec<[Id; 2]>,
        authorities: Vec<L1Authority>,
    ) -> Result<CommitReport> {
        let mut expected = BTreeSet::from([self.core.root.account]);
        for pair in &accounts {
            expected.extend(pair);
        }
        if authorities.len() != expected.len() {
            return Err(Error::Bound("initial authority set"));
        }
        for authority in &authorities {
            if !expected.remove(&authority.account)
                || authority.raw != self.authority(authority.account)?
            {
                return Err(Error::Integrity("original authority pin binding"));
            }
        }
        let next_author = self.next_author_sequence()?;
        self.commit(
            L1Operation::Initialize(L1Initialization {
                accounts,
                next_author,
                authorities,
            }),
            None,
        )
    }
    pub fn spec(&mut self, c: u8) -> Result<Vec<u8>> {
        let context = self.occupied(c)?;
        self.core
            .get(original_control(c, context, "spec")?)?
            .ok_or(Error::Integrity("copied L1 Spec"))
    }
    pub fn snapshot(&mut self, c: u8) -> Result<L1Snapshot> {
        self.occupied(c)?;
        let raw = self
            .core
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("Head7"))?;
        let value = codec::read_record(&raw, 10, 8192)?;
        let f = value.fields(9)?;
        if f[6].uint()? != 1
            || f[7].optional()?.is_some()
            || f[8]
                .fields(2)?
                .iter()
                .any(|v| !v.optional().is_ok_and(|o| o.is_none()))
        {
            return Err(Error::UnsupportedRuntimeProfile);
        }
        let head = f[4].bytes()?.to_vec();
        let native = self.core.get(Key::hash(c, Namespace::Native, [0; 32])?)?;
        if f[5].optional()?.map(Value::fixed::<32>).transpose()?
            != native.as_ref().map(|n| hash(b"kg:od1:native-state\0", n))
        {
            return Err(Error::Integrity("current Native pointer"));
        }
        let meta = unpack(
            &self
                .core
                .get(key(c, b"meta")?)?
                .ok_or(Error::Integrity("L1 owner not initialized"))?,
        )?;
        Ok(L1Snapshot { head, native, meta })
    }
    pub fn head(&mut self, c: u8) -> Result<Vec<u8>> {
        self.occupied(c)?;
        let raw = self
            .core
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("Head7 missing"))?;
        Ok(codec::read_record(&raw, 10, 8192)?.fields(9)?[4]
            .bytes()?
            .to_vec())
    }
    pub fn metadata(&mut self, c: u8) -> Result<L1Metadata> {
        self.occupied(c)?;
        unpack(
            &self
                .core
                .get(key(c, b"meta")?)?
                .ok_or(Error::Integrity("L1 metadata"))?,
        )
    }
    pub fn seen(&mut self, c: u8, role: u8, id: Id) -> Result<Option<Vec<u8>>> {
        self.occupied(c)?;
        let ns = match role {
            0 => Namespace::SeenA,
            1 => Namespace::SeenB,
            _ => return Err(Error::Bound("seen role")),
        };
        self.core.get(Key::hash(c, ns, id)?)
    }
    pub fn seen_count(&self, c: u8, role: u8) -> Result<u64> {
        self.occupied(c)?;
        if role > 1 {
            return Err(Error::Bound("seen role"));
        }
        Ok(self.core.root.roots[usize::from(c) * 24 + 10 + usize::from(role)].count)
    }
    pub fn queue_at(&mut self, c: u8, n: u64) -> Result<L1Queue> {
        self.occupied(c)?;
        let raw = self
            .core
            .get(Key::ordinal(c, Namespace::Queue, n)?)?
            .ok_or(Error::Integrity("queue ticket missing"))?;
        let value = codec::read_record(&raw, 13, 32 * 1024)?;
        let f = value.fields(10)?;
        let body = f[7].fields(2)?;
        Ok(L1Queue {
            sequence: n,
            request: body[0].fixed()?,
            sealed: body[1].bytes()?.to_vec(),
        })
    }
    pub fn authored(&mut self, signer: Id, sequence: u64) -> Result<Option<(Id, Id)>> {
        let slot = self
            .profile
            .signers
            .iter()
            .position(|d| *d == signer)
            .ok_or(Error::OwnerMismatch)?;
        let ns = [Namespace::Author0, Namespace::Author1, Namespace::Author2][slot];
        self.core
            .get(Key::ordinal(0, ns, sequence)?)?
            .map(|raw| {
                if raw.len() != 64 {
                    return Err(Error::Integrity("author row"));
                }
                Ok((
                    raw[..32]
                        .try_into()
                        .map_err(|_| Error::Integrity("author C"))?,
                    raw[32..]
                        .try_into()
                        .map_err(|_| Error::Integrity("author ID"))?,
                ))
            })
            .transpose()
    }
    pub fn requires_reopen(&self) -> bool {
        self.core.poisoned
    }
    pub fn config_revision(&self) -> u64 {
        self.profile.plan_revision
    }
    pub fn control(&mut self, c: u8, control: L1Control) -> Result<Option<Vec<u8>>> {
        let context = self.occupied(c)?;
        let name = control.name()?;
        if let Some(value) = self.core.get(key(c, name.as_bytes())?)? {
            return Ok(Some(value));
        }
        self.core.get(original_control(c, context, &name)?)
    }
    pub fn signed_control(&mut self, c: u8, id: Id) -> Result<Vec<u8>> {
        let context = self.occupied(c)?;
        if let Some(value) = self.core.get(Key::hash(c, Namespace::Control, id)?)? {
            return Ok(value);
        }
        self.core
            .get(original_control(
                c,
                context,
                &format!("controls/{}", super::super::source::hex(&id)),
            )?)?
            .ok_or(Error::Integrity("selected L1 control"))
    }
    pub fn authority(&mut self, account: Id) -> Result<Vec<u8>> {
        let mut name = b"authority/".to_vec();
        name.extend(account);
        if let Some(raw) = self.core.get(key(0, &name)?)? {
            return Ok(unpack::<L1Authority>(&raw)?.raw);
        }
        let (path, kind) = if account == self.core.root.account {
            ("account-authority.snapshot".to_owned(), 5)
        } else {
            (
                format!(
                    "peer-authority/{}.snapshot",
                    super::super::source::hex(&account)
                ),
                31,
            )
        };
        self.original_point(&path, kind, 16 * 1024)?
            .ok_or(Error::Integrity("registered authority pin"))
    }
    pub fn next_author_sequence(&mut self) -> Result<u64> {
        if let Some(raw) = self.core.get(key(0, b"global")?)? {
            return Ok(unpack::<L1Global>(&raw)?.next_author);
        }
        match self.original_point("next-sequence", 6, 32)? {
            Some(raw) => std::str::from_utf8(&raw)
                .map_err(|_| Error::Integrity("next sequence UTF8"))?
                .trim()
                .parse()
                .map_err(|_| Error::Integrity("next sequence integer")),
            None => Ok(0),
        }
    }
    pub fn event(&mut self, c: u8, id: Id) -> Result<Option<Vec<u8>>> {
        self.occupied(c)?;
        self.core.get(Key::hash(c, Namespace::Event, id)?)
    }
    /// One immutable full-L1 witness plus the separate actual receipt overlay.
    /// Imported snapshots deliberately do not become full-L1 proof objects.
    pub fn full_l1_proof(&mut self, c: u8, id: Id) -> Result<Option<L1ProofRefs>> {
        let context = self.occupied(c)?;
        let source = self
            .event(c, id)?
            .ok_or(Error::Integrity("proof source absent"))?;
        let event = super::super::copy::event_fields(&source)?;
        if event.c != context || event.id != id {
            return Err(Error::Integrity("proof source scope"));
        }
        let event_value = codec::decode(&source, 16 * 1024)?;
        let event_core = event_value.fields(5)?[1].fields(14)?;
        let key = Key::hash(
            c,
            Namespace::Control,
            hash(b"kg:od1:full-L1-proof/v7\0", &id),
        )?;
        let Some(raw) = self.core.get(key)? else {
            if event.kind == 3 {
                let projection = self.projection(c, id)?;
                let binding = codec::decode(&projection.binding, 2304)?;
                if matches!(&binding.fields(5)?[3], Value::A(_)) {
                    return Err(Error::Integrity("full L1 proof absent"));
                }
            }
            return Ok(None);
        };
        let value = codec::read_record(&raw, 41, 4096)?;
        let f = value.fields(11)?;
        if event.c != context
            || event.id != id
            || f[0].uint()? != 7
            || f[1].fixed::<32>()? != self.profile.mid
            || f[2].fixed::<32>()? != context
            || f[3].uint()? != u64::from(event.role)
            || f[4].uint()? != event.n
            || f[5].fixed::<32>()? != id
            || f[6].fixed::<32>()? != event.original
        {
            return Err(Error::Integrity("full L1 proof exact source"));
        }
        let optional =
            |v: &Value| -> Result<Option<Id>> { v.optional()?.map(Value::fixed::<32>).transpose() };
        let mut proof = L1ProofRefs {
            original: event.original,
            admission: optional(&f[7])?,
            final_auth: optional(&f[8])?,
            receiver: optional(&f[9])?,
            receipt: optional(&f[10])?,
            receipt_overlay: None,
            receiver_overlay: None,
        };
        check_capture_shape(
            event.role == self.profile.contexts[usize::from(c)].1,
            event.kind == 3,
            [
                proof.admission.is_some(),
                proof.final_auth.is_some(),
                proof.receiver.is_some(),
                proof.receipt.is_some(),
            ],
        )?;
        for reference in [
            Some(proof.original),
            proof.admission,
            proof.final_auth,
            proof.receiver,
        ]
        .into_iter()
        .flatten()
        {
            let raw = self
                .auth(c, reference)?
                .ok_or(Error::Integrity("proof capture absent"))?;
            if hash(b"", &raw) != reference {
                return Err(Error::Integrity("proof capture hash"));
            }
        }
        if let Some(receipt) = self.receipt(c, event.role, event.n)? {
            let value = codec::decode(&receipt, 1024)?;
            let r = value.fields(2)?[0].fields(8)?;
            if r[0].uint()? != 1
                || r[1] != event_core[2]
                || r[2] != event_core[3]
                || r[3].uint()? != u64::from(event.role)
                || r[4].uint()? != event.n
                || r[5].fixed::<32>()? != id
            {
                return Err(Error::Integrity("proof receipt scope"));
            }
            let receiver = r[7].fixed::<32>()?;
            let raw = self
                .auth(c, receiver)?
                .ok_or(Error::Integrity("receipt capture absent"))?;
            if hash(b"", &raw) != receiver {
                return Err(Error::Integrity("receipt capture hash"));
            }
            proof.receipt_overlay = Some(hash(b"", &receipt));
            proof.receiver_overlay = Some(receiver);
        }
        if proof.receipt.is_some() && proof.receipt != proof.receipt_overlay {
            return Err(Error::Integrity("proof actual receipt correspondence"));
        }
        Ok(Some(proof))
    }
    pub fn event_at(&mut self, c: u8, role: u8, n: u64) -> Result<Option<Vec<u8>>> {
        let Some(id) = self
            .core
            .get(super::super::super::history::lane(c, role, n)?)?
        else {
            return Ok(None);
        };
        let id = id
            .as_slice()
            .try_into()
            .map_err(|_| Error::Integrity("lane ID width"))?;
        self.event(c, id)
    }
    pub fn auth(&mut self, c: u8, id: Id) -> Result<Option<Vec<u8>>> {
        self.occupied(c)?;
        self.core.get(Key::hash(c, Namespace::Auth, id)?)
    }
    pub fn projection(&mut self, c: u8, id: Id) -> Result<ProjectionValue> {
        self.occupied(c)?;
        ProjectionValue::decode(
            &self
                .core
                .get(Key::hash(c, Namespace::Projection, id)?)?
                .ok_or(Error::Integrity("readable projection"))?,
        )
    }
    pub fn receipt(&mut self, c: u8, role: u8, n: u64) -> Result<Option<Vec<u8>>> {
        self.occupied(c)?;
        self.core
            .get(super::super::super::history::receipt(c, role, n)?)
    }
    pub fn request(&mut self, c: u8, request: [u8; 16]) -> Result<Option<L1Request>> {
        self.occupied(c)?;
        let Some(raw) = self.core.get(Key::request(c, request)?)? else {
            return Ok(None);
        };
        if raw.len() == 32 {
            let event: Id = raw
                .as_slice()
                .try_into()
                .map_err(|_| Error::Integrity("request Event ID"))?;
            let projection = self.projection(c, event)?;
            return Ok(Some(L1Request {
                queue_sequence: None,
                event: Some(event),
                text_hash: hash(b"kg:od1:request-text\0", projection.text.as_bytes()),
            }));
        }
        Ok(Some(unpack(&raw)?))
    }
    pub fn queue_page(&mut self, c: u8, after: Option<u64>, limit: usize) -> Result<Vec<L1Queue>> {
        self.occupied(c)?;
        if !(1..=32).contains(&limit) {
            return Err(Error::Bound("L1 queue page"));
        }
        let start = after.map_or(Ok(0), |n| {
            n.checked_add(1).ok_or(Error::Bound("queue order overflow"))
        })?;
        self.core
            .range(Key::ordinal(c, Namespace::Queue, start)?, limit)?
            .into_iter()
            .map(|(k, raw)| {
                let value = codec::read_record(&raw, 13, 32 * 1024)?;
                let f = value.fields(10)?;
                let body = f[7].fields(2)?;
                if f[4].uint()? != 1
                    || f[3].uint()? != u64::from(c)
                    || f[5].uint()? != k.ordinal_value()?
                {
                    return Err(Error::Integrity("L1 queue ticket"));
                }
                Ok(L1Queue {
                    sequence: k.ordinal_value()?,
                    request: body[0].fixed()?,
                    sealed: body[1].bytes()?.to_vec(),
                })
            })
            .collect()
    }
    pub fn pending(&mut self, c: u8, n: u64) -> Result<Option<L1Pending>> {
        self.occupied(c)?;
        let Some(raw) = self.core.get(Key::ordinal(c, Namespace::Pending, n)?)? else {
            return Ok(None);
        };
        let value = codec::read_record(&raw, 13, 32 * 1024)?;
        let f = value.fields(10)?;
        let body = f[7].fields(2)?;
        let state = codec::decode(body[1].bytes()?, 2048)?;
        let s = state.fields(8)?;
        if f[4].uint()? != 2 || f[3].uint()? != u64::from(c) || f[5].uint()? != n {
            return Err(Error::Integrity("L1 pending ticket"));
        }
        Ok(Some(L1Pending {
            ordinal: n,
            raw: body[0].bytes()?.to_vec(),
            state: body[1].bytes()?.to_vec(),
            head: s[2].uint()? == 1,
            quarantined: s[3].uint()? == 1,
            original: s[4].fixed()?,
            admission: s[5].fixed()?,
        }))
    }
    pub fn pending_page(
        &mut self,
        c: u8,
        after: Option<u64>,
        limit: usize,
    ) -> Result<Vec<L1Pending>> {
        self.occupied(c)?;
        if !(1..=32).contains(&limit) {
            return Err(Error::Bound("pending page"));
        }
        let start = after.map_or(Ok(0), |n| {
            n.checked_add(1)
                .ok_or(Error::Bound("pending order overflow"))
        })?;
        let rows = self
            .core
            .range(Key::ordinal(c, Namespace::Pending, start)?, limit)?;
        rows.into_iter()
            .map(|(k, _)| {
                self.pending(c, k.ordinal_value()?)?
                    .ok_or(Error::Integrity("pending page membership"))
            })
            .collect()
    }
    pub fn counts(&self, c: u8) -> Result<RuntimeCounts> {
        self.occupied(c)?;
        let count = |ns| -> u64 { self.core.root.roots[usize::from(c) * 24 + ns as usize].count };
        Ok(RuntimeCounts {
            sources: count(Namespace::Event),
            queued: count(Namespace::Queue),
            debt: count(Namespace::Undelivered),
            pending: count(Namespace::Pending),
            receipts: [count(Namespace::ReceiptA), count(Namespace::ReceiptB)],
        })
    }
    pub fn repair_state(&self, c: u8) -> Result<RuntimeRepairState> {
        self.occupied(c)?;
        Ok(self.core.root.ledger[usize::from(c)].runtime_state())
    }
    pub fn debt_page(&mut self, c: u8, after: Option<u64>, limit: usize) -> Result<Vec<(u64, Id)>> {
        self.occupied(c)?;
        if !(1..=32).contains(&limit) {
            return Err(Error::Bound("debt page"));
        }
        let start = after.map_or(Ok(0), |n| {
            n.checked_add(1).ok_or(Error::Bound("debt order overflow"))
        })?;
        self.core
            .range(Key::ordinal(c, Namespace::Undelivered, start)?, limit)?
            .into_iter()
            .map(|(k, raw)| {
                let value = codec::read_record(&raw, 13, 32 * 1024)?;
                let f = value.fields(10)?;
                let old = codec::decode(f[7].bytes()?, 1024)?;
                let body = old.fields(3)?;
                Ok((k.ordinal_value()?, body[1].fixed()?))
            })
            .collect()
    }
}
