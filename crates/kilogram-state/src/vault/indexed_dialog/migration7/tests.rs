//! Structural storage tests use opaque IDs, never claim historical Native origin.
use super::super::*;
use super::{
    codec::{self, a, b, u},
    records::*,
};
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
#[test]
fn migration_s5_closed_local_codec() -> TestResult {
    let value = a([u(7), b([8; 32]), a([]), a([u(u64::MAX)])]);
    let encoded = codec::record(6, &value, 4096)?;
    assert_eq!(codec::read_record(&encoded, 6, 4096)?, value);
    assert!(codec::read_record(&encoded, 5, 4096).is_err());
    for raw in [
        vec![0x18, 0],
        vec![0x9f, 0xff],
        vec![0xa0],
        vec![0x60],
        vec![0xf6],
        vec![0x1b, 0, 0, 0, 0, 0, 0, 0, 24],
        vec![0x40, 0],
    ] {
        assert!(codec::decode(&raw, 4096).is_err(), "{raw:?}");
    }
    assert!(codec::decode(&[0x5a, 0xff, 0xff, 0xff, 0xff], 4096).is_err());
    assert!(a([u(0), u(1)]).optional().is_err());
    Ok(())
}
#[test]
fn migration_s5_budget_authentication_and_scratch_digest() -> TestResult {
    let dir = tempfile::tempdir()?;
    let owner = ExpectedOwner {
        account: [1; 32],
        claim: [2; 32],
    };
    let store = IndexedDialogVault::create(
        StateDirectoryLock::acquire(dir.path())?,
        CreationClaim {
            account: owner.account,
            claim: owner.claim,
            contexts: [[5; 32], [6; 32]],
            signing_secret: Zeroizing::new([3; 32]),
            encryption_secret: Zeroizing::new([4; 32]),
        },
        Config::default(),
    )?;
    let tx = store.vault.database.begin_read().map_err(db)?;
    let nodes = tx.open_table(NODES).map_err(db)?;
    let values = tx.open_table(VALUES).map_err(db)?;
    let mut root = store.root.clone();
    root.layout = RootLayout::Migrated7;
    root.ledger = [
        credits::Ledger::migration_unfunded(u64::MAX),
        credits::Ledger::migration_unfunded(u64::MAX),
    ];
    let mut w = Work::new(&nodes, &values, &store.crypto, root);
    let profile = Profile {
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
        plan_hash: [12; 32],
    };
    w.put(
        Key::hash(0, Namespace::Event, [13; 32])?,
        &[4; 1024],
        128 * 1024,
        false,
    )?;
    w.migration_meta_put(
        migration_key(profile.mid, 8, b"")?,
        &codec::record(
            8,
            &a([
                u(7),
                b(profile.mid),
                b(profile.source),
                b([14; 32]),
                u(5),
                u(B4 * 2),
            ]),
            META_CAP,
        )?,
        META_CAP,
        B4 * 2,
    )?;
    w.migration_budget_put(&profile, MigrationStage::Copying)?;
    for c in 0..2 {
        w.migration_budget_check(&profile, c, MigrationStage::Copying)?;
    }
    let c1 = w
        .get(budget_key(1)?)?
        .ok_or(Error::Integrity("test budget"))?;
    let c1_value = codec::read_record(&c1, 6, META_CAP)?;
    assert_eq!(
        c1_value.fields(12)?[7].uint()?,
        B4,
        "slot0 retained source cannot pay slot1 hold"
    );
    let key = budget_key(0)?;
    let raw = w.get(key)?.ok_or(Error::Integrity("test budget"))?;
    let mut value = codec::read_record(&raw, 6, META_CAP)?.fields(12)?.to_vec();
    value[7] = u(value[7].uint()? + 1);
    w.put(key, &codec::record(6, &a(value), META_CAP)?, 0, true)?;
    assert!(
        w.migration_budget_check(&profile, 0, MigrationStage::Copying)
            .is_err(),
        "authenticated false hold payload"
    );
    w.migration_budget_put(&profile, MigrationStage::Copying)?;
    let saved = w.root.roots[23];
    w.root.roots[23].reserved[0] += 1;
    assert!(
        w.migration_budget_check(&profile, 0, MigrationStage::Copying)
            .is_err(),
        "payload vs authenticated summary"
    );
    w.root.roots[23] = saved;
    let leaf = w
        .tree
        .find(23, w.root.roots[23], key.bytes)?
        .ok_or(Error::Integrity("test leaf"))?;
    let mut wrong = leaf.clone();
    if let tree::Body::Leaf { owner, .. } = &mut wrong.body {
        *owner = 1;
    }
    w.root.roots[23] = w.tree.put(23, w.root.roots[23], wrong)?;
    assert!(
        w.migration_budget_check(&profile, 0, MigrationStage::Copying)
            .is_err(),
        "wrong owner"
    );
    w.root.roots[23] = w.tree.put(23, w.root.roots[23], leaf)?;
    w.migration_budget_put(&profile, MigrationStage::Copying)?;
    w.remove(budget_key(1)?)?;
    assert!(
        w.migration_budget_check(&profile, 1, MigrationStage::Copying)
            .is_err(),
        "required budget absent"
    );
    w.migration_budget_put(&profile, MigrationStage::Copying)?;
    let digest = w.migration_output_digest(profile.mid)?;
    let parent = w.root.roots;
    for kind in [2, 3] {
        w.put(
            migration_key(profile.mid, kind, b"")?,
            b"excluded audit row",
            0,
            true,
        )?;
    }
    assert_eq!(digest, w.migration_output_digest(profile.mid)?);
    assert_ne!(parent, w.root.roots);
    assert!(
        w.get(migration_key(profile.mid, 3, b"")?)?.is_some(),
        "scratch exclusion never removes original membership"
    );
    w.put(
        Key::hash(1, Namespace::Event, [15; 32])?,
        b"changed output",
        0,
        false,
    )?;
    assert_ne!(digest, w.migration_output_digest(profile.mid)?);
    w.root.ledger[0].migration_ready(0, None, true)?;
    assert!(
        w.root.ledger[0].migration_ready(0, None, true).is_err(),
        "no second endowment"
    );
    Ok(())
}
