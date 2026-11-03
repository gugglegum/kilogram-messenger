//! Real F storage provenance, injected ledger wiring, and a labelled adapter.
use super::*;
use cold_witness_fixture::Fresh;
use control::MigrationSources;
use memory_scope::tests::DeferredModel;

struct Verified {
    fresh: Fresh,
    genesis: [Legacy; 2],
    witnesses: [Option<Legacy>; 2],
    endpoints: [Option<intervals::Endpoint>; 2],
    rows: BTreeMap<String, Value>,
}
impl Verified {
    fn f(slot: u8) -> Result<Self> {
        let (mut fresh, genesis) = Fresh::f_with_genesis(slot)?;
        let witnesses = [fresh.capture(0)?, fresh.capture(1)?];
        for owner in 0..2 {
            genesis[owner].same_identity(&witnesses[owner])?;
            ensure!(
                genesis[owner].source_id != witnesses[owner].source_id,
                "genuine genesis to traffic source growth"
            );
        }
        fresh.migrate()?;
        let mut endpoints = Vec::new();
        let mut rows = BTreeMap::new();
        for (owner, w) in witnesses.iter().enumerate() {
            let endpoint = w.after_migration(&fresh.nodes[owner], &fresh.ready[owner])?;
            ensure!(
                endpoint.lanes[0].role == owner as u8
                    && endpoint.lanes[0].slot == slot
                    && endpoint.lanes[0].next == [3, 3]
                    && endpoint.lanes[1].next == [1, 1],
                "actual F committed mapping"
            );
            let mut repo = fresh.open(owner)?;
            ensure!(repo.snapshot(slot).is_err(), "metadata remains cold");
            ensure!(
                repo.queue_page(slot, None, 32)?.len() == if owner == 0 { 2 } else { 0 },
                "real F queue excludes committed frontier"
            );
            drop(repo);
            rows.insert(format!("ready-{owner}"), fresh.ready[owner].clone());
            endpoints.push(Some(endpoint));
        }
        let value = Self {
            fresh,
            genesis,
            witnesses: witnesses.map(Some),
            endpoints: endpoints
                .try_into()
                .map_err(|_| anyhow::anyhow!("two verified owners"))?,
            rows,
        };
        for owner in 0..2 {
            ensure!(
                value.sources().source(owner)?
                    == value.witnesses[owner]
                        .as_ref()
                        .context("witness")?
                        .source_id,
                "AN checked actual source"
            );
        }
        Ok(value)
    }
    fn sources(&self) -> MigrationSources<'_> {
        MigrationSources::new(
            &self.fresh.contexts,
            &self.witnesses,
            &self.rows,
            &self.endpoints,
        )
    }
}
#[test]
fn ao_real_f_growth_deferred_backfill_roles_and_slots() -> Result<()> {
    for slot in [0, 1] {
        let data = Verified::f(slot)?;
        let model = DeferredModel::enter()?;
        model.legacy(0, 100, true)?;
        model.legacy(1, 101, true)?;
        ensure!(
            model.source(0)?.is_none() && model.source(1)?.is_none() && model.bound(0)? == 0,
            "unbound legacy records"
        );
        // Register only A's migration facts first; B cannot supply a fallback.
        let witnesses = [data.witnesses[0].clone(), None];
        let endpoints = [data.endpoints[0].clone(), None];
        let mut rows = data.rows.clone();
        rows.remove("ready-1");
        let a = MigrationSources::new(&data.fresh.contexts, &witnesses, &rows, &endpoints);
        a.bind_memory(0)?;
        let source = a.source(0)?;
        ensure!(
            model.source(0)? == Some(source) && model.bound(0)? == 1 && model.source(1)?.is_none(),
            "actual shared deferred A binding"
        );
        let current = model.current(0, 200, source, [1; 32])?;
        model.stats(current, source)?;
        model.close(current)?;
        let next = model.current(0, 201, source, [2; 32])?;
        model.stats(next, source)?;
        model.close(next)?;
        let extra = model.legacy(1, 102, true)?;
        ensure!(
            extra > 0 && model.bound(1)? == 0,
            "B later legacy registration still unbound"
        );
        data.sources().bind_memory(1)?;
        ensure!(
            model.bound(1)? == 2 && model.source(1)? == Some(data.sources().source(1)?),
            "all original B lives late backfilled"
        );
        model.legacy(1, 103, true)?;
        ensure!(
            model.bound(1)? == 3,
            "later component registration uses bound source"
        );
        ensure!(!model.failed()?, "positive observer healthy");
        println!(
            "ao_real_F=passed;C1slot={slot};roles=2;source_growth=real;frontiers=3_3_and1_1;Aqueue3_4=true;ledger_observations=injected;mixed_A_bound_B_unbound=true"
        );
    }
    Ok(())
}
#[test]
fn ao_real_migration_binding_refuses_genesis_missing_foreign_and_nonce_reuse() -> Result<()> {
    let data = Verified::f(1)?;
    let a = data.sources().source(0)?;
    let b = data.sources().source(1)?;
    let mut refused = 0;
    for case in 0..7 {
        let model = DeferredModel::enter()?;
        model.legacy(0, 100, true)?;
        model.legacy(1, 101, true)?;
        let result = match case {
            0 => {
                let missing = [None, None];
                MigrationSources::new(&data.fresh.contexts, &missing, &data.rows, &data.endpoints)
                    .bind_memory(0)
            }
            1 => {
                let mut wrong = data.endpoints.clone();
                wrong[0].as_mut().context("endpoint")?.source = data.genesis[0].source_id;
                MigrationSources::new(&data.fresh.contexts, &data.witnesses, &data.rows, &wrong)
                    .bind_memory(0)
            }
            2 => {
                memory_scope::bind_legacy(0, data.genesis[0].source_id)?;
                data.sources().bind_memory(0)
            }
            3 => {
                data.sources().bind_memory(0)?;
                data.sources().bind_memory(1)?;
                model.current(0, 200, b, [1; 32]).map(|_| ())
            }
            4 => {
                data.sources().bind_memory(0)?;
                let current = model.current(0, 200, a, [1; 32])?;
                model.stats(current, data.genesis[0].source_id)
            }
            5 => {
                data.sources().bind_memory(0)?;
                let current = model.current(0, 200, a, [1; 32])?;
                model.stats(current, a)?;
                model.close(current)?;
                model.current(0, 201, a, [1; 32]).map(|_| ())
            }
            _ => data.sources().bind_memory(2),
        };
        ensure!(result.is_err(), "independent deferred negative {case}");
        if case == 0 || case == 1 || case == 6 {
            ensure!(
                model.source(0)?.is_none() && !model.failed()?,
                "lookup refuses before observer mutation"
            );
        } else {
            ensure!(
                model.failed()?,
                "poisoned negative is never continued as success"
            );
        }
        refused += 1;
    }
    println!(
        "ao_deferred_real_source_negatives={refused};independent_observers=true;old_genesis_binding_refused=true;Stats_and_nonce_guard_preserved=true"
    );
    Ok(())
}
#[cfg(windows)]
#[test]
fn ao_actual_child_query_deferred_binding_and_new_nonce_restart() -> Result<()> {
    let data = Verified::f(0)?;
    let ids = data.sources().pair()?;
    memory_scope::tests::actual_deferred_adapter(
        data.fresh._dir.path(),
        data.fresh
            .nodes
            .each_ref()
            .map(|n| n.branch.state.as_path()),
        data.fresh.nodes.each_ref().map(|n| n.branch.base.as_path()),
        ids,
        |owner| data.sources().bind_memory(owner),
    )
}
