use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
static NEXT: AtomicU64 = AtomicU64::new(1000);
static ARMED: AtomicBool = AtomicBool::new(false);
pub(super) fn checkpoint(point: &str) {
    if !ARMED.load(Ordering::Acquire)
        || std::env::var("KILOGRAM_S1_PAUSE").ok().as_deref() != Some(point)
    {
        return;
    }
    if let Ok(path) = std::env::var("KILOGRAM_S1_MARKER")
        && fs::write(path, point).is_ok()
    {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
fn id(n: u64) -> Id {
    let mut h = blake3::Hasher::new();
    h.update(b"kilogram S1 synthetic fixture\0");
    h.update(&n.to_be_bytes());
    *h.finalize().as_bytes()
}
fn owner() -> ExpectedOwner {
    ExpectedOwner {
        account: [1; 32],
        claim: [2; 32],
    }
}
fn claim() -> CreationClaim {
    CreationClaim {
        account: owner().account,
        claim: owner().claim,
        signing_secret: Zeroizing::new([3; 32]),
        encryption_secret: Zeroizing::new([4; 32]),
        contexts: [[5; 32], [6; 32]],
    }
}
fn fixture(
    config: Config,
) -> std::result::Result<(tempfile::TempDir, IndexedDialogVault), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store =
        IndexedDialogVault::create(StateDirectoryLock::acquire(dir.path())?, claim(), config)?;
    Ok((dir, store))
}
fn apply(
    store: &mut IndexedDialogVault,
    mutations: Vec<Mutation>,
    credits: Vec<CreditAction>,
) -> Result<CommitReport> {
    store.commit(
        store.generation(),
        id(NEXT.fetch_add(1, Ordering::Relaxed)),
        mutations,
        credits,
    )
}
fn event(c: u8, n: u64) -> Result<Key> {
    Key::hash(c, Namespace::Event, id(n))
}
fn records(c: u8, n: u64, with_receipt: bool, text: &str) -> Result<(Vec<Mutation>, Vec<Key>)> {
    let e = event(c, n)?;
    let p = Key::hash(c, Namespace::Projection, id(n))?;
    let mut rows = vec![
        Mutation::put(e, vec![42; 1024]),
        Mutation::put(
            p,
            ProjectionValue {
                text: text.to_owned(),
                binding: Vec::new(),
            }
            .encode()?,
        ),
    ];
    let mut refs = vec![e, p];
    if with_receipt {
        let receipt = Key::ordinal(c, Namespace::ReceiptA, n)?;
        rows.push(Mutation::put(
            receipt,
            b"fixture receipt; no network proof".to_vec(),
        ));
        refs.push(receipt);
    }
    Ok((rows, refs))
}
#[path = "credit_tests.rs"]
mod credit;
#[path = "integrity_tests.rs"]
mod integrity;
#[path = "memory_tests.rs"]
mod memory;
#[path = "process_tests.rs"]
mod processes;
#[path = "scale_tests.rs"]
mod scale;

#[test]
fn indexed_smoke_reopen() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let owner = ExpectedOwner {
        account: [1; 32],
        claim: [2; 32],
    };
    let claim = CreationClaim {
        account: owner.account,
        claim: owner.claim,
        signing_secret: Zeroizing::new([3; 32]),
        encryption_secret: Zeroizing::new([4; 32]),
        contexts: [[5; 32], [6; 32]],
    };
    let mut store = IndexedDialogVault::create(
        StateDirectoryLock::acquire(dir.path())?,
        claim,
        Config::default(),
    )?;
    let key = Key::hash(0, Namespace::Event, [7; 32])?;
    store.commit(
        store.generation(),
        [8; 32],
        vec![Mutation::put(key, b"retained".to_vec())],
        vec![],
    )?;
    assert_eq!(store.get(key)?, Some(b"retained".to_vec()));
    drop(store);
    let mut store = IndexedDialogVault::open(
        StateDirectoryLock::acquire(dir.path())?,
        owner,
        Config::default(),
    )?;
    assert_eq!(store.get(key)?, Some(b"retained".to_vec()));
    assert_eq!(*store.root.signing, [3; 32]);
    assert_eq!(*store.root.encryption, [4; 32]);
    Ok(())
}
