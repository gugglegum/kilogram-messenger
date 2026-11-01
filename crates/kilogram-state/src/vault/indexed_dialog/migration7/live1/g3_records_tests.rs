//! Structural encoding/charging only: no valid Native origin or protocol world.
use super::*;

fn metadata(n: u64) -> L1Metadata {
    L1Metadata {
        policy_generation: 3,
        wall_high_water: n,
        next: [2, 1],
        tails: [Some([7; 32]), None],
        next_queue: 2,
        request_index_after: None,
        request_index_ready: true,
        ack_next: 1,
        peer_source_next: 1,
        peer_source_last: Some([8; 32]),
    }
}
fn existing(revision: u64) -> ExistingState7 {
    ExistingState7 {
        mode: ExistingMode::Active,
        current_consent: Some([9; 32]),
        selected_consent: Some([9; 32]),
        q: Some([10; 32]),
        cuts: [Some(2), Some(1)],
        previous: [Some([7; 32]), Some([8; 32])],
        locator_revision: [revision, revision],
        ..Default::default()
    }
}
fn locator(revision: u64, now: u64, ip: bool) -> Result<Vec<u8>> {
    codec::encode(
        &a([
            a([
                u(2),
                b([1; 32]),
                b([2; 32]),
                b([3; 32]),
                b([4; 32]),
                a(if ip {
                    vec![a([u(4), b([127, 0, 0, 1]), u(40123)])]
                } else {
                    vec![]
                }),
                opt(Some(b(b"http://127.0.0.1:40123/"))),
                u(revision),
                u(now),
                u(now + 3600),
            ]),
            b([0; 64]),
        ]),
        8192,
    )
}
#[test]
fn g3_fixed_records_and_real_work_encoding_charge_both_slots() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let core = IndexedDialogVault::create(
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
    let tx = core.vault.database.begin_read().map_err(db)?;
    let nodes = tx.open_table(NODES).map_err(db)?;
    let values = tx.open_table(VALUES).map_err(db)?;
    let mut root = core.root.clone();
    root.layout = RootLayout::Migrated7;
    root.ledger = std::array::from_fn(|_| credits::Ledger::migration_unfunded(1 << 30));
    let mut w = Work::new(&nodes, &values, &core.crypto, root);
    for c in 0..2 {
        assert!(read(c, |key| w.get(key)).is_err());
        // Real record envelopes; this fixture claims only fixed metadata identity.
        let h = codec::record(10, &a([u(7), b([0; 32])]), 8192)?;
        let budget = codec::record(6, &a([u(7), u(c as u64), u(1), u(0)]), 4096)?;
        w.put(head_key(c)?, &h, (8192 - h.len()) as u64, true)?;
        assert!(read(c, |key| w.get(key)).is_err());
        w.put(budget_key(c)?, &budget, (4096 - budget.len()) as u64, true)?;
        let sparse = read(c, |key| w.get(key))?;
        assert!(sparse.own_locator.is_none() && sparse.metadata.is_none());
        let rows = [
            (b"locator2-own".as_slice(), locator(23, 23, false)?, 8192),
            (b"locator2-peer".as_slice(), locator(23, 23, false)?, 8192),
            (b"existing".as_slice(), packed(&existing(127))?, 24 * 1024),
            (b"meta".as_slice(), packed(&metadata(127))?, META),
        ];
        for (name, raw, cap) in rows {
            w.put(key(c, name)?, &raw, (cap - raw.len()) as u64, true)?;
        }
    }
    let immutable = [read(0, |key| w.get(key))?, read(1, |key| w.get(key))?];
    for (revision, wall, ip) in [
        (24, 24, true),
        (255, 255, false),
        (256, 256, true),
        (16383, 16383, false),
        (16384, 16384, true),
    ] {
        for c in 0..2 {
            let before = accounting::snapshot(&w.root.roots)?;
            let prior = read(c, |key| w.get(key))?;
            let rows = [
                (
                    b"locator2-own".as_slice(),
                    locator(revision, wall, ip)?,
                    8192,
                ),
                (
                    b"locator2-peer".as_slice(),
                    locator(revision, wall, false)?,
                    8192,
                ),
                (
                    b"existing".as_slice(),
                    packed(&existing(revision))?,
                    24 * 1024,
                ),
                (b"meta".as_slice(), packed(&metadata(wall))?, META),
            ];
            for (name, raw, cap) in rows {
                w.put(key(c, name)?, &raw, (cap - raw.len()) as u64, true)?;
            }
            let after = accounting::snapshot(&w.root.roots)?;
            let current = read(c, |key| w.get(key))?;
            let lengths = |r: &L1FixedRecords7| -> Result<u64> {
                [&r.own_locator, &r.peer_locator, &r.existing, &r.metadata]
                    .into_iter()
                    .try_fold(0u64, |sum, v| {
                        sum.checked_add(
                            v.as_ref()
                                .ok_or(Error::Integrity("G3 required fixture record"))?
                                .length,
                        )
                        .ok_or(Error::Bound("G3 length sum"))
                    })
            };
            let delta = i128::from(lengths(&current)?) - i128::from(lengths(&prior)?);
            assert_eq!(
                i128::from(after.runtime.used[c as usize])
                    - i128::from(before.runtime.used[c as usize]),
                delta
            );
            assert_eq!(
                i128::from(after.runtime.reserved[c as usize])
                    - i128::from(before.runtime.reserved[c as usize]),
                -delta
            );
            assert_eq!(after.runtime.records, before.runtime.records);
            assert_eq!(
                after.runtime.used[1 - c as usize],
                before.runtime.used[1 - c as usize]
            );
            assert_eq!(
                after.runtime.reserved[1 - c as usize],
                before.runtime.reserved[1 - c as usize]
            );
            assert_eq!(before.prepaid_metadata, after.prepaid_metadata);
            assert_eq!(current.head, immutable[c as usize].head);
            assert_eq!(current.legacy_budget, immutable[c as usize].legacy_budget);
        }
    }
    assert!(read(2, |key| w.get(key)).is_err());
    w.put(key(0, b"locator2-own")?, &vec![0; 8193], 0, true)?;
    assert!(read(0, |key| w.get(key)).is_err());
    assert!(fingerprint(Some(vec![0; 4097]), 4096).is_err());
    Ok(())
}
