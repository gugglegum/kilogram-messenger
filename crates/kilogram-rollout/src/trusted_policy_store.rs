//! Atomic explicit-file inbox for rotation-aware release-policy bundles.

use std::{fs, path::Path};

use redb::{
    Database, Durability, ReadOnlyDatabase, ReadTransaction, ReadableDatabase, ReadableTable,
    ReadableTableMetadata, TableDefinition,
};
use thiserror::Error;

use crate::{
    trust::{ReleaseTrustError, ReleaseTrustRoot},
    trusted_policy::{
        MAX_TRUSTED_RELEASE_POLICY_BUNDLE_BYTES, TrustedPolicyBundleId, TrustedPolicyError,
        TrustedReleasePolicyBundle,
    },
};

const DATABASE_FILE: &str = "rollout-trusted-policy-v1.redb";
const META_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("rollout-trusted-policy-meta-v1");
const BUNDLE_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("rollout-trusted-policy-bundles-v1");
const SCHEMA_KEY: &str = "schema-version";
const TRUST_ROOT_KEY: &str = "release-trust-root";
const HEAD_KEY: &str = "trusted-policy-head";
const SCHEMA_VERSION: u8 = 1;

pub const MAX_DURABLE_TRUSTED_POLICY_BUNDLES: u64 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitTrustedPolicyImport {
    NotAuthorized,
    UserOrTestOperatorRequested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedPolicyImportOutcome {
    Installed {
        bundle_id: TrustedPolicyBundleId,
        policy_epoch: u64,
        keyring_epoch: u64,
    },
    AlreadyCurrent {
        bundle_id: TrustedPolicyBundleId,
        policy_epoch: u64,
        keyring_epoch: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedPolicyReadOnlySnapshot {
    pub current_bundle: Option<TrustedReleasePolicyBundle>,
}

/// Inspect an existing trusted-policy store without creating or modifying it.
/// A missing directory or database is an ordinary dormant state (`None`).
pub fn inspect_existing_trusted_policy_store(
    installation_local_directory: impl AsRef<Path>,
    trust_root: ReleaseTrustRoot,
) -> Result<Option<TrustedPolicyReadOnlySnapshot>, TrustedPolicyStoreError> {
    let requested = installation_local_directory.as_ref();
    let metadata = match fs::symlink_metadata(requested) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(TrustedPolicyStoreError::Io {
                action: "inspect trusted-policy store directory",
                source,
            });
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(TrustedPolicyStoreError::UnsafeLocalDirectory);
    }
    let canonical = fs::canonicalize(requested).map_err(|source| TrustedPolicyStoreError::Io {
        action: "canonicalize trusted-policy store directory",
        source,
    })?;
    let database_path = canonical.join(DATABASE_FILE);
    let database_metadata = match fs::symlink_metadata(&database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(TrustedPolicyStoreError::Io {
                action: "inspect trusted-policy database file",
                source,
            });
        }
    };
    if !database_metadata.is_file() || database_metadata.file_type().is_symlink() {
        return Err(TrustedPolicyStoreError::UnsafeDatabaseFile);
    }
    let database = ReadOnlyDatabase::open(&database_path).map_err(database_error)?;
    let read = database.begin_read().map_err(database_error)?;
    Ok(Some(TrustedPolicyReadOnlySnapshot {
        current_bundle: current_bundle_from_read(&read, trust_root)?,
    }))
}

pub struct TrustedPolicyStore {
    database: Database,
    trust_root: ReleaseTrustRoot,
}

impl TrustedPolicyStore {
    pub fn open(
        installation_local_directory: impl AsRef<Path>,
        trust_root: ReleaseTrustRoot,
    ) -> Result<Self, TrustedPolicyStoreError> {
        let directory = prepare_real_directory(installation_local_directory.as_ref())?;
        let database_path = directory.join(DATABASE_FILE);
        if let Ok(metadata) = fs::symlink_metadata(&database_path)
            && (!metadata.is_file() || metadata.file_type().is_symlink())
        {
            return Err(TrustedPolicyStoreError::UnsafeDatabaseFile);
        }
        let database = Database::create(&database_path).map_err(database_error)?;
        let store = Self {
            database,
            trust_root,
        };
        store.initialize()?;
        let _ = store.current_bundle()?;
        Ok(store)
    }

    pub fn import_explicit_file(
        &self,
        policy_file: impl AsRef<Path>,
        authorization: ExplicitTrustedPolicyImport,
    ) -> Result<TrustedPolicyImportOutcome, TrustedPolicyStoreError> {
        if authorization != ExplicitTrustedPolicyImport::UserOrTestOperatorRequested {
            return Err(TrustedPolicyStoreError::ExplicitImportRequired);
        }
        let bytes = read_bounded_real_file(policy_file.as_ref())?;
        let candidate = TrustedReleasePolicyBundle::decode_and_verify(&bytes, self.trust_root)?;
        let candidate_id = candidate.bundle_id()?;
        let current = self.current_bundle()?;
        if let Some(current) = &current {
            if current.bundle_id()? == candidate_id {
                if current.encode()? != bytes {
                    return Err(TrustedPolicyStoreError::BundleIdCollision);
                }
                return outcome(false, &candidate);
            }
            candidate.verify_successor_of(current)?;
        } else if candidate.policy().policy_epoch() != 1
            || candidate.policy().previous_policy_id().is_some()
            || candidate.keyring().keyring_epoch() != 1
            || candidate.keyring().previous_keyring_id().is_some()
        {
            return Err(TrustedPolicyStoreError::InvalidGenesisBundle);
        }

        let expected_head = current
            .as_ref()
            .map(TrustedReleasePolicyBundle::bundle_id)
            .transpose()?;
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
            let actual_head = read_head_id(&meta)?;
            if actual_head != expected_head {
                return Err(TrustedPolicyStoreError::ConcurrentHeadChanged);
            }
            let mut bundles = write.open_table(BUNDLE_TABLE).map_err(database_error)?;
            if bundles.len().map_err(database_error)? >= MAX_DURABLE_TRUSTED_POLICY_BUNDLES {
                return Err(TrustedPolicyStoreError::BundleCapacity);
            }
            if let Some(existing) = bundles
                .get(candidate_id.as_bytes().as_slice())
                .map_err(database_error)?
            {
                if existing.value() != bytes.as_slice() {
                    return Err(TrustedPolicyStoreError::BundleIdCollision);
                }
            } else {
                bundles
                    .insert(candidate_id.as_bytes().as_slice(), bytes.as_slice())
                    .map_err(database_error)?;
            }
            meta.insert(HEAD_KEY, candidate_id.as_bytes().as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)?;
        outcome(true, &candidate)
    }

    pub fn current_bundle(
        &self,
    ) -> Result<Option<TrustedReleasePolicyBundle>, TrustedPolicyStoreError> {
        let read = self.database.begin_read().map_err(database_error)?;
        current_bundle_from_read(&read, self.trust_root)
    }

    /// Return the complete verified chain from genesis through the current
    /// trusted-policy tip. This is used to migrate installation-local health
    /// state without trusting only the newest operational signing key.
    pub fn bundle_chain(&self) -> Result<Vec<TrustedReleasePolicyBundle>, TrustedPolicyStoreError> {
        let read = self.database.begin_read().map_err(database_error)?;
        bundle_chain_from_read(&read, self.trust_root)
    }

    fn initialize(&self) -> Result<(), TrustedPolicyStoreError> {
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut meta = write.open_table(META_TABLE).map_err(database_error)?;
            let existing_schema = meta
                .get(SCHEMA_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match existing_schema {
                Some(value) if value.as_slice() == [SCHEMA_VERSION] => {}
                Some(_) => return Err(TrustedPolicyStoreError::DatabaseSchema),
                None => {
                    meta.insert(SCHEMA_KEY, [SCHEMA_VERSION].as_slice())
                        .map_err(database_error)?;
                }
            }
            let existing_root = meta
                .get(TRUST_ROOT_KEY)
                .map_err(database_error)?
                .map(|value| value.value().to_vec());
            match existing_root {
                Some(value) if value.as_slice() == self.trust_root.as_bytes() => {}
                Some(_) => return Err(TrustedPolicyStoreError::TrustRootMismatch),
                None => {
                    meta.insert(TRUST_ROOT_KEY, self.trust_root.as_bytes().as_slice())
                        .map_err(database_error)?;
                }
            }
        }
        write.open_table(BUNDLE_TABLE).map_err(database_error)?;
        write.commit().map_err(database_error)
    }

    #[cfg(test)]
    fn corrupt_head_bundle_for_test(&self) -> Result<(), TrustedPolicyStoreError> {
        let current = self
            .current_bundle()?
            .ok_or(TrustedPolicyStoreError::MissingHead)?;
        let id = current.bundle_id()?;
        let mut bytes = current.encode()?;
        let last = bytes
            .last_mut()
            .ok_or(TrustedPolicyStoreError::InputLength(0))?;
        *last ^= 1;
        let mut write = self.database.begin_write().map_err(database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        {
            let mut bundles = write.open_table(BUNDLE_TABLE).map_err(database_error)?;
            bundles
                .insert(id.as_bytes().as_slice(), bytes.as_slice())
                .map_err(database_error)?;
        }
        write.commit().map_err(database_error)
    }
}

fn current_bundle_from_read(
    read: &ReadTransaction,
    trust_root: ReleaseTrustRoot,
) -> Result<Option<TrustedReleasePolicyBundle>, TrustedPolicyStoreError> {
    Ok(bundle_chain_from_read(read, trust_root)?.last().cloned())
}

fn bundle_chain_from_read(
    read: &ReadTransaction,
    trust_root: ReleaseTrustRoot,
) -> Result<Vec<TrustedReleasePolicyBundle>, TrustedPolicyStoreError> {
    let meta = read.open_table(META_TABLE).map_err(database_error)?;
    verify_meta(&meta, trust_root)?;
    let head = read_head_id(&meta)?;
    let bundles = read.open_table(BUNDLE_TABLE).map_err(database_error)?;
    let count = bundles.len().map_err(database_error)?;
    if count > MAX_DURABLE_TRUSTED_POLICY_BUNDLES {
        return Err(TrustedPolicyStoreError::BundleCapacity);
    }
    if count == 0 {
        if head.is_some() {
            return Err(TrustedPolicyStoreError::MissingHeadBundle);
        }
        return Ok(Vec::new());
    }
    let head = head.ok_or(TrustedPolicyStoreError::MissingHead)?;
    let mut chain = Vec::with_capacity(count as usize);
    for entry in bundles.iter().map_err(database_error)? {
        let (key, value) = entry.map_err(database_error)?;
        let bundle = TrustedReleasePolicyBundle::decode_and_verify(value.value(), trust_root)?;
        if key.value() != bundle.bundle_id()?.as_bytes() {
            return Err(TrustedPolicyStoreError::BundleIdBinding);
        }
        chain.push(bundle);
    }
    chain.sort_by_key(|bundle| bundle.policy().policy_epoch());
    for (index, bundle) in chain.iter().enumerate() {
        let expected_epoch = u64::try_from(index)
            .map_err(|_| TrustedPolicyStoreError::BundleCapacity)?
            .checked_add(1)
            .ok_or(TrustedPolicyStoreError::BundleCapacity)?;
        if bundle.policy().policy_epoch() != expected_epoch {
            return Err(TrustedPolicyStoreError::PolicyChainGap);
        }
        if let Some(previous) = index.checked_sub(1).and_then(|value| chain.get(value)) {
            bundle.verify_successor_of(previous)?;
        } else if bundle.policy().previous_policy_id().is_some()
            || bundle.keyring().keyring_epoch() != 1
            || bundle.keyring().previous_keyring_id().is_some()
        {
            return Err(TrustedPolicyStoreError::InvalidGenesisBundle);
        }
    }
    let current = chain.last().ok_or(TrustedPolicyStoreError::MissingHead)?;
    if current.bundle_id()? != head {
        return Err(TrustedPolicyStoreError::HeadIsNotChainTip);
    }
    Ok(chain)
}

fn outcome(
    installed: bool,
    bundle: &TrustedReleasePolicyBundle,
) -> Result<TrustedPolicyImportOutcome, TrustedPolicyStoreError> {
    let fields = (
        bundle.bundle_id()?,
        bundle.policy().policy_epoch(),
        bundle.keyring().keyring_epoch(),
    );
    Ok(if installed {
        TrustedPolicyImportOutcome::Installed {
            bundle_id: fields.0,
            policy_epoch: fields.1,
            keyring_epoch: fields.2,
        }
    } else {
        TrustedPolicyImportOutcome::AlreadyCurrent {
            bundle_id: fields.0,
            policy_epoch: fields.1,
            keyring_epoch: fields.2,
        }
    })
}

fn read_bounded_real_file(path: &Path) -> Result<Vec<u8>, TrustedPolicyStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| TrustedPolicyStoreError::Io {
        action: "inspect explicit trusted-policy input",
        source,
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(TrustedPolicyStoreError::UnsafeInputFile);
    }
    let declared = usize::try_from(metadata.len())
        .map_err(|_| TrustedPolicyStoreError::InputLength(usize::MAX))?;
    if declared == 0 || declared > MAX_TRUSTED_RELEASE_POLICY_BUNDLE_BYTES {
        return Err(TrustedPolicyStoreError::InputLength(declared));
    }
    let bytes = fs::read(path).map_err(|source| TrustedPolicyStoreError::Io {
        action: "read explicit trusted-policy input",
        source,
    })?;
    let after = fs::symlink_metadata(path).map_err(|source| TrustedPolicyStoreError::Io {
        action: "reinspect explicit trusted-policy input",
        source,
    })?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || after.len() != metadata.len()
        || bytes.len() != declared
    {
        return Err(TrustedPolicyStoreError::InputChangedDuringRead);
    }
    Ok(bytes)
}

fn prepare_real_directory(path: &Path) -> Result<std::path::PathBuf, TrustedPolicyStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(TrustedPolicyStoreError::UnsafeLocalDirectory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|source| TrustedPolicyStoreError::Io {
                action: "create trusted-policy store directory",
                source,
            })?;
        }
        Err(source) => {
            return Err(TrustedPolicyStoreError::Io {
                action: "inspect trusted-policy store directory",
                source,
            });
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| TrustedPolicyStoreError::Io {
        action: "verify trusted-policy store directory",
        source,
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(TrustedPolicyStoreError::UnsafeLocalDirectory);
    }
    fs::canonicalize(path).map_err(|source| TrustedPolicyStoreError::Io {
        action: "canonicalize trusted-policy store directory",
        source,
    })
}

fn read_head_id(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
) -> Result<Option<TrustedPolicyBundleId>, TrustedPolicyStoreError> {
    let Some(bytes) = meta.get(HEAD_KEY).map_err(database_error)? else {
        return Ok(None);
    };
    let bytes: [u8; 32] = bytes
        .value()
        .try_into()
        .map_err(|_| TrustedPolicyStoreError::InvalidHeadLength)?;
    Ok(Some(TrustedPolicyBundleId::from_bytes_for_store(bytes)))
}

fn verify_meta(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
    trust_root: ReleaseTrustRoot,
) -> Result<(), TrustedPolicyStoreError> {
    let schema = meta
        .get(SCHEMA_KEY)
        .map_err(database_error)?
        .ok_or(TrustedPolicyStoreError::DatabaseSchema)?;
    if schema.value() != [SCHEMA_VERSION] {
        return Err(TrustedPolicyStoreError::DatabaseSchema);
    }
    let root = meta
        .get(TRUST_ROOT_KEY)
        .map_err(database_error)?
        .ok_or(TrustedPolicyStoreError::TrustRootMismatch)?;
    if root.value() != trust_root.as_bytes() {
        return Err(TrustedPolicyStoreError::TrustRootMismatch);
    }
    Ok(())
}

fn database_error(error: impl std::fmt::Display) -> TrustedPolicyStoreError {
    TrustedPolicyStoreError::Database(error.to_string())
}

#[derive(Debug, Error)]
pub enum TrustedPolicyStoreError {
    #[error("trusted-policy store I/O failed while trying to {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("trusted-policy database operation failed: {0}")]
    Database(String),
    #[error("trusted-policy store directory must be a real directory")]
    UnsafeLocalDirectory,
    #[error("trusted-policy database must be a real file")]
    UnsafeDatabaseFile,
    #[error("trusted-policy explicit input must be a real regular file")]
    UnsafeInputFile,
    #[error("trusted-policy explicit input has invalid length {0}")]
    InputLength(usize),
    #[error("trusted-policy explicit input changed while it was being read")]
    InputChangedDuringRead,
    #[error("trusted-policy import requires an explicit user or test-operator request")]
    ExplicitImportRequired,
    #[error("trusted-policy database schema is unsupported")]
    DatabaseSchema,
    #[error("trusted-policy store trust root does not match the pinned root")]
    TrustRootMismatch,
    #[error("trusted-policy store head has invalid length")]
    InvalidHeadLength,
    #[error("trusted-policy store contains records without a head")]
    MissingHead,
    #[error("trusted-policy store head record is missing")]
    MissingHeadBundle,
    #[error("trusted-policy record ID is not bound to its exact bytes")]
    BundleIdBinding,
    #[error("distinct trusted-policy bytes produced the same bundle ID")]
    BundleIdCollision,
    #[error("trusted-policy chain does not start with an exact genesis bundle")]
    InvalidGenesisBundle,
    #[error("trusted-policy chain has an epoch gap or fork")]
    PolicyChainGap,
    #[error("trusted-policy head is not the verified chain tip")]
    HeadIsNotChainTip,
    #[error("trusted-policy store changed concurrently")]
    ConcurrentHeadChanged,
    #[error("trusted-policy durable history capacity is exhausted")]
    BundleCapacity,
    #[error(transparent)]
    TrustedPolicy(#[from] TrustedPolicyError),
    #[error(transparent)]
    Trust(#[from] ReleaseTrustError),
    #[error(transparent)]
    Rollout(#[from] crate::RolloutError),
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use ed25519_dalek::SigningKey;
    use tempfile::{NamedTempFile, TempDir};

    use super::*;
    use crate::{
        ReleaseArtifactId, RolloutDisposition, RolloutError, RolloutPhase,
        trust::{
            ReleaseSigningKeyId, ReleaseSigningKeySpec, ReleaseSigningPermissions,
            ReleaseTrustError, SignedReleaseKeyring,
        },
    };

    fn key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    fn root(key: &SigningKey) -> Result<ReleaseTrustRoot, ReleaseTrustError> {
        ReleaseTrustRoot::from_bytes(key.verifying_key().to_bytes())
    }

    fn artifact(byte: u8) -> Result<ReleaseArtifactId, RolloutError> {
        ReleaseArtifactId::from_bytes([byte; 32])
    }

    fn write_bundle(
        parent: &TempDir,
        name: &str,
        bundle: &TrustedReleasePolicyBundle,
    ) -> Result<std::path::PathBuf, TrustedPolicyStoreError> {
        let path = parent.path().join(name);
        fs::write(&path, bundle.encode()?).map_err(|source| TrustedPolicyStoreError::Io {
            action: "write test bundle",
            source,
        })?;
        Ok(path)
    }

    fn genesis(
        root_key: &SigningKey,
        signer: &SigningKey,
    ) -> Result<(SignedReleaseKeyring, TrustedReleasePolicyBundle), TrustedPolicyStoreError> {
        let keyring = SignedReleaseKeyring::issue_genesis(
            root_key,
            &[ReleaseSigningKeySpec::from_signing_key(
                signer,
                ReleaseSigningPermissions::NORMAL_AND_CONTAINMENT,
            )],
        )?;
        let bundle = TrustedReleasePolicyBundle::issue_genesis_hold(
            root(root_key)?,
            &keyring,
            signer,
            artifact(1)?,
        )?;
        Ok((keyring, bundle))
    }

    #[test]
    fn explicit_import_is_atomic_idempotent_and_restart_stable()
    -> Result<(), TrustedPolicyStoreError> {
        let store_dir = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test store",
            source,
        })?;
        let inbox = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test inbox",
            source,
        })?;
        let root_key = key(51);
        let old = key(52);
        let new = key(53);
        let (first_keyring, first) = genesis(&root_key, &old)?;
        let first_path = write_bundle(&inbox, "first.policy", &first)?;
        let store = TrustedPolicyStore::open(store_dir.path(), root(&root_key)?)?;
        assert!(matches!(
            store.import_explicit_file(
                &first_path,
                ExplicitTrustedPolicyImport::UserOrTestOperatorRequested
            )?,
            TrustedPolicyImportOutcome::Installed {
                policy_epoch: 1,
                ..
            }
        ));
        assert!(matches!(
            store.import_explicit_file(
                &first_path,
                ExplicitTrustedPolicyImport::UserOrTestOperatorRequested
            )?,
            TrustedPolicyImportOutcome::AlreadyCurrent {
                policy_epoch: 1,
                ..
            }
        ));
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
            root(&root_key)?,
            &second_keyring,
            &new,
            &first,
            artifact(1)?,
            RolloutPhase::AdvertiseCanary,
            RolloutDisposition::Normal,
            10,
        )?;
        let second_path = write_bundle(&inbox, "second.policy", &second)?;
        assert!(matches!(
            store.import_explicit_file(
                &second_path,
                ExplicitTrustedPolicyImport::UserOrTestOperatorRequested
            )?,
            TrustedPolicyImportOutcome::Installed {
                policy_epoch: 2,
                keyring_epoch: 2,
                ..
            }
        ));
        drop(store);
        let reopened = TrustedPolicyStore::open(store_dir.path(), root(&root_key)?)?;
        assert_eq!(
            reopened
                .current_bundle()?
                .ok_or(TrustedPolicyStoreError::MissingHead)?
                .bundle_id()?,
            second.bundle_id()?
        );
        Ok(())
    }

    #[test]
    fn invalid_stale_and_unauthorized_inputs_never_change_head()
    -> Result<(), TrustedPolicyStoreError> {
        let store_dir = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test store",
            source,
        })?;
        let inbox = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test inbox",
            source,
        })?;
        let root_key = key(54);
        let signer = key(55);
        let (_, first) = genesis(&root_key, &signer)?;
        let first_path = write_bundle(&inbox, "first.policy", &first)?;
        let store = TrustedPolicyStore::open(store_dir.path(), root(&root_key)?)?;
        assert!(
            store
                .import_explicit_file(&first_path, ExplicitTrustedPolicyImport::NotAuthorized)
                .is_err()
        );
        assert!(store.current_bundle()?.is_none());
        store.import_explicit_file(
            &first_path,
            ExplicitTrustedPolicyImport::UserOrTestOperatorRequested,
        )?;
        let before = store
            .current_bundle()?
            .ok_or(TrustedPolicyStoreError::MissingHead)?
            .bundle_id()?;
        let mut tampered = first.encode()?;
        let last = tampered
            .last_mut()
            .ok_or(TrustedPolicyStoreError::InputLength(0))?;
        *last ^= 1;
        let tampered_path = inbox.path().join("tampered.policy");
        fs::write(&tampered_path, tampered).map_err(|source| TrustedPolicyStoreError::Io {
            action: "write tampered test input",
            source,
        })?;
        assert!(
            store
                .import_explicit_file(
                    &tampered_path,
                    ExplicitTrustedPolicyImport::UserOrTestOperatorRequested
                )
                .is_err()
        );
        assert_eq!(
            store
                .current_bundle()?
                .ok_or(TrustedPolicyStoreError::MissingHead)?
                .bundle_id()?,
            before
        );
        Ok(())
    }

    #[test]
    fn non_file_and_oversized_input_are_quarantined_without_state_change()
    -> Result<(), TrustedPolicyStoreError> {
        let store_dir = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test store",
            source,
        })?;
        let inbox = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test inbox",
            source,
        })?;
        let root_key = key(56);
        let store = TrustedPolicyStore::open(store_dir.path(), root(&root_key)?)?;
        assert!(
            store
                .import_explicit_file(
                    inbox.path(),
                    ExplicitTrustedPolicyImport::UserOrTestOperatorRequested
                )
                .is_err()
        );
        let mut oversized =
            NamedTempFile::new_in(inbox.path()).map_err(|source| TrustedPolicyStoreError::Io {
                action: "create oversized test input",
                source,
            })?;
        oversized
            .write_all(&vec![0_u8; MAX_TRUSTED_RELEASE_POLICY_BUNDLE_BYTES + 1])
            .map_err(|source| TrustedPolicyStoreError::Io {
                action: "write oversized test input",
                source,
            })?;
        assert!(
            store
                .import_explicit_file(
                    oversized.path(),
                    ExplicitTrustedPolicyImport::UserOrTestOperatorRequested
                )
                .is_err()
        );
        assert!(store.current_bundle()?.is_none());
        Ok(())
    }

    #[test]
    fn corruption_and_foreign_root_fail_closed_after_restart() -> Result<(), TrustedPolicyStoreError>
    {
        let store_dir = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test store",
            source,
        })?;
        let inbox = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create test inbox",
            source,
        })?;
        let root_key = key(57);
        let signer = key(58);
        let (_, first) = genesis(&root_key, &signer)?;
        let path = write_bundle(&inbox, "first.policy", &first)?;
        let store = TrustedPolicyStore::open(store_dir.path(), root(&root_key)?)?;
        store.import_explicit_file(
            path,
            ExplicitTrustedPolicyImport::UserOrTestOperatorRequested,
        )?;
        assert!(TrustedPolicyStore::open(store_dir.path(), root(&key(59))?).is_err());
        store.corrupt_head_bundle_for_test()?;
        drop(store);
        assert!(TrustedPolicyStore::open(store_dir.path(), root(&root_key)?).is_err());
        Ok(())
    }

    #[test]
    fn read_only_inspection_does_not_create_a_missing_store() -> Result<(), TrustedPolicyStoreError>
    {
        let parent = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create read-only inspection parent",
            source,
        })?;
        let missing = parent.path().join("not-created-by-observer");
        let trust_root = root(&key(60))?;

        assert_eq!(
            inspect_existing_trusted_policy_store(&missing, trust_root)?,
            None
        );
        assert!(!missing.exists());
        Ok(())
    }

    #[test]
    fn read_only_inspection_verifies_the_head_without_changing_database_bytes()
    -> Result<(), TrustedPolicyStoreError> {
        let store_dir = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create read-only inspection store",
            source,
        })?;
        let inbox = TempDir::new().map_err(|source| TrustedPolicyStoreError::Io {
            action: "create read-only inspection inbox",
            source,
        })?;
        let root_key = key(61);
        let signer = key(62);
        let (_, bundle) = genesis(&root_key, &signer)?;
        let input = write_bundle(&inbox, "current.policy", &bundle)?;
        let store = TrustedPolicyStore::open(store_dir.path(), root(&root_key)?)?;
        store.import_explicit_file(
            input,
            ExplicitTrustedPolicyImport::UserOrTestOperatorRequested,
        )?;
        drop(store);

        let database_path = store_dir.path().join(DATABASE_FILE);
        let before = fs::read(&database_path).map_err(|source| TrustedPolicyStoreError::Io {
            action: "read trusted-policy database before inspection",
            source,
        })?;
        let snapshot = inspect_existing_trusted_policy_store(store_dir.path(), root(&root_key)?)?
            .ok_or(TrustedPolicyStoreError::MissingHead)?;
        assert_eq!(
            snapshot
                .current_bundle
                .ok_or(TrustedPolicyStoreError::MissingHead)?
                .bundle_id()?,
            bundle.bundle_id()?
        );
        let after = fs::read(&database_path).map_err(|source| TrustedPolicyStoreError::Io {
            action: "read trusted-policy database after inspection",
            source,
        })?;
        assert_eq!(after, before);
        Ok(())
    }
}
