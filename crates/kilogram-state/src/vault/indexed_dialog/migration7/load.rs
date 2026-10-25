//! A bounded read-only discovery of stored configuration, then the closed owner.
use super::super::*;
use super::{MigrationRepository, codec, records::*, source::Inventory};

pub(super) fn open_repository(
    lease: StateDirectoryLock,
    account: Id,
    device: Id,
    recovery: Option<&MigrationConfig>,
) -> Result<MigrationRepository> {
    let path = lease.root().join(STATE_VAULT_FILE);
    crate::reject_symlink(&path)?;
    let key = load_master_key(&lease.root().join(STATE_VAULT_KEY_FILE))?;
    let mut builder = Database::builder();
    builder.set_cache_size(64 * 1024 * 1024);
    let reader = match builder.open_read_only(&path) {
        Ok(reader) => reader,
        Err(redb::DatabaseError::RepairAborted) => {
            let supplied = recovery.ok_or(Error::Bound("RecoveryNeedsCapacityConfig"))?;
            let config = account_external(
                lease.root(),
                super::preview::physical_config(lease.root(), supplied.backend(0))?,
            )?;
            let (backend, _control) = backend::Backend::open(&path, false, config)?;
            let database = builder.create_with_backend(backend).map_err(db)?;
            let tx = database.begin_read().map_err(db)?;
            let meta = tx.open_table(META_TABLE).map_err(db)?;
            let root = read_root(&meta, &key.master_key.0, RootLayout::Migrated7)?;
            if root.layout != RootLayout::Migrated7 || root.account != account {
                return Err(Error::OwnerMismatch);
            }
            drop(meta);
            drop(tx);
            drop(database);
            builder.open_read_only(&path).map_err(db)?
        }
        Err(error) => return Err(db(error)),
    };
    let (owner, config) = {
        let tx = reader.begin_read().map_err(db)?;
        let meta = tx.open_table(META_TABLE).map_err(db)?;
        let root = read_root(&meta, &key.master_key.0, RootLayout::Migrated7)?;
        if root.layout != RootLayout::Migrated7 || root.account != account {
            return Err(Error::OwnerMismatch);
        }
        let crypto = Crypto::new(
            &key.master_key.0,
            root.incarnation,
            root.contexts,
            root.account,
        );
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &crypto, root);
        w.migration_check()?;
        let profile = w.migration_profile()?;
        if profile.device != device {
            return Err(Error::OwnerMismatch);
        }
        let value = profile.identity.fields(14)?;
        if value[10].text()?
            != lease
                .root()
                .to_str()
                .ok_or(Error::Bound("state directory UTF8"))?
        {
            return Err(Error::OwnerMismatch);
        }
        let paths = value[11].fields(3)?;
        let raw = w
            .get(migration_key(profile.mid, 7, b"")?)?
            .ok_or(Error::Integrity("effective config"))?;
        let effective = Effective::parse(&raw, &profile)?;
        let config = MigrationConfig {
            logical_quota: effective.quotas,
            physical_capacity: effective.physical,
            output_profile: paths[0].text()?.into(),
            control_ipc_file: paths[1].text()?.into(),
            locator_directory: paths[2].text()?.into(),
        };
        if let Some(candidate) = recovery
            && candidate.paths()? != value[11]
        {
            return Err(Error::Conflict);
        }
        (
            ExpectedOwner {
                account,
                claim: w.root.claim,
            },
            config,
        )
    };
    drop(reader);
    drop(key);
    let mut backend_config = super::preview::physical_config(lease.root(), config.backend(0))?;
    if let Some(candidate) = recovery {
        backend_config.physical_quota = backend_config
            .physical_quota
            .max(candidate.physical_capacity);
    }
    let loaded =
        IndexedDialogVault::load_borrowed(&lease, owner, backend_config, RootLayout::Migrated7)?;
    let mut core = IndexedDialogVault::from_loaded(lease, loaded);
    let profile = Profile::parse(
        &core
            .get(profile_key()?)?
            .ok_or(Error::Integrity("migration marker"))?,
    )?;
    let raw = core
        .get(migration_key(profile.mid, 1, b"")?)?
        .ok_or(Error::Integrity("migration intent"))?;
    let value = codec::read_record(&raw, 1, 24 * 1024)?;
    let f = value.fields(8)?;
    let mode = f[4].fields(3)?;
    let inventory = Inventory::resume(&core.vault, &f[1], mode[1].bytes()?)?;
    {
        let tx = core.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut w = Work::new(&nodes, &values, &core.crypto, core.root.clone());
        w.migration_check()?;
        let backing = w
            .get(migration_key(profile.mid, 8, b"")?)?
            .ok_or(Error::Integrity("source backing"))?;
        let value = codec::read_record(&backing, 8, META_CAP)?;
        let f = value.fields(6)?;
        if f[3].fixed::<32>()? != inventory.backing_digest
            || f[4].uint()? != inventory.backing_entries
            || f[5].uint()? != inventory.backing_bytes
        {
            return Err(Error::Integrity("retained primary backing changed"));
        }
    }
    Ok(MigrationRepository { core, inventory })
}
