use super::*;

#[test]
fn indexed_component_accepted_source_cannot_be_replaced() -> TestResult {
    let (_dir, mut store) = fixture(Config::default())?;
    let pending = Key::ordinal(0, Namespace::Pending, 1)?;
    apply(
        &mut store,
        vec![],
        vec![CreditAction::Pending {
            context: 0,
            ordinal: 1,
            event: id(1),
            body: b"original accepted ciphertext".to_vec(),
            head: false,
            grant: None,
        }],
    )?;
    let original = store.get(pending)?;
    let old = store.generation();
    let (rows, refs) = records(0, 1, true, "fixture")?;
    let bad = apply(
        &mut store,
        rows,
        vec![CreditAction::Readable {
            context: 0,
            ordinal: 1,
            event: id(1),
            records: refs,
        }],
    );
    assert!(
        bad.is_err(),
        "pending retirement accepted different source bytes"
    );
    assert_eq!(store.generation(), old);
    assert_eq!(store.get(pending)?, original);
    Ok(())
}

#[test]
fn indexed_component_queued_text_cannot_be_replaced() -> TestResult {
    let (_dir, mut store) = fixture(Config::default())?;
    let queue = Key::ordinal(0, Namespace::Queue, 1)?;
    apply(
        &mut store,
        vec![],
        vec![CreditAction::Queue {
            context: 0,
            sequence: 1,
            id: id(9),
            body: b"original accepted text".to_vec(),
        }],
    )?;
    let original = store.get(queue)?;
    let old = store.generation();
    let (rows, refs) = records(0, 1, false, "different text")?;
    assert!(matches!(
        apply(
            &mut store,
            rows,
            vec![CreditAction::Materialize {
                context: 0,
                queue: 1,
                request: id(9),
                ordinal: 1,
                event: id(1),
                records: refs
            }]
        ),
        Err(Error::Conflict)
    ));
    assert_eq!(store.generation(), old);
    assert_eq!(store.get(queue)?, original);
    Ok(())
}

#[test]
fn indexed_component_long_prefix_split_merge_and_credit_retirement() -> TestResult {
    let (_dir, mut store) = fixture(Config::default())?;
    let mut rows = Vec::new();
    for n in 0..128u8 {
        let mut bytes = [0; 32];
        bytes[31] = n;
        rows.push(Mutation::put(
            Key::hash(0, Namespace::Event, bytes)?,
            vec![n],
        ));
    }
    apply(&mut store, rows, vec![])?;
    assert_eq!(store.record_counts()?, (128, 255, 128));
    assert_eq!(
        store
            .range(Key::hash(0, Namespace::Event, [0; 32])?, 128)?
            .len(),
        128
    );
    for n in 0..16 {
        apply(
            &mut store,
            vec![],
            vec![CreditAction::Queue {
                context: 0,
                sequence: n,
                id: id(n),
                body: b"accepted plaintext queue".to_vec(),
            }],
        )?;
    }
    for n in 0..16 {
        let (rows, refs) = records(0, n + 10000, false, "accepted plaintext queue")?;
        apply(
            &mut store,
            rows,
            vec![CreditAction::Materialize {
                context: 0,
                queue: n,
                request: id(n),
                ordinal: n,
                event: id(n + 10000),
                records: refs,
            }],
        )?;
        assert_eq!(store.get(Key::ordinal(0, Namespace::Queue, n)?)?, None);
    }
    assert_eq!(
        store.root.roots[Namespace::Queue as usize],
        Summary::default()
    );
    assert!(matches!(
        apply(
            &mut store,
            vec![Mutation::Retire {
                key: event(0, 0)?,
                replacement: event(0, 1)?
            }],
            vec![]
        ),
        Err(Error::Bound(_))
    ));
    let (logical, nodes, values) = store.record_counts()?;
    let nonempty = store.root.roots.iter().filter(|s| s.count != 0).count() as u64;
    assert_eq!(nodes, 2 * logical - nonempty);
    assert_eq!(values, logical);
    Ok(())
}
#[test]
fn indexed_component_queue_debt_receipt_and_context_conservation() -> TestResult {
    let (_dir, mut store) = fixture(Config::default())?;
    let action = CreditAction::Queue {
        context: 0,
        sequence: 1,
        id: id(77),
        body: b"accepted".to_vec(),
    };
    apply(&mut store, vec![], vec![action.clone()])?;
    assert_eq!(store.credits()?.reserved, [FRAME_CREDIT, 0]);
    let g = store.generation();
    assert!(apply(&mut store, vec![], vec![action])?.duplicate);
    assert_eq!(store.generation(), g);
    let (rows, refs) = records(0, 22, false, "accepted")?;
    apply(
        &mut store,
        rows,
        vec![CreditAction::Materialize {
            context: 0,
            queue: 1,
            request: id(77),
            ordinal: 1,
            event: id(22),
            records: refs,
        }],
    )?;
    assert!(store.credits()?.reserved[0] >= 80 * 1024);
    assert_eq!(store.credits()?.reserved[1], 0);
    assert!(matches!(
        apply(
            &mut store,
            vec![],
            vec![CreditAction::Acknowledge {
                context: 0,
                ordinal: 1,
                event: id(22),
                records: vec![]
            }]
        ),
        Err(Error::Bound(_))
    ));
    let receipt = Key::ordinal(0, Namespace::ReceiptA, 1)?;
    apply(
        &mut store,
        vec![Mutation::put(receipt, b"storage fixture receipt".to_vec())],
        vec![CreditAction::Acknowledge {
            context: 0,
            ordinal: 1,
            event: id(22),
            records: vec![receipt],
        }],
    )?;
    assert_eq!(store.credits()?.reserved, [0, 0]);
    assert!(store.get(event(0, 22)?)?.is_some());
    let c0 = store.credits()?.used[0];
    apply(
        &mut store,
        vec![Mutation::put(event(1, 99)?, b"other C".to_vec())],
        vec![],
    )?;
    assert_eq!(store.credits()?.used[0], c0);
    Ok(())
}
#[test]
fn indexed_component_funded_head_full_pending_remainder_and_refill() -> TestResult {
    let config = Config {
        logical_quota: [80 * 1024 * 1024; 2],
        ..Config::default()
    };
    let (dir, mut store) = fixture(config)?;
    for n in 512..576 {
        apply(
            &mut store,
            vec![],
            vec![CreditAction::Pending {
                context: 0,
                ordinal: n,
                event: id(n),
                body: vec![4; 100],
                head: false,
                grant: None,
            }],
        )?;
    }
    let generation = store.generation();
    assert!(matches!(
        apply(
            &mut store,
            vec![],
            vec![CreditAction::Pending {
                context: 0,
                ordinal: 600,
                event: id(600),
                body: vec![4; 100],
                head: false,
                grant: None
            }]
        ),
        Err(Error::Quota)
    ));
    assert_eq!(store.generation(), generation);
    apply(
        &mut store,
        vec![],
        vec![CreditAction::Grant {
            context: 0,
            id: id(1000),
            start: 1,
            end: 257,
        }],
    )?;
    assert_eq!(store.credits()?.grant_unissued[0], 256 * FRAME_CREDIT);
    let before = store.credits()?;
    let author = Key::hash(0, Namespace::Auth, id(101000))?;
    let admission = Key::hash(0, Namespace::Auth, id(101001))?;
    apply(
        &mut store,
        vec![
            Mutation::put(author, vec![1; 16 * 1024]),
            Mutation::put(admission, vec![2; 16 * 1024]),
        ],
        vec![
            CreditAction::Pending {
                context: 0,
                ordinal: 1,
                event: id(1),
                body: vec![42; 1024],
                head: true,
                grant: Some(id(1000)),
            },
            CreditAction::CapturePending {
                context: 0,
                ordinal: 1,
                event: id(1),
                original_auth: author.bytes,
                admission_auth: admission.bytes,
            },
        ],
    )?;
    let held = store.credits()?;
    let g = store.generation();
    assert!(
        apply(
            &mut store,
            vec![],
            vec![CreditAction::Pending {
                context: 0,
                ordinal: 1,
                event: id(1),
                body: vec![42; 1024],
                head: false,
                grant: None
            }]
        )?
        .duplicate
    );
    assert_eq!(store.generation(), g);
    let view = store.pending(0, 1)?.ok_or("pending view")?;
    assert_eq!(view.original_auth, Some(author.bytes));
    assert_eq!(view.admission_auth, Some(admission.bytes));
    assert!(view.head);
    assert_eq!(held.pending[0], 64);
    assert!(held.head_in_use[0]);
    assert_eq!(
        before.used[0] + before.reserved[0] + before.grant_unissued[0] + TEMP_CREDIT,
        held.used[0] + held.reserved[0] + held.grant_unissued[0]
    );
    let native = Key::hash(0, Namespace::Native, [0; 32])?;
    let exact = held.used[0] + held.reserved[0] + held.grant_unissued[0] + held.repair_free[0];
    drop(store);
    let mut tight = config;
    tight.logical_quota[0] = exact;
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), tight)?;
    let (mut rows, mut refs) = records(0, 1, true, "readable head fixture")?;
    refs.extend([author, admission]);
    rows.push(Mutation::put(native, vec![8; 2048]));
    refs.push(native);
    apply(
        &mut store,
        rows,
        vec![CreditAction::Readable {
            context: 0,
            ordinal: 1,
            event: id(1),
            records: refs,
        }],
    )?;
    assert!(!store.credits()?.head_in_use[0]);
    let provenance = store
        .get(Key::hash(0, Namespace::Admission, id(1))?)?
        .ok_or("permanent admission")?;
    assert_eq!(&provenance[1..33], &author.bytes);
    assert_eq!(&provenance[33..], &admission.bytes);
    assert_eq!(store.credits()?.pending[0], 64);
    apply(
        &mut store,
        vec![],
        vec![CreditAction::CancelGrant {
            context: 0,
            id: id(1000),
        }],
    )?;
    let spent = REPAIR_POOL - store.credits()?.repair_free[0];
    assert!(spent > 0);
    assert!(matches!(
        apply(
            &mut store,
            vec![],
            vec![CreditAction::Refill {
                context: 0,
                bytes: spent
            }]
        ),
        Err(Error::Quota)
    ));
    drop(store);
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), config)?;
    apply(
        &mut store,
        vec![],
        vec![CreditAction::Refill {
            context: 0,
            bytes: spent,
        }],
    )?;
    assert_eq!(store.credits()?.repair_free[0], REPAIR_POOL);
    assert!(matches!(
        apply(
            &mut store,
            vec![],
            vec![CreditAction::Refill {
                context: 0,
                bytes: 1
            }]
        ),
        Err(Error::Quota)
    ));
    let reserved = store.credits()?.reserved[0];
    apply(
        &mut store,
        vec![],
        vec![CreditAction::Grant {
            context: 0,
            id: id(1001),
            start: 500,
            end: 756,
        }],
    )?;
    assert_eq!(store.credits()?.grant_unissued[0], 192 * FRAME_CREDIT);
    assert_eq!(store.credits()?.reserved[0], reserved);
    drop(store);
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), config)?;
    for n in 512..576 {
        assert!(
            store
                .get(Key::ordinal(0, Namespace::Pending, n)?)?
                .is_some()
        );
    }
    drop(store);
    let small = Config {
        logical_quota: [REPAIR_POOL + TEMP_CREDIT; 2],
        ..config
    };
    assert!(matches!(
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), small),
        Err(Error::Quota)
    ));
    Ok(())
}
#[test]
fn indexed_component_io_failures_atomic_root_records_credits() -> TestResult {
    for (point, n) in [
        ("storage_full", 0),
        ("before_write", 0),
        ("after_write", 0),
        ("before_sync", 0),
        ("after_sync", 0),
        ("after_sync", 1),
        ("after_sync", 2),
    ] {
        let (dir, mut store) = fixture(Config::default())?;
        apply(
            &mut store,
            vec![Mutation::put(event(0, 0)?, b"already accepted".to_vec())],
            vec![],
        )?;
        let old = store.generation();
        let commit = id(9999);
        store.backend.fail(point, n)?;
        let result = store.commit(
            old,
            commit,
            vec![Mutation::put(event(0, 1)?, b"trial".to_vec())],
            vec![CreditAction::Queue {
                context: 0,
                sequence: 1,
                id: id(1),
                body: b"queue".to_vec(),
            }],
        );
        if result.is_err() {
            assert!(matches!(
                store.get(event(0, 0)?),
                Err(Error::ReopenRequired)
            ));
            assert!(matches!(store.last_commit(), Err(Error::ReopenRequired)));
            assert!(matches!(store.credits(), Err(Error::ReopenRequired)));
        }
        drop(store);
        let mut store = IndexedDialogVault::open(
            StateDirectoryLock::acquire(dir.path())?,
            owner(),
            Config::default(),
        )?;
        assert_eq!(store.get(event(0, 0)?)?, Some(b"already accepted".to_vec()));
        let present = store.get(event(0, 1)?)?.is_some();
        assert_eq!(
            store.get(Key::ordinal(0, Namespace::Queue, 1)?)?.is_some(),
            present
        );
        assert_eq!(
            store.credits()?.reserved[0],
            if present { FRAME_CREDIT } else { 0 }
        );
        assert_eq!(store.generation(), old + u64::from(present));
        if present {
            assert_eq!(store.last_commit()?, commit);
        }
        println!(
            "A3_S1_IO point={point} index={n} returned_error={} recovered_new={present}",
            result.is_err()
        );
    }
    Ok(())
}
#[test]
fn indexed_component_owner_schema_and_old_recovery_fences() -> TestResult {
    let (dir, store) = fixture(Config::default())?;
    assert!(matches!(
        StateDirectoryLock::acquire(dir.path()),
        Err(crate::StateError::AlreadyLocked { .. })
    ));
    assert!(EncryptedStateVault::open_existing(dir.path()).is_err());
    drop(store);
    assert!(matches!(
        crate::StateTransaction::begin(dir.path()),
        Err(crate::StateError::OrderedWriterRequired)
    ));
    let generic = EncryptedStateVault::open_existing(dir.path())?;
    assert!(matches!(
        generic.verify(),
        Err(crate::StateError::UnsupportedVaultSchemaVersion(5))
    ));
    drop(generic);
    assert!(matches!(
        IndexedDialogVault::open(
            StateDirectoryLock::acquire(dir.path())?,
            ExpectedOwner {
                claim: id(13),
                ..owner()
            },
            Config::default()
        ),
        Err(Error::OwnerMismatch)
    ));
    assert!(crate::OrderedVault::open(StateDirectoryLock::acquire(dir.path())?).is_err());
    let legacy = tempfile::tempdir()?;
    let tx = crate::StateTransaction::begin(legacy.path())?;
    let active = dir.path().join(".kilogram-transactions/active");
    fs::create_dir_all(&active)?;
    for name in ["manifest.json", "prepared"] {
        fs::copy(tx.active.join(name), active.join(name))?;
    }
    assert!(matches!(
        StateDirectoryLock::acquire(dir.path()),
        Err(crate::StateError::OrderedWriterRequired)
    ));
    fs::remove_dir_all(&active)?;
    let _store = IndexedDialogVault::open(
        StateDirectoryLock::acquire(dir.path())?,
        owner(),
        Config::default(),
    )?;
    let partial = tempfile::tempdir()?;
    fs::create_dir_all(partial.path().join("ratchet"))?;
    fs::write(
        partial.path().join("ratchet/account.pickle"),
        ORDERED_ACCOUNT_FENCE,
    )?;
    assert!(matches!(
        IndexedDialogVault::create(
            StateDirectoryLock::acquire(partial.path())?,
            claim(),
            Config::default()
        ),
        Err(Error::CreationConflict)
    ));
    Ok(())
}
