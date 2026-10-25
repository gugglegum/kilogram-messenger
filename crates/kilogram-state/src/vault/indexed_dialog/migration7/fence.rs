//! One forward-only transaction in the existing primary database.
use super::super::*;
use super::{
    MigrationRepository,
    codec::{self, Value, a, b, u},
    records::*,
    source::{self, Inventory, V4Source},
};
use crate::vault::{self, VaultRecord};
use redb::ReadableTableMetadata;

pub(super) fn fence(
    source: V4Source,
    facts: &MigrationOriginFacts,
    config: &MigrationConfig,
) -> Result<MigrationRepository> {
    source.require_preflight(facts, config)?;
    if profile_stage(&config.output_profile)?.exists() {
        return Err(Error::CreationConflict);
    }
    if source.backend.stats()?.physical_quota != config.physical_capacity {
        return Err(Error::Integrity("configured migration backend"));
    }
    let identity = source.inventory.identity.fields(9)?;
    let claim_value = codec::decode(identity[5].bytes()?, 1024)?;
    let claim = claim_value.fields(5)?;
    if facts.source_id != source.source_id()
        || facts.account != identity[6].fixed::<32>()?
        || facts.device != identity[7].fixed::<32>()?
        || facts.claim != claim[2].fixed::<32>()?
    {
        return Err(Error::OwnerMismatch);
    }
    let contexts = source
        .entries()
        .iter()
        .filter(|e| e.kind == 8)
        .map(|e| e.context.ok_or(Error::Integrity("source context")))
        .collect::<Result<BTreeSet<_>>>()?;
    if contexts.len() != facts.contexts.len()
        || contexts
            .iter()
            .copied()
            .ne(facts.contexts.iter().map(|c| c.context))
    {
        return Err(Error::Integrity("complete origin contexts"));
    }
    let paths = config.paths()?;
    for path in paths.fields(3)? {
        if std::path::Path::new(path.text()?).starts_with(source.root()) {
            return Err(Error::Bound("public paths outside state"));
        }
    }
    let mut incarnation = [0; 32];
    getrandom::fill(&mut incarnation).map_err(|_| Error::Bound("migration incarnation"))?;
    let source_id = source.source_id();
    let mid = codec::digest(
        b"kg:od2:migration-instance\0",
        &a([b(source_id), b(incarnation)]),
    )?;
    let mut slots = [[0; 32]; 2];
    for (c, slot) in slots.iter_mut().enumerate() {
        *slot = if let Some(ctx) = facts.contexts.get(c) {
            ctx.context
        } else {
            codec::digest(
                b"kg:od2:unused-storage-slot\0",
                &a([b(source_id), u(c as u64)]),
            )?
        };
    }
    let mask = ((1u8) << facts.contexts.len()) - 1;
    let pinned = a([
        u(7),
        b(facts.account),
        b(facts.claim),
        b(facts.device),
        b(source_id),
        b(mid),
        b(incarnation),
        a(slots.map(b)),
        u(mask.into()),
        a(facts.contexts.iter().map(|c| b(c.context))),
        b(source
            .root()
            .to_str()
            .ok_or(Error::Bound("state path UTF8"))?
            .as_bytes()),
        paths,
        b(b"direct-only"),
        u(1),
    ]);
    let identity_digest = codec::digest(b"kg:od2:migration-config-identity\0", &pinned)?;
    let initial = codec::digest(
        b"kg:od2:migration-config-initial\0",
        &a([pinned.clone(), config.capacities()]),
    )?;
    let mut signers = BTreeSet::from([facts.device]);
    for c in &facts.contexts {
        if c.devices[usize::from(c.role)] != facts.device || c.devices[0] >= c.devices[1] {
            return Err(Error::Integrity("origin signer roles"));
        }
        signers.extend(c.devices);
    }
    if signers.len() > 3 {
        return Err(Error::Bound("migration signer slots"));
    }
    let mut profile = Profile {
        source: source_id,
        mid,
        device: facts.device,
        mask,
        identity: pinned,
        initial_config: initial,
        origin: facts.digest()?,
        contexts: facts.contexts.iter().map(|c| (c.context, c.role)).collect(),
        signers: signers.into_iter().collect(),
        plan_revision: 1,
        plan_hash: [0; 32],
    };
    let mode_key = source.vault.record_key(source::MODE_PATH);
    let fenced_mode = source.vault.encrypt_record(
        &VaultRecord {
            version: 1,
            relative_path: source::MODE_PATH.to_owned(),
            content: source::MODE5.to_vec(),
        },
        &mode_key,
    )?;
    let backing = Inventory::backing(&source.vault, Some((mode_key, &fenced_mode)))?;
    let root = Root {
        layout: RootLayout::Migrated7,
        incarnation,
        account: facts.account,
        claim: facts.claim,
        signing: Zeroizing::new(
            source
                .required("device-secret.key")?
                .as_slice()
                .try_into()
                .map_err(|_| Error::Integrity("signing key length"))?,
        ),
        encryption: Zeroizing::new(
            source
                .required("device-encryption-secret.key")?
                .as_slice()
                .try_into()
                .map_err(|_| Error::Integrity("encryption key length"))?,
        ),
        contexts: slots,
        generation: source
            .generation()
            .checked_add(1)
            .ok_or(Error::Bound("fence generation"))?,
        commit_id: codec::digest(b"kg:od2:migration-fence\0", &a([b(mid), b(initial)]))?,
        request_digest: initial,
        roots: [Summary::default(); ROOTS],
        ledger: [
            credits::Ledger::migration_unfunded(config.logical_quota[0]),
            credits::Ledger::migration_unfunded(config.logical_quota[1]),
        ],
    };
    let crypto = Crypto::new(
        &source.vault.master_key.0,
        incarnation,
        slots,
        facts.account,
    );
    let V4Source {
        vault,
        lease,
        backend,
        mut inventory,
        ..
    } = source;
    inventory.backing_digest = backing.0;
    inventory.backing_entries = backing.1;
    inventory.backing_bytes = backing.2;
    let mut core = IndexedDialogVault {
        vault,
        _lease: lease,
        crypto,
        root,
        backend,
        poisoned: false,
    };
    let mut progress = Progress {
        mid,
        stage: MigrationStage::Copying,
        phase: 0,
        position: 0,
        last_path: None,
        processed: [0; 32],
        counts: [0; 15],
        output: [0; 32],
        reason: Vec::new(),
    };
    let effective = Effective {
        mid,
        revision: 1,
        identity: identity_digest,
        initial,
        quotas: config.logical_quota,
        physical: config.physical_capacity,
        previous: None,
    };
    profile.plan_hash = profile_plan(&profile, &effective)?;
    let intent = codec::record(
        1,
        &a([
            u(7),
            inventory.identity.clone(),
            b(source_id),
            b(mid),
            a([b(mode_key), b(&inventory.mode_raw), b(source::MODE1)]),
            b(initial),
            b(identity_digest),
            u(mask.into()),
        ]),
        24 * 1024,
    )?;
    core.backend.begin()?;
    let result = (|| -> Result<Root> {
        let mut tx = core.vault.database.begin_write().map_err(db)?;
        tx.set_durability(Durability::Immediate).map_err(db)?;
        {
            let meta = tx.open_table(META_TABLE).map_err(db)?;
            if meta.get(ANCHOR).map_err(db)?.is_some()
                || meta
                    .get(MANIFEST_KEY)
                    .map_err(db)?
                    .is_none_or(|v| v.value() != inventory.manifest_raw)
            {
                return Err(Error::CreationConflict);
            }
        }
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        if nodes.len().map_err(db)? != 0 || values.len().map_err(db)? != 0 {
            return Err(Error::CreationConflict);
        }
        let mut w = Work::new(&nodes, &values, &core.crypto, core.root.clone());
        w.migration_meta_put(profile_key()?, &profile.raw()?, 16 * 1024, 0)?;
        w.migration_meta_put(migration_key(mid, 1, b"")?, &intent, 24 * 1024, 0)?;
        w.migration_meta_put(migration_key(mid, 2, b"")?, &progress.raw()?, 8192, 0)?;
        w.migration_meta_put(migration_key(mid, 7, b"")?, &effective.raw()?, META_CAP, 0)?;
        let backing_raw = codec::record(
            8,
            &a([
                u(7),
                b(mid),
                b(source_id),
                b(backing.0),
                u(backing.1),
                u(backing.2),
            ]),
            META_CAP,
        )?;
        w.migration_meta_put(
            migration_key(mid, 8, b"")?,
            &backing_raw,
            META_CAP,
            backing.2,
        )?;
        let origin = codec::record(
            12,
            &a([u(7), b(mid), b(source_id), b(profile.origin)]),
            META_CAP,
        )?;
        w.migration_meta_put(migration_key(mid, 12, b"")?, &origin, META_CAP, 0)?;
        for (c, fact) in facts.contexts.iter().enumerate() {
            let head_path = format!("runtime/od1/contexts/{}/head", source::hex(&fact.context));
            let raw = inventory
                .get(&core.vault, &head_path, None)?
                .ok_or(Error::Integrity("source Head"))?;
            if *blake3::hash(&raw).as_bytes() != fact.head_hash {
                return Err(Error::Integrity("origin Head witness"));
            }
            let h = codec::decode(&raw, 4096)?;
            let hf = h.fields(13)?;
            if hf[0].uint()? != 1
                || hf[1].fixed::<32>()? != fact.context
                || hf[2].fixed::<32>()? != fact.certificates[usize::from(fact.role)]
                || hf[3].uint()? != u64::from(fact.role)
                || hf[5].uint()? > 3
            {
                return Err(Error::Integrity("origin Head shape"));
            }
            let spec_path = format!("runtime/od1/contexts/{}/spec", source::hex(&fact.context));
            let spec_raw = inventory
                .get(&core.vault, &spec_path, None)?
                .ok_or(Error::Integrity("source spec"))?;
            let spec = codec::decode(&spec_raw, 96 * 1024)?;
            let sf = spec.fields(6)?;
            for r in 0..2 {
                let credentials = codec::decode(sf[2 + r].bytes()?, 16 * 1024)?;
                let cf = credentials.fields(3)?;
                if *blake3::hash(cf[1].bytes()?).as_bytes() != fact.certificates[r] {
                    return Err(Error::Integrity("actual source certificate refs"));
                }
            }
            let native_path = format!("ratchet/od1-contexts/{}/native", source::hex(&fact.context));
            let native = inventory.get(&core.vault, &native_path, None)?;
            let native_hash = if let Some(raw) = native.as_ref() {
                let mut hash = blake3::Hasher::new();
                hash.update(b"kg:od1:native-state\0");
                hash.update(raw);
                Some(*hash.finalize().as_bytes())
            } else {
                None
            };
            if native_hash != fact.native_hash
                || hf[12].optional()?.map(Value::fixed).transpose()? != native_hash
                || (hf[5].uint()? == 0) != native.is_none()
            {
                return Err(Error::Integrity("actual Native source witness"));
            }
            let head = codec::record(
                10,
                &a([
                    u(7),
                    b(source_id),
                    b(fact.context),
                    u(fact.role.into()),
                    b(&raw[..]),
                    a([]),
                    u(1),
                    a([]),
                    a([a([]), a([])]),
                ]),
                8192,
            )?;
            w.migration_meta_put(head_key(c as u8)?, &head, 8192, 0)?;
            for path in [Some(head_path), native.map(|_| native_path)]
                .into_iter()
                .flatten()
            {
                let entry = inventory
                    .entries
                    .iter()
                    .find(|e| e.path == path)
                    .ok_or(Error::Integrity("witness ref"))?;
                let r = codec::record(4, &entry.reference(source_id), 1024)?;
                w.put(migration_key(mid, 4, &entry.record_key)?, &r, 0, false)?;
            }
        }
        w.migration_budget_put(&profile, MigrationStage::Copying)?;
        progress.output = w.migration_output_digest(mid)?;
        w.migration_meta_put(migration_key(mid, 2, b"")?, &progress.raw()?, 8192, 0)?;
        w.migration_check()?;
        w.tree.seal_changes()?;
        if w.buffer_bound_bytes() > MAX_PLAN || w.history_buffer_peak + MAX_PLAN > 192 * 1024 * 1024
        {
            return Err(Error::Bound("migration fence plan"));
        }
        let next = w.root.clone();
        let node_changes = w.tree.changed;
        let value_changes = w.changed;
        drop(nodes);
        drop(values);
        {
            let mut table = tx.open_table(NODES).map_err(db)?;
            for (key, value) in node_changes {
                if let Some(value) = value {
                    table.insert(key.as_slice(), value.as_slice()).map_err(db)?;
                } else {
                    return Err(Error::Integrity("new fence node retirement"));
                }
            }
        }
        {
            let mut table = tx.open_table(VALUES).map_err(db)?;
            for (key, value) in value_changes {
                if let Some(value) = value {
                    table.insert(key.as_slice(), value.as_slice()).map_err(db)?;
                } else {
                    return Err(Error::Integrity("new fence value retirement"));
                }
            }
        }
        tx.open_table(vault::RECORD_TABLE)
            .map_err(db)?
            .insert(mode_key.as_slice(), fenced_mode.as_slice())
            .map_err(db)?;
        core.write_anchor(&tx, &next)?;
        super::checkpoint(core._lease.root(), "fence-staged")?;
        tx.commit().map_err(db)?;
        Ok(next)
    })();
    match result {
        Ok(root) => core.root = root,
        Err(error) => {
            core.poisoned = true;
            return Err(error);
        }
    }
    super::checkpoint(core._lease.root(), "fence-committed")?;
    Ok(MigrationRepository { core, inventory })
}
impl<N: ReadableTable<&'static [u8], &'static [u8]>, V: ReadableTable<&'static [u8], &'static [u8]>>
    Work<'_, N, V>
{
    pub(super) fn migration_meta_put(
        &mut self,
        key: Key,
        raw: &[u8],
        capacity: usize,
        extra: u64,
    ) -> Result<()> {
        let unused = capacity
            .checked_sub(raw.len())
            .ok_or(Error::Bound("migration prepaid metadata"))? as u64;
        self.put(
            key,
            raw,
            unused
                .checked_add(extra)
                .ok_or(Error::Bound("migration reservation"))?,
            true,
        )
    }
}
