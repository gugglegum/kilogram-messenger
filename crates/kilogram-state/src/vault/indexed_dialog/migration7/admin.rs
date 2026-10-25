//! Offline administration under the repository's single owner lease.
use super::super::*;
use super::{
    LegacyEntry, MigrationRepository, MigrationStage, Operation, copy::MigrationProjection,
    records::*,
};
use serde::Serialize;

macro_rules! read_migration {
    ($this:ident,$w:ident,$body:block) => {{
        if $this.core.poisoned {
            return Err(Error::ReopenRequired);
        }
        let tx = $this.core.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut $w = Work::new(&nodes, &values, &$this.core.crypto, $this.core.root.clone());
        $w.migration_check()?;
        $body
    }};
}
#[derive(Serialize)]
pub struct MigrationStatus {
    pub source_id: String,
    pub migration_id: String,
    pub generation: u64,
    pub storage_stage: MigrationStage,
    pub copied_entries: u64,
    pub total_entries: u64,
    pub work_phase: u64,
    pub reason: String,
    pub config_revision: u64,
    pub config_hash: String,
    pub output_digest: String,
    pub credits: CreditReport,
    pub runtime_available: bool,
    pub contexts: Vec<MigrationContextStatus>,
}
#[derive(Serialize)]
pub struct MigrationContextStatus {
    pub context: String,
    pub role: u8,
    pub native_phase: u64,
    pub mode: &'static str,
    pub provenance: &'static str,
    pub readable: [u64; 2],
    pub queue: u64,
    pub pending: u64,
    pub head_in_use: bool,
    pub receipts: u64,
    pub undelivered: u64,
    pub legacy_hold: u64,
    pub repair_funded: bool,
}
impl MigrationRepository {
    fn metadata(&mut self) -> Result<(Profile, Progress, Effective)> {
        read_migration!(self, w, {
            let profile = w.migration_profile()?;
            let progress = w.migration_progress(profile.mid)?;
            let raw = w
                .get(migration_key(profile.mid, 7, b"")?)?
                .ok_or(Error::Integrity("effective config"))?;
            let effective = Effective::parse(&raw, &profile)?;
            Ok((profile, progress, effective))
        })
    }
    pub fn status(&mut self) -> Result<MigrationStatus> {
        let (p, s, cfg) = self.metadata()?;
        let contexts = read_migration!(self, w, {
            let mut contexts = Vec::new();
            for (c, (id, role)) in p.contexts.iter().enumerate() {
                let raw = w
                    .get(head_key(c as u8)?)?
                    .ok_or(Error::Integrity("Head7 status"))?;
                let v = super::codec::read_record(&raw, 10, 8192)?;
                let legacy = super::codec::decode(v.fields(9)?[4].bytes()?, 4096)?;
                let budget = w
                    .get(budget_key(c as u8)?)?
                    .ok_or(Error::Integrity("budget status"))?;
                let b = super::codec::read_record(&budget, 6, META_CAP)?;
                let f = b.fields(12)?;
                let ledger = w.root.ledger[c].runtime_state();
                contexts.push(MigrationContextStatus {
                    context: super::source::hex(id),
                    role: *role,
                    native_phase: legacy.fields(13)?[5].uint()?,
                    mode: "L1",
                    provenance: "LegacySnapshot7",
                    readable: [
                        w.root.roots[c * 24 + 1].count,
                        w.root.roots[c * 24 + 2].count,
                    ],
                    queue: w.root.roots[c * 24 + 5].count,
                    pending: ledger.pending,
                    head_in_use: ledger.head.is_some(),
                    receipts: w.root.roots[c * 24 + 7].count + w.root.roots[c * 24 + 8].count,
                    undelivered: w.root.roots[c * 24 + 6].count,
                    legacy_hold: f[7].uint()?,
                    repair_funded: f[8].uint()? == 1,
                });
            }
            Ok::<_, Error>(contexts)
        })?;
        Ok(MigrationStatus {
            source_id: super::source::hex(&p.source),
            migration_id: super::source::hex(&p.mid),
            generation: self.generation(),
            storage_stage: s.stage,
            copied_entries: s.position,
            total_entries: self.inventory.entries.len() as u64,
            work_phase: s.phase,
            reason: String::from_utf8(s.reason)
                .map_err(|_| Error::Integrity("migration reason UTF8"))?,
            config_revision: cfg.revision,
            config_hash: super::source::hex(&Effective::hash(&cfg.raw()?)),
            output_digest: super::source::hex(&s.output),
            credits: self.core.credits()?,
            runtime_available: false,
            contexts,
        })
    }
    pub fn effective_config(&mut self) -> Result<MigrationConfig> {
        let (p, _, c) = self.metadata()?;
        let f = p.identity.fields(14)?;
        let paths = f[11].fields(3)?;
        Ok(MigrationConfig {
            logical_quota: c.quotas,
            physical_capacity: c.physical,
            output_profile: paths[0].text()?.into(),
            control_ipc_file: paths[1].text()?.into(),
            locator_directory: paths[2].text()?.into(),
        })
    }
    fn commit_operation(
        &mut self,
        expected: u64,
        operation: Operation,
        point: &str,
    ) -> Result<CommitReport> {
        let (_, _, cfg) = self.metadata()?;
        let actual = self.core.backend.stats()?.physical_quota;
        if !matches!(operation, Operation::Capacity { .. }) && actual != cfg.physical {
            return Err(Error::Bound("CapacityCASRequiredBeforeWriter"));
        }
        let id = operation.id()?;
        let digest = operation.digest()?;
        let bound = operation.input_bound()?;
        let root = self.core._lease.root().to_path_buf();
        let mut gate = || super::checkpoint(&root, point);
        self.core.commit_with_gate(
            expected,
            id,
            digest,
            CommitInput::Migration7(Box::new(operation)),
            bound,
            Some(&mut gate),
        )
    }
    pub fn resume_copy(&mut self, origin: &MigrationOriginFacts) -> Result<()> {
        let (p, s, _) = self.metadata()?;
        if origin.source_id != p.source || origin.digest()? != p.origin {
            return Err(Error::Conflict);
        }
        if matches!(
            s.stage,
            MigrationStage::Ready | MigrationStage::Copying | MigrationStage::Verifying
        ) {
            return Ok(());
        }
        let stage = if s.phase == 3 {
            MigrationStage::Verifying
        } else {
            MigrationStage::Copying
        };
        self.commit_operation(
            self.generation(),
            Operation::Stage {
                mid: p.mid,
                origin: p.origin,
                stage,
                reason: Vec::new(),
            },
            "resume-staged",
        )?;
        Ok(())
    }
    pub fn pause(&mut self) -> Result<()> {
        let (p, s, _) = self.metadata()?;
        if s.stage == MigrationStage::Paused {
            return Ok(());
        }
        self.commit_operation(
            self.generation(),
            Operation::Stage {
                mid: p.mid,
                origin: p.origin,
                stage: MigrationStage::Paused,
                reason: Vec::new(),
            },
            "pause-staged",
        )?;
        Ok(())
    }
    pub fn block(&mut self, reason: &str) -> Result<()> {
        if reason.len() > 1024 {
            return Err(Error::Bound("blocked reason"));
        }
        let (p, s, _) = self.metadata()?;
        if s.stage == MigrationStage::Ready {
            return Err(Error::Integrity("LifecycleClosed: block"));
        }
        self.commit_operation(
            self.generation(),
            Operation::Stage {
                mid: p.mid,
                origin: p.origin,
                stage: MigrationStage::Blocked,
                reason: reason.as_bytes().to_vec(),
            },
            "blocked-staged",
        )?;
        Ok(())
    }
    pub fn copy_batch(
        &mut self,
        origin: &MigrationOriginFacts,
        mut projection: impl FnMut(&LegacyEntry, &[u8]) -> Result<Option<MigrationProjection>>,
    ) -> Result<u64> {
        let (p, s, _) = self.metadata()?;
        if s.stage != MigrationStage::Copying {
            return Err(Error::Integrity("LifecycleClosed: copy"));
        }
        if origin.digest()? != p.origin || origin.source_id != p.source {
            return Err(Error::Conflict);
        }
        let start = usize::try_from(s.position).map_err(|_| Error::Integrity("copy position"))?;
        if start == self.inventory.entries.len() {
            return Ok(0);
        }
        let mut items = Vec::new();
        let mut bytes = 0usize;
        for entry in self.inventory.entries.iter().skip(start).take(32) {
            if !items.is_empty()
                && bytes
                    .checked_add(entry.length as usize)
                    .is_none_or(|n| n > 1024 * 1024)
            {
                break;
            }
            let raw = self
                .inventory
                .get(
                    &self.core.vault,
                    &entry.path,
                    Some(&self.inventory.mode_raw),
                )?
                .ok_or(Error::Integrity("source entry"))?;
            let derived = projection(entry, &raw)?;
            bytes = bytes
                .checked_add(raw.len())
                .ok_or(Error::Bound("copy bytes"))?;
            items.push(super::copy::prepare_item(self, &p, origin, entry, derived)?);
        }
        let count = items.len() as u64;
        self.commit_operation(
            self.generation(),
            Operation::Copy {
                mid: p.mid,
                origin: p.origin,
                position: s.position,
                items,
            },
            "copy-staged",
        )?;
        super::checkpoint(self.core._lease.root(), "copy-committed")?;
        Ok(count)
    }
    pub fn begin_verification(&mut self, origin: &MigrationOriginFacts) -> Result<()> {
        let (p, s, _) = self.metadata()?;
        if s.stage == MigrationStage::Verifying {
            return Ok(());
        }
        if origin.digest()? != p.origin {
            return Err(Error::Conflict);
        }
        self.commit_operation(
            self.generation(),
            Operation::Stage {
                mid: p.mid,
                origin: p.origin,
                stage: MigrationStage::Verifying,
                reason: Vec::new(),
            },
            "verify-staged",
        )?;
        Ok(())
    }
    pub fn ready(&mut self, origin: MigrationOriginFacts) -> Result<()> {
        let (p, s, _) = self.metadata()?;
        if s.stage == MigrationStage::Ready {
            if origin.digest()? != p.origin {
                return Err(Error::Conflict);
            }
            return Ok(());
        }
        self.commit_operation(
            self.generation(),
            Operation::Ready {
                mid: p.mid,
                output: s.output,
                origin: Box::new(origin),
            },
            "ready-staged",
        )?;
        super::checkpoint(self.core._lease.root(), "ready-committed")?;
        Ok(())
    }
    pub fn increase_capacity(
        &mut self,
        expected_root: u64,
        revision: u64,
        hash: Id,
        candidate: &MigrationConfig,
    ) -> Result<()> {
        let (p, _, old) = self.metadata()?;
        if candidate.paths()? != p.identity.fields(14)?[11] {
            return Err(Error::Conflict);
        }
        if candidate.physical_capacity > self.core.backend.stats()?.physical_quota {
            return Err(Error::ConfiguredCapacity {
                requested: candidate.physical_capacity,
                quota: self.core.backend.stats()?.physical_quota,
            });
        }
        if candidate
            .logical_quota
            .iter()
            .zip(old.quotas)
            .any(|(new, old)| *new < old)
            || candidate.physical_capacity < old.physical
        {
            return Err(Error::Conflict);
        }
        self.commit_operation(
            expected_root,
            Operation::Capacity {
                mid: p.mid,
                revision,
                hash,
                quotas: candidate.logical_quota,
                physical: candidate.physical_capacity,
            },
            "capacity-staged",
        )?;
        super::checkpoint(self.core._lease.root(), "capacity-committed")?;
        Ok(())
    }
    pub fn desired_profile(&mut self) -> Result<Vec<u8>> {
        let (p, _, c) = self.metadata()?;
        desired_profile(&p, &c)
    }
    /// The owner compares every readable output against the immutable source and
    /// actual self-HPKE plaintext before asking for Ready. This never mutates it.
    pub fn verify_readable_copy(
        &mut self,
        origin: &MigrationOriginFacts,
        c: Id,
        role: u8,
        n: u64,
        projection: Option<&MigrationProjection>,
    ) -> Result<()> {
        use super::LegacyRead;
        if role > 1 || n >= 512 {
            return Err(Error::Bound("legacy readable slot"));
        }
        let (p, _, _) = self.metadata()?;
        let slot = super::copy::context(&p, c)?;
        let original = self.required(&format!(
            "runtime/od1/contexts/{}/lane/{role}/{n:020}",
            super::source::hex(&c)
        ))?;
        let id: Id = original
            .as_slice()
            .try_into()
            .map_err(|_| Error::Integrity("source lane ID"))?;
        let ns = if role == 0 {
            Namespace::LaneA
        } else {
            Namespace::LaneB
        };
        if self.core.get(Key::ordinal(slot, ns, n)?)?.as_deref() != Some(id.as_slice()) {
            return Err(Error::Integrity("copied lane exact ID"));
        }
        let expected = self.required(&format!(
            "events/od1/{}/{}",
            super::source::hex(&c),
            super::source::hex(&id)
        ))?;
        if self
            .core
            .get(Key::hash(slot, Namespace::Event, id)?)?
            .as_deref()
            != Some(expected.as_slice())
        {
            return Err(Error::Integrity("copied exact signed Event1"));
        }
        let e = super::copy::event_fields(&expected)?;
        if e.c != c || e.role != role || e.n != n {
            return Err(Error::Integrity("copied readable scope"));
        }
        let snapshot = self
            .core
            .get(migration_key(p.mid, 5, &id)?)?
            .ok_or(Error::Integrity("readable migration membership"))?;
        if origin.source_id != p.source || origin.digest()? != p.origin {
            return Err(Error::Conflict);
        }
        let path = format!(
            "events/od1/{}/{}",
            super::source::hex(&c),
            super::source::hex(&id)
        );
        let entry = self
            .inventory
            .entries
            .iter()
            .find(|e| e.path == path)
            .ok_or(Error::Integrity("exact source event membership"))?;
        let expected_snapshot = super::copy::prepare_item(self, &p, origin, entry, None)?
            .snapshot
            .ok_or(Error::Integrity("migration snapshot"))?;
        if snapshot != expected_snapshot {
            return Err(Error::Integrity(
                "exact Snapshot O/A/F/R applicability and membership",
            ));
        }
        let s = super::codec::read_record(&snapshot, 5, 4096)?;
        let fields = s.fields(13)?;
        if fields[0].uint()? != 7
            || fields[1].fixed::<32>()? != p.mid
            || fields[3].fixed::<32>()? != c
            || fields[4].uint()? != u64::from(role)
            || fields[5].uint()? != n
            || fields[6].fixed::<32>()? != id
            || fields[7].fixed::<16>()? != e.request
        {
            return Err(Error::Integrity("closed LegacySnapshot membership"));
        }
        let captures = fields[11].fields(4)?;
        if captures[0] != super::codec::a([super::codec::u(0), super::codec::b(e.original)]) {
            return Err(Error::Integrity("required Recorded O"));
        }
        if let Some(projection) = projection {
            if e.kind != 3 || projection.request != e.request {
                return Err(Error::Integrity("projection/source request"));
            }
            let raw = self
                .core
                .get(Key::hash(slot, Namespace::Projection, id)?)?
                .ok_or(Error::HistoryUnavailable)?;
            let actual = ProjectionValue::decode(&raw)?;
            if actual.text != projection.text {
                return Err(Error::Integrity("actual self-HPKE projection comparison"));
            }
            let binding = super::codec::decode(&actual.binding, 2304)?;
            let f = binding.fields(5)?;
            if f[0].uint()? != 7
                || f[1].fixed::<32>()? != p.mid
                || f[2].fixed::<32>()? != p.source
                || f[3].fixed::<32>()? != migration_key(p.mid, 5, &id)?.bytes
                || f[4].fixed::<16>()? != e.request
            {
                return Err(Error::Integrity("projection migration binding"));
            }
        } else if e.kind == 3 {
            return Err(Error::Integrity("Text needs verified projection"));
        }
        Ok(())
    }
    pub fn publish_profile(&mut self, expected_existing: Option<Id>) -> Result<()> {
        use std::io::{Read, Write};
        let (p, s, c) = self.metadata()?;
        if s.stage != MigrationStage::Ready {
            return Err(Error::Integrity("profile publication before Ready"));
        }
        let raw = desired_profile(&p, &c)?;
        let path = std::path::PathBuf::from(p.identity.fields(14)?[11].fields(3)?[0].text()?);
        let staged = profile_stage(&path)?;
        let prior = if path.exists() {
            crate::reject_symlink(&path)?;
            if fs::metadata(&path)?.len() > 64 * 1024 {
                return Err(Error::Conflict);
            }
            let mut bytes = Vec::new();
            fs::File::open(&path)?
                .take(64 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            Some(bytes)
        } else {
            None
        };
        if prior.as_deref() == Some(raw.as_slice()) {
            if staged.exists() {
                crate::reject_symlink(&staged)?;
                fs::remove_file(&staged)?;
                crate::sync_directory(path.parent().ok_or(Error::Bound("profile parent"))?)?;
            }
            return Ok(());
        }
        if let Some(prior) = prior.as_ref() {
            if expected_existing != Some(super::copy::raw_hash(b"", prior)) {
                return Err(Error::Bound("ReadyNeedsProfileUpdate"));
            }
        } else if expected_existing.is_some() {
            return Err(Error::Conflict);
        }
        let parent = path.parent().ok_or(Error::Bound("profile parent"))?;
        // This single fixed staging path was reserved at preflight/Fence. A kill
        // cannot leave an unbounded family of public temporary files. Its bytes
        // have no authority and are rebuilt from the latest committed plan.
        if staged.exists() {
            crate::reject_symlink(&staged)?;
            let meta = fs::metadata(&staged)?;
            if !meta.is_file() || meta.len() > 64 * 1024 {
                return Err(Error::Conflict);
            }
            fs::remove_file(&staged)?;
        }
        let mut temp = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)?;
        temp.write_all(&raw)?;
        temp.sync_all()?;
        drop(temp);
        crate::sync_directory(parent)?;
        super::checkpoint(self.core._lease.root(), "profile-staged")?;
        if let Some(prior) = prior {
            crate::reject_symlink(&path)?;
            if fs::metadata(&path)?.len() > 64 * 1024 || fs::read(&path)? != prior {
                return Err(Error::Conflict);
            }
            fs::rename(&staged, &path)?;
        } else {
            fs::hard_link(&staged, &path)?;
            fs::remove_file(&staged)?;
        }
        crate::sync_directory(parent)?;
        Ok(())
    }
    pub fn last_commit(&self) -> Result<Id> {
        self.core.last_commit()
    }
    pub fn backend_stats(&self) -> Result<BackendStats> {
        self.core.backend_stats()
    }
    #[cfg(any(test, debug_assertions))]
    pub fn verification_io_fault(&self, point: &str, after: u64) -> Result<()> {
        let point = match point {
            "before_write" => "before_write",
            "after_write" => "after_write",
            "before_sync" => "before_sync",
            "after_sync" => "after_sync",
            "storage_full" => "storage_full",
            _ => return Err(Error::Bound("migration fault point")),
        };
        self.core.backend.fail(point, after)
    }
}
