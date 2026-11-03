//! Synthetic unequal-ID unit records through Controller's exact shared seams.
use super::*;
use source::migration_source_fixture::Fixture;
fn sources(f: &Fixture) -> MigrationSources<'_> {
    MigrationSources::new(&f.contexts, &f.witnesses, &f.rows, &f.endpoints)
}
#[test]
fn s54_migration_source_roles_slots_frontiers_and_per_owner_absence() -> Result<()> {
    let mut cases = 0;
    for slot in [0, 1] {
        for frontiers in [[[3, 3], [3, 3]], [[17, 16], [25, 25]]] {
            let mut f = Fixture::new(slot, frontiers);
            for (owner, frontier) in frontiers.iter().enumerate() {
                let e = f.endpoints[owner].as_ref().context("synthetic endpoint")?;
                ensure!(
                    e.source != f.genesis[owner]
                        && e.lanes[0].slot == slot
                        && e.lanes[0].next == *frontier
                        && e.lanes[1].next == [1, 1],
                    "case prerequisites"
                );
                ensure!(
                    sources(&f).source(owner)? == e.source,
                    "exact migration source"
                );
                cases += 1;
            }
            ensure!(
                sources(&f).pair()? == [[31; 32], [32; 32]],
                "both migrated owners"
            );
            // Later interval frontiers are separate records; lookup is a migration baseline.
            let mut live = f.endpoints[0].clone().context("endpoint")?;
            live.lanes[0].next[0] += 1;
            ensure!(
                live != *f.endpoints[0].as_ref().context("baseline")?
                    && sources(&f).source(0)? == [31; 32],
                "live growth does not rewrite migration"
            );
            f.witnesses[1] = None;
            f.endpoints[1] = None;
            f.rows.remove("ready-1");
            ensure!(
                sources(&f).source(0)? == [31; 32] && sources(&f).pair().is_err(),
                "A current while B absent"
            );
            ensure!(
                sources(&f).source(2).is_err() && sources(&f).source(usize::MAX).is_err(),
                "range before indexing"
            );
        }
    }
    println!(
        "s54_verified_migration_source=synthetic;positive_owner_cases={cases};slot_mappings=2;mixed_owner=true;io=false"
    );
    Ok(())
}
#[test]
fn s54_migration_source_refuses_missing_and_contradictory_records() -> Result<()> {
    let mut negatives = 0;
    for mutation in 0..18 {
        let mut f = Fixture::new(1, [[3, 3], [3, 3]]);
        match mutation {
            0 => f.witnesses[0] = None,
            1 => f.endpoints[0] = None,
            2 => {
                f.rows.remove("ready-0");
            }
            3 => f.endpoints[0].as_mut().context("endpoint")?.source = f.genesis[0],
            4 => f.endpoints[0].as_mut().context("endpoint")?.lanes[0].role = 1,
            5 => f.endpoints[0].as_mut().context("endpoint")?.lanes[0].context = [93; 32],
            6 => {
                for lane in &mut f.endpoints[0].as_mut().context("endpoint")?.lanes {
                    lane.slot = 1 - lane.slot;
                }
            }
            7 => f.endpoints[0].as_mut().context("endpoint")?.lanes[0].next[0] += 1,
            8 => {
                f.rows.get_mut("ready-0").context("Ready")?["source_id"] =
                    state::hex(&f.genesis[0]).into()
            }
            9 => f.rows.get_mut("ready-0").context("Ready")?["contexts"][1]["role"] = 1.into(),
            10 => {
                f.rows.get_mut("ready-0").context("Ready")?["contexts"][1]["readable"] =
                    json!([4, 3])
            }
            11 => f.rows.get_mut("ready-0").context("Ready")?["contexts"][1]["queue"] = 0.into(),
            12 => f.contexts.swap(0, 1),
            13..=16 => f.corrupt_witness(mutation - 13)?,
            17 => f.rows.get_mut("ready-0").context("Ready")?["storage_stage"] = "Copying".into(),
            _ => unreachable!(),
        }
        ensure!(
            sources(&f).source(0).is_err(),
            "actual helper negative {mutation}"
        );
        negatives += 1;
    }
    println!(
        "s54_verified_migration_source=synthetic;record_negatives={negatives};no_genesis_fallback=true;io=false"
    );
    Ok(())
}
fn normal_fixture(ids: [old::Id; 2]) -> ([Closure; 2], Vec<Value>) {
    let pair = std::array::from_fn(|o| Closure {
        key: oracle::Incarnation {
            owner: ids[o],
            pid: o as u32 + 70,
            nonce: [o as u8 + 50; 32],
        },
        sequence: 1,
        counts: Default::default(),
        eof: 123,
        generation: 7,
        operation: [8; 32],
        point: [4, 2],
        outcome: o,
    });
    let rows=pair.iter().enumerate().map(|(o,c)|json!({"owner":o,"pid":c.key.pid,"code":0,"success":true,"waited":true,
        "counter_outcome":"normal exit0/final/EOF/source resolved",
        "normal_stop":{"source":state::hex(&c.key.owner),"incarnation":state::hex(&c.key.nonce),"acknowledged":true,"waited":true},
        "normal_closure":{"source":state::hex(&c.key.owner),"pid":c.key.pid,"incarnation":state::hex(&c.key.nonce),"eof":c.eof,
        "sequence":c.sequence,"root_generation":c.generation,"last_commit":state::hex(&c.operation),"source_point":c.point}})).collect();
    (pair, rows)
}
#[test]
fn s54_migration_source_normal_seams_reject_genesis_swapped_and_stale_closures() -> Result<()> {
    let f = Fixture::new(0, [[17, 16], [25, 25]]);
    let migration = sources(&f).pair()?;
    let (pair, rows) = normal_fixture(migration);
    ensure!(
        sources(&f).normal_pair(Some(&pair), &rows)? == pair,
        "three Controller pair consumers share actual seam"
    );
    for (owner, closure) in pair.iter().enumerate() {
        let expected = sources(&f).source(owner)?;
        normal::request_key(closure.key, closure.key.pid, expected)?;
        ensure!(
            normal::request_key(closure.key, closure.key.pid, f.genesis[owner]).is_err()
                && normal::request_key(closure.key, 999, expected).is_err(),
            "actual normal request source/PID seam"
        );
    }
    for ids in [f.genesis, [migration[1], migration[0]]] {
        let (bad, bad_rows) = normal_fixture(ids);
        ensure!(
            sources(&f).normal_pair(Some(&bad), &bad_rows).is_err(),
            "strict source selection"
        );
    }
    for field in [
        "owner",
        "pid",
        "code",
        "success",
        "waited",
        "normal_stop",
        "normal_closure",
    ] {
        let mut bad = rows.clone();
        bad[0][field] = Value::Null;
        ensure!(
            sources(&f).normal_pair(Some(&pair), &bad).is_err(),
            "retained actual field {field}"
        );
    }
    for field in [
        "source",
        "pid",
        "incarnation",
        "eof",
        "sequence",
        "root_generation",
        "last_commit",
        "source_point",
    ] {
        let mut bad = rows.clone();
        bad[0]["normal_closure"][field] = Value::Null;
        ensure!(
            sources(&f).normal_pair(Some(&pair), &bad).is_err(),
            "retained actual closure {field}"
        );
    }
    let mut later = rows.clone();
    later.push(json!({"owner":0}));
    ensure!(
        sources(&f).normal_pair(Some(&pair), &later).is_err(),
        "stale pair refused"
    );
    let mut wrong_nonce = pair.clone();
    wrong_nonce[0].key.nonce = [99; 32];
    ensure!(
        sources(&f).normal_pair(Some(&wrong_nonce), &rows).is_err(),
        "foreign nonce refused"
    );
    let mut missing = f;
    missing.witnesses[1] = None;
    ensure!(
        sources(&missing).normal_pair(Some(&pair), &rows).is_err(),
        "normal pair needs both migrations"
    );
    println!(
        "s54_verified_migration_normal=synthetic;all4_consumer_seams=true;genesis_swapped_stale_refused=true;process_wait_claim=false"
    );
    Ok(())
}
