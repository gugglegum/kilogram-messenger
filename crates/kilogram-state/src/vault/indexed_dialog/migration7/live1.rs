//! Closed Ready Root7 L1 owner. No Inventory, legacy writer or arbitrary path API.
use super::super::*;
use super::{
    codec::{self, Value, a, b, opt, u},
    records::*,
};
use serde::{Deserialize, Serialize};
#[path = "live1/types.rs"]
mod types;
pub use types::*;
#[path = "live1/history.rs"]
mod history;
#[path = "live1/ops.rs"]
mod ops;
#[path = "live1/read.rs"]
mod read;
#[cfg(test)]
#[path = "live1/tests.rs"]
mod tests;

const META: usize = 32 * 1024;
const OVERHEAD1: u64 = 41 + 32 + 2 * NODE_CAP as u64;
fn key(c: u8, name: &[u8]) -> Result<Key> {
    let mut h = blake3::Hasher::new();
    h.update(b"kg:od1:indexed-current/v7\0");
    h.update(name);
    Key::hash(c, Namespace::Current, *h.finalize().as_bytes())
}
fn packed<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut out = b"L1M7".to_vec();
    out.extend(postcard::to_allocvec(value).map_err(|_| Error::Bound("L1 metadata codec"))?);
    if out.len() > META {
        return Err(Error::Bound("L1 metadata bytes"));
    }
    Ok(out)
}
fn unpack<T: for<'de> Deserialize<'de>>(raw: &[u8]) -> Result<T> {
    if raw.len() > META || raw.get(..4) != Some(b"L1M7") {
        return Err(Error::Integrity("L1 metadata envelope"));
    }
    let (value, tail) =
        postcard::take_from_bytes(&raw[4..]).map_err(|_| Error::Integrity("L1 metadata codec"))?;
    if !tail.is_empty() {
        return Err(Error::Integrity("L1 metadata suffix"));
    }
    Ok(value)
}
fn hash(domain: &[u8], raw: &[u8]) -> Id {
    super::copy::raw_hash(domain, raw)
}
fn prefix(c: Id) -> String {
    format!("runtime/od1/contexts/{}/", super::source::hex(&c))
}
fn original_control(c: u8, context: Id, suffix: &str) -> Result<Key> {
    super::copy::old_key(
        c,
        Namespace::Control,
        &format!("{}{suffix}", prefix(context)),
    )
}

pub struct L1Repository {
    core: IndexedDialogVault,
    config: super::MigrationConfig,
    profile: Profile,
}
pub struct L1ReopenFailure {
    pub lease: StateDirectoryLock,
    pub error: Error,
}
pub struct L1CapacityPlan {
    expected: u64,
    revision: u64,
    hash: Id,
    candidate: super::MigrationConfig,
    prior_profile_hash: Id,
}

impl L1Repository {
    pub fn open_ready(
        lease: StateDirectoryLock,
        expected: ExpectedOwner,
        device: Id,
        config: super::MigrationConfig,
    ) -> Result<Self> {
        let (loaded, profile, _) =
            super::load::open_core_borrowed(&lease, expected.account, device, Some(&config))?;
        let mut core = IndexedDialogVault::from_loaded(lease, loaded);
        let effective = Effective::parse(
            &core
                .get(migration_key(profile.mid, 7, b"")?)?
                .ok_or(Error::Integrity("effective config"))?,
            &profile,
        )?;
        let progress = Progress::parse(
            &core
                .get(migration_key(profile.mid, 2, b"")?)?
                .ok_or(Error::Integrity("migration progress"))?,
            profile.mid,
        )?;
        if core.root.claim != expected.claim
            || profile.device != device
            || progress.stage != super::MigrationStage::Ready
            || config.logical_quota != effective.quotas
            || config.physical_capacity != effective.physical
            || config.paths()? != profile.identity.fields(14)?[11]
            || profile.identity.fields(14)?[10].text()?
                != core
                    ._lease
                    .root()
                    .to_str()
                    .ok_or(Error::Bound("state path UTF8"))?
        {
            return Err(Error::UnsupportedRuntimeProfile);
        }
        {
            let tx = core.vault.database.begin_read().map_err(db)?;
            let nodes = tx.open_table(NODES).map_err(db)?;
            let values = tx.open_table(VALUES).map_err(db)?;
            // Cold, bounded correspondence check, never Inventory::resume/load.
            Work::new(&nodes, &values, &core.crypto, core.root.clone()).migration_check()?;
        }
        Ok(Self {
            core,
            config,
            profile,
        })
    }
    pub fn generation(&self) -> u64 {
        self.core.generation()
    }
    pub fn last_commit(&self) -> Result<Id> {
        self.core.last_commit()
    }
    pub fn source_id(&self) -> Id {
        self.profile.source
    }
    pub fn migration_id(&self) -> Id {
        self.profile.mid
    }
    pub fn contexts(&self) -> &[(Id, u8)] {
        &self.profile.contexts
    }
    pub fn root(&self) -> &std::path::Path {
        self.core._lease.root()
    }
    pub fn backend_stats(&self) -> Result<BackendStats> {
        self.core.backend_stats()
    }
    pub fn reopen(self) -> std::result::Result<Self, L1ReopenFailure> {
        let device = self.profile.device;
        let config = self.config;
        let (lease, owner) = self.core.into_owner_lease();
        let (loaded, profile, actual_config) =
            match super::load::open_core_borrowed(&lease, owner.account, device, Some(&config)) {
                Ok(value) => value,
                Err(error) => return Err(L1ReopenFailure { lease, error }),
            };
        let mut core = IndexedDialogVault::from_loaded(lease, loaded);
        // A failed prospective capacity CAS may recover the old effective config.
        // Reopen under that exact bound before admitting any live writer.
        if core
            .backend
            .stats()
            .is_ok_and(|s| s.physical_quota != actual_config.physical_capacity)
        {
            let (held, _) = core.into_owner_lease();
            let loaded = match super::load::open_core_borrowed(
                &held,
                owner.account,
                device,
                Some(&actual_config),
            ) {
                Ok((loaded, _, _)) => loaded,
                Err(error) => return Err(L1ReopenFailure { lease: held, error }),
            };
            core = IndexedDialogVault::from_loaded(held, loaded);
        }
        // Keep the lock even when recovery fails. Validation is performed before use.
        let mut restored = Self {
            core,
            config: actual_config,
            profile,
        };
        match restored.validate_ready(device) {
            Ok(()) => Ok(restored),
            Err(error) => {
                let (lease, _) = restored.core.into_owner_lease();
                Err(L1ReopenFailure { lease, error })
            }
        }
    }
    pub fn config_status(&mut self) -> Result<(u64, Id)> {
        let raw = self
            .core
            .get(migration_key(self.profile.mid, 7, b"")?)?
            .ok_or(Error::Integrity("effective config"))?;
        let config = Effective::parse(&raw, &self.profile)?;
        Ok((config.revision, Effective::hash(&raw)))
    }
    pub fn prepare_capacity(
        &mut self,
        expected: u64,
        revision: u64,
        hash: Id,
        candidate: super::MigrationConfig,
    ) -> Result<L1CapacityPlan> {
        if self.core.poisoned {
            return Err(Error::ReopenRequired);
        }
        if candidate.paths()? != self.profile.identity.fields(14)?[11]
            || candidate
                .logical_quota
                .iter()
                .zip(self.config.logical_quota)
                .any(|(n, o)| *n < o)
            || candidate.physical_capacity < self.config.physical_capacity
        {
            return Err(Error::Conflict);
        }
        let op = super::Operation::Capacity {
            mid: self.profile.mid,
            revision,
            hash,
            quotas: candidate.logical_quota,
            physical: candidate.physical_capacity,
        };
        if self.core.last_commit()? != op.id()?
            && (self.generation() != expected || self.config_status()? != (revision, hash))
        {
            return Err(Error::StaleGeneration);
        }
        let raw = self
            .core
            .get(migration_key(self.profile.mid, 7, b"")?)?
            .ok_or(Error::Integrity("effective config"))?;
        let old = Effective::parse(&raw, &self.profile)?;
        let expected_profile = desired_profile(&self.profile, &old)?;
        let path = &self.config.output_profile;
        if fs::metadata(path)?.len() > 64 * 1024 || fs::read(path)? != expected_profile {
            return Err(Error::Bound("ReadyNeedsProfileUpdate"));
        }
        Ok(L1CapacityPlan {
            expected,
            revision,
            hash,
            candidate,
            prior_profile_hash: super::copy::raw_hash(b"", &expected_profile),
        })
    }
    pub fn apply_capacity(
        self,
        plan: L1CapacityPlan,
    ) -> std::result::Result<(Self, Result<()>), L1ReopenFailure> {
        let account = self.core.root.account;
        let device = self.profile.device;
        let (lease, _) = self.core.into_owner_lease();
        let (loaded, profile, actual) =
            match super::load::open_core_borrowed(&lease, account, device, Some(&plan.candidate)) {
                Ok(v) => v,
                Err(error) => return Err(L1ReopenFailure { lease, error }),
            };
        let mut repository = Self {
            core: IndexedDialogVault::from_loaded(lease, loaded),
            config: actual,
            profile,
        };
        let op = super::Operation::Capacity {
            mid: repository.profile.mid,
            revision: plan.revision,
            hash: plan.hash,
            quotas: plan.candidate.logical_quota,
            physical: plan.candidate.physical_capacity,
        };
        let result = (|| -> Result<()> {
            let id = op.id()?;
            let digest = op.digest()?;
            let bound = op.input_bound()?;
            repository.core.commit_with(
                plan.expected,
                id,
                digest,
                CommitInput::Migration7(Box::new(op)),
                bound,
            )?;
            repository.profile = Profile::parse(
                &repository
                    .core
                    .get(profile_key()?)?
                    .ok_or(Error::Integrity("updated profile"))?,
            )?;
            repository.config = plan.candidate.clone();
            let raw = repository
                .core
                .get(migration_key(repository.profile.mid, 7, b"")?)?
                .ok_or(Error::Integrity("effective config"))?;
            let effective = Effective::parse(&raw, &repository.profile)?;
            super::admin::publish_profile_core(
                &mut repository.core,
                &repository.profile,
                super::MigrationStage::Ready,
                &effective,
                Some(plan.prior_profile_hash),
            )
        })();
        if result.is_err() && repository.core.poisoned {
            repository.config = plan.candidate;
        }
        if result.is_err()
            && !repository.core.poisoned
            && repository
                .core
                .backend
                .stats()
                .is_ok_and(|s| s.physical_quota != repository.config.physical_capacity)
        {
            repository = repository.reopen()?;
        }
        Ok((repository, result))
    }
    fn validate_ready(&mut self, device: Id) -> Result<()> {
        if self.profile.device != device {
            return Err(Error::OwnerMismatch);
        }
        let tx = self.core.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        Work::new(&nodes, &values, &self.core.crypto, self.core.root.clone()).migration_check()
    }
    pub fn commit(
        &mut self,
        operation: L1Operation,
        mut gate: Option<&mut dyn FnMut() -> Result<()>>,
    ) -> Result<CommitReport> {
        if gate.is_none()
            && matches!(&operation,
            L1Operation::Bootstrap { change, .. } if change.native.is_some())
        {
            return Err(Error::PolicyRejected("Native requires final live gate"));
        }
        if gate.is_none()
            && matches!(
                &operation,
                L1Operation::Materialize { .. } | L1Operation::Drain { .. }
            )
        {
            return Err(Error::PolicyRejected("Native requires final live gate"));
        }
        if self.core.backend.stats()?.physical_quota != self.config.physical_capacity {
            return Err(Error::Bound("CapacityCASRequiredBeforeWriter"));
        }
        operation.bound()?;
        let raw = Zeroizing::new(
            postcard::to_allocvec(&(self.core.root.generation, &operation))
                .map_err(|_| Error::Bound("L1 operation codec"))?,
        );
        if raw.len() > 1024 * 1024 {
            return Err(Error::Bound("L1 operation bytes"));
        }
        let digest = self.core.crypto.hash(0, b"L1-commit/v7", &raw);
        let mut id = digest;
        id[0] |= 1;
        let bound = raw.len() * 3 + 16384;
        self.core.commit_with_gate(
            self.core.generation(),
            id,
            digest,
            CommitInput::Legacy1(Box::new(operation)),
            bound,
            gate.take(),
        )
    }
}
