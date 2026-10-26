//! Closed-lowering negative only; this does not claim Native/Event provenance.
use super::*;

#[test]
fn indexed_l1_review1_closed_materialize_over_limit_has_no_durable_effect() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let store = IndexedDialogVault::create(
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
    let durable = store.root.encode();
    let tx = store.vault.database.begin_read().map_err(db)?;
    let nodes = tx.open_table(NODES).map_err(db)?;
    let values = tx.open_table(VALUES).map_err(db)?;
    let p = Profile {
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
    for (debt, batch) in [(127, 2), (127, 4), (128, 1), (129, 1)] {
        // Only a volatile copied summary is malformed. The actual component
        // root/store is never relabeled, changed or presented as Native origin.
        let mut root = store.root.clone();
        root.layout = RootLayout::Migrated7;
        root.roots[Namespace::Undelivered as usize].count = debt;
        let before = root.encode();
        let mut w = Work::new(&nodes, &values, &store.crypto, root);
        let raw = vec![1];
        let auth = HistoryAuth {
            id: hash(b"", &raw),
            bytes: raw,
        };
        let row = L1Readable {
            author: p.device,
            source: vec![],
            text: Some("invalid Event negative input".into()),
            original: auth.clone(),
            admission: None,
            final_auth: Some(auth),
            receiver: None,
            receipt: None,
            queue: Some(1),
        };
        let change = L1NativeChange {
            context: 0,
            expected_head: [0; 32],
            policy_generation: 1,
            wall_time: 1,
            head: vec![],
            native: Some(vec![1]),
        };
        assert!(matches!(
            w.l1_native_rows(&p, change, vec![row; batch], true),
            Err(Error::Quota)
        ));
        assert_eq!(w.root.encode(), before);
        assert!(w.changed.is_empty() && w.tree.changed.is_empty());
        assert_eq!(store.root.encode(), durable);
    }
    Ok(())
}
