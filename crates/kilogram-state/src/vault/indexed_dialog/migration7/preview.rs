//! A bounded dry run uses the very same row recipe as copying. Only row sizes and
//! hashes survive each iteration; no source body or plaintext history is retained.
use super::super::*;
use super::{
    MigrationRepository,
    codec::{self, a},
    copy::{self, MigrationProjection, ProducedRows},
    records::*,
    source::*,
};
use serde::Serialize;

const PUBLIC_CAP: u64 = 256 * 1024;
#[derive(Clone, Debug, Serialize)]
pub struct MigrationPreview {
    pub source_id: String,
    pub entries: u64,
    pub logical_need: [u64; 2],
    pub final_need: [u64; 2],
    pub retained_source_bytes: u64,
    pub retained_files_bytes: u64,
    pub physical_minimum: u64,
    pub runtime_available: bool,
}
pub(super) struct CheckedPreview {
    pub identity: Id,
    pub origin: Id,
    pub report: MigrationPreview,
}
fn add(x: u64, y: u64) -> Result<u64> {
    x.checked_add(y)
        .ok_or(Error::Bound("migration quota arithmetic"))
}
fn identity(config: &MigrationConfig) -> Result<Id> {
    codec::digest(
        b"kg:od2:migration-preflight-config\0",
        &a([config.paths()?, config.capacities()]),
    )
}
fn files(
    root: &std::path::Path,
    dir: &std::path::Path,
    depth: u8,
    count: &mut usize,
) -> Result<u64> {
    if depth > 16 {
        return Err(Error::Bound("retained directory depth"));
    }
    let mut bytes = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        crate::reject_symlink(&path)?;
        *count += 1;
        if *count > 100_000 {
            return Err(Error::Bound("retained file inventory"));
        }
        let meta = entry.metadata()?;
        if meta.is_dir() {
            bytes = add(bytes, files(root, &path, depth + 1, count)?)?;
        } else if meta.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::Integrity("retained path"))?;
            if [
                std::path::Path::new(STATE_VAULT_FILE),
                std::path::Path::new(STATE_VAULT_KEY_FILE),
                std::path::Path::new(crate::LOCK_FILE),
                std::path::Path::new("ratchet/account.pickle"),
            ]
            .contains(&relative)
            {
                continue;
            }
            bytes = add(bytes, meta.len())?;
        } else {
            return Err(Error::Bound("retained file type"));
        }
    }
    Ok(bytes)
}
pub(super) fn physical_config(root: &std::path::Path, mut config: Config) -> Result<Config> {
    config.retained_external_bytes = add(
        config.retained_external_bytes,
        add(files(root, root, 0, &mut 0)?, PUBLIC_CAP)?,
    )?;
    Ok(config)
}
impl V4Source {
    pub fn preflight(
        &mut self,
        facts: &MigrationOriginFacts,
        config: &MigrationConfig,
        mut projection: impl FnMut(&LegacyEntry, &[u8]) -> Result<Option<MigrationProjection>>,
    ) -> Result<MigrationPreview> {
        self.preflight = None;
        for path in config.paths()?.fields(3)? {
            if std::path::Path::new(path.text()?).starts_with(self.root()) {
                return Err(Error::Bound("public paths outside state"));
            }
        }
        if facts.source_id != self.source_id() {
            return Err(Error::OwnerMismatch);
        }
        if profile_stage(&config.output_profile)?.exists() {
            return Err(Error::CreationConflict);
        }
        let fields = self.inventory.identity.fields(9)?;
        if fields[6].fixed::<32>()? != facts.account || fields[7].fixed::<32>()? != facts.device {
            return Err(Error::OwnerMismatch);
        }
        let mut signers = BTreeSet::from([facts.device]);
        for c in &facts.contexts {
            signers.extend(c.devices);
        }
        if signers.len() > 3 || facts.contexts.len() > 2 {
            return Err(Error::Bound("migration origin slots"));
        }
        // Hash keys are deliberately private scratch keys. They never become an
        // account, context, session or storage incarnation and are never persisted.
        let p = Profile {
            source: self.source_id(),
            mid: [0; 32],
            device: facts.device,
            mask: (1 << facts.contexts.len()) - 1,
            identity: a([]),
            initial_config: [0; 32],
            origin: facts.digest()?,
            contexts: facts.contexts.iter().map(|c| (c.context, c.role)).collect(),
            signers: signers.into_iter().collect(),
            plan_revision: 1,
            plan_hash: [0; 32],
        };
        let mut rows = BTreeMap::<Key, (u64, u64, Id)>::new();
        let mut total = [0u64; 2];
        let mut covered = [0u64; 2];
        let mut heads = [false; 2];
        // Every fixed-capacity metadata row costs cap+585 regardless of CBOR uint
        // widths. Source mode1 and mode5 have exactly equal encrypted lengths.
        for cap in [16 * 1024, 24 * 1024, 8192, 4096, 4096, 4096] {
            total[0] = add(total[0], cap + OVERHEAD)?;
        }
        total[0] = add(total[0], self.inventory.backing_bytes)?;
        for amount in total.iter_mut().take(facts.contexts.len()) {
            *amount = add(*amount, 8192 + OVERHEAD + META_CAP as u64 + OVERHEAD)?;
        }
        let need = |total: [u64; 2], covered: [u64; 2], heads: [bool; 2]| -> Result<[u64; 2]> {
            let mut out = total;
            for c in 0..2 {
                out[c] = add(out[c], if heads[c] { 0 } else { TEMP_CREDIT })?;
                if c < facts.contexts.len() {
                    out[c] = add(out[c], add(B4.saturating_sub(covered[c]), REPAIR_POOL)?)?;
                }
            }
            Ok(out)
        };
        let insert = |rows: &mut BTreeMap<Key, (u64, u64, Id)>,
                      total: &mut [u64; 2],
                      covered: &mut [u64; 2],
                      key: Key,
                      raw: &[u8],
                      reserved: u64|
         -> Result<()> {
            let charge = add(raw.len() as u64, OVERHEAD)?;
            let hash = *blake3::hash(raw).as_bytes();
            if let Some(old) = rows.insert(key, (charge, reserved, hash)) {
                if old != (charge, reserved, hash) {
                    return Err(Error::Integrity("preflight duplicate conflict"));
                }
                return Ok(());
            }
            let c = usize::from(key.context);
            let sum = add(charge, reserved)?;
            total[c] = add(total[c], sum)?;
            if matches!(
                key.namespace,
                Namespace::Event
                    | Namespace::LaneA
                    | Namespace::LaneB
                    | Namespace::Projection
                    | Namespace::Request
                    | Namespace::Queue
                    | Namespace::Undelivered
                    | Namespace::ReceiptA
                    | Namespace::ReceiptB
                    | Namespace::Pending
                    | Namespace::SeenA
                    | Namespace::SeenB
                    | Namespace::Auth
                    | Namespace::Native
                    | Namespace::Admission
            ) {
                covered[c] = add(covered[c], sum)?;
            }
            Ok(())
        };
        // These V4Ref witnesses already exist in Fence, then are exact duplicates
        // when their source positions are copied.
        for entry in self.entries().iter().filter(|e| matches!(e.kind, 9 | 10)) {
            let raw = codec::record(4, &entry.reference(p.source), 1024)?;
            insert(
                &mut rows,
                &mut total,
                &mut covered,
                migration_key(p.mid, 4, &entry.record_key)?,
                &raw,
                0,
            )?;
        }
        let mut peak = need(total, covered, heads)?;
        for entry in self.entries() {
            let raw = self.required(&entry.path)?;
            let derived = projection(entry, &raw)?;
            let item = copy::prepare_item(self, &p, facts, entry, derived)?;
            let produced = ProducedRows::generate(&p, &item)?;
            for row in produced.rows {
                insert(
                    &mut rows,
                    &mut total,
                    &mut covered,
                    row.key,
                    &row.raw,
                    row.reserved,
                )?;
            }
            if let Some((c, _, true)) = produced.pending {
                heads[usize::from(c)] = true;
            }
            let n = need(total, covered, heads)?;
            for c in 0..2 {
                peak[c] = peak[c].max(n[c]);
            }
        }
        total[0] = add(total[0], META_CAP as u64 + OVERHEAD)?; // Ready marker
        for c in 0..2 {
            let control = rows
                .iter()
                .filter(|(key, _)| {
                    usize::from(key.context) == c
                        && matches!(
                            key.namespace,
                            Namespace::Control | Namespace::Current | Namespace::Native
                        )
                })
                .try_fold(0u64, |sum, (_, (charge, reserved, _))| {
                    add(sum, add(*charge, *reserved)?)
                })?;
            let fixed = if c == 0 { 16 * 1024 + OVERHEAD } else { 0 }
                + if c < facts.contexts.len() {
                    8192 + OVERHEAD + META_CAP as u64 + OVERHEAD
                } else {
                    0
                };
            if add(control, fixed)? > 4 * 1024 * 1024 {
                return Err(Error::Quota);
            }
        }
        let final_need = need(total, covered, heads)?;
        for c in 0..2 {
            peak[c] = peak[c].max(final_need[c]);
        }
        let external = account_external(
            self.root(),
            physical_config(self.root(), config.backend(0))?,
        )?
        .retained_external_bytes;
        // Two encrypted copies plus redb page/tree/transaction overhead. Runtime
        // backend enforcement remains authoritative for actual filesystem growth.
        let growth = rows.values().try_fold(0u64, |sum, (charge, _, _)| {
            add(
                sum,
                charge
                    .checked_mul(4)
                    .ok_or(Error::Bound("physical growth"))?,
            )
        })?;
        let physical_minimum = add(
            add(add(self.backend.stats()?.length, external)?, HEADROOM)?,
            add(growth, 2 * 1024 * 1024)?,
        )?;
        let report = MigrationPreview {
            source_id: hex(&self.source_id()),
            entries: self.entries().len() as u64,
            logical_need: peak,
            final_need,
            retained_source_bytes: self.inventory.backing_bytes,
            retained_files_bytes: external,
            physical_minimum,
            runtime_available: false,
        };
        self.preflight = Some(CheckedPreview {
            identity: identity(config)?,
            origin: facts.digest()?,
            report: report.clone(),
        });
        Ok(report)
    }
    pub(super) fn require_preflight(
        &self,
        facts: &MigrationOriginFacts,
        config: &MigrationConfig,
    ) -> Result<()> {
        let p = self
            .preflight
            .as_ref()
            .ok_or(Error::Bound("PreflightRequired"))?;
        if p.identity != identity(config)? || p.origin != facts.digest()? {
            return Err(Error::Conflict);
        }
        if p.report
            .logical_need
            .iter()
            .zip(config.logical_quota)
            .any(|(n, q)| *n > q)
        {
            return Err(Error::Quota);
        }
        if p.report.physical_minimum > config.physical_capacity {
            return Err(Error::ConfiguredCapacity {
                requested: p.report.physical_minimum,
                quota: config.physical_capacity,
            });
        }
        Ok(())
    }
}

/// Dispatch the authenticated primary schema while keeping the same lease. A
/// Root7 never falls back to an old writer and a damaged header is not adopted.
pub enum MigrationInput {
    Legacy(Box<V4Source>),
    Indexed(Box<MigrationRepository>),
}
impl MigrationInput {
    pub fn open(
        lease: StateDirectoryLock,
        account: Id,
        device: Id,
        config: &MigrationConfig,
    ) -> Result<Self> {
        let path = lease.root().join(STATE_VAULT_FILE);
        crate::reject_symlink(&path)?;
        let mut builder = Database::builder();
        builder.set_cache_size(64 * 1024 * 1024);
        let reader = match builder.open_read_only(&path) {
            Ok(reader) => reader,
            Err(redb::DatabaseError::RepairAborted) => {
                let cfg = account_external(
                    lease.root(),
                    physical_config(lease.root(), config.backend(0))?,
                )?;
                let (backend, _) = backend::Backend::open(&path, false, cfg)?;
                drop(builder.create_with_backend(backend).map_err(db)?);
                builder.open_read_only(&path).map_err(db)?
            }
            Err(error) => return Err(db(error)),
        };
        let schema = {
            let tx = reader.begin_read().map_err(db)?;
            let meta = tx.open_table(META_TABLE).map_err(db)?;
            let raw = meta
                .get(MANIFEST_KEY)
                .map_err(db)?
                .ok_or(Error::Integrity("primary manifest"))?;
            if raw.value().len() > 256 {
                return Err(Error::Bound("manifest cap"));
            }
            let manifest: crate::vault::VaultManifest = postcard::from_bytes(raw.value())
                .map_err(|_| Error::Integrity("primary manifest"))?;
            manifest.schema_version
        };
        drop(reader);
        match schema {
            4 => Ok(Self::Legacy(Box::new(V4Source::open(
                lease,
                config.backend(0),
            )?))),
            5 => Ok(Self::Indexed(Box::new(MigrationRepository::open(
                lease,
                account,
                device,
                Some(config),
            )?))),
            _ => Err(Error::ExistingStateRequiresMigration),
        }
    }
}
