use kilogram_state::StateDirectoryLock;
use kilogram_state::indexed_dialog::{
    Config, CreationClaim, Error, ExpectedOwner, HistoryCursor, HistoryExtras, HistoryInit,
    HistoryNativeTransition, HistoryOperation, HistoryParent, HistoryPending, HistoryScope, Id,
    IndexedDialogVault, Key, Namespace, PreparedReadable,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const CHILD_ROOT: &str = "KILOGRAM_A3_S2_REVIEW_CHILD_ROOT";

fn owner() -> ExpectedOwner {
    ExpectedOwner {
        account: [31; 32],
        claim: [32; 32],
    }
}

fn scope(context: u8) -> HistoryScope {
    HistoryScope {
        context: [35 + context; 32],
        dialog: [37 + context; 32],
        epoch: [39 + context; 32],
        session_id: "r".repeat(43),
    }
}

fn apply(store: &mut IndexedDialogVault, context: u8, op: HistoryOperation) -> TestResult {
    let generation = store.generation();
    let mut id = [0x71; 32];
    id[..8].copy_from_slice(&generation.to_be_bytes());
    store.commit_history(context, generation, id, op)?;
    Ok(())
}

// Synthetic trusted adapter DTOs exercise storage only; no native/wire proof is claimed.
fn row(
    role: u8,
    ordinal: u64,
    previous: Option<Id>,
    request: u8,
    text: &str,
    parent: Option<&PreparedReadable>,
) -> Result<PreparedReadable, Box<dyn std::error::Error>> {
    let parents = if let Some(parent) = parent {
        vec![HistoryParent {
            event: parent.event_id()?,
            role: parent.role,
            ordinal: parent.ordinal,
        }]
    } else {
        vec![]
    };
    let source = postcard::to_allocvec(&(
        scope(0),
        role,
        ordinal,
        previous,
        [request; 16],
        text,
        &parents,
    ))?;
    Ok(PreparedReadable {
        scope: scope(0),
        format: 1,
        core_offset: 0,
        core_len: u16::try_from(source.len())?,
        source,
        kind: if ordinal == 0 { role + 1 } else { 3 },
        role,
        ordinal,
        sequence: 2 * ordinal + u64::from(role),
        request: [request; 16],
        previous,
        parents,
        text: text.to_owned(),
        extras: HistoryExtras::default(),
    })
}

fn native_transition(
    store: &mut IndexedDialogVault,
    rows: &[PreparedReadable],
) -> Result<HistoryNativeTransition, Box<dyn std::error::Error>> {
    let old = store.get(Key::hash(0, Namespace::Native, [0; 32])?)?;
    let frame_ids = rows
        .iter()
        .map(PreparedReadable::event_id)
        .collect::<Result<Vec<_>, _>>()?;
    let mut after_blob = vec![0x72; 256];
    after_blob[..32].copy_from_slice(frame_ids.last().ok_or("empty fixture batch")?);
    Ok(HistoryNativeTransition {
        before_hash: old.map(|v| *blake3::hash(&v).as_bytes()),
        after_blob,
        frame_ids,
    })
}

fn accept(store: &mut IndexedDialogVault, row: &PreparedReadable) -> TestResult {
    let operation = if row.role == 0 {
        HistoryOperation::QueueAccept {
            request: row.request,
            text: row.text.clone(),
        }
    } else {
        HistoryOperation::AcceptPending(HistoryPending {
            scope: row.scope.clone(),
            format: row.format,
            ordinal: row.ordinal,
            source: row.source.clone(),
            head: false,
            grant: None,
            capture: None,
        })
    };
    apply(store, 0, operation)
}

#[derive(Deserialize, Serialize)]
struct Capsule {
    cursor: String,
    finished_cursor: String,
    expected_ids: Vec<Id>,
    generation: u64,
    credits: serde_json::Value,
    native_hash: Id,
}

#[test]
fn a3_s2_review_cursor_survives_new_process_and_append() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut store = IndexedDialogVault::create(
        StateDirectoryLock::acquire(dir.path())?,
        CreationClaim {
            account: owner().account,
            claim: owner().claim,
            signing_secret: Zeroizing::new([33; 32]),
            encryption_secret: Zeroizing::new([34; 32]),
            contexts: [[35; 32], [36; 32]],
        },
        Config::default(),
    )?;
    for context in [0, 1] {
        apply(
            &mut store,
            context,
            HistoryOperation::Init(HistoryInit {
                scope: scope(context),
                own_role: 0,
            }),
        )?;
    }
    apply(
        &mut store,
        1,
        HistoryOperation::QueueAccept {
            request: [9; 16],
            text: "preserve second context".into(),
        },
    )?;
    let a0 = row(0, 0, None, 0, "", None)?;
    let b0 = row(1, 0, None, 0, "", None)?;
    for genesis in [&a0, &b0] {
        let native = native_transition(&mut store, std::slice::from_ref(genesis))?;
        apply(
            &mut store,
            0,
            HistoryOperation::Genesis {
                row: genesis.clone(),
                native,
            },
        )?;
    }
    let a1 = row(0, 1, Some(a0.event_id()?), 1, "A1 α\0", None)?;
    let b1 = row(1, 1, Some(b0.event_id()?), 2, "B1", Some(&a1))?;
    let a2 = row(0, 2, Some(a1.event_id()?), 3, "A2", Some(&b1))?;
    let b2 = row(1, 2, Some(b1.event_id()?), 4, "B2", Some(&a2))?;
    let a3 = row(0, 3, Some(a2.event_id()?), 5, "new A3", Some(&b2))?;
    let rows = vec![a1, b1, a2, b2];
    let mut expected_ids = rows
        .iter()
        .map(PreparedReadable::event_id)
        .collect::<Result<Vec<_>, _>>()?;
    for row in &rows {
        accept(&mut store, row)?;
    }
    let native = native_transition(&mut store, &rows)?;
    apply(&mut store, 0, HistoryOperation::TextBatch { rows, native })?;
    let first = store.history_first(0, 1)?;
    assert_eq!(first.items[0].id, expected_ids[0]);
    let finished = store.history_first(0, 32)?;
    assert!(finished.finished);
    accept(&mut store, &a3)?;
    let native = native_transition(&mut store, std::slice::from_ref(&a3))?;
    expected_ids.push(a3.event_id()?);
    apply(
        &mut store,
        0,
        HistoryOperation::TextBatch {
            rows: vec![a3],
            native,
        },
    )?;
    let saved_native = store
        .get(Key::hash(0, Namespace::Native, [0; 32])?)?
        .ok_or("native state missing")?;
    let capsule = Capsule {
        cursor: first.cursor.to_base64(),
        finished_cursor: finished.cursor.to_base64(),
        expected_ids,
        generation: store.generation(),
        credits: serde_json::to_value(store.credits()?)?,
        native_hash: *blake3::hash(&saved_native).as_bytes(),
    };
    fs::write(
        dir.path().join("cursor-review.json"),
        serde_json::to_vec(&capsule)?,
    )?;
    drop(store);

    // The stable runner supplies this executable's constant path; no hash-named child.
    let output = Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "a3_s2_review_cursor_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ROOT, dir.path())
        .output()?;
    assert!(
        output.status.success(),
        "cursor child failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("A3_S2_REVIEW_CHILD_CURSOR_PASS"));
    print!("{stdout}");
    Ok(())
}

#[test]
#[ignore = "owned child entry point"]
fn a3_s2_review_cursor_child() -> TestResult {
    let root = PathBuf::from(std::env::var_os(CHILD_ROOT).ok_or("missing child fixture")?);
    let capsule: Capsule = serde_json::from_slice(&fs::read(root.join("cursor-review.json"))?)?;
    let mut store = IndexedDialogVault::open(
        StateDirectoryLock::acquire(&root)?,
        owner(),
        Config::default(),
    )?;
    let mut cursor = HistoryCursor::from_base64(&capsule.cursor)?;
    let mut suffix = Vec::new();
    loop {
        let page = store.history_page(0, &cursor, 2)?;
        assert_eq!(page.frontiers, [3, 3]);
        suffix.extend(page.items.iter().map(|r| r.id));
        cursor = page.cursor;
        if page.finished {
            break;
        }
    }
    assert_eq!(suffix, capsule.expected_ids[1..4]);
    let old_end = HistoryCursor::from_base64(&capsule.finished_cursor)?;
    let end = store.history_page(0, &old_end, 32)?;
    assert!(end.finished && end.items.is_empty());
    assert_eq!(end.frontiers, [3, 3]);
    let current = store.history_first(0, 32)?;
    assert_eq!(current.frontiers, [4, 3]);
    assert_eq!(
        current.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        capsule.expected_ids
    );
    assert_eq!(current.items[0].text, "A1 α\0");
    assert!(matches!(
        store.history_page(1, &old_end, 32),
        Err(Error::InvalidCursor)
    ));
    assert!(
        store
            .history_request(1, [9; 16])?
            .is_some_and(|r| r.event.is_none())
    );
    assert_eq!(store.generation(), capsule.generation);
    assert_eq!(serde_json::to_value(store.credits()?)?, capsule.credits);
    let native = store
        .get(Key::hash(0, Namespace::Native, [0; 32])?)?
        .ok_or("native state missing")?;
    assert_eq!(*blake3::hash(&native).as_bytes(), capsule.native_hash);
    println!("A3_S2_REVIEW_CHILD_CURSOR_PASS pid={}", std::process::id());
    Ok(())
}
