//! Read-only namespace accounting. Invariant selection belongs to the caller.
use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L1AccountingGroup {
    pub records: u64,
    pub used: [u64; 2],
    pub reserved: [u64; 2],
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L1Accounting {
    pub runtime: L1AccountingGroup,
    pub prepaid_metadata: L1AccountingGroup,
}
pub(super) fn snapshot(roots: &[Summary]) -> Result<L1Accounting> {
    let metadata = [
        Key::hash(0, Namespace::RuntimeProfile, [0; 32])?.slot()?,
        Key::hash(0, Namespace::Migration, [0; 32])?.slot()?,
    ];
    let mut result = L1Accounting {
        runtime: L1AccountingGroup::default(),
        prepaid_metadata: L1AccountingGroup::default(),
    };
    for (slot, summary) in roots.iter().enumerate() {
        let group = if metadata.contains(&slot) {
            &mut result.prepaid_metadata
        } else {
            &mut result.runtime
        };
        group.records = group
            .records
            .checked_add(summary.count)
            .ok_or(Error::Bound("accounting record count"))?;
        for lane in 0..2 {
            group.used[lane] = group.used[lane]
                .checked_add(summary.used[lane])
                .ok_or(Error::Bound("accounting used"))?;
            group.reserved[lane] = group.reserved[lane]
                .checked_add(summary.reserved[lane])
                .ok_or(Error::Bound("accounting reserved"))?;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn g2a_accounting_real_prepaid_metadata_redistributes_at_encoding_widths() -> Result<()> {
        // Real authenticated-index Work writes on fresh local storage; structural
        // metadata premise only, not a historical Native origin or runtime claim.
        let dir = tempfile::tempdir()?;
        let core = IndexedDialogVault::create(
            StateDirectoryLock::acquire(dir.path())?,
            CreationClaim {
                account: [1; 32],
                claim: [2; 32],
                signing_secret: Zeroizing::new([3; 32]),
                encryption_secret: Zeroizing::new([4; 32]),
                contexts: [[5; 32], [6; 32]],
            },
            Config::default(),
        )?;
        let tx = core.vault.database.begin_read().map_err(db)?;
        let nodes = tx.open_table(NODES).map_err(db)?;
        let values = tx.open_table(VALUES).map_err(db)?;
        let mut root = core.root.clone();
        root.layout = RootLayout::Migrated7;
        root.ledger = std::array::from_fn(|_| credits::Ledger::migration_unfunded(1 << 30));
        let mut w = Work::new(&nodes, &values, &core.crypto, root);
        w.put(
            Key::hash(0, Namespace::Event, [12; 32])?,
            &[13; 64],
            128,
            true,
        )?;
        let mut profile = Profile {
            source: [7; 32],
            mid: [8; 32],
            device: [9; 32],
            mask: 3,
            identity: a([]),
            initial_config: [10; 32],
            origin: [11; 32],
            contexts: vec![([5; 32], 0), ([6; 32], 1)],
            signers: vec![[9; 32]],
            plan_revision: 1,
            plan_hash: [14; 32],
        };
        for revision in [1, 23, 255] {
            let mut effective = Effective {
                mid: profile.mid,
                revision,
                identity: [15; 32],
                initial: [16; 32],
                quotas: [65535; 2],
                physical: 1 << 30,
                previous: (revision != 1).then_some([17; 32]),
            };
            profile.plan_revision = revision;
            let old = effective.raw()?;
            w.migration_meta_put(migration_key(profile.mid, 7, b"")?, &old, META_CAP, 0)?;
            w.migration_meta_put(profile_key()?, &profile.raw()?, 16 * 1024, 0)?;
            let before = snapshot(&w.root.roots)?;
            effective.revision += 1;
            effective.previous = Some(Effective::hash(&old));
            effective.quotas = [65536; 2];
            profile.plan_revision = effective.revision;
            profile.plan_hash = Effective::hash(&effective.raw()?);
            w.migration_meta_put(
                migration_key(profile.mid, 7, b"")?,
                &effective.raw()?,
                META_CAP,
                0,
            )?;
            w.migration_meta_put(profile_key()?, &profile.raw()?, 16 * 1024, 0)?;
            let after = snapshot(&w.root.roots)?;
            assert_eq!(before.runtime, after.runtime);
            assert_eq!(
                before.prepaid_metadata.records,
                after.prepaid_metadata.records
            );
            assert_ne!(before.prepaid_metadata.used, after.prepaid_metadata.used);
            assert_ne!(
                before.prepaid_metadata.reserved,
                after.prepaid_metadata.reserved
            );
            let report = credits::report(&w.root)?;
            for lane in 0..2 {
                assert_eq!(
                    before.prepaid_metadata.used[lane]
                        .checked_add(before.prepaid_metadata.reserved[lane]),
                    after.prepaid_metadata.used[lane]
                        .checked_add(after.prepaid_metadata.reserved[lane])
                );
                assert_eq!(
                    after.runtime.used[lane].checked_add(after.prepaid_metadata.used[lane]),
                    Some(report.used[lane])
                );
                assert_eq!(
                    after.runtime.reserved[lane].checked_add(after.prepaid_metadata.reserved[lane]),
                    Some(report.reserved[lane])
                );
            }
        }
        Ok(())
    }
}
