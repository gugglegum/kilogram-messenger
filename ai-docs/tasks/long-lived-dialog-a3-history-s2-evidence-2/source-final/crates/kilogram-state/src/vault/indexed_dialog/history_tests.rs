use super::*;
use std::path::Path;

pub(super) type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
pub(super) fn id(n: u64) -> Id {
    hash_parts(b"A3 S2 test ID\0", &[&n.to_be_bytes()])
}
pub(super) fn owner() -> ExpectedOwner {
    ExpectedOwner {
        account: [1; 32],
        claim: [2; 32],
    }
}
pub(super) fn claim() -> CreationClaim {
    CreationClaim {
        account: owner().account,
        claim: owner().claim,
        signing_secret: Zeroizing::new([3; 32]),
        encryption_secret: Zeroizing::new([4; 32]),
        contexts: [[5; 32], [6; 32]],
    }
}
pub(super) fn fixture()
-> std::result::Result<(tempfile::TempDir, IndexedDialogVault), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store = IndexedDialogVault::create(
        StateDirectoryLock::acquire(dir.path())?,
        claim(),
        Config::default(),
    )?;
    Ok((dir, store))
}
pub(super) fn reopen(path: &Path) -> Result<IndexedDialogVault> {
    IndexedDialogVault::open(
        StateDirectoryLock::acquire(path)?,
        owner(),
        Config::default(),
    )
}
pub(super) fn init(c: u8) -> HistoryInit {
    HistoryInit {
        scope: HistoryScope {
            context: [5 + c; 32],
            dialog: [7 + c; 32],
            epoch: [9 + c; 32],
            session_id: "s".repeat(43),
        },
        own_role: 0,
    }
}
pub(super) fn apply(
    store: &mut IndexedDialogVault,
    c: u8,
    op: HistoryOperation,
) -> Result<CommitReport> {
    store.commit_history(c, store.generation(), id(store.generation()), op)
}
pub(super) fn prepared(
    c: u8,
    role: u8,
    n: u64,
    previous: Option<Id>,
    request: [u8; 16],
    text: &str,
    mut parents: Vec<HistoryParent>,
) -> Result<PreparedReadable> {
    parents.sort_by_key(|p| p.event);
    let scope = init(c).scope;
    let kind = if n == 0 { role + 1 } else { 3 };
    let sequence = n * 2 + u64::from(role);
    let mut row = PreparedReadable {
        scope,
        format: 1,
        source: Vec::new(),
        core_offset: 0,
        core_len: 0,
        kind,
        role,
        ordinal: n,
        sequence,
        request,
        previous,
        parents,
        text: text.into(),
        extras: HistoryExtras::default(),
    };
    recode(&mut row)?;
    Ok(row)
}
fn recode(row: &mut PreparedReadable) -> Result<()> {
    // A synthetic Core-shaped claim, not a signed/canonical OD1 or native frame.
    let core = postcard::to_allocvec(&(
        &row.scope,
        row.kind,
        row.role,
        row.ordinal,
        row.sequence,
        row.request,
        row.previous,
        &row.parents,
        blake3::hash(row.text.as_bytes()).as_bytes(),
    ))
    .map_err(|_| Error::Bound("fixture Core"))?;
    if core.len() > 1024 {
        return Err(Error::Bound("fixture Core size"));
    }
    row.core_len = core.len() as u16;
    row.source = core;
    row.source.resize(1024, 0x53);
    Ok(())
}
pub(super) fn native(
    store: &mut IndexedDialogVault,
    c: u8,
    rows: &[PreparedReadable],
) -> Result<HistoryNativeTransition> {
    let old = store.get(Key::hash(c, Namespace::Native, [0; 32])?)?;
    let frame_ids: Vec<_> = rows
        .iter()
        .map(PreparedReadable::event_id)
        .collect::<Result<_>>()?;
    let mut after_blob = vec![0x4e; 256];
    if let Some(last) = frame_ids.last() {
        after_blob[..32].copy_from_slice(last);
    }
    Ok(HistoryNativeTransition {
        before_hash: old.map(|v| *blake3::hash(&v).as_bytes()),
        after_blob,
        frame_ids,
    })
}
pub(super) fn setup(store: &mut IndexedDialogVault, c: u8) -> Result<[Id; 2]> {
    apply(store, c, HistoryOperation::Init(init(c)))?;
    let mut ids = [[0; 32]; 2];
    for (role, id) in ids.iter_mut().enumerate() {
        let row = prepared(c, role as u8, 0, None, [0; 16], "", vec![])?;
        *id = row.event_id()?;
        let state = native(store, c, std::slice::from_ref(&row))?;
        apply(store, c, HistoryOperation::Genesis { row, native: state })?;
    }
    Ok(ids)
}
pub(super) fn accept_peer(
    store: &mut IndexedDialogVault,
    c: u8,
    row: &PreparedReadable,
) -> Result<CommitReport> {
    apply(
        store,
        c,
        HistoryOperation::AcceptPending(HistoryPending {
            scope: row.scope.clone(),
            format: row.format,
            ordinal: row.ordinal,
            source: row.source.clone(),
            head: false,
            grant: None,
            capture: None,
        }),
    )
}
pub(super) fn append(
    store: &mut IndexedDialogVault,
    c: u8,
    row: PreparedReadable,
) -> Result<CommitReport> {
    if row.role == init(c).own_role {
        apply(
            store,
            c,
            HistoryOperation::QueueAccept {
                request: row.request,
                text: row.text.clone(),
            },
        )?;
    } else {
        accept_peer(store, c, &row)?;
    }
    let state = native(store, c, std::slice::from_ref(&row))?;
    apply(
        store,
        c,
        HistoryOperation::TextBatch {
            rows: vec![row],
            native: state,
        },
    )
}
fn reference(rows: &[PreparedReadable]) -> Result<Vec<Id>> {
    let mut remaining: BTreeMap<Id, &PreparedReadable> = rows
        .iter()
        .map(|r| Ok((r.event_id()?, r)))
        .collect::<Result<_>>()?;
    let mut next = [1u64; 2];
    let mut result = Vec::new();
    while !remaining.is_empty() {
        let id = remaining
            .iter()
            .filter(|(_, r)| {
                r.ordinal == next[usize::from(r.role)]
                    && r.parents.iter().all(|p| result.contains(&p.event))
            })
            .map(|(id, _)| *id)
            .next()
            .ok_or(Error::Integrity("small complete reference blocked"))?;
        let row = remaining
            .remove(&id)
            .ok_or(Error::Integrity("reference row"))?;
        next[usize::from(row.role)] += 1;
        result.push(id);
    }
    Ok(result)
}
fn snapshot(store: &IndexedDialogVault) -> Vec<u8> {
    store.root.encode().to_vec()
}

#[test]
fn history_causal_pages_all_limits_snapshot_append_reopen_and_point() -> TestResult {
    let (dir, mut store) = fixture()?;
    let genesis = setup(&mut store, 0)?;
    assert!(store.history_first(0, 1)?.finished);
    assert!(store.history_point(0, genesis[0])?.is_none());
    let a1 = prepared(0, 0, 1, Some(genesis[0]), [1; 16], "α\n\0", vec![])?;
    let b1 = prepared(
        0,
        1,
        1,
        Some(genesis[1]),
        [2; 16],
        "b1",
        vec![HistoryParent {
            event: a1.event_id()?,
            role: 0,
            ordinal: 1,
        }],
    )?;
    let a2 = prepared(
        0,
        0,
        2,
        Some(a1.event_id()?),
        [3; 16],
        "a2",
        vec![HistoryParent {
            event: b1.event_id()?,
            role: 1,
            ordinal: 1,
        }],
    )?;
    let b2 = prepared(
        0,
        1,
        2,
        Some(b1.event_id()?),
        [4; 16],
        "b2",
        vec![HistoryParent {
            event: a1.event_id()?,
            role: 0,
            ordinal: 1,
        }],
    )?;
    let rows = vec![a1, b1, a2, b2];
    for row in &rows {
        if row.role == 0 {
            apply(
                &mut store,
                0,
                HistoryOperation::QueueAccept {
                    request: row.request,
                    text: row.text.clone(),
                },
            )?;
        } else {
            accept_peer(&mut store, 0, row)?;
        }
    }
    let state = native(&mut store, 0, &rows)?;
    apply(
        &mut store,
        0,
        HistoryOperation::TextBatch {
            rows: rows.clone(),
            native: state,
        },
    )?;
    let expected = reference(&rows)?;
    for limit in 1..=32 {
        let mut page = store.history_first(0, limit)?;
        let mut ids: Vec<_> = page.items.iter().map(|r| r.id).collect();
        assert!(page.stats.point_gets <= 274 && page.stats.max_candidates <= 2);
        while !page.finished {
            page = store.history_page(0, &page.cursor, limit)?;
            ids.extend(page.items.iter().map(|r| r.id));
        }
        assert_eq!(ids, expected);
        let empty = store.history_page(0, &page.cursor, limit)?;
        assert!(empty.finished && empty.items.is_empty());
        assert_eq!(
            HistoryCursor::from_base64(&page.cursor.to_base64())?,
            page.cursor
        );
    }
    for row in &rows {
        assert_eq!(
            store.history_point(0, row.event_id()?)?.map(|r| r.text),
            Some(row.text.clone())
        );
    }
    assert!(store.history_point(0, id(999))?.is_none());
    let first = store.history_first(0, 1)?;
    let a3 = prepared(
        0,
        0,
        3,
        Some(rows[2].event_id()?),
        [5; 16],
        "after snapshot",
        vec![],
    )?;
    append(&mut store, 0, a3.clone())?;
    drop(store);
    let mut store = reopen(dir.path())?;
    let mut suffix = Vec::new();
    let mut cursor = first.cursor;
    loop {
        let page = store.history_page(0, &cursor, 1)?;
        suffix.extend(page.items.iter().map(|r| r.id));
        cursor = page.cursor;
        if page.finished {
            break;
        }
    }
    assert_eq!(suffix, expected[1..]);
    let mut all = rows;
    all.push(a3);
    assert_eq!(
        store
            .history_first(0, 32)?
            .items
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        reference(&all)?
    );
    Ok(())
}

#[test]
fn history_scope_cursor_domains_tampering_and_snapshot_unavailable() -> TestResult {
    let (_dir, mut store) = fixture()?;
    let genesis = setup(&mut store, 0)?;
    setup(&mut store, 1)?;
    append(
        &mut store,
        0,
        prepared(0, 0, 1, Some(genesis[0]), [1; 16], "one", vec![])?,
    )?;
    let page = store.history_first(0, 1)?;
    assert!(matches!(
        store.history_page(1, &page.cursor, 1),
        Err(Error::InvalidCursor)
    ));
    for offset in [0, 8, 40, 72, 104, 136, 168, 211, 243, 274] {
        let mut raw = page.cursor.as_bytes().to_vec();
        raw[offset] ^= 1;
        let outcome =
            HistoryCursor::from_bytes(&raw).and_then(|cursor| store.history_page(0, &cursor, 1));
        assert!(matches!(outcome, Err(Error::InvalidCursor)));
    }
    let text = page.cursor.to_base64();
    assert!(HistoryCursor::from_base64(&(text.clone() + "=")).is_err());
    assert!(HistoryCursor::from_base64(&text[..366]).is_err());
    let (other_dir, mut other) = fixture()?;
    setup(&mut other, 0)?;
    assert!(matches!(
        other.history_page(0, &page.cursor, 1),
        Err(Error::InvalidCursor)
    ));
    drop(other);
    drop(other_dir);
    let tx = store.vault.database.begin_read()?;
    let nodes = tx.open_table(NODES)?;
    let values = tx.open_table(VALUES)?;
    let mut w = Work::new(&nodes, &values, &store.crypto, store.root.clone());
    let (identity, heads) = w.history_view(0)?;
    let impossible = cursor::seal(
        &store.crypto,
        &identity,
        &Position {
            frontiers: [heads.frontiers[0] + 1, heads.frontiers[1]],
            next: [1, 1],
        },
    )?;
    drop(w);
    drop(values);
    drop(nodes);
    drop(tx);
    assert!(matches!(
        store.history_page(0, &impossible, 1),
        Err(Error::SnapshotUnavailable)
    ));
    for limit in [0, 33, usize::MAX] {
        assert!(store.history_first(0, limit).is_err());
    }
    Ok(())
}

#[test]
fn history_codecs_exact_maxima_and_strict_roundtrips() -> TestResult {
    let (_dir, mut store) = fixture()?;
    setup(&mut store, 0)?;
    let mut page = store.history_first(0, 32)?;
    page.frontiers = [u64::MAX; 2];
    page.finished = false;
    let row = HistoryRow {
        id: [255; 32],
        role: 1,
        ordinal: u64::MAX,
        sequence: u64::MAX,
        request: [255; 16],
        text: "\0".repeat(4096),
    };
    page.items = vec![row; 32];
    let cbor = page.encode_cbor()?;
    let json = page.encode_json()?;
    assert_eq!(cbor.len(), 133775);
    assert_eq!(json.len(), 793405);
    assert_eq!(&cbor[..4], &[0x87, 0x02, 0x58, 0x20]);
    // Independent canonical CBOR max-row vector (arrays, bstr, unsigned u64).
    let mut known = vec![0x86, 0x58, 0x20];
    known.extend([255; 32]);
    known.push(1);
    for _ in 0..2 {
        known.push(0x1b);
        known.extend([255; 8]);
    }
    known.push(0x50);
    known.extend([255; 16]);
    known.extend([0x59, 0x10, 0x00]);
    known.extend([0; 4096]);
    assert_eq!(known.len(), 4170);
    assert_eq!(&cbor[335..], known.repeat(32));
    let decoded: serde_json::Value = serde_json::from_slice(&json)?;
    assert_eq!(decoded["items"][0]["text"], "\0".repeat(4096));
    assert_eq!(decoded["cursor"].as_str().map(str::len), Some(367));
    assert_eq!(decoded["items"][0]["ordinal"].as_u64(), Some(u64::MAX));
    for text in ["Ж🌐".repeat(512), "\n\r\t\u{1}\"\\".repeat(400)] {
        page.items[0].text = text.clone();
        let encoded = page.encode_json()?;
        let parsed: serde_json::Value = serde_json::from_slice(&encoded)?;
        assert_eq!(parsed["items"][0]["text"], text);
        assert!(page.encode_cbor()?.len() <= 256 * 1024);
    }
    page.items[0].text = "a".repeat(4097);
    assert!(page.encode_json().is_err());
    assert!(page.encode_cbor().is_err());
    let identity = Identity {
        incarnation: store.root.incarnation,
        init: init(0),
    };
    assert_eq!(identity.encode()?.len(), 177);
    assert_eq!(Identity::decode(&identity.encode()?)?, identity);
    let heads = Heads {
        view: identity.view_id()?,
        frontiers: [1, 1],
        tails: [None, None],
    };
    assert_eq!(heads.encode().len(), 118);
    assert_eq!(Heads::decode(&heads.encode())?, heads);
    let request = RequestRecord {
        queue: u64::MAX,
        request: [1; 16],
        body_hash: [2; 32],
        event: Some([3; 32]),
    };
    assert_eq!(request.encode().len(), 94);
    assert_eq!(
        RequestRecord::decode(&request.encode())?.event,
        request.event
    );
    let binding = Binding {
        format: 2,
        view: identity.view_id()?,
        kind: 3,
        role: 1,
        ordinal: u64::MAX,
        sequence: u64::MAX,
        request: [1; 16],
        previous: Some([2; 32]),
        core_offset: u16::MAX,
        core_len: 2048,
        core_hash: [3; 32],
        source_hash: [4; 32],
        parents: vec![
            HistoryParent {
                event: [1; 32],
                role: 0,
                ordinal: 1,
            },
            HistoryParent {
                event: [2; 32],
                role: 1,
                ordinal: 2,
            },
        ],
    };
    let raw = binding.encode()?;
    assert_eq!(raw.len(), 255);
    assert_eq!(Binding::decode(&raw)?.encode()?, raw);
    let mut trailing = raw.clone();
    trailing.push(0);
    assert!(Binding::decode(&trailing).is_err());
    let mut too_many = raw;
    too_many[172] = 3;
    assert!(Binding::decode(&too_many).is_err());
    let mut bad = request.encode();
    bad[4] = 0;
    assert!(RequestRecord::decode(&bad).is_err());
    let mut bad = heads.encode();
    bad[53] = 1;
    assert!(Heads::decode(&bad).is_err());
    Ok(())
}

#[test]
fn history_request_fifo_retry_and_ack_do_not_replay_native_or_consume() -> TestResult {
    let (dir, mut store) = fixture()?;
    let genesis = setup(&mut store, 0)?;
    let row = prepared(0, 0, 1, Some(genesis[0]), [1; 16], "hello", vec![])?;
    apply(
        &mut store,
        0,
        HistoryOperation::QueueAccept {
            request: row.request,
            text: row.text.clone(),
        },
    )?;
    let queued = store.history_request(0, row.request)?.ok_or("queued")?;
    assert!(queued.event.is_none());
    let state = native(&mut store, 0, std::slice::from_ref(&row))?;
    let op = HistoryOperation::TextBatch {
        rows: vec![row.clone()],
        native: state,
    };
    let expected = store.generation();
    let commit = id(expected);
    store.commit_history(0, expected, commit, op.clone())?;
    let after = snapshot(&store);
    assert!(
        store
            .commit_history(0, expected, commit, op.clone())?
            .duplicate
    );
    assert_eq!(snapshot(&store), after);
    let mut altered = op.clone();
    if let HistoryOperation::TextBatch { native, .. } = &mut altered {
        native.after_blob[0] ^= 1;
    }
    assert!(matches!(
        store.commit_history(0, expected, commit, altered),
        Err(Error::Conflict)
    ));
    apply(
        &mut store,
        0,
        HistoryOperation::QueueAccept {
            request: [2; 16],
            text: "hello".into(),
        },
    )?;
    let before = snapshot(&store);
    assert!(apply(&mut store, 0, op).is_err());
    assert_eq!(snapshot(&store), before);
    assert!(
        store
            .history_request(0, [2; 16])?
            .is_some_and(|r| r.event.is_none())
    );
    let same = apply(
        &mut store,
        0,
        HistoryOperation::QueueAccept {
            request: row.request,
            text: "hello".into(),
        },
    )?;
    assert!(same.duplicate);
    assert_eq!(snapshot(&store), before);
    assert!(
        apply(
            &mut store,
            0,
            HistoryOperation::QueueAccept {
                request: row.request,
                text: "changed".into()
            }
        )
        .is_err()
    );
    assert_eq!(
        store.history_request(0, row.request)?.and_then(|r| r.event),
        Some(row.event_id()?)
    );
    let ack = HistoryOperation::Acknowledge {
        ordinal: 1,
        event: row.event_id()?,
        auth: vec![],
    };
    apply(&mut store, 0, ack.clone())?;
    let after_ack = snapshot(&store);
    apply(&mut store, 0, ack)?;
    assert_eq!(snapshot(&store), after_ack);
    drop(store);
    let mut store = reopen(dir.path())?;
    assert_eq!(
        store.history_request(0, row.request)?.and_then(|r| r.event),
        Some(row.event_id()?)
    );
    Ok(())
}

#[test]
fn history_invalid_rows_native_and_generic_fences_preserve_obligations_and_c2() -> TestResult {
    let (_dir, mut store) = fixture()?;
    let genesis = setup(&mut store, 0)?;
    setup(&mut store, 1)?;
    apply(
        &mut store,
        0,
        HistoryOperation::QueueAccept {
            request: [1; 16],
            text: "hello".into(),
        },
    )?;
    apply(
        &mut store,
        0,
        HistoryOperation::QueueAccept {
            request: [2; 16],
            text: "second".into(),
        },
    )?;
    let row = prepared(0, 0, 1, Some(genesis[0]), [1; 16], "hello", vec![])?;
    for case in 0..11 {
        let mut wrong = row.clone();
        match case {
            0 => wrong.scope.context = [6; 32],
            1 => wrong.scope.dialog = [42; 32],
            2 => wrong.role = 2,
            3 => wrong.ordinal = 2,
            4 => wrong.previous = Some(id(999)),
            5 => wrong.request = [3; 16],
            6 => wrong.text = "different".into(),
            7 => wrong.core_offset = u16::MAX,
            8 => {
                wrong.parents = vec![HistoryParent {
                    event: id(999),
                    role: 1,
                    ordinal: 1,
                }]
            }
            9 => {
                wrong.parents = vec![HistoryParent {
                    event: wrong.event_id()?,
                    role: 0,
                    ordinal: 1,
                }]
            }
            _ => {
                wrong.parents = vec![
                    HistoryParent {
                        event: [2; 32],
                        role: 1,
                        ordinal: 1,
                    },
                    HistoryParent {
                        event: [1; 32],
                        role: 0,
                        ordinal: 1,
                    },
                ]
            }
        }
        let state = native(&mut store, 0, std::slice::from_ref(&wrong))?;
        let before = snapshot(&store);
        assert!(
            apply(
                &mut store,
                0,
                HistoryOperation::TextBatch {
                    rows: vec![wrong],
                    native: state
                }
            )
            .is_err(),
            "case{case}"
        );
        assert_eq!(snapshot(&store), before);
    }
    // FIFO: the second accepted request cannot steal the first ordinal.
    let second = prepared(0, 0, 1, Some(genesis[0]), [2; 16], "second", vec![])?;
    let state = native(&mut store, 0, std::slice::from_ref(&second))?;
    let before = snapshot(&store);
    assert!(
        apply(
            &mut store,
            0,
            HistoryOperation::TextBatch {
                rows: vec![second],
                native: state
            }
        )
        .is_err()
    );
    assert_eq!(snapshot(&store), before);
    for case in 0..4 {
        let mut state = native(&mut store, 0, std::slice::from_ref(&row))?;
        match case {
            0 => state.before_hash = Some(id(999)),
            1 => state.frame_ids.push(id(999)),
            2 => state.after_blob.clear(),
            _ => state.after_blob.resize(128 * 1024, 0x7f),
        };
        let before = snapshot(&store);
        assert!(
            apply(
                &mut store,
                0,
                HistoryOperation::TextBatch {
                    rows: vec![row.clone()],
                    native: state
                }
            )
            .is_err()
        );
        assert_eq!(snapshot(&store), before);
    }
    let before = snapshot(&store);
    let native = native(&mut store, 0, &[])?;
    assert!(
        apply(
            &mut store,
            0,
            HistoryOperation::TextBatch {
                rows: vec![],
                native
            }
        )
        .is_err()
    );
    let generic = vec![
        CreditAction::Queue {
            context: 1,
            sequence: 999,
            id: id(999),
            body: b"c2".to_vec(),
        },
        CreditAction::Queue {
            context: 0,
            sequence: 999,
            id: id(999),
            body: b"bound".to_vec(),
        },
    ];
    assert!(
        store
            .commit(store.generation(), id(900), vec![], generic)
            .is_err()
    );
    assert_eq!(snapshot(&store), before);
    for key in [
        identity_key(0)?,
        heads_key(0)?,
        Key::hash(0, Namespace::Native, [0; 32])?,
    ] {
        assert!(
            store
                .commit(
                    store.generation(),
                    id(901),
                    vec![Mutation::put(key, vec![1; 32])],
                    vec![]
                )
                .is_err()
        );
        assert_eq!(snapshot(&store), before);
    }
    assert!(
        store
            .history_request(0, [1; 16])?
            .is_some_and(|r| r.event.is_none())
    );
    assert!(
        store
            .history_request(0, [2; 16])?
            .is_some_and(|r| r.event.is_none())
    );
    Ok(())
}

#[test]
fn history_fresh_rejects_invisible_domain_state_and_allows_only_profile_bootstrap() -> TestResult {
    for case in 0..9 {
        let (_dir, mut store) = fixture()?;
        let mut puts = vec![];
        let mut credits = vec![];
        match case {
            0 => credits.push(CreditAction::Queue {
                context: 0,
                sequence: 1,
                id: id(1),
                body: b"queued".to_vec(),
            }),
            1 => {
                credits.push(CreditAction::Pending {
                    context: 0,
                    ordinal: 1,
                    event: id(1),
                    body: vec![9; 1024],
                    head: false,
                    grant: None,
                });
                puts.push(Mutation::put(
                    Key::hash(0, Namespace::Native, [0; 32])?,
                    vec![8; 64],
                ));
            }
            2 => puts.push(Mutation::put(
                Key::hash(0, Namespace::Native, [0; 32])?,
                vec![8; 64],
            )),
            3 => puts.push(Mutation::put(
                Key::hash(0, Namespace::SeenA, id(1))?,
                vec![0; 40],
            )),
            4 => puts.push(Mutation::put(Key::request(0, [1; 16])?, vec![1; 16])),
            5 => puts.push(Mutation::put(
                Key::hash(0, Namespace::Auth, id(1))?,
                vec![1; 16],
            )),
            6 => puts.push(Mutation::put(
                Key::hash(0, Namespace::Current, id(1))?,
                vec![1; 16],
            )),
            7 => {
                let mut v = vec![5; 32];
                v.extend(id(1));
                puts.push(Mutation::put(Key::ordinal(0, Namespace::Author0, 7)?, v));
            }
            _ => credits.push(CreditAction::Grant {
                context: 0,
                id: id(1),
                start: 1,
                end: 2,
            }),
        }
        store.commit(store.generation(), id(88), puts, credits)?;
        assert_eq!(store.root.roots[Namespace::Event as usize].count, 0);
        let before = snapshot(&store);
        assert!(
            matches!(
                apply(&mut store, 0, HistoryOperation::Init(init(0))),
                Err(Error::ExistingStateRequiresMigration)
            ),
            "case{case}"
        );
        assert_eq!(snapshot(&store), before);
    }
    let (_dir, mut store) = fixture()?;
    let mut author = vec![6; 32];
    author.extend(id(77));
    let author_key = Key::ordinal(1, Namespace::Author0, 77)?;
    store.commit(
        store.generation(),
        id(88),
        vec![Mutation::put(author_key, author.clone())],
        vec![],
    )?;
    apply(&mut store, 0, HistoryOperation::Init(init(0)))?;
    assert_eq!(store.get(author_key)?, Some(author));
    assert!(matches!(
        apply(&mut store, 1, HistoryOperation::Init(init(1))),
        Err(Error::ExistingStateRequiresMigration)
    ));
    Ok(())
}

#[test]
fn history_active_repeat_and_global_author_owner_are_isolated() -> TestResult {
    let (_dir, mut store) = fixture()?;
    let g0 = setup(&mut store, 0)?;
    let g1 = setup(&mut store, 1)?;
    let mut a = prepared(0, 0, 1, Some(g0[0]), [1; 16], "c0", vec![])?;
    a.sequence = 42;
    recode(&mut a)?;
    a.extras.author_slot = Some(0);
    append(&mut store, 0, a)?;
    let mut b = prepared(1, 0, 1, Some(g1[0]), [2; 16], "c1", vec![])?;
    b.sequence = 42;
    recode(&mut b)?;
    b.extras.author_slot = Some(0);
    apply(
        &mut store,
        1,
        HistoryOperation::QueueAccept {
            request: b.request,
            text: b.text.clone(),
        },
    )?;
    let state = native(&mut store, 1, std::slice::from_ref(&b))?;
    let before = snapshot(&store);
    assert!(
        apply(
            &mut store,
            1,
            HistoryOperation::TextBatch {
                rows: vec![b],
                native: state
            }
        )
        .is_err()
    );
    assert_eq!(snapshot(&store), before);
    let pending = prepared(0, 1, 1, Some(g0[1]), [3; 16], "pending", vec![])?;
    accept_peer(&mut store, 0, &pending)?;
    let before = snapshot(&store);
    assert!(apply(&mut store, 0, HistoryOperation::Init(init(0)))?.duplicate);
    assert_eq!(snapshot(&store), before);
    let mut other = init(0);
    other.scope.epoch = [91; 32];
    assert!(matches!(
        apply(&mut store, 0, HistoryOperation::Init(other)),
        Err(Error::Conflict)
    ));
    assert_eq!(snapshot(&store), before);
    Ok(())
}

#[test]
fn history_corrupt_expected_payload_never_becomes_empty_or_initialized() -> TestResult {
    for ns in [Namespace::Current, Namespace::Event] {
        let (_dir, mut store) = fixture()?;
        let g = setup(&mut store, 0)?;
        let row = prepared(0, 0, 1, Some(g[0]), [1; 16], "body", vec![])?;
        let event = row.event_id()?;
        append(&mut store, 0, row)?;
        let key = if ns == Namespace::Current {
            heads_key(0)?
        } else {
            Key::hash(0, ns, event)?
        };
        let address = store
            .crypto
            .address(key.slot()? as u8, b"value", &key.bytes);
        let tx = store.vault.database.begin_write()?;
        {
            let mut table = tx.open_table(VALUES)?;
            let mut raw = table
                .get(address.as_slice())?
                .ok_or("cipher")?
                .value()
                .to_vec();
            let last = raw.len() - 1;
            raw[last] ^= 1;
            table.insert(address.as_slice(), raw.as_slice())?;
        }
        tx.commit()?;
        let before = snapshot(&store);
        assert!(matches!(
            store.history_first(0, 32),
            Err(Error::Integrity(_))
        ));
        assert!(matches!(
            apply(&mut store, 0, HistoryOperation::Init(init(0))),
            Err(Error::Integrity(_))
        ));
        assert_eq!(snapshot(&store), before);
    }
    Ok(())
}

#[test]
fn history_fault_reopen_resolves_old_or_new_atomic_state_without_replay() -> TestResult {
    for point in ["before_write", "before_sync", "after_sync"] {
        let (dir, mut store) = fixture()?;
        let g = setup(&mut store, 0)?;
        let row = prepared(0, 0, 1, Some(g[0]), [1; 16], "funded", vec![])?;
        apply(
            &mut store,
            0,
            HistoryOperation::QueueAccept {
                request: row.request,
                text: row.text.clone(),
            },
        )?;
        let state = native(&mut store, 0, std::slice::from_ref(&row))?;
        let op = HistoryOperation::TextBatch {
            rows: vec![row.clone()],
            native: state,
        };
        let expected = store.generation();
        let commit = id(expected);
        let old = snapshot(&store);
        store.backend.fail(point, 0)?;
        assert!(
            store
                .commit_history(0, expected, commit, op.clone())
                .is_err()
        );
        assert!(matches!(
            store.history_first(0, 32),
            Err(Error::ReopenRequired)
        ));
        drop(store);
        let mut store = reopen(dir.path())?;
        let old_visible = store.history_point(0, row.event_id()?)?.is_none();
        if old_visible {
            assert_eq!(snapshot(&store), old);
            assert!(
                store
                    .history_request(0, row.request)?
                    .is_some_and(|r| r.event.is_none())
            );
        } else {
            let event = row.event_id()?;
            assert!(
                store
                    .history_request(0, row.request)?
                    .is_some_and(|r| r.event == Some(event))
            );
        }
        store.commit_history(0, expected, commit, op.clone())?;
        let once = snapshot(&store);
        assert!(store.commit_history(0, expected, commit, op)?.duplicate);
        assert_eq!(snapshot(&store), once);
        assert_eq!(store.history_first(0, 32)?.items.len(), 1);
    }
    Ok(())
}

#[test]
fn history_peer_capture_and_local_role_b_use_distinct_receipt_and_request_lanes() -> TestResult {
    let (_dir, mut store) = fixture()?;
    let mut identity = init(0);
    identity.own_role = 1;
    apply(&mut store, 0, HistoryOperation::Init(identity))?;
    let mut genesis = [[0; 32]; 2];
    for (role, event) in genesis.iter_mut().enumerate() {
        let row = prepared(0, role as u8, 0, None, [0; 16], "", vec![])?;
        *event = row.event_id()?;
        let native = native(&mut store, 0, std::slice::from_ref(&row))?;
        apply(&mut store, 0, HistoryOperation::Genesis { row, native })?;
    }
    let peer = prepared(0, 0, 1, Some(genesis[0]), [1; 16], "peer A", vec![])?;
    accept_peer(&mut store, 0, &peer)?;
    let capture = HistoryCapture {
        original: HistoryAuth {
            id: id(700),
            bytes: vec![7; 1024],
        },
        admission: HistoryAuth {
            id: id(701),
            bytes: vec![8; 1024],
        },
    };
    apply(
        &mut store,
        0,
        HistoryOperation::CapturePending {
            ordinal: 1,
            event: peer.event_id()?,
            capture: capture.clone(),
        },
    )?;
    let before = snapshot(&store);
    apply(
        &mut store,
        0,
        HistoryOperation::CapturePending {
            ordinal: 1,
            event: peer.event_id()?,
            capture: capture.clone(),
        },
    )?;
    assert_eq!(snapshot(&store), before);
    let own = prepared(
        0,
        1,
        1,
        Some(genesis[1]),
        [2; 16],
        "own B",
        vec![HistoryParent {
            event: peer.event_id()?,
            role: 0,
            ordinal: 1,
        }],
    )?;
    apply(
        &mut store,
        0,
        HistoryOperation::QueueAccept {
            request: own.request,
            text: own.text.clone(),
        },
    )?;
    let rows = vec![peer.clone(), own.clone()];
    let native = native(&mut store, 0, &rows)?;
    apply(&mut store, 0, HistoryOperation::TextBatch { rows, native })?;
    assert_eq!(
        store
            .history_first(0, 32)?
            .items
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        vec![peer.event_id()?, own.event_id()?]
    );
    let mut admission = vec![5];
    admission.extend(capture.original.id);
    admission.extend(capture.admission.id);
    assert_eq!(
        store.get(Key::hash(0, Namespace::Admission, peer.event_id()?)?)?,
        Some(admission)
    );
    assert_eq!(
        store.get(receipt(0, 0, 1)?)?,
        Some(peer.event_id()?.to_vec())
    );
    assert!(store.get(receipt(0, 1, 1)?)?.is_none());
    assert_eq!(
        store.history_request(0, own.request)?.and_then(|r| r.event),
        Some(own.event_id()?)
    );
    apply(
        &mut store,
        0,
        HistoryOperation::Acknowledge {
            ordinal: 1,
            event: own.event_id()?,
            auth: vec![],
        },
    )?;
    assert_eq!(
        store.get(receipt(0, 1, 1)?)?,
        Some(own.event_id()?.to_vec())
    );
    let before = snapshot(&store);
    accept_peer(&mut store, 0, &peer)?;
    assert_eq!(snapshot(&store), before);
    Ok(())
}

#[test]
fn history_generic_credit_fence_uses_actual_global_author_owner() -> TestResult {
    let (_dir, mut store) = fixture()?;
    let g = setup(&mut store, 0)?;
    let mut row = prepared(0, 0, 1, Some(g[0]), [1; 16], "own", vec![])?;
    row.sequence = 777;
    row.extras.author_slot = Some(0);
    recode(&mut row)?;
    append(&mut store, 0, row)?;
    let e = Key::hash(1, Namespace::Event, id(800))?;
    let p = Key::hash(1, Namespace::Projection, id(800))?;
    store.commit(
        store.generation(),
        id(801),
        vec![
            Mutation::put(e, vec![1; 1024]),
            Mutation::put(
                p,
                ProjectionValue {
                    text: "raw C1".into(),
                    binding: vec![],
                }
                .encode()?,
            ),
        ],
        vec![CreditAction::Queue {
            context: 1,
            sequence: 800,
            id: id(802),
            body: b"raw C1".to_vec(),
        }],
    )?;
    let before = snapshot(&store);
    let cross = CreditAction::Materialize {
        context: 1,
        queue: 800,
        request: id(802),
        ordinal: 1,
        event: id(800),
        records: vec![e, p, Key::ordinal(1, Namespace::Author0, 777)?],
    };
    assert!(
        store
            .commit(store.generation(), id(803), vec![], vec![cross])
            .is_err()
    );
    assert_eq!(snapshot(&store), before);
    // The same unbound S1 transition without the bound C0 reference is still supported.
    store.commit(
        store.generation(),
        id(804),
        vec![],
        vec![CreditAction::Materialize {
            context: 1,
            queue: 800,
            request: id(802),
            ordinal: 1,
            event: id(800),
            records: vec![e, p],
        }],
    )?;
    assert_eq!(store.get(e)?, Some(vec![1; 1024]));
    Ok(())
}

#[test]
fn history_private_selection_errors_on_blocked_remaining_and_selects_min_eligible_id() -> TestResult
{
    let binding = |role, parents| Binding {
        format: 1,
        view: [0; 32],
        kind: 3,
        role,
        ordinal: 1,
        sequence: 0,
        request: [0; 16],
        previous: Some([0; 32]),
        core_offset: 0,
        core_len: 1,
        core_hash: [0; 32],
        source_hash: [0; 32],
        parents,
    };
    let row = |id, role| HistoryRow {
        id: [id; 32],
        role,
        ordinal: 1,
        sequence: 0,
        request: [0; 16],
        text: String::new(),
    };
    let mut candidates = [
        Some(Candidate {
            row: row(2, 0),
            binding: binding(
                0,
                vec![HistoryParent {
                    event: [1; 32],
                    role: 1,
                    ordinal: 1,
                }],
            ),
        }),
        Some(Candidate {
            row: row(1, 1),
            binding: binding(
                1,
                vec![HistoryParent {
                    event: [2; 32],
                    role: 0,
                    ordinal: 1,
                }],
            ),
        }),
    ];
    assert!(matches!(
        choose_candidate(&candidates, [1, 1]),
        Err(Error::Integrity(_))
    ));
    for candidate in candidates.iter_mut().flatten() {
        candidate.binding.parents.clear();
    }
    assert_eq!(choose_candidate(&candidates, [1, 1])?, 1);
    Ok(())
}
