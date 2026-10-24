use kilogram_state::StateDirectoryLock;
use kilogram_state::indexed_dialog::{
    Config, CreationClaim, CreditAction, ExpectedOwner, IndexedDialogVault, Key, Mutation, Namespace,
    ProjectionValue,
};
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn std::error::Error>>;

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

fn readable_rows(receipt_ordinal: u64) -> Result<(Vec<Mutation>, Vec<Key>), Box<dyn std::error::Error>> {
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
    let (_dir, mut store) = fixture()?;
    pending(&mut store)?;
    let generation = store.generation();
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
    assert!(result.is_err(), "unrelated receipt must not release pending credit");
    assert_eq!(store.generation(), generation);
    assert!(store.pending(0, 1)?.is_some());
    Ok(())
}
