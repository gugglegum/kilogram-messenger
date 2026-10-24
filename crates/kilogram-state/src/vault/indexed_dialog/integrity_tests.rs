use super::*;
fn author_value(n: u64) -> Vec<u8> {
    [[5u8; 32].as_slice(), id(n).as_slice()].concat()
}

#[test]
fn indexed_component_projection_codec_and_caps() -> TestResult {
    let value = ProjectionValue {
        text: "x".repeat(4096),
        binding: vec![5; 2304],
    }
    .encode()?;
    assert_eq!(value.len(), 6405);
    assert_eq!(ProjectionValue::decode(&value)?.text.len(), 4096);
    let mut invalid = value.clone();
    invalid[3] = 0xff;
    assert!(ProjectionValue::decode(&invalid).is_err());
    let mut invalid = value;
    invalid.push(0);
    assert!(ProjectionValue::decode(&invalid).is_err());
    assert!(
        ProjectionValue {
            text: "x".repeat(4097),
            binding: vec![]
        }
        .encode()
        .is_err()
    );
    Ok(())
}

#[test]
fn indexed_component_membership_absence_numeric_and_reopen() -> TestResult {
    let (dir, mut store) = fixture(Config::default())?;
    assert_eq!(store.get(event(0, 0)?)?, None);
    let ns = [0, 1, 255, 256, 1 << 32, 1 << 63, u64::MAX];
    for n in ns.into_iter().rev() {
        let k = Key::ordinal(0, Namespace::LaneA, n)?;
        apply(&mut store, vec![Mutation::put(k, id(n).to_vec())], vec![])?;
    }
    let page = store.range(Key::ordinal(0, Namespace::LaneA, 0)?, 128)?;
    assert_eq!(
        page.iter()
            .map(|(k, _)| k.ordinal_value())
            .collect::<Result<Vec<_>>>()?,
        ns
    );
    for (k, v) in &page {
        assert_eq!(store.get(*k)?.as_ref(), Some(v));
    }
    assert_eq!(store.get(Key::ordinal(0, Namespace::LaneA, 17)?)?, None);
    let g = store.generation();
    let k = Key::ordinal(0, Namespace::LaneA, 1)?;
    assert!(apply(&mut store, vec![Mutation::put(k, id(1).to_vec())], vec![])?.duplicate);
    assert_eq!(store.generation(), g);
    assert!(matches!(
        apply(&mut store, vec![Mutation::put(k, id(99).to_vec())], vec![]),
        Err(Error::Conflict)
    ));
    assert_eq!(store.generation(), g);
    drop(store);
    let mut store = IndexedDialogVault::open(
        StateDirectoryLock::acquire(dir.path())?,
        owner(),
        Config::default(),
    )?;
    assert_eq!(
        store.range(Key::ordinal(0, Namespace::LaneA, 0)?, 128)?,
        page
    );
    assert!(matches!(
        Key::hash(0, Namespace::Queue, [0; 32]),
        Err(Error::Bound(_))
    ));
    assert!(matches!(
        Key {
            context: 0,
            namespace: Namespace::LaneA,
            bytes: [1; 32]
        }
        .slot(),
        Err(Error::Bound(_))
    ));
    Ok(())
}
#[test]
fn indexed_component_global_fork_generation_and_commit_identity() -> TestResult {
    let (dir, mut store) = fixture(Config::default())?;
    let key = Key::ordinal(0, Namespace::Author0, 9)?;
    let g = store.generation();
    let commit = id(123);
    store.commit(g, commit, vec![Mutation::put(key, author_value(9))], vec![])?;
    assert!(
        store
            .commit(g, commit, vec![Mutation::put(key, author_value(9))], vec![])?
            .duplicate
    );
    assert!(matches!(
        store.commit(g, commit, vec![Mutation::put(key, id(10).to_vec())], vec![]),
        Err(Error::Conflict)
    ));
    assert!(matches!(
        store.commit(g, id(124), vec![], vec![]),
        Err(Error::StaleGeneration)
    ));
    assert_eq!(
        store.get(Key::ordinal(1, Namespace::Author0, 9)?)?,
        Some(author_value(9))
    );
    assert!(matches!(
        apply(
            &mut store,
            vec![Mutation::put(
                Key::ordinal(1, Namespace::Author0, 9)?,
                author_value(9)
            )],
            vec![]
        ),
        Err(Error::Conflict)
    ));
    drop(store);
    let mut store = IndexedDialogVault::open(
        StateDirectoryLock::acquire(dir.path())?,
        owner(),
        Config::default(),
    )?;
    assert_eq!(store.last_commit()?, commit);
    assert!(
        store
            .commit(g, commit, vec![Mutation::put(key, author_value(9))], vec![])?
            .duplicate
    );
    let mutable = Key::request(0, [9; 16])?;
    apply(
        &mut store,
        vec![Mutation::put(mutable, b"queued".to_vec())],
        vec![],
    )?;
    apply(
        &mut store,
        vec![Mutation::put(mutable, b"materialized".to_vec())],
        vec![],
    )?;
    assert_eq!(store.get(mutable)?, Some(b"materialized".to_vec()));
    Ok(())
}
fn raw_row(
    store: &IndexedDialogVault,
    table: TableDefinition<&[u8], &[u8]>,
    key: Id,
) -> Result<Vec<u8>> {
    let tx = store.vault.database.begin_read().map_err(db)?;
    let t = tx.open_table(table).map_err(db)?;
    Ok(t.get(key.as_slice())
        .map_err(db)?
        .ok_or(Error::Integrity("test row"))?
        .value()
        .to_vec())
}
fn raw_set(
    store: &IndexedDialogVault,
    table: TableDefinition<&[u8], &[u8]>,
    key: Id,
    value: Option<&[u8]>,
) -> Result<()> {
    let tx = store.vault.database.begin_write().map_err(db)?;
    {
        let mut t = tx.open_table(table).map_err(db)?;
        if let Some(v) = value {
            t.insert(key.as_slice(), v).map_err(db)?;
        } else {
            t.remove(key.as_slice()).map_err(db)?;
        }
    }
    tx.commit().map_err(db)?;
    Ok(())
}
#[test]
fn indexed_component_deletion_corruption_scope_and_complete_range() -> TestResult {
    let (_dir, mut store) = fixture(Config::default())?;
    for n in 0..8 {
        apply(
            &mut store,
            vec![
                Mutation::put(event(0, n)?, vec![n as u8; 64]),
                Mutation::put(event(1, n)?, vec![n as u8; 64]),
            ],
            vec![],
        )?;
    }
    let k = event(0, 2)?;
    let a = store.crypto.address(k.slot()? as u8, b"value", &k.bytes);
    let original = raw_row(&store, VALUES, a)?;
    raw_set(&store, VALUES, a, None)?;
    assert!(matches!(store.get(k), Err(Error::Integrity(_))));
    assert!(matches!(
        store.range(Key::hash(0, Namespace::Event, [0; 32])?, 128),
        Err(Error::Integrity(_))
    ));
    raw_set(&store, VALUES, a, Some(&original))?;
    let swapped = event(0, 3)?;
    let swapped_address = store.crypto.address(0, b"value", &swapped.bytes);
    raw_set(
        &store,
        VALUES,
        a,
        Some(&raw_row(&store, VALUES, swapped_address)?),
    )?;
    assert!(matches!(store.get(k), Err(Error::Integrity(_))));
    raw_set(&store, VALUES, a, Some(&original))?;
    let auth = Key::hash(0, Namespace::Auth, id(2))?;
    apply(&mut store, vec![Mutation::put(auth, vec![2; 64])], vec![])?;
    let auth_address = store
        .crypto
        .address(auth.slot()? as u8, b"value", &auth.bytes);
    raw_set(
        &store,
        VALUES,
        a,
        Some(&raw_row(&store, VALUES, auth_address)?),
    )?;
    assert!(matches!(store.get(k), Err(Error::Integrity(_))));
    raw_set(&store, VALUES, a, Some(&original))?;
    let other = event(1, 2)?;
    let b = store
        .crypto
        .address(other.slot()? as u8, b"value", &other.bytes);
    raw_set(&store, VALUES, a, Some(&raw_row(&store, VALUES, b)?))?;
    assert!(matches!(store.get(k), Err(Error::Integrity(_))));
    raw_set(&store, VALUES, a, Some(&original))?;
    let mut entrance = vec![0, 0];
    entrance.extend([0; 32]);
    let root_address = store.crypto.address(0, b"node", &entrance);
    let original = raw_row(&store, NODES, root_address)?;
    raw_set(&store, NODES, root_address, None)?;
    assert!(matches!(store.get(k), Err(Error::Integrity(_))));
    raw_set(&store, NODES, root_address, Some(&original))?;
    let mut bad = original.clone();
    bad[30] ^= 1;
    raw_set(&store, NODES, root_address, Some(&bad))?;
    assert!(matches!(store.get(k), Err(Error::Integrity(_))));
    assert!(store.get(event(1, 2)?)?.is_some());
    raw_set(&store, NODES, root_address, Some(&original))?;
    let before = store.root.roots[0];
    store.root.roots[0] = store.root.roots[24];
    assert!(matches!(store.get(k), Err(Error::Integrity(_))));
    store.root.roots[0] = before;
    assert_eq!(store.record_counts()?, (17, 31, 17));
    Ok(())
}
#[test]
fn indexed_component_anchor_tamper_never_reinitializes() -> TestResult {
    for mode in 0..3 {
        let (dir, mut store) = fixture(Config::default())?;
        apply(
            &mut store,
            vec![Mutation::put(event(0, 0)?, b"accepted".to_vec())],
            vec![],
        )?;
        let tx = store.vault.database.begin_write().map_err(db)?;
        {
            let mut t = tx.open_table(META_TABLE).map_err(db)?;
            if mode == 0 {
                t.remove(ANCHOR).map_err(db)?;
            } else if mode == 1 {
                let mut bytes = { t.get(ANCHOR).map_err(db)?.ok_or("anchor")?.value().to_vec() };
                bytes[60] ^= 1;
                t.insert(ANCHOR, bytes.as_slice()).map_err(db)?;
            } else {
                let raw = {
                    t.get(MANIFEST_KEY)
                        .map_err(db)?
                        .ok_or("manifest")?
                        .value()
                        .to_vec()
                };
                let mut m: VaultManifest = postcard::from_bytes(&raw)?;
                m.schema_version = 4;
                let raw = postcard::to_allocvec(&m)?;
                t.insert(MANIFEST_KEY, raw.as_slice()).map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        drop(store);
        assert!(
            IndexedDialogVault::open(
                StateDirectoryLock::acquire(dir.path())?,
                owner(),
                Config::default()
            )
            .is_err()
        );
        assert!(matches!(
            IndexedDialogVault::create(
                StateDirectoryLock::acquire(dir.path())?,
                claim(),
                Config::default()
            ),
            Err(Error::CreationConflict)
        ));
    }
    Ok(())
}
#[test]
fn indexed_component_maximal_plan_and_bounds() -> TestResult {
    let (_dir, mut store) = fixture(Config::default())?;
    let mut rows = Vec::new();
    for n in 0..900 {
        rows.push(Mutation::put(event(0, n)?, vec![3; 16 * 1024]));
    }
    let report = apply(&mut store, rows, vec![])?;
    assert!(report.encoded_plan_bytes > 14 * 1024 * 1024);
    assert!(report.encoded_plan_bytes <= MAX_PLAN as u64);
    let old = store.generation();
    let mut rows = Vec::new();
    for n in 1000..2000 {
        rows.push(Mutation::put(event(0, n)?, vec![3; 16 * 1024]));
    }
    assert!(matches!(
        apply(&mut store, rows, vec![]),
        Err(Error::Bound(_))
    ));
    assert_eq!(store.generation(), old);
    assert_eq!(store.get(event(0, 1000)?)?, None);
    assert!(matches!(
        apply(
            &mut store,
            vec![Mutation::put(event(0, 3000)?, vec![0; 16 * 1024 + 1])],
            vec![]
        ),
        Err(Error::Bound(_))
    ));
    assert!(matches!(
        apply(
            &mut store,
            vec![Mutation::put(
                Key::hash(0, Namespace::Native, id(1))?,
                vec![1]
            )],
            vec![]
        ),
        Err(Error::Bound(_))
    ));
    println!(
        "A3_S1_MAX_PLAN encoded_bytes={} changed_nodes={} nodes_read={}",
        report.encoded_plan_bytes, report.changed_nodes, report.nodes_read
    );
    Ok(())
}
