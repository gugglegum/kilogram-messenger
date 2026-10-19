//! Rotation-aware authenticated local health for root-verified policy bundles.

use std::{fmt, fs, path::Path};

use redb::{
    Database, Durability, ReadOnlyDatabase, ReadTransaction, ReadableDatabase, ReadableTable,
    ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CohortEvaluation, RolloutError, RolloutHealthCounters, RolloutHealthEvent,
    RolloutHealthSnapshot, evaluate_cohort,
    trust::{ReleaseTrustError, ReleaseTrustRoot},
    trusted_policy::{TrustedPolicyBundleId, TrustedPolicyError, TrustedReleasePolicyBundle},
};

const DATABASE_FILE: &str = "rollout-trusted-health-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("rollout-trusted-health-meta-v1");
const HEALTH_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("rollout-trusted-health-records-v1");
const SCHEMA_KEY: &str = "schema-version";
const TRUST_ROOT_KEY: &str = "release-trust-root";
const INSTALLATION_SEED_KEY: &str = "installation-seed";
const HEAD_KEY: &str = "trusted-health-head";
const SCHEMA_VERSION: u8 = 1;
const RECORD_VERSION: u8 = 1;
const RECORD_MAC_DOMAIN: &[u8] = b"kilogram:rollout:trusted-health-record:v1\0";

pub const MAX_TRUSTED_HEALTH_RECORDS: u64 = 32;
pub const MAX_TRUSTED_HEALTH_RECORD_BYTES: usize = 12 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TrustedHealthContent {
    version: u8,
    trusted_policy_bundle: Vec<u8>,
    counters: RolloutHealthCounters,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StoredTrustedHealthRecord {
    content: TrustedHealthContent,
    mac: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedHealthActivationOutcome {
    Installed {
        bundle_id: TrustedPolicyBundleId,
        evaluation: CohortEvaluation,
        health: RolloutHealthSnapshot,
    },
    AlreadyCurrent {
        bundle_id: TrustedPolicyBundleId,
        evaluation: CohortEvaluation,
        health: RolloutHealthSnapshot,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrustedRolloutHealthReadOnlySnapshot {
    pub bundle_id: TrustedPolicyBundleId,
    pub evaluation: CohortEvaluation,
    pub health: RolloutHealthSnapshot,
}

pub struct TrustedRolloutHealthStore {
    database: Database,
    trust_root: ReleaseTrustRoot,
}

impl TrustedRolloutHealthStore {
    pub fn open(
        installation_local_directory: impl AsRef<Path>,
        trust_root: ReleaseTrustRoot,
    ) -> Result<Self, TrustedRolloutHealthStoreError> {
        let directory = prepare_real_directory(installation_local_directory.as_ref())?;
        let database_path = directory.join(DATABASE_FILE);
        if let Ok(metadata) = fs::symlink_metadata(&database_path)
            && (!metadata.is_file() || metadata.file_type().is_symlink())
        {
            return Err(TrustedRolloutHealthStoreError::UnsafeDatabaseFile);
        }
        let database = Database::create(&database_path).map_err(database_error)?;
        let store = Self {
            database,
            trust_root,
        };
        store.initialize()?;
        let _ = store.current_snapshot()?;
        Ok(store)
    }

    #[cfg(test)]
    pub(crate) fn open_with_test_seed(
        installation_local_directory: impl AsRef<Path>,
        trust_root: ReleaseTrustRoot,
        seed: [u8; 32],
    ) -> Result<Self, TrustedRolloutHealthStoreError> {
        let store = Self::open(installation_local_directory, trust_root)?;
        if store.current_snapshot()?.is_some() {
            return Err(TrustedRolloutHealthStoreError::TestSeedAfterActivation);
        }
        let mut write = store.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
            meta.insert(INSTALLATION_SEED_KEY, seed.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        Ok(store)
    }

    pub fn activate_bundle(
        &self,
        bundle: &TrustedReleasePolicyBundle,
    ) -> Result<TrustedHealthActivationOutcome, TrustedRolloutHealthStoreError> {
        bundle.verify(self.trust_root)?;
        let candidate_id = bundle.bundle_id()?;
        let encoded_bundle = bundle.encode()?;
        let current = self.current_record()?;
        if let Some((current_bundle, counters)) = &current {
            if current_bundle.bundle_id()? == candidate_id {
                if current_bundle.encode()? != encoded_bundle {
                    return Err(TrustedRolloutHealthStoreError::BundleIdCollision);
                }
                return activation_outcome(false, self.seed()?, bundle, *counters);
            }
            bundle.verify_successor_of(current_bundle)?;
        } else if bundle.policy().policy_epoch() != 1
            || bundle.policy().previous_policy_id().is_some()
            || bundle.keyring().keyring_epoch() != 1
            || bundle.keyring().previous_keyring_id().is_some()
        {
            return Err(TrustedRolloutHealthStoreError::InvalidGenesisBundle);
        }

        let expected_head = current
            .as_ref()
            .map(|(bundle, _)| bundle.bundle_id())
            .transpose()?;
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        let seed = {
            let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
            if read_head_id(&meta)? != expected_head {
                return Err(TrustedRolloutHealthStoreError::ConcurrentHeadChanged);
            }
            let seed = read_seed(&meta)?;
            let mut records = write.open_table(HEALTH_TABLE).map_err(database_error)?;
            if records.len().map_err(database_error)? >= MAX_TRUSTED_HEALTH_RECORDS {
                return Err(TrustedRolloutHealthStoreError::RecordCapacity);
            }
            if records
                .get(candidate_id.as_bytes().as_slice())
                .map_err(database_error)?
                .is_some()
            {
                return Err(TrustedRolloutHealthStoreError::UnexpectedRetainedRecord);
            }
            let encoded = encode_record(
                TrustedHealthContent {
                    version: RECORD_VERSION,
                    trusted_policy_bundle: encoded_bundle,
                    counters: RolloutHealthCounters::default(),
                },
                &seed,
            )?;
            records
                .insert(candidate_id.as_bytes().as_slice(), encoded.as_slice())
                .map_err(database_error)?;
            drop(records);
            meta.insert(HEAD_KEY, candidate_id.as_bytes().as_slice())
                .map_err(database_error)?;
            seed
        };
        write.commit().map_err(database_error)?;
        activation_outcome(true, seed, bundle, RolloutHealthCounters::default())
    }

    /// Bring the authenticated health chain to the exact tip of a root-verified
    /// trusted-policy chain. The operation is restart-safe: already installed
    /// prefixes are verified and skipped, while only missing successors are
    /// appended.
    pub fn synchronize_bundle_chain(
        &self,
        bundles: &[TrustedReleasePolicyBundle],
    ) -> Result<TrustedRolloutHealthReadOnlySnapshot, TrustedRolloutHealthStoreError> {
        validate_bundle_chain(bundles, self.trust_root)?;
        let start = match self.current_snapshot()? {
            None => 0,
            Some(snapshot) => bundles
                .iter()
                .position(|bundle| {
                    bundle
                        .bundle_id()
                        .is_ok_and(|bundle_id| bundle_id == snapshot.bundle_id)
                })
                .ok_or(TrustedRolloutHealthStoreError::CurrentBundleNotInChain)?
                .checked_add(1)
                .ok_or(TrustedRolloutHealthStoreError::RecordCapacity)?,
        };
        for bundle in &bundles[start..] {
            let _ = self.activate_bundle(bundle)?;
        }
        self.current_snapshot()?
            .ok_or(TrustedRolloutHealthStoreError::MissingHead)
    }

    pub fn record_health(
        &self,
        expected_bundle_id: TrustedPolicyBundleId,
        event: RolloutHealthEvent,
    ) -> Result<RolloutHealthSnapshot, TrustedRolloutHealthStoreError> {
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        let snapshot = {
            let meta = write.open_table(META_TABLE).map_err(database_error)?;
            verify_meta(&meta, self.trust_root)?;
            let seed = read_seed(&meta)?;
            let head = read_head_id(&meta)?.ok_or(TrustedRolloutHealthStoreError::MissingHead)?;
            if head != expected_bundle_id {
                return Err(TrustedRolloutHealthStoreError::StaleBundle);
            }
            let mut records = write.open_table(HEALTH_TABLE).map_err(database_error)?;
            let bytes = records
                .get(head.as_bytes().as_slice())
                .map_err(database_error)?
                .ok_or(TrustedRolloutHealthStoreError::MissingHeadRecord)?
                .value()
                .to_vec();
            let (mut record, bundle) = decode_record(&bytes, &seed, self.trust_root)?;
            if bundle.bundle_id()? != head {
                return Err(TrustedRolloutHealthStoreError::RecordIdBinding);
            }
            record.content.counters.increment(event)?;
            let encoded = encode_record(record.content.clone(), &seed)?;
            records
                .insert(head.as_bytes().as_slice(), encoded.as_slice())
                .map_err(database_error)?;
            health_snapshot(&bundle, record.content.counters)?
        };
        write.commit().map_err(database_error)?;
        Ok(snapshot)
    }

    pub fn current_snapshot(
        &self,
    ) -> Result<Option<TrustedRolloutHealthReadOnlySnapshot>, TrustedRolloutHealthStoreError> {
        let read = self.database.begin_read().map_err(database_error)?;
        read_snapshot(&read, self.trust_root, None)
    }

    fn current_record(
        &self,
    ) -> Result<
        Option<(TrustedReleasePolicyBundle, RolloutHealthCounters)>,
        TrustedRolloutHealthStoreError,
    > {
        let read = self.database.begin_read().map_err(database_error)?;
        let meta = read.open_table(META_TABLE).map_err(database_error)?;
        verify_meta(&meta, self.trust_root)?;
        let seed = read_seed(&meta)?;
        let Some(head) = read_head_id(&meta)? else {
            let records = read.open_table(HEALTH_TABLE).map_err(database_error)?;
            if !records.is_empty().map_err(database_error)? {
                return Err(TrustedRolloutHealthStoreError::MissingHead);
            }
            return Ok(None);
        };
        let records = read.open_table(HEALTH_TABLE).map_err(database_error)?;
        let bytes = records
            .get(head.as_bytes().as_slice())
            .map_err(database_error)?
            .ok_or(TrustedRolloutHealthStoreError::MissingHeadRecord)?
            .value()
            .to_vec();
        let (record, bundle) = decode_record(&bytes, &seed, self.trust_root)?;
        if bundle.bundle_id()? != head {
            return Err(TrustedRolloutHealthStoreError::RecordIdBinding);
        }
        Ok(Some((bundle, record.content.counters)))
    }

    fn seed(&self) -> Result<[u8; 32], TrustedRolloutHealthStoreError> {
        let read = self.database.begin_read().map_err(database_error)?;
        let meta = read.open_table(META_TABLE).map_err(database_error)?;
        verify_meta(&meta, self.trust_root)?;
        read_seed(&meta)
    }

    fn initialize(&self) -> Result<(), TrustedRolloutHealthStoreError> {
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
            let stored_schema = meta
                .get(SCHEMA_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match stored_schema {
                Some(value) if value.as_slice() == [SCHEMA_VERSION] => {}
                Some(_) => return Err(TrustedRolloutHealthStoreError::DatabaseSchema),
                None => {
                    meta.insert(SCHEMA_KEY, [SCHEMA_VERSION].as_slice())
                        .map_err(database_error)?;
                }
            }
            let stored_trust_root = meta
                .get(TRUST_ROOT_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match stored_trust_root {
                Some(value) if value.as_slice() == self.trust_root.as_bytes() => {}
                Some(_) => return Err(TrustedRolloutHealthStoreError::TrustRootMismatch),
                None => {
                    meta.insert(TRUST_ROOT_KEY, self.trust_root.as_bytes().as_slice())
                        .map_err(database_error)?;
                }
            }
            if meta
                .get(INSTALLATION_SEED_KEY)
                .map_err(database_error)?
                .is_none()
            {
                let mut seed = [0_u8; 32];
                getrandom::fill(&mut seed).map_err(TrustedRolloutHealthStoreError::Random)?;
                meta.insert(INSTALLATION_SEED_KEY, seed.as_slice())
                    .map_err(database_error)?;
            }
        }
        write.open_table(HEALTH_TABLE).map_err(database_error)?;
        write.commit().map_err(database_error)?;
        Ok(())
    }

    #[cfg(test)]
    fn corrupt_head_for_test(&self) -> Result<(), TrustedRolloutHealthStoreError> {
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let meta = write.open_table(META_TABLE).map_err(database_error)?;
            let head = read_head_id(&meta)?.ok_or(TrustedRolloutHealthStoreError::MissingHead)?;
            let mut records = write.open_table(HEALTH_TABLE).map_err(database_error)?;
            let mut bytes = records
                .get(head.as_bytes().as_slice())
                .map_err(database_error)?
                .ok_or(TrustedRolloutHealthStoreError::MissingHeadRecord)?
                .value()
                .to_vec();
            let last = bytes
                .last_mut()
                .ok_or(TrustedRolloutHealthStoreError::RecordLength(0))?;
            *last ^= 1;
            records
                .insert(head.as_bytes().as_slice(), bytes.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)
    }
}

impl fmt::Debug for TrustedRolloutHealthStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TrustedRolloutHealthStore")
            .field("trust_root", &self.trust_root)
            .finish_non_exhaustive()
    }
}

pub fn inspect_existing_trusted_rollout_health_store(
    installation_local_directory: impl AsRef<Path>,
    trust_root: ReleaseTrustRoot,
    expected_bundle: &TrustedReleasePolicyBundle,
) -> Result<Option<TrustedRolloutHealthReadOnlySnapshot>, TrustedRolloutHealthStoreError> {
    expected_bundle.verify(trust_root)?;
    let requested = installation_local_directory.as_ref();
    let metadata = match fs::symlink_metadata(requested) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(TrustedRolloutHealthStoreError::Io {
                action: "inspect trusted rollout-health directory",
                source,
            });
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(TrustedRolloutHealthStoreError::UnsafeLocalDirectory);
    }
    let canonical =
        fs::canonicalize(requested).map_err(|source| TrustedRolloutHealthStoreError::Io {
            action: "canonicalize trusted rollout-health directory",
            source,
        })?;
    let database_path = canonical.join(DATABASE_FILE);
    let database_metadata = match fs::symlink_metadata(&database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(TrustedRolloutHealthStoreError::Io {
                action: "inspect trusted rollout-health database",
                source,
            });
        }
    };
    if !database_metadata.is_file() || database_metadata.file_type().is_symlink() {
        return Err(TrustedRolloutHealthStoreError::UnsafeDatabaseFile);
    }
    let database = ReadOnlyDatabase::open(&database_path).map_err(database_error)?;
    let read = database.begin_read().map_err(database_error)?;
    read_snapshot(&read, trust_root, Some(expected_bundle))
}

fn read_snapshot(
    read: &ReadTransaction,
    trust_root: ReleaseTrustRoot,
    expected_bundle: Option<&TrustedReleasePolicyBundle>,
) -> Result<Option<TrustedRolloutHealthReadOnlySnapshot>, TrustedRolloutHealthStoreError> {
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(&meta, trust_root)?;
    let seed = read_seed(&meta)?;
    let records = read.open_table(HEALTH_TABLE).map_err(database_error)?;
    let count = records.len().map_err(database_error)?;
    let head = match read_head_id(&meta)? {
        Some(head) => head,
        None if count == 0 => return Ok(None),
        None => return Err(TrustedRolloutHealthStoreError::MissingHead),
    };
    if count == 0 {
        return Err(TrustedRolloutHealthStoreError::MissingHeadRecord);
    }
    if count > MAX_TRUSTED_HEALTH_RECORDS {
        return Err(TrustedRolloutHealthStoreError::RecordCapacity);
    }
    let mut chain = Vec::with_capacity(count as usize);
    for entry in records.iter().map_err(database_error)? {
        let (key, value) = entry.map_err(database_error)?;
        let (record, bundle) = decode_record(value.value(), &seed, trust_root)?;
        if key.value() != bundle.bundle_id()?.as_bytes() {
            return Err(TrustedRolloutHealthStoreError::RecordIdBinding);
        }
        chain.push((bundle, record.content.counters));
    }
    chain.sort_by_key(|(bundle, _)| bundle.policy().policy_epoch());
    for (index, (bundle, _)) in chain.iter().enumerate() {
        let expected_epoch = u64::try_from(index)
            .map_err(|_| TrustedRolloutHealthStoreError::RecordCapacity)?
            .checked_add(1)
            .ok_or(TrustedRolloutHealthStoreError::RecordCapacity)?;
        if bundle.policy().policy_epoch() != expected_epoch {
            return Err(TrustedRolloutHealthStoreError::PolicyChainGap);
        }
        if let Some((previous, _)) = index.checked_sub(1).and_then(|value| chain.get(value)) {
            bundle.verify_successor_of(previous)?;
        } else if bundle.policy().previous_policy_id().is_some()
            || bundle.keyring().keyring_epoch() != 1
            || bundle.keyring().previous_keyring_id().is_some()
        {
            return Err(TrustedRolloutHealthStoreError::InvalidGenesisBundle);
        }
    }
    let (current, counters) = chain
        .last()
        .ok_or(TrustedRolloutHealthStoreError::MissingHeadRecord)?;
    if current.bundle_id()? != head {
        return Err(TrustedRolloutHealthStoreError::HeadIsNotChainTip);
    }
    if let Some(expected) = expected_bundle
        && (expected.bundle_id()? != head || expected.encode()? != current.encode()?)
    {
        return Err(TrustedRolloutHealthStoreError::ExpectedBundleMismatch);
    }
    Ok(Some(TrustedRolloutHealthReadOnlySnapshot {
        bundle_id: current.bundle_id()?,
        evaluation: evaluate_cohort(seed, current.policy(), current.policy().policy_id()?)?,
        health: health_snapshot(current, *counters)?,
    }))
}

fn validate_bundle_chain(
    bundles: &[TrustedReleasePolicyBundle],
    trust_root: ReleaseTrustRoot,
) -> Result<(), TrustedRolloutHealthStoreError> {
    if bundles.is_empty() || bundles.len() > MAX_TRUSTED_HEALTH_RECORDS as usize {
        return Err(TrustedRolloutHealthStoreError::InvalidBundleChainLength(
            bundles.len(),
        ));
    }
    for (index, bundle) in bundles.iter().enumerate() {
        bundle.verify(trust_root)?;
        let expected_epoch = u64::try_from(index)
            .map_err(|_| TrustedRolloutHealthStoreError::RecordCapacity)?
            .checked_add(1)
            .ok_or(TrustedRolloutHealthStoreError::RecordCapacity)?;
        if bundle.policy().policy_epoch() != expected_epoch {
            return Err(TrustedRolloutHealthStoreError::PolicyChainGap);
        }
        if let Some(previous) = index.checked_sub(1).and_then(|value| bundles.get(value)) {
            bundle.verify_successor_of(previous)?;
        } else if bundle.policy().previous_policy_id().is_some()
            || bundle.keyring().keyring_epoch() != 1
            || bundle.keyring().previous_keyring_id().is_some()
        {
            return Err(TrustedRolloutHealthStoreError::InvalidGenesisBundle);
        }
    }
    Ok(())
}

fn activation_outcome(
    installed: bool,
    seed: [u8; 32],
    bundle: &TrustedReleasePolicyBundle,
    counters: RolloutHealthCounters,
) -> Result<TrustedHealthActivationOutcome, TrustedRolloutHealthStoreError> {
    let bundle_id = bundle.bundle_id()?;
    let evaluation = evaluate_cohort(seed, bundle.policy(), bundle.policy().policy_id()?)?;
    let health = health_snapshot(bundle, counters)?;
    Ok(if installed {
        TrustedHealthActivationOutcome::Installed {
            bundle_id,
            evaluation,
            health,
        }
    } else {
        TrustedHealthActivationOutcome::AlreadyCurrent {
            bundle_id,
            evaluation,
            health,
        }
    })
}

fn health_snapshot(
    bundle: &TrustedReleasePolicyBundle,
    counters: RolloutHealthCounters,
) -> Result<RolloutHealthSnapshot, TrustedRolloutHealthStoreError> {
    counters.zero_tolerance_events()?;
    Ok(RolloutHealthSnapshot {
        policy_id: bundle.policy().policy_id()?,
        release_artifact_id: bundle.policy().release_artifact_id(),
        phase: bundle.policy().phase(),
        cohort_upper_bound_basis_points: bundle.policy().cohort_upper_bound_basis_points(),
        counters,
    })
}

fn encode_record(
    content: TrustedHealthContent,
    seed: &[u8; 32],
) -> Result<Vec<u8>, TrustedRolloutHealthStoreError> {
    validate_content(&content)?;
    let mac = record_mac(seed, &content)?;
    let encoded = postcard::to_allocvec(&StoredTrustedHealthRecord { content, mac })
        .map_err(TrustedRolloutHealthStoreError::Codec)?;
    if encoded.is_empty() || encoded.len() > MAX_TRUSTED_HEALTH_RECORD_BYTES {
        return Err(TrustedRolloutHealthStoreError::RecordLength(encoded.len()));
    }
    Ok(encoded)
}

fn decode_record(
    encoded: &[u8],
    seed: &[u8; 32],
    trust_root: ReleaseTrustRoot,
) -> Result<(StoredTrustedHealthRecord, TrustedReleasePolicyBundle), TrustedRolloutHealthStoreError>
{
    if encoded.is_empty() || encoded.len() > MAX_TRUSTED_HEALTH_RECORD_BYTES {
        return Err(TrustedRolloutHealthStoreError::RecordLength(encoded.len()));
    }
    let record: StoredTrustedHealthRecord =
        postcard::from_bytes(encoded).map_err(TrustedRolloutHealthStoreError::Codec)?;
    validate_content(&record.content)?;
    if record.mac != record_mac(seed, &record.content)? {
        return Err(TrustedRolloutHealthStoreError::RecordAuthentication);
    }
    if postcard::to_allocvec(&record).map_err(TrustedRolloutHealthStoreError::Codec)? != encoded {
        return Err(TrustedRolloutHealthStoreError::NonCanonicalRecord);
    }
    let bundle = TrustedReleasePolicyBundle::decode_and_verify(
        &record.content.trusted_policy_bundle,
        trust_root,
    )?;
    Ok((record, bundle))
}

fn validate_content(content: &TrustedHealthContent) -> Result<(), TrustedRolloutHealthStoreError> {
    if content.version != RECORD_VERSION || content.trusted_policy_bundle.is_empty() {
        return Err(TrustedRolloutHealthStoreError::InvalidRecord);
    }
    content.counters.zero_tolerance_events()?;
    Ok(())
}

fn record_mac(
    seed: &[u8; 32],
    content: &TrustedHealthContent,
) -> Result<[u8; 32], TrustedRolloutHealthStoreError> {
    let encoded = postcard::to_allocvec(content).map_err(TrustedRolloutHealthStoreError::Codec)?;
    let mut input = Vec::with_capacity(RECORD_MAC_DOMAIN.len() + encoded.len());
    input.extend_from_slice(RECORD_MAC_DOMAIN);
    input.extend_from_slice(&encoded);
    Ok(*blake3::keyed_hash(seed, &input).as_bytes())
}

fn prepare_real_directory(
    path: &Path,
) -> Result<std::path::PathBuf, TrustedRolloutHealthStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(TrustedRolloutHealthStoreError::UnsafeLocalDirectory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|source| TrustedRolloutHealthStoreError::Io {
                action: "create trusted rollout-health directory",
                source,
            })?;
        }
        Err(source) => {
            return Err(TrustedRolloutHealthStoreError::Io {
                action: "inspect trusted rollout-health directory",
                source,
            });
        }
    }
    fs::canonicalize(path).map_err(|source| TrustedRolloutHealthStoreError::Io {
        action: "canonicalize trusted rollout-health directory",
        source,
    })
}

fn verify_meta(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    trust_root: ReleaseTrustRoot,
) -> Result<(), TrustedRolloutHealthStoreError> {
    let schema = meta
        .get(SCHEMA_KEY)
        .map_err(database_error)?
        .ok_or(TrustedRolloutHealthStoreError::DatabaseSchema)?;
    if schema.value() != [SCHEMA_VERSION] {
        return Err(TrustedRolloutHealthStoreError::DatabaseSchema);
    }
    let pinned = meta
        .get(TRUST_ROOT_KEY)
        .map_err(database_error)?
        .ok_or(TrustedRolloutHealthStoreError::TrustRootMismatch)?;
    if pinned.value() != trust_root.as_bytes() {
        return Err(TrustedRolloutHealthStoreError::TrustRootMismatch);
    }
    Ok(())
}

fn read_seed(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
) -> Result<[u8; 32], TrustedRolloutHealthStoreError> {
    meta.get(INSTALLATION_SEED_KEY)
        .map_err(database_error)?
        .ok_or(TrustedRolloutHealthStoreError::MissingInstallationSeed)?
        .value()
        .try_into()
        .map_err(|_| TrustedRolloutHealthStoreError::InstallationSeedLength)
}

fn read_head_id(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
) -> Result<Option<TrustedPolicyBundleId>, TrustedRolloutHealthStoreError> {
    let Some(bytes) = meta.get(HEAD_KEY).map_err(database_error)? else {
        return Ok(None);
    };
    let bytes: [u8; 32] = bytes
        .value()
        .try_into()
        .map_err(|_| TrustedRolloutHealthStoreError::InvalidHeadLength)?;
    Ok(Some(TrustedPolicyBundleId::from_bytes_for_store(bytes)))
}

fn database_error(error: impl fmt::Display) -> TrustedRolloutHealthStoreError {
    TrustedRolloutHealthStoreError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum TrustedRolloutHealthStoreError {
    #[error("trusted rollout-health I/O failed while trying to {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("trusted rollout-health database operation failed: {0}")]
    Database(String),
    #[error("trusted rollout-health directory must be a real directory")]
    UnsafeLocalDirectory,
    #[error("trusted rollout-health database must be a real file")]
    UnsafeDatabaseFile,
    #[error("trusted rollout-health database schema is unsupported")]
    DatabaseSchema,
    #[error("trusted rollout-health trust root does not match")]
    TrustRootMismatch,
    #[error("trusted rollout-health installation seed is missing")]
    MissingInstallationSeed,
    #[error("trusted rollout-health installation seed has invalid length")]
    InstallationSeedLength,
    #[error("trusted rollout-health head is missing")]
    MissingHead,
    #[error("trusted rollout-health head has invalid length")]
    InvalidHeadLength,
    #[error("trusted rollout-health head record is missing")]
    MissingHeadRecord,
    #[error("trusted rollout-health head is not the chain tip")]
    HeadIsNotChainTip,
    #[error("trusted rollout-health record ID is not bound to the bundle")]
    RecordIdBinding,
    #[error("trusted rollout-health record has invalid length {0}")]
    RecordLength(usize),
    #[error("trusted rollout-health record is invalid")]
    InvalidRecord,
    #[error("trusted rollout-health record authentication failed")]
    RecordAuthentication,
    #[error("trusted rollout-health record is not canonically encoded")]
    NonCanonicalRecord,
    #[error("trusted rollout-health record encoding failed: {0}")]
    Codec(postcard::Error),
    #[error("trusted rollout-health record capacity is exhausted")]
    RecordCapacity,
    #[error("trusted rollout-health bundle chain has invalid length {0}")]
    InvalidBundleChainLength(usize),
    #[error("trusted rollout-health policy chain has a gap or fork")]
    PolicyChainGap,
    #[error("trusted rollout-health bundle is not an exact genesis")]
    InvalidGenesisBundle,
    #[error("trusted rollout-health expected bundle does not match the head")]
    ExpectedBundleMismatch,
    #[error("trusted rollout-health current bundle is not present in the supplied trusted chain")]
    CurrentBundleNotInChain,
    #[error("trusted rollout-health update is bound to a stale bundle")]
    StaleBundle,
    #[error("trusted rollout-health bundle ID collision")]
    BundleIdCollision,
    #[error("trusted rollout-health retained record unexpectedly reused an ID")]
    UnexpectedRetainedRecord,
    #[error("trusted rollout-health head changed concurrently")]
    ConcurrentHeadChanged,
    #[error("secure random generation failed: {0}")]
    Random(getrandom::Error),
    #[cfg(test)]
    #[error("test-only installation seed cannot be changed after bundle activation")]
    TestSeedAfterActivation,
    #[error(transparent)]
    Rollout(#[from] RolloutError),
    #[error(transparent)]
    TrustedPolicy(#[from] TrustedPolicyError),
    #[error(transparent)]
    Trust(#[from] ReleaseTrustError),
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        ReleaseArtifactId, RolloutDisposition, RolloutPhase,
        trust::{
            ReleaseSigningKeyId, ReleaseSigningKeySpec, ReleaseSigningPermissions,
            SignedReleaseKeyring,
        },
    };

    fn key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    fn root(key: &SigningKey) -> Result<ReleaseTrustRoot, ReleaseTrustError> {
        ReleaseTrustRoot::from_bytes(key.verifying_key().to_bytes())
    }

    fn bundles() -> Result<
        (
            ReleaseTrustRoot,
            TrustedReleasePolicyBundle,
            TrustedReleasePolicyBundle,
        ),
        TrustedRolloutHealthStoreError,
    > {
        let root_key = key(70);
        let old = key(71);
        let new = key(72);
        let trust_root = root(&root_key)?;
        let first_keyring = SignedReleaseKeyring::issue_genesis(
            &root_key,
            &[ReleaseSigningKeySpec::from_signing_key(
                &old,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let first = TrustedReleasePolicyBundle::issue_genesis_hold(
            trust_root,
            &first_keyring,
            &old,
            ReleaseArtifactId::from_bytes([73_u8; 32])?,
        )?;
        let second_keyring = SignedReleaseKeyring::issue_successor(
            &root_key,
            &first_keyring,
            &[ReleaseSigningKeySpec::from_signing_key(
                &new,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
            &[ReleaseSigningKeyId::from_public_key(
                old.verifying_key().to_bytes(),
            )?],
        )?;
        let second = TrustedReleasePolicyBundle::issue_successor(
            trust_root,
            &second_keyring,
            &new,
            &first,
            ReleaseArtifactId::from_bytes([73_u8; 32])?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            100,
        )?;
        Ok((trust_root, first, second))
    }

    #[test]
    fn operational_key_rotation_preserves_authenticated_health_chain()
    -> Result<(), TrustedRolloutHealthStoreError> {
        let directory = TempDir::new().map_err(|source| TrustedRolloutHealthStoreError::Io {
            action: "create rotation test directory",
            source,
        })?;
        let (trust_root, first, second) = bundles()?;
        let store = TrustedRolloutHealthStore::open(directory.path(), trust_root)?;
        assert!(matches!(
            store.activate_bundle(&first)?,
            TrustedHealthActivationOutcome::Installed { .. }
        ));
        assert!(matches!(
            store.activate_bundle(&second)?,
            TrustedHealthActivationOutcome::Installed { .. }
        ));
        let second_id = second.bundle_id()?;
        store.record_health(second_id, RolloutHealthEvent::RuntimeStartSucceeded)?;
        let health = store.record_health(second_id, RolloutHealthEvent::CanonicalSyncCompleted)?;
        assert_eq!(health.counters.runtime_start_succeeded, 1);
        assert_eq!(health.counters.canonical_sync_completed, 1);
        drop(store);
        let snapshot =
            inspect_existing_trusted_rollout_health_store(directory.path(), trust_root, &second)?
                .ok_or(TrustedRolloutHealthStoreError::MissingHead)?;
        assert_eq!(snapshot.health, health);
        assert_eq!(snapshot.evaluation.policy_epoch, 2);
        Ok(())
    }

    #[test]
    fn full_verified_chain_bootstraps_and_resumes_idempotently()
    -> Result<(), TrustedRolloutHealthStoreError> {
        let directory = TempDir::new().map_err(|source| TrustedRolloutHealthStoreError::Io {
            action: "create chain bootstrap test directory",
            source,
        })?;
        let (trust_root, first, second) = bundles()?;
        let chain = vec![first, second.clone()];
        let store = TrustedRolloutHealthStore::open(directory.path(), trust_root)?;
        let installed = store.synchronize_bundle_chain(&chain)?;
        assert_eq!(installed.bundle_id, second.bundle_id()?);
        assert_eq!(installed.evaluation.policy_epoch, 2);
        let resumed = store.synchronize_bundle_chain(&chain)?;
        assert_eq!(resumed, installed);
        Ok(())
    }

    #[test]
    fn read_only_missing_store_is_not_created() -> Result<(), TrustedRolloutHealthStoreError> {
        let parent = TempDir::new().map_err(|source| TrustedRolloutHealthStoreError::Io {
            action: "create missing-store test parent",
            source,
        })?;
        let missing = parent.path().join("absent");
        let (trust_root, first, _) = bundles()?;
        assert_eq!(
            inspect_existing_trusted_rollout_health_store(&missing, trust_root, &first)?,
            None
        );
        assert!(!missing.exists());
        Ok(())
    }

    #[test]
    fn stale_bundle_foreign_root_and_corruption_fail_closed()
    -> Result<(), TrustedRolloutHealthStoreError> {
        let directory = TempDir::new().map_err(|source| TrustedRolloutHealthStoreError::Io {
            action: "create fail-closed test directory",
            source,
        })?;
        let (trust_root, first, second) = bundles()?;
        let store = TrustedRolloutHealthStore::open(directory.path(), trust_root)?;
        store.activate_bundle(&first)?;
        store.activate_bundle(&second)?;
        assert!(matches!(
            store.record_health(
                first.bundle_id()?,
                RolloutHealthEvent::RuntimeStartSucceeded
            ),
            Err(TrustedRolloutHealthStoreError::StaleBundle)
        ));
        assert!(
            inspect_existing_trusted_rollout_health_store(
                directory.path(),
                root(&key(74))?,
                &second,
            )
            .is_err()
        );
        store.corrupt_head_for_test()?;
        drop(store);
        assert!(
            inspect_existing_trusted_rollout_health_store(directory.path(), trust_root, &second,)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn reopening_with_a_foreign_root_fails_closed() -> Result<(), TrustedRolloutHealthStoreError> {
        let directory = TempDir::new().map_err(|source| TrustedRolloutHealthStoreError::Io {
            action: "create root-pinning test directory",
            source,
        })?;
        let (trust_root, first, _) = bundles()?;
        let store = TrustedRolloutHealthStore::open(directory.path(), trust_root)?;
        store.activate_bundle(&first)?;
        drop(store);
        assert!(TrustedRolloutHealthStore::open(directory.path(), root(&key(75))?).is_err());
        Ok(())
    }
}
