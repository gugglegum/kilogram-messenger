use super::{tests as fixture, *};

#[test]
fn t11e1_tail_snapshot_resumes_without_a_prefix_scan() -> fixture::TestResult {
    let (directory, mut store) = fixture::fixture()?;
    assert!(matches!(
        store.history_position(0, None),
        Err(Error::HistoryUnavailable)
    ));
    assert!(store.history_position(2, None).is_err());
    fixture::setup(&mut store, 1)?;
    let empty = store.history_position(1, None)?;
    assert_eq!(empty.frontiers, [1, 1]);
    assert_eq!(empty.next, empty.frontiers);
    assert_eq!(
        store.history_position(1, Some(&empty.cursor))?.cursor,
        empty.cursor
    );
    assert!(store.history_page(1, &empty.cursor, 1)?.items.is_empty());

    let genesis = fixture::setup(&mut store, 0)?;
    let mut tails = genesis;
    let mut next = [1, 1];
    let mut append = |store: &mut IndexedDialogVault, role: u8, nonce: u8| -> Result<Id> {
        let own = usize::from(role);
        let peer = 1 - own;
        let parents = (next[peer] > 1).then(|| HistoryParent {
            event: tails[peer],
            role: 1 - role,
            ordinal: next[peer] - 1,
        });
        let row = fixture::prepared(
            0,
            role,
            next[own],
            Some(tails[own]),
            [nonce; 16],
            "tail test",
            parents.into_iter().collect(),
        )?;
        tails[own] = row.event_id()?;
        next[own] += 1;
        fixture::append(store, 0, row)?;
        Ok(tails[own])
    };
    let first_prefix = store.history_position(0, None)?;
    let a1 = append(&mut store, 0, 1)?;
    let single = store.history_position(0, Some(&first_prefix.cursor))?;
    assert_eq!((single.next, single.frontiers), ([1, 1], [2, 1]));
    let first = store.history_page(0, &single.cursor, 1)?;
    assert_eq!(first.items[0].id, a1);
    assert_eq!(first.stats.first_candidate, [Some(1), None]);
    assert!(first.finished);
    append(&mut store, 0, 2)?;
    append(&mut store, 1, 3)?;
    let anchor = store.history_position(0, None)?;
    assert_eq!((anchor.next, anchor.frontiers), ([3, 2], [3, 2]));
    let expected = [
        append(&mut store, 1, 4)?,
        append(&mut store, 0, 5)?,
        append(&mut store, 1, 6)?,
    ];
    let delta = store.history_position(0, Some(&anchor.cursor))?;
    assert_eq!((delta.next, delta.frontiers), ([3, 2], [4, 4]));
    let page1 = store.history_page(0, &delta.cursor, 1)?;
    assert_eq!(page1.items[0].id, expected[0]);
    assert_eq!(page1.stats.first_candidate, [Some(3), Some(2)]);
    let late_a = append(&mut store, 0, 7)?;
    let repeat = store.history_page(0, &delta.cursor, 1)?;
    assert_eq!(repeat.cursor, page1.cursor);
    assert_eq!(repeat.items[0].id, expected[0]);
    let page2 = store.history_page(0, &page1.cursor, 1)?;
    assert_eq!(page2.items[0].id, expected[1]);
    let late_b = append(&mut store, 1, 8)?;
    let repeat = store.history_page(0, &page1.cursor, 1)?;
    assert_eq!(repeat.cursor, page2.cursor);
    assert_eq!(repeat.items[0].id, expected[1]);
    let page3 = store.history_page(0, &page2.cursor, 1)?;
    assert!(page3.finished);
    assert_eq!(page3.frontiers, delta.frontiers);
    assert_eq!(page3.items[0].id, expected[2]);

    for (slot, cursor) in [(0, &page2.cursor), (1, &anchor.cursor)] {
        assert!(matches!(
            store.history_position(slot, Some(cursor)),
            Err(Error::InvalidCursor)
        ));
    }
    for offset in [8, 40, 72, 211, 227, 274] {
        let mut bytes = anchor.cursor.as_bytes().to_vec();
        bytes[offset] ^= 1;
        let bad = HistoryCursor::from_bytes(&bytes)?;
        assert!(matches!(
            store.history_position(0, Some(&bad)),
            Err(Error::InvalidCursor)
        ));
    }
    let (_other_directory, mut other) = fixture::fixture()?;
    fixture::setup(&mut other, 0)?;
    assert!(matches!(
        other.history_position(0, Some(&anchor.cursor)),
        Err(Error::InvalidCursor)
    ));
    {
        let tx = store.vault.database.begin_read()?;
        let nodes = tx.open_table(NODES)?;
        let values = tx.open_table(VALUES)?;
        let mut work = Work::new(&nodes, &values, &store.crypto, store.root.clone());
        let (identity, heads) = work.history_view(0)?;
        let mut changed_view = identity.clone();
        changed_view.init.scope.epoch[0] ^= 1;
        let obsolete = cursor::seal(
            &store.crypto,
            &changed_view,
            &Position {
                frontiers: heads.frontiers,
                next: heads.frontiers,
            },
        )?;
        assert!(matches!(
            store.history_position(0, Some(&obsolete)),
            Err(Error::InvalidCursor)
        ));
        let mut frontiers = heads.frontiers;
        frontiers[1] += 1;
        let future = cursor::seal(
            &store.crypto,
            &identity,
            &Position {
                frontiers,
                next: frontiers,
            },
        )?;
        assert!(matches!(
            store.history_position(0, Some(&future)),
            Err(Error::SnapshotUnavailable)
        ));
    }
    drop(store);
    let mut store = fixture::reopen(directory.path())?;
    let advanced = store.history_position(0, Some(&page3.cursor))?;
    let page = store.history_page(0, &advanced.cursor, 32)?;
    assert!(page.finished);
    assert_eq!(
        page.items.iter().map(|row| row.id).collect::<Vec<_>>(),
        [late_a, late_b]
    );
    let whole = store.history_first(0, 32)?;
    assert!(whole.finished && whole.items.len() == 8);
    assert_eq!(whole.cursor, page.cursor);
    assert_eq!(
        store.history_position(0, Some(&whole.cursor))?.next,
        whole.frontiers
    );
    println!(
        "T11E1_FIXED_DELTA_PASS both_roles=true cross_role_parents=true replay_after_append=true"
    );
    Ok(())
}

#[test]
fn t11e1_tail_ordinal_limits_use_no_wrapping_arithmetic() -> fixture::TestResult {
    let max = [u64::MAX, 3];
    let position = Position {
        frontiers: max,
        next: max,
    };
    assert_eq!(position.clone().follow(max)?.next, max);
    assert!(matches!(
        position.follow([u64::MAX - 1, 4]),
        Err(Error::SnapshotUnavailable)
    ));
    assert!(matches!(
        (Position {
            frontiers: [0, 1],
            next: [0, 1]
        })
        .follow([1, 1]),
        Err(Error::InvalidCursor)
    ));
    Ok(())
}

fn fill(store: &mut IndexedDialogVault, count: u64) -> Result<()> {
    let mut heads = scale::current_heads(store)?;
    while scale::count(&heads) < count {
        let old = scale::count(&heads);
        let mut rows = Vec::new();
        for i in old..(old + 64).min(count) {
            let row = scale::next_row(&mut heads, i)?;
            if row.role == 0 {
                fixture::apply(
                    store,
                    0,
                    HistoryOperation::QueueAccept {
                        request: row.request,
                        text: row.text.clone(),
                    },
                )?;
            }
            rows.push(row);
        }
        store.history_fixture_commit(rows)?;
    }
    Ok(())
}

#[test]
fn t11e1_tail_work_depends_on_delta_and_reads_do_not_write() -> fixture::TestResult {
    let mut work_counts = Vec::new();
    let (_directory, mut store) = fixture::fixture()?;
    fixture::setup(&mut store, 0)?;
    for size in [32, 512] {
        fill(&mut store, size)?;
        let root = store.root.encode();
        let generation = store.generation();
        let records = store.record_counts()?;
        let anchor = store.history_position(0, None)?;
        let idle = store.history_position(0, Some(&anchor.cursor))?;
        assert_eq!(idle.cursor, anchor.cursor);
        for position in [&anchor, &idle] {
            assert!(position.stats.point_gets <= 3);
            assert_eq!(position.stats.candidate_loads, 0);
            assert_eq!(position.stats.returned_payload_bytes, 0);
        }
        assert_eq!(
            (
                store.root.encode(),
                store.generation(),
                store.record_counts()?
            ),
            (root, generation, records)
        );

        fill(&mut store, size + 3)?;
        let root = store.root.encode();
        let generation = store.generation();
        let records = store.record_counts()?;
        let delta = store.history_position(0, Some(&anchor.cursor))?;
        let page = store.history_page(0, &delta.cursor, 3)?;
        assert!(page.finished);
        assert_eq!(page.items.len(), 3);
        assert_eq!(page.stats.first_candidate, anchor.frontiers.map(Some));
        assert_eq!(page.stats.candidate_loads, 3);
        assert!(page.stats.max_candidates <= 2 && page.stats.point_gets <= 8 * (3 + 2) + 2);
        for row in &page.items {
            assert!(row.ordinal >= delta.next[usize::from(row.role)]);
        }
        assert_eq!(
            (
                store.root.encode(),
                store.generation(),
                store.record_counts()?
            ),
            (root, generation, records)
        );
        work_counts.push([
            anchor.stats.point_gets,
            idle.stats.point_gets,
            page.stats.point_gets,
        ]);
        println!(
            "T11E1_TAIL_WORK old={size} anchor={:?} idle={:?} delta={:?}",
            anchor.stats, idle.stats, page.stats
        );
    }
    assert_eq!(work_counts[0], work_counts[1]);
    Ok(())
}
