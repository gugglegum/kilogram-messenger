use super::*;
#[path = "data.rs"]
mod data;
#[cfg(test)]
#[path = "ops_review1_tests.rs"]
mod review1_tests;

fn authority_key(account: Id) -> Result<Key> {
    let mut name = b"authority/".to_vec();
    name.extend(account);
    key(0, &name)
}
fn own_head(raw: &[u8]) -> Result<Value> {
    let value = codec::decode(raw, 2048)?;
    let f = value.fields(13)?;
    if f[0].uint()? != 1 || f[3].uint()? > 1 {
        return Err(Error::Integrity("Head1 version/role"));
    }
    f[1].fixed::<32>()?;
    f[2].fixed::<32>()?;
    f[4].uint()?;
    for field in &f[7..11] {
        field.optional()?.map(Value::fixed::<32>).transpose()?;
    }
    let phase = f[5].uint()?;
    let generation = f[6].uint()?;
    let offer = f[7].optional()?.is_some();
    let grant = f[8].optional()?.is_some();
    let epoch = grant && f[9].optional()?.is_some() && f[10].optional()?.is_some();
    let sid = f[11].optional()?.map(Value::bytes).transpose()?;
    let blob = f[12].optional()?.map(Value::fixed::<32>).transpose()?;
    let valid = match phase {
        0 => {
            generation == 0
                && !offer
                && !grant
                && !epoch
                && f[9].optional()?.is_none()
                && f[10].optional()?.is_none()
                && sid.is_none()
                && blob.is_none()
        }
        1 => {
            f[3].uint()? == 0
                && generation == 1
                && offer
                && !grant
                && f[9].optional()?.is_none()
                && f[10].optional()?.is_none()
                && sid.is_none()
                && blob.is_some()
        }
        2 => {
            f[3].uint()? == 1
                && generation == 1
                && offer
                && epoch
                && sid.is_none()
                && blob.is_some()
        }
        3 => {
            generation >= 2
                && offer
                && epoch
                && sid.is_some_and(|s| s.len() == 43 && s.is_ascii())
                && blob.is_some()
        }
        _ => false,
    };
    if !valid {
        return Err(Error::Integrity("Head1 real phase shape"));
    }
    Ok(value)
}

impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(super) fn l1_meta(&mut self, c: u8) -> Result<L1Metadata> {
        unpack(
            &self
                .get(key(c, b"meta")?)?
                .ok_or(Error::Integrity("L1 metadata missing"))?,
        )
    }
    pub(super) fn l1_global(&mut self) -> Result<L1Global> {
        unpack(
            &self
                .get(key(0, b"global")?)?
                .ok_or(Error::Integrity("L1 global metadata missing"))?,
        )
    }
    fn l1_meta_put(&mut self, c: u8, meta: &L1Metadata) -> Result<()> {
        let raw = packed(meta)?;
        self.put(key(c, b"meta")?, &raw, (META - raw.len()) as u64, true)
    }
    fn l1_global_put(&mut self, global: &L1Global) -> Result<()> {
        let raw = packed(global)?;
        self.put(key(0, b"global")?, &raw, (META - raw.len()) as u64, true)
    }
    fn l1_current_put(&mut self, c: u8, name: &[u8], raw: &[u8], cap: usize) -> Result<()> {
        if raw.len() > cap {
            return Err(Error::Bound("L1 current record"));
        }
        self.put(key(c, name)?, raw, (cap - raw.len()) as u64, true)
    }
    pub(super) fn l1_head(&mut self, c: u8) -> Result<Value> {
        let raw = self
            .get(head_key(c)?)?
            .ok_or(Error::Integrity("Head7 missing"))?;
        let value = codec::read_record(&raw, 10, 8192)?;
        let h = value.fields(9)?;
        if h[0].uint()? != 7
            || h[6].uint()? != 1
            || h[7].optional()?.is_some()
            || h[8]
                .fields(2)?
                .iter()
                .any(|x| !x.optional().is_ok_and(|o| o.is_none()))
        {
            return Err(Error::UnsupportedRuntimeProfile);
        }
        Ok(value)
    }
    pub(super) fn l1_control(
        &mut self,
        profile: &Profile,
        c: u8,
        control: L1Control,
    ) -> Result<Option<Vec<u8>>> {
        let name = control.name()?;
        if let Some(raw) = self.get(key(c, name.as_bytes())?)? {
            return Ok(Some(raw));
        }
        self.get(original_control(
            c,
            profile.contexts[usize::from(c)].0,
            &name,
        )?)
    }
    fn l1_cert_refs(&mut self, profile: &Profile, c: u8) -> Result<[Id; 2]> {
        let raw = self
            .get(original_control(
                c,
                profile.contexts[usize::from(c)].0,
                "spec",
            )?)?
            .ok_or(Error::Integrity("original Spec"))?;
        let spec = codec::decode(&raw, 96 * 1024)?;
        let fields = spec.fields(6)?;
        let cert = |v: &Value| -> Result<Id> {
            let credential = codec::decode(v.bytes()?, 16 * 1024)?;
            Ok(hash(b"", credential.fields(3)?[1].bytes()?))
        };
        Ok([cert(&fields[2])?, cert(&fields[3])?])
    }
    pub(in crate::vault::indexed_dialog) fn l1_check(
        &mut self,
        op: &L1Operation,
        duplicate: bool,
    ) -> Result<()> {
        op.bound()?;
        let profile = self.migration_profile()?;
        if self.migration_progress(profile.mid)?.stage != super::super::MigrationStage::Ready {
            return Err(Error::UnsupportedRuntimeProfile);
        }
        if let Some(c) = op.context() {
            if usize::from(c) >= profile.contexts.len() {
                return Err(Error::Bound("unoccupied L1 context"));
            }
            self.l1_head(c)?;
            self.migration_budget_check(
                &profile,
                usize::from(c),
                super::super::MigrationStage::Ready,
            )?;
        }
        if matches!(op, L1Operation::Initialize(_)) {
            if self.get(key(0, b"global")?)?.is_some() && !duplicate {
                return Err(Error::Conflict);
            }
        } else {
            self.l1_global()?;
        }
        Ok(())
    }
    pub(in crate::vault::indexed_dialog) fn l1_apply(&mut self, op: L1Operation) -> Result<()> {
        let profile = self.migration_profile()?;
        match op {
            L1Operation::Initialize(init) => {
                if init.accounts.len() != profile.contexts.len()
                    || self.get(key(0, b"global")?)?.is_some()
                {
                    return Err(Error::Conflict);
                }
                for (c, pair) in init.accounts.iter().enumerate() {
                    let role = usize::from(profile.contexts[c].1);
                    if pair[role] != self.root.account || pair[0] == pair[1] {
                        return Err(Error::OwnerMismatch);
                    }
                    let h = self.l1_head(c as u8)?;
                    let h = own_head(h.fields(9)?[4].bytes()?)?;
                    let f = h.fields(13)?;
                    let next = [
                        self.root.roots[c * 24 + 1].count,
                        self.root.roots[c * 24 + 2].count,
                    ];
                    if next.iter().any(|n| *n > 512) {
                        return Err(Error::Integrity("L1 lane counts"));
                    }
                    let mut tails = [None, None];
                    for r in 0..2 {
                        if next[r] > 0 {
                            tails[r] = Some(
                                self.get(super::super::super::history::lane(
                                    c as u8,
                                    r as u8,
                                    next[r] - 1,
                                )?)?
                                .ok_or(Error::Integrity("L1 lane tail"))?
                                .as_slice()
                                .try_into()
                                .map_err(|_| Error::Integrity("L1 lane ID"))?,
                            );
                        }
                    }
                    self.l1_meta_put(
                        c as u8,
                        &L1Metadata {
                            policy_generation: 1,
                            wall_high_water: 0,
                            next,
                            tails,
                            next_queue: f[4]
                                .uint()?
                                .checked_add(1)
                                .ok_or(Error::Bound("queue sequence"))?,
                            request_index_after: None,
                            request_index_ready: false,
                            ack_next: 1,
                            peer_source_next: 0,
                            peer_source_last: None,
                        },
                    )?;
                }
                for pin in init.authorities {
                    let raw = packed(&pin)?;
                    self.put(
                        authority_key(pin.account)?,
                        &raw,
                        (META - raw.len()) as u64,
                        true,
                    )?;
                }
                self.l1_global_put(&L1Global {
                    accounts: init.accounts,
                    next_author: init.next_author,
                })?;
            }
            L1Operation::PrepareRequests { context: c } => {
                let mut meta = self.l1_meta(c)?;
                if !meta.request_index_ready {
                    let start = meta.request_index_after.map_or(Ok(0), |n| {
                        n.checked_add(1).ok_or(Error::Bound("queue preparation"))
                    })?;
                    let slot = usize::from(c) * 24 + 5;
                    let first = Key::ordinal(c, Namespace::Queue, start)?;
                    let leaves =
                        self.tree
                            .range(slot as u8, self.root.roots[slot], first.bytes, 32)?;
                    for leaf in &leaves {
                        let k = Key {
                            context: c,
                            namespace: Namespace::Queue,
                            bytes: leaf.prefix,
                        };
                        let raw = self
                            .get(k)?
                            .ok_or(Error::Integrity("queue preparation ticket"))?;
                        let value = codec::read_record(&raw, 13, 32 * 1024)?;
                        let f = value.fields(10)?;
                        let body = f[7].fields(2)?;
                        let request = body[0].fixed()?;
                        let value = packed(&L1Request {
                            queue_sequence: Some(k.ordinal_value()?),
                            event: None,
                            text_hash: [0; 32],
                        })?;
                        if self.get(Key::request(c, request)?)?.is_some() {
                            return Err(Error::Conflict);
                        }
                        self.put(Key::request(c, request)?, &value, 0, false)?;
                        meta.request_index_after = Some(k.ordinal_value()?);
                    }
                    meta.request_index_ready = leaves.len() < 32;
                    self.l1_meta_put(c, &meta)?;
                }
            }
            L1Operation::InitializeHistory { context: c } => {
                self.l1_initialize_history(&profile, c)?;
            }
            L1Operation::PinAuthority { authority, now } => {
                let global = self.l1_global()?;
                if authority.account != self.root.account
                    && !global
                        .accounts
                        .iter()
                        .any(|p| p.contains(&authority.account))
                {
                    return Err(Error::OwnerMismatch);
                }
                let old: L1Authority = unpack(
                    &self
                        .get(authority_key(authority.account)?)?
                        .ok_or(Error::Integrity("authority pin missing"))?,
                )?;
                if authority.revision < old.revision
                    || authority.revision == old.revision && authority.raw != old.raw
                {
                    return Err(Error::Conflict);
                }
                if authority.revision > old.revision {
                    let raw = packed(&authority)?;
                    self.put(
                        authority_key(authority.account)?,
                        &raw,
                        (META - raw.len()) as u64,
                        true,
                    )?;
                    for (c, pair) in global.accounts.iter().enumerate() {
                        if pair.contains(&authority.account) {
                            let mut meta = self.l1_meta(c as u8)?;
                            meta.policy_generation = meta
                                .policy_generation
                                .checked_add(1)
                                .ok_or(Error::Bound("policy generation"))?;
                            meta.wall_high_water = meta.wall_high_water.max(now);
                            self.l1_meta_put(c as u8, &meta)?;
                        }
                    }
                }
            }
            L1Operation::RenewConsent {
                context: c,
                expected,
                raw,
                now,
            } => {
                let prior = self.l1_control(&profile, c, L1Control::Consent)?;
                if prior.as_ref().map(|v| hash(b"kg:od1:consent\0", v)) != expected {
                    return Err(Error::Conflict);
                }
                let value = codec::decode(&raw, 4096)?;
                let f = value.fields(3)?;
                let body = f[0].fields(8)?;
                let certs = self.l1_cert_refs(&profile, c)?;
                if body[0].uint()? != 1
                    || body[1].fixed::<32>()? != profile.contexts[usize::from(c)].0
                    || body[2].fixed::<32>()? != certs[0]
                    || body[3].fixed::<32>()? != certs[1]
                    || body[4].uint()? != 1
                    || body[5].uint()? != 1
                    || body[7].uint()? <= now
                    || now.saturating_add(120) < body[6].uint()?
                    || f[1].bytes()?.len() != 64
                    || f[2].bytes()?.len() != 64
                {
                    return Err(Error::Integrity("current Consent1 structural scope"));
                }
                if prior.as_deref() != Some(raw.as_slice()) {
                    self.l1_current_put(c, L1Control::Consent.name()?.as_bytes(), &raw, 4096)?;
                    let mut meta = self.l1_meta(c)?;
                    meta.policy_generation = meta
                        .policy_generation
                        .checked_add(1)
                        .ok_or(Error::Bound("policy generation"))?;
                    meta.wall_high_water = meta.wall_high_water.max(now);
                    self.l1_meta_put(c, &meta)?;
                }
            }
            L1Operation::Locator {
                context: c,
                local,
                revision,
                raw,
                now,
            } => {
                let value = codec::decode(&raw, 8192)?;
                let f = value.fields(2)?;
                let body = f[0].fields(9)?;
                let role = usize::from(profile.contexts[usize::from(c)].1);
                let issuer = if local { role } else { 1 - role };
                let certs = self.l1_cert_refs(&profile, c)?;
                if body[0].uint()? != 1
                    || body[1].fixed::<32>()? != profile.contexts[usize::from(c)].0
                    || body[2].fixed::<32>()? != certs[issuer]
                    || body[3].fixed::<32>()? != certs[1 - issuer]
                    || body[6].uint()? != revision
                    || revision == 0
                    || body[8].uint()? <= now
                    || f[1].bytes()?.len() != 64
                {
                    return Err(Error::Integrity("Locator1 structural scope"));
                }
                if let Some(old) = self.l1_control(&profile, c, L1Control::Locator(local))? {
                    let old_value = codec::decode(&old, 8192)?;
                    let old_rev = old_value.fields(2)?[0].fields(9)?[6].uint()?;
                    if revision < old_rev || revision == old_rev && old != raw {
                        return Err(Error::Conflict);
                    }
                    if old == raw {
                        return Ok(());
                    }
                }
                self.l1_current_put(c, L1Control::Locator(local).name()?.as_bytes(), &raw, 8192)?;
                let mut meta = self.l1_meta(c)?;
                meta.policy_generation = meta
                    .policy_generation
                    .checked_add(1)
                    .ok_or(Error::Bound("policy generation"))?;
                meta.wall_high_water = meta.wall_high_water.max(now);
                self.l1_meta_put(c, &meta)?;
            }
            L1Operation::AdmissionPair { context: c, pair } => {
                let value = codec::decode(&pair, 192 * 1024)?;
                value.fields(6)?;
                self.l1_current_put(
                    c,
                    L1Control::CurrentAuth.name()?.as_bytes(),
                    &pair,
                    192 * 1024,
                )?;
            }
            L1Operation::Bootstrap {
                kind,
                change,
                pair,
                controls,
                rows,
                ready,
            } => {
                self.l1_bootstrap(&profile, kind, change, pair, controls, rows, ready)?;
            }
            L1Operation::Queue {
                context: c,
                request,
                text_hash,
                sealed,
                now,
            } => {
                let mut meta = self.l1_meta(c)?;
                if now < meta.wall_high_water {
                    return Err(Error::PolicyRejected("clock rollback before queue"));
                }
                if !meta.request_index_ready {
                    return Err(Error::PolicyRejected("request index not ready"));
                }
                if self.get(Key::request(c, request)?)?.is_some() {
                    return Err(Error::Conflict);
                }
                let role = usize::from(profile.contexts[usize::from(c)].1);
                let queued = self.root.roots[usize::from(c) * 24 + 5].count;
                if meta.next[role].max(1) + queued >= 512 || queued >= 128 {
                    return Err(Error::PolicyRejected("UpgradeRequired/queue capacity"));
                }
                let order = meta.next_queue;
                let ticket = self.l1_ticket(
                    &profile,
                    c,
                    1,
                    order,
                    a([b(request), b(sealed)]),
                    0,
                    Vec::new(),
                )?;
                self.put(
                    Key::ordinal(c, Namespace::Queue, order)?,
                    &ticket,
                    FRAME_CREDIT,
                    false,
                )?;
                self.put(
                    Key::request(c, request)?,
                    &packed(&L1Request {
                        queue_sequence: Some(order),
                        event: None,
                        text_hash,
                    })?,
                    0,
                    false,
                )?;
                meta.next_queue = meta
                    .next_queue
                    .checked_add(1)
                    .ok_or(Error::Bound("queue order"))?;
                meta.wall_high_water = now;
                self.l1_meta_put(c, &meta)?;
            }
            L1Operation::Materialize { change, rows } => {
                self.l1_native_rows(&profile, change, rows, true)?;
            }
            L1Operation::Pending {
                context,
                author,
                raw,
                original,
                admission,
                head,
            } => {
                self.l1_pending(&profile, context, author, raw, original, admission, head)?;
            }
            L1Operation::Drain { change, rows } => {
                self.l1_native_rows(&profile, change, rows, false)?;
            }
            L1Operation::Quarantine {
                context: c,
                ordinal: n,
            } => {
                let k = Key::ordinal(c, Namespace::Pending, n)?;
                let raw = self.get(k)?.ok_or(Error::Conflict)?;
                let value = codec::read_record(&raw, 13, 32 * 1024)?;
                let mut f = value.fields(10)?.to_vec();
                let mut body = f[7].fields(2)?.to_vec();
                let state = codec::decode(body[1].bytes()?, 2048)?;
                let mut s = state.fields(8)?.to_vec();
                s[3] = u(1);
                body[1] = b(codec::encode(&a(s), 2048)?);
                f[7] = a(body);
                let reserved = self.l1_leaf_reserved(k)?;
                self.put(k, &codec::record(13, &a(f), 32 * 1024)?, reserved, true)?;
            }
            L1Operation::Acknowledge {
                context,
                event,
                receipt,
                receiver,
            } => {
                self.l1_ack(&profile, context, event, receipt, receiver)?;
            }
            L1Operation::AckProgress { context: c, next } => {
                let mut meta = self.l1_meta(c)?;
                let peer = 1 - usize::from(profile.contexts[usize::from(c)].1);
                if next < meta.ack_next || next > meta.next[peer] {
                    return Err(Error::Conflict);
                }
                meta.ack_next = next;
                self.l1_meta_put(c, &meta)?;
            }
            L1Operation::ObserveHead {
                context: c,
                next,
                last,
            } => {
                let mut meta = self.l1_meta(c)?;
                if next > 512
                    || (next == 0) != last.is_none()
                    || next == meta.peer_source_next && last != meta.peer_source_last
                {
                    return Err(Error::Conflict);
                }
                if next > meta.peer_source_next {
                    meta.peer_source_next = next;
                    meta.peer_source_last = last;
                    self.l1_meta_put(c, &meta)?;
                }
            }
            L1Operation::Block { context: c, proof } => {
                let name = L1Control::BlockedFork.name()?;
                if let Some(old) = self.l1_control(&profile, c, L1Control::BlockedFork)? {
                    if old != proof {
                        return Err(Error::Conflict);
                    }
                } else {
                    self.l1_current_put(c, name.as_bytes(), &proof, 64 * 1024)?;
                    let mut meta = self.l1_meta(c)?;
                    meta.policy_generation = meta
                        .policy_generation
                        .checked_add(1)
                        .ok_or(Error::Bound("policy generation"))?;
                    self.l1_meta_put(c, &meta)?;
                }
            }
        }
        self.migration_budget_put(&profile, super::super::MigrationStage::Ready)?;
        Ok(())
    }
}
