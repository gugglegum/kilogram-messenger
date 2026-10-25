//! Migration-only state transitions. Lifecycle/purpose checks precede retry shortcuts.
use super::super::*;
use super::{
    codec::{self, a, b, opt, u},
    copy::CopyItem,
    records::*,
};

pub(in crate::vault::indexed_dialog) enum Operation {
    Copy {
        mid: Id,
        origin: Id,
        position: u64,
        items: Vec<CopyItem>,
    },
    Stage {
        mid: Id,
        origin: Id,
        stage: MigrationStage,
        reason: Vec<u8>,
    },
    Ready {
        mid: Id,
        output: Id,
        origin: Box<MigrationOriginFacts>,
    },
    Capacity {
        mid: Id,
        revision: u64,
        hash: Id,
        quotas: [u64; 2],
        physical: u64,
    },
}
impl Operation {
    pub fn input_bound(&self) -> Result<usize> {
        let n = match self {
            Self::Copy { items, .. } => {
                if items.is_empty() || items.len() > 32 {
                    return Err(Error::Bound("copy source count"));
                }
                items.iter().try_fold(0usize, |n, item| {
                    n.checked_add(
                        item.body.len()
                            + item.snapshot.as_ref().map_or(0, Vec::len)
                            + item.projection.as_ref().map_or(0, |p| p.text.len())
                            + item.pending_state.as_ref().map_or(0, Vec::len)
                            + 2048,
                    )
                    .ok_or(Error::Bound("copy input"))
                })?
            }
            Self::Ready { origin, .. } => codec::encode(&origin.value()?, 512 * 1024)?.len(),
            Self::Stage { reason, .. } => reason.len() + 1024,
            Self::Capacity { .. } => 4096,
        };
        if n > 2 * 1024 * 1024 {
            return Err(Error::Bound("migration owner input"));
        }
        Ok(n)
    }
    pub fn digest(&self) -> Result<Id> {
        let value = match self {
            Self::Copy {
                mid,
                origin,
                position,
                items,
            } => a([
                u(0),
                b(mid),
                b(origin),
                u(*position),
                a(items.iter().map(|i| {
                    a([
                        i.entry.reference([0; 32]),
                        b(super::copy::raw_hash(b"", &i.body)),
                        opt(i
                            .projection
                            .as_ref()
                            .map(|p| a([b(p.request), b(p.text.as_bytes())]))),
                        opt(i.snapshot.as_ref().map(b)),
                        opt(i
                            .pending_state
                            .as_ref()
                            .map(|v| b(super::copy::raw_hash(b"", v)))),
                        opt(i
                            .seen
                            .map(|(r, key, g, first)| a([u(r.into()), b(key), u(g), b(first)]))),
                        opt(i.author_context.map(|c| u(c.into()))),
                        a(i.paid_auth.iter().map(b)),
                        a(i.paid_auth_sizes
                            .iter()
                            .map(|(id, size)| a([b(id), u(*size)]))),
                    ])
                })),
            ]),
            Self::Stage {
                mid,
                origin,
                stage,
                reason,
            } => a([u(1), b(mid), b(origin), u(*stage as u64), b(reason)]),
            Self::Ready {
                mid,
                output,
                origin,
            } => a([u(2), b(mid), b(output), b(origin.digest()?)]),
            Self::Capacity {
                mid,
                revision,
                hash,
                quotas,
                physical,
            } => a([
                u(3),
                b(mid),
                u(*revision),
                b(hash),
                a(quotas.map(u)),
                u(*physical),
            ]),
        };
        codec::digest(b"kg:od2:migration-operation\0", &value)
    }
    pub fn id(&self) -> Result<Id> {
        codec::digest(b"kg:od2:migration-operation-id\0", &b(self.digest()?))
    }
}
impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(in crate::vault::indexed_dialog) fn migration_check_input(
        &mut self,
        op: &Operation,
        duplicate: bool,
    ) -> Result<()> {
        self.migration_check()?;
        let profile = self.migration_profile()?;
        let p = self.migration_progress(profile.mid)?;
        op.input_bound()?;
        let mid = match op {
            Operation::Copy { mid, .. }
            | Operation::Stage { mid, .. }
            | Operation::Ready { mid, .. }
            | Operation::Capacity { mid, .. } => *mid,
        };
        if mid != profile.mid {
            return Err(Error::Conflict);
        }
        match op {
            Operation::Copy {
                origin,
                position,
                items,
                ..
            } => {
                if p.stage != MigrationStage::Copying {
                    return Err(Error::Integrity("LifecycleClosed: copy"));
                }
                if *origin != profile.origin || items.is_empty() || items.len() > 32 {
                    return Err(Error::Conflict);
                }
                let end = position
                    .checked_add(items.len() as u64)
                    .ok_or(Error::Bound("copy position"))?;
                if p.position != if duplicate { end } else { *position } {
                    return Err(Error::StaleGeneration);
                }
                if !duplicate {
                    let mut previous = p.last_path.as_deref();
                    let mut bytes = 0usize;
                    for item in items {
                        if previous.is_some_and(|path| path >= item.entry.path.as_str()) {
                            return Err(Error::Integrity("source copy order"));
                        }
                        previous = Some(&item.entry.path);
                        bytes = bytes
                            .checked_add(item.body.len())
                            .ok_or(Error::Bound("copy bytes"))?;
                    }
                    if bytes > 1024 * 1024 {
                        return Err(Error::Bound("copy input bytes"));
                    }
                }
            }
            Operation::Stage {
                origin,
                stage,
                reason,
                ..
            } => {
                if p.stage == MigrationStage::Ready
                    || *stage == MigrationStage::Ready
                    || *origin != profile.origin
                    || reason.len() > 1024
                {
                    return Err(Error::Integrity("LifecycleClosed: stage"));
                }
                if *stage == MigrationStage::Verifying
                    && p.position != source_count(self, &profile)?
                {
                    return Err(Error::Integrity("incomplete copy"));
                }
            }
            Operation::Ready { origin, output, .. } => {
                if p.stage != MigrationStage::Verifying
                    && !(duplicate && p.stage == MigrationStage::Ready)
                {
                    return Err(Error::Integrity("Ready requires verification"));
                }
                if origin.source_id != profile.source
                    || origin.digest()? != profile.origin
                    || origin.contexts.len() != profile.contexts.len()
                    || (!duplicate && *output != p.output)
                {
                    return Err(Error::Conflict);
                }
                if p.position != source_count(self, &profile)? || p.counts[0] != p.position {
                    return Err(Error::Integrity("copy completeness"));
                }
            }
            Operation::Capacity {
                revision,
                hash,
                quotas,
                physical,
                ..
            } => {
                let raw = self
                    .get(migration_key(profile.mid, 7, b"")?)?
                    .ok_or(Error::Integrity("effective config"))?;
                let old = Effective::parse(&raw, &profile)?;
                if !duplicate && (old.revision != *revision || Effective::hash(&raw) != *hash) {
                    return Err(Error::StaleGeneration);
                }
                if !duplicate
                    && (quotas.iter().zip(old.quotas).any(|(new, old)| *new < old)
                        || *physical < old.physical)
                {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(())
    }
    pub(in crate::vault::indexed_dialog) fn migration_apply(
        &mut self,
        op: Operation,
    ) -> Result<()> {
        let mut profile = self.migration_profile()?;
        let mut p = self.migration_progress(profile.mid)?;
        match op {
            Operation::Copy { items, .. } => {
                for item in items {
                    self.migration_copy_item(&profile, &item)?;
                    p.processed = codec::digest(
                        b"kg:od2:migration-inventory\0",
                        &a([b(p.processed), item.entry.reference([0; 32])]),
                    )?;
                    p.position = p
                        .position
                        .checked_add(1)
                        .ok_or(Error::Bound("copy position"))?;
                    p.counts[0] += 1;
                    if let Some(id) = item.entry.context {
                        let c = super::copy::context(&profile, id)? as usize;
                        let offset = 1 + c * 7;
                        let column = match item.entry.kind {
                            11 => Some(super::copy::event_fields(&item.body)?.role as usize),
                            14 => {
                                let v = codec::decode(
                                    item.pending_state
                                        .as_ref()
                                        .ok_or(Error::Integrity("pending state"))?,
                                    2048,
                                )?;
                                Some(if v.fields(8)?[2].uint()? == 1 { 3 } else { 2 })
                            }
                            13 => Some(4),
                            17 => Some(5),
                            _ => None,
                        };
                        if let Some(column) = column {
                            p.counts[offset + column] += 1;
                        }
                        p.counts[offset + 6] += 1;
                    }
                    p.last_path = Some(item.entry.path);
                }
            }
            Operation::Stage { stage, reason, .. } => {
                p.stage = stage;
                p.reason = reason;
                if stage == MigrationStage::Verifying {
                    p.phase = 3;
                }
            }
            Operation::Capacity {
                quotas, physical, ..
            } => {
                let raw = self
                    .get(migration_key(profile.mid, 7, b"")?)?
                    .ok_or(Error::Integrity("effective config"))?;
                let mut next = Effective::parse(&raw, &profile)?;
                next.revision = next
                    .revision
                    .checked_add(1)
                    .ok_or(Error::Bound("config revision"))?;
                next.previous = Some(Effective::hash(&raw));
                next.quotas = quotas;
                next.physical = physical;
                profile.plan_revision = next.revision;
                profile.plan_hash = profile_plan(&profile, &next)?;
                self.migration_meta_put(
                    migration_key(profile.mid, 7, b"")?,
                    &next.raw()?,
                    META_CAP,
                    0,
                )?;
                self.migration_meta_put(profile_key()?, &profile.raw()?, 16 * 1024, 0)?;
                for (ledger, quota) in self.root.ledger.iter_mut().zip(quotas) {
                    ledger.quota = quota;
                }
            }
            Operation::Ready { origin, .. } => {
                let intent = self
                    .get(migration_key(profile.mid, 1, b"")?)?
                    .ok_or(Error::Integrity("source intent"))?;
                let iv = codec::read_record(&intent, 1, 24 * 1024)?;
                if p.processed != iv.fields(8)?[1].fields(9)?[8].fixed::<32>()? {
                    return Err(Error::Integrity("processed source inventory digest"));
                }
                let mut pointers = Vec::new();
                let mut retired = Vec::new();
                for (c, fact) in origin.contexts.iter().enumerate() {
                    if profile.contexts[c] != (fact.context, fact.role) {
                        return Err(Error::Integrity("Ready context identity"));
                    }
                    for (n, count) in [
                        (0, fact.next[0] + fact.next[1]),
                        (1, fact.next[0]),
                        (2, fact.next[1]),
                        (
                            3,
                            fact.next[0].saturating_sub(1) + fact.next[1].saturating_sub(1),
                        ),
                        (5, p.counts[1 + c * 7 + 4]),
                        (
                            9,
                            fact.ordinary_pending + u64::from(fact.head_pending.is_some()),
                        ),
                    ] {
                        if self.root.roots[c * 24 + n].count != count {
                            return Err(Error::Integrity("Ready cross-index completeness"));
                        }
                    }
                    if self.root.roots[c * 24 + 7].count + self.root.roots[c * 24 + 8].count
                        != p.counts[1 + c * 7 + 5]
                    {
                        return Err(Error::Integrity("Ready receipt count"));
                    }
                    for r in 0..2u8 {
                        if self.root.roots[c * 24 + 10 + r as usize].count
                            != fact.seen.iter().filter(|s| s.0 == r).count() as u64
                        {
                            return Err(Error::Integrity("Ready Seen count"));
                        }
                    }
                    for (role, key, g, first) in &fact.seen {
                        let ns = if *role == 0 {
                            Namespace::SeenA
                        } else {
                            Namespace::SeenB
                        };
                        let mut expected = g.to_be_bytes().to_vec();
                        expected.extend(first);
                        if self.get(Key::hash(c as u8, ns, *key)?)?.as_deref()
                            != Some(expected.as_slice())
                        {
                            return Err(Error::Integrity("Ready exact Seen"));
                        }
                    }
                    let native = self.get(Key::hash(c as u8, Namespace::Native, [0; 32])?)?;
                    let hash = native
                        .as_ref()
                        .map(|raw| super::copy::raw_hash(b"kg:od1:native-state\0", raw));
                    if hash != fact.native_hash {
                        return Err(Error::Integrity("Ready exact Native"));
                    }
                    let old = self
                        .get(head_key(c as u8)?)?
                        .ok_or(Error::Integrity("Head7"))?;
                    let mut value = codec::read_record(&old, 10, 8192)?.fields(9)?.to_vec();
                    if super::copy::raw_hash(b"", value[4].bytes()?) != fact.head_hash
                        || value[6].uint()? != 1
                    {
                        return Err(Error::Integrity("Ready original Head/L1 mode"));
                    }
                    value[5] = opt(hash.map(b));
                    let raw = codec::record(10, &a(value), 8192)?;
                    self.migration_meta_put(head_key(c as u8)?, &raw, 8192, 0)?;
                    pointers.push(a([
                        b(fact.context),
                        b(super::copy::raw_hash(b"", &raw)),
                        opt(hash.map(b)),
                    ]));
                    retired.push(a([
                        b(fact.context),
                        b(fact.head_hash),
                        opt(fact.native_hash.map(b)),
                    ]));
                    self.root.ledger[c].migration_ready(
                        fact.ordinary_pending,
                        fact.head_pending,
                        true,
                    )?;
                }
                p.stage = MigrationStage::Ready;
                p.phase = 3;
                p.reason.clear();
                self.migration_budget_put(&profile, p.stage)?;
                p.output = self.migration_output_digest(profile.mid)?;
                let ready = codec::record(
                    3,
                    &a([
                        u(7),
                        b(profile.mid),
                        b(p.processed),
                        b(p.output),
                        b(codec::digest(
                            b"kg:od2:migration-current-pointers\0",
                            &a(pointers),
                        )?),
                        b(codec::digest(
                            b"kg:od2:migration-retired-source\0",
                            &a(retired),
                        )?),
                        b(profile.plan_hash),
                    ]),
                    META_CAP,
                )?;
                self.migration_meta_put(migration_key(profile.mid, 3, b"")?, &ready, META_CAP, 0)?;
            }
        }
        self.migration_budget_put(&profile, p.stage)?;
        if p.stage != MigrationStage::Ready {
            p.output = self.migration_output_digest(profile.mid)?;
        }
        self.migration_meta_put(migration_key(profile.mid, 2, b"")?, &p.raw()?, 8192, 0)?;
        self.migration_check()
    }
}
fn source_count<
    N: ReadableTable<&'static [u8], &'static [u8]>,
    V: ReadableTable<&'static [u8], &'static [u8]>,
>(
    w: &mut Work<'_, N, V>,
    profile: &Profile,
) -> Result<u64> {
    let raw = w
        .get(migration_key(profile.mid, 1, b"")?)?
        .ok_or(Error::Integrity("intent"))?;
    let v = codec::read_record(&raw, 1, 24 * 1024)?;
    let identity = v.fields(8)?[1].fields(9)?;
    let manifest: crate::vault::VaultManifest = postcard::from_bytes(identity[1].bytes()?)
        .map_err(|_| Error::Integrity("source manifest"))?;
    Ok(manifest.record_count)
}
