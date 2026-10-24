use kilogram_state::StateDirectoryLock;
use kilogram_state::indexed_dialog::{
    Config, CreationClaim, CreditAction, ExpectedOwner, IndexedDialogVault, Key, Mutation,
    Namespace, ProjectionValue, REPAIR_POOL,
};
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn std::error::Error>>;
type RetentionRows = (Vec<Mutation>, Vec<Key>);

fn owner() -> ExpectedOwner {
    ExpectedOwner {
        account: [31; 32],
        claim: [32; 32],
    }
}

fn fixture() -> Result<(tempfile::TempDir, IndexedDialogVault), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let claim = CreationClaim {
        account: owner().account,
        claim: owner().claim,
        signing_secret: Zeroizing::new([33; 32]),
        encryption_secret: Zeroizing::new([34; 32]),
        contexts: [[35; 32], [36; 32]],
    };
    let store = IndexedDialogVault::create(
        StateDirectoryLock::acquire(dir.path())?,
        claim,
        Config::default(),
    )?;
    Ok((dir, store))
}

fn readable_rows(receipt_ordinal: u64) -> Result<RetentionRows, Box<dyn std::error::Error>> {
    let event = Key::hash(0, Namespace::Event, [41; 32])?;
    let projection = Key::hash(0, Namespace::Projection, [41; 32])?;
    let receipt = Key::ordinal(0, Namespace::ReceiptA, receipt_ordinal)?;
    let native = Key::hash(0, Namespace::Native, [0; 32])?;
    Ok((
        vec![
            Mutation::put(event, vec![42; 1024]),
            Mutation::put(
                projection,
                ProjectionValue {
                    text: "review fixture".to_owned(),
                    binding: Vec::new(),
                }
                .encode()?,
            ),
            Mutation::put(receipt, b"local storage fixture receipt".to_vec()),
            Mutation::put(native, vec![43; 2048]),
        ],
        vec![event, projection, receipt, native],
    ))
}

fn pending(store: &mut IndexedDialogVault) -> TestResult {
    store.commit(
        store.generation(),
        [44; 32],
        vec![],
        vec![
            CreditAction::Grant {
                context: 0,
                id: [45; 32],
                start: 1,
                end: 2,
            },
            CreditAction::Pending {
                context: 0,
                ordinal: 1,
                event: [41; 32],
                body: vec![42; 1024],
                head: true,
                grant: Some([45; 32]),
            },
        ],
    )?;
    Ok(())
}

#[test]
fn a3_s1_review_mutable_growth_is_charged_from_committed_baseline() -> TestResult {
    for repeated_mutable in [false, true] {
        let (dir, mut store) = fixture()?;
        pending(&mut store)?;
        let held = store.credits()?;
        let mut tight = Config::default();
        tight.logical_quota[0] =
            held.used[0] + held.reserved[0] + held.grant_unissued[0] + held.repair_free[0];
        drop(store);
        let mut store =
            IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), tight)?;
        let (mut rows, refs) = readable_rows(1)?;
        if repeated_mutable {
            rows.insert(
                0,
                Mutation::put(Key::hash(0, Namespace::Native, [0; 32])?, vec![43; 1024]),
            );
        }
        let result = store.commit(
            store.generation(),
            [46; 32],
            rows,
            vec![CreditAction::Readable {
                context: 0,
                ordinal: 1,
                event: [41; 32],
                records: refs,
            }],
        );
        println!("REVIEW mutable repeated={repeated_mutable} result={result:?}");
        assert!(
            result.is_ok(),
            "same final retained state must fit its funded credit; repeated={repeated_mutable}: {result:?}"
        );
        assert!(store.pending(0, 1)?.is_none());
    }
    Ok(())
}

#[test]
fn a3_s1_review_readable_requires_receipt_for_pending_ordinal() -> TestResult {
    let (dir, mut store) = fixture()?;
    pending(&mut store)?;
    let generation = store.generation();
    let ledger = serde_json::to_value(store.credits()?)?;
    let count = store.record_counts()?;
    let last = store.last_commit()?;
    let (rows, refs) = readable_rows(2)?;
    let result = store.commit(
        generation,
        [47; 32],
        rows,
        vec![CreditAction::Readable {
            context: 0,
            ordinal: 1,
            event: [41; 32],
            records: refs,
        }],
    );
    println!("REVIEW pending_ordinal=1 receipt_ordinal=2 result={result:?}");
    assert!(
        result.is_err(),
        "unrelated receipt must not release pending credit"
    );
    assert_eq!(store.generation(), generation);
    assert!(store.pending(0, 1)?.is_some());
    assert_eq!(serde_json::to_value(store.credits()?)?, ledger);
    assert_eq!(store.record_counts()?, count);
    assert_eq!(store.last_commit()?, last);
    assert!(
        store
            .get(Key::ordinal(0, Namespace::ReceiptA, 2)?)?
            .is_none()
    );
    assert!(
        store
            .get(Key::hash(0, Namespace::Event, [41; 32])?)?
            .is_none()
    );
    drop(store);
    let mut store = IndexedDialogVault::open(
        StateDirectoryLock::acquire(dir.path())?,
        owner(),
        Config::default(),
    )?;
    assert_eq!(store.generation(), generation);
    assert_eq!(serde_json::to_value(store.credits()?)?, ledger);
    assert!(store.pending(0, 1)?.is_some());
    let (rows, refs) = readable_rows(1)?;
    store.commit(
        generation,
        [48; 32],
        rows,
        vec![CreditAction::Readable {
            context: 0,
            ordinal: 1,
            event: [41; 32],
            records: refs,
        }],
    )?;
    assert!(store.pending(0, 1)?.is_none());
    Ok(())
}

fn mutable_key(ns: Namespace) -> Result<Key, Box<dyn std::error::Error>> {
    Ok(if ns == Namespace::Request {
        Key::request(0, [51; 16])?
    } else {
        Key::hash(0, ns, [0; 32])?
    })
}

#[test]
fn a3_s1_review_mutable_plan_conservation_matrix_and_reopen_retry() -> TestResult {
    // Both plans have the same final bytes, even when intermediate bytes grow,
    // shrink, repeat, or return to the original committed state.
    type Versions = &'static [(usize, u8)];
    let cases: [(Option<usize>, Versions); 9] = [
        (None, &[(32, 1), (64, 2)]),
        (None, &[(96, 1), (32, 2)]),
        (None, &[(64, 1), (64, 2)]),
        (None, &[(64, 2), (64, 2)]),
        (Some(32), &[(64, 1), (96, 2)]),
        (Some(64), &[(96, 1), (32, 2)]),
        (Some(64), &[(32, 1), (96, 2)]),
        (Some(64), &[(64, 1), (64, 2)]),
        (Some(64), &[(96, 1), (64, 0x61)]),
    ];
    for ns in [Namespace::Native, Namespace::Current, Namespace::Request] {
        let key = mutable_key(ns)?;
        for (initial, steps) in cases {
            let mut control = None;
            for repeated in [false, true] {
                let (dir, mut store) = fixture()?;
                if let Some(size) = initial {
                    store.commit(
                        store.generation(),
                        [50; 32],
                        vec![Mutation::put(key, vec![0x61; size])],
                        vec![],
                    )?;
                }
                pending(&mut store)?;
                let held = store.credits()?;
                let mut tight = Config::default();
                tight.logical_quota[0] =
                    held.used[0] + held.reserved[0] + held.grant_unissued[0] + held.repair_free[0];
                drop(store);
                let mut store = IndexedDialogVault::open(
                    StateDirectoryLock::acquire(dir.path())?,
                    owner(),
                    tight,
                )?;
                let (mut rows, mut refs) = readable_rows(1)?;
                rows.retain(
                    |m| !matches!(m,Mutation::Put{key,..} if key.namespace==Namespace::Native),
                );
                refs.retain(|k| k.namespace != Namespace::Native);
                let immutable_cost: u64 = rows
                    .iter()
                    .map(|m| match m {
                        Mutation::Put { value, .. } => value.len() as u64 + 585,
                        _ => 0,
                    })
                    .sum();
                let &(final_size, final_byte) = steps.last().ok_or("steps")?;
                let native_delta = (final_size as u64 + 585)
                    .saturating_sub(initial.map_or(0, |size| size as u64 + 585));
                let versions = if repeated {
                    steps
                } else {
                    &steps[steps.len() - 1..]
                };
                for &(size, byte) in versions {
                    rows.push(Mutation::put(key, vec![byte; size]));
                }
                refs.push(key);
                let action = CreditAction::Readable {
                    context: 0,
                    ordinal: 1,
                    event: [41; 32],
                    records: refs,
                };
                let generation = store.generation();
                store.commit(generation, [52; 32], rows.clone(), vec![action.clone()])?;
                let after = store.credits()?;
                assert_eq!(
                    REPAIR_POOL - after.repair_free[0],
                    immutable_cost + native_delta
                );
                assert_eq!(after.repair_free[1], held.repair_free[1]);
                let after = serde_json::to_value(after)?;
                if let Some(expected) = &control {
                    assert_eq!(&after, expected);
                } else {
                    control = Some(after.clone());
                }
                assert_eq!(store.get(key)?, Some(vec![final_byte; final_size]));
                assert!(store.pending(0, 1)?.is_none());
                drop(store);
                let mut store = IndexedDialogVault::open(
                    StateDirectoryLock::acquire(dir.path())?,
                    owner(),
                    tight,
                )?;
                assert_eq!(serde_json::to_value(store.credits()?)?, after);
                assert!(
                    store
                        .commit(generation, [52; 32], rows, vec![action])?
                        .duplicate
                );
                assert_eq!(serde_json::to_value(store.credits()?)?, after);
            }
            println!(
                "A3_S1_R1_MATRIX namespace={ns:?} initial={initial:?} versions={steps:?} control_equal=true"
            );
        }
    }
    Ok(())
}

#[test]
fn a3_s1_review_multi_frame_shared_checkpoint_funded_once() -> TestResult {
    let (dir, mut store) = fixture()?;
    pending(&mut store)?;
    store.commit(
        store.generation(),
        [60; 32],
        vec![],
        vec![
            CreditAction::CancelGrant {
                context: 0,
                id: [45; 32],
            },
            CreditAction::Grant {
                context: 0,
                id: [61; 32],
                start: 2,
                end: 3,
            },
            CreditAction::Pending {
                context: 0,
                ordinal: 2,
                event: [62; 32],
                body: vec![42; 1024],
                head: false,
                grant: Some([61; 32]),
            },
        ],
    )?;
    let before = store.credits()?;
    let mut tight = Config::default();
    tight.logical_quota[0] =
        before.used[0] + before.reserved[0] + before.grant_unissued[0] + before.repair_free[0];
    drop(store);
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), tight)?;
    let native = mutable_key(Namespace::Native)?;
    let (mut rows, refs1) = readable_rows(1)?;
    rows.insert(0, Mutation::put(native, vec![43; 1024]));
    let (second, mut refs2) = readable_rows(2)?;
    for row in second {
        if let Mutation::Put { mut key, value } = row {
            if matches!(key.namespace, Namespace::Event | Namespace::Projection) {
                key.bytes = [62; 32];
            }
            rows.push(Mutation::put(key, value));
        }
    }
    for key in &mut refs2 {
        if matches!(key.namespace, Namespace::Event | Namespace::Projection) {
            key.bytes = [62; 32];
        }
    }
    let g = store.generation();
    let ledger = serde_json::to_value(store.credits()?)?;
    let mut bad_rows = rows.clone();
    bad_rows.push(Mutation::put(
        Key::hash(0, Namespace::Current, [63; 32])?,
        vec![0; 64],
    ));
    let actions = vec![
        CreditAction::Readable {
            context: 0,
            ordinal: 1,
            event: [41; 32],
            records: refs1,
        },
        CreditAction::Readable {
            context: 0,
            ordinal: 2,
            event: [62; 32],
            records: refs2,
        },
    ];
    assert!(
        store
            .commit(g, [64; 32], bad_rows, actions.clone())
            .is_err()
    );
    assert_eq!(store.generation(), g);
    assert_eq!(serde_json::to_value(store.credits()?)?, ledger);
    assert!(store.pending(0, 1)?.is_some());
    assert!(store.pending(0, 2)?.is_some());
    drop(store);
    let mut store =
        IndexedDialogVault::open(StateDirectoryLock::acquire(dir.path())?, owner(), tight)?;
    store.commit(g, [64; 32], rows.clone(), actions.clone())?;
    assert!(store.pending(0, 1)?.is_none());
    assert!(store.pending(0, 2)?.is_none());
    assert_eq!(store.get(native)?, Some(vec![43; 2048]));
    let after = store.credits()?;
    let (_, base_refs) = readable_rows(1)?;
    let immutable_cost: u64 = base_refs
        .iter()
        .filter(|k| k.namespace != Namespace::Native)
        .map(|k| store.get(*k).map(|v| v.map_or(0, |v| v.len() as u64 + 585)))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();
    assert_eq!(
        REPAIR_POOL - after.repair_free[0],
        2 * immutable_cost + 2048 + 585
    );
    assert!(store.commit(g, [64; 32], rows, actions)?.duplicate);
    Ok(())
}

#[test]
fn a3_s1_review_matching_receipt_must_exist_and_context_match() -> TestResult {
    for bad_case in ["missing", "other_context"] {
        let (_dir, mut store) = fixture()?;
        pending(&mut store)?;
        let g = store.generation();
        let before = serde_json::to_value(store.credits()?)?;
        let (mut rows, mut refs) = readable_rows(1)?;
        if bad_case == "missing" {
            rows.retain(
                |m| !matches!(m,Mutation::Put{key,..} if key.namespace==Namespace::ReceiptA),
            );
        } else {
            for row in &mut rows {
                if let Mutation::Put { key, .. } = row
                    && key.namespace == Namespace::ReceiptA
                {
                    key.context = 1;
                }
            }
            for key in &mut refs {
                if key.namespace == Namespace::ReceiptA {
                    key.context = 1;
                }
            }
        }
        assert!(
            store
                .commit(
                    g,
                    [70; 32],
                    rows,
                    vec![CreditAction::Readable {
                        context: 0,
                        ordinal: 1,
                        event: [41; 32],
                        records: refs
                    }]
                )
                .is_err()
        );
        assert_eq!(store.generation(), g);
        assert_eq!(serde_json::to_value(store.credits()?)?, before);
        assert!(store.pending(0, 1)?.is_some());
    }
    Ok(())
}
