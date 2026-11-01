//! Same-controller relay phase observations; former DirectOnly/L1 guards untouched.
use super::*;
pub(super) struct Baseline {
    pub rows: [[Value; 2]; 2],
    pub controls: [[Value; 2]; 2],
    pub requests: old::Id,
}
pub(super) struct Sample {
    pub raw: [Value; 2],
    pub rows: [[Value; 2]; 2],
}
pub(super) fn controls(s: &control::Controller) -> Result<[[Value; 2]; 2]> {
    let mut out = Vec::new();
    for c in &s.controls {
        let mut both = Vec::new();
        for (path, cached) in [(&c.od1, &c.old), (&c.s54, &c.current)] {
            let value: Value = serde_json::from_slice(
                &kilogram_cli::ordered_dialog::admin::read_public(path, 16384)?,
            )?;
            ensure!(value == *cached, "published controls equal retained values");
            both.push(value);
        }
        out.push(
            both.try_into()
                .map_err(|_| anyhow::anyhow!("actual two files"))?,
        );
    }
    out.try_into()
        .map_err(|_| anyhow::anyhow!("actual two owners"))
}
fn requests(s: &control::Controller) -> Result<old::Id> {
    ensure!(s.requests.len() == 1344, "same original Text request map");
    Ok(*blake3::hash(&serde_json::to_vec(
        &s.requests
            .iter()
            .map(|(r, v)| (r.numeric(), v))
            .collect::<Vec<_>>(),
    )?)
    .as_bytes())
}
pub(super) fn baseline(
    s: &mut control::Controller,
    before: &Value,
    selection: &Value,
    consent: old::Id,
) -> Result<Baseline> {
    ensure!(
        s.phases.next == 10
            && !s.phases.complete()
            && s.running.iter().all(Option::is_some)
            && s.relay.is_none(),
        "same owned P10A input, no relay yet"
    );
    let mut rows = Vec::new();
    for owner in 0..2 {
        let pair: [Value; 2] = serde_json::from_value(before["status"][owner].clone())?;
        ensure!(
            pair[1]["current_consent2"] == state::hex(&consent)
                && pair[1]["q"] == selection["q"]
                && pair[1]["cuts"] == json!([33, 33])
                && ["p", "a", "k", "committed"]
                    .iter()
                    .enumerate()
                    .all(|(i, k)| !selection["controls"][i].is_null()
                        && pair[1]["controls"][k] == selection["controls"][i]),
            "actual original P10A ceremony/selected controls"
        );
        rows.push(pair);
    }
    let controls = controls(s)?;
    proofs::held(&controls)?;
    Ok(Baseline {
        rows: rows
            .try_into()
            .map_err(|_| anyhow::anyhow!("actual original rows"))?,
        controls,
        requests: requests(s)?,
    })
}
pub(super) fn keys(s: &mut control::Controller) -> Result<[oracle::Incarnation; 2]> {
    s.check()?;
    Ok([
        s.running[0]
            .as_ref()
            .context("same A")?
            .decoder
            .incarnation()?,
        s.running[1]
            .as_ref()
            .context("same B")?
            .decoder
            .incarnation()?,
    ])
}
pub(super) fn sample(
    s: &mut control::Controller,
    base: &Baseline,
    keys: [oracle::Incarnation; 2],
    stage: proofs::Stage,
    relay: bool,
    held: old::Id,
) -> Result<Sample> {
    s.check()?;
    ensure!(
        requests(s)? == base.requests,
        "same exact original Text requests"
    );
    proofs::controls(&controls(s)?, &base.controls, stage)?;
    let mut raw = Vec::new();
    let mut all = Vec::new();
    for (owner, key) in keys.iter().enumerate() {
        let node = s.view(owner, 0)?;
        let status = node.status()?;
        let rows = [
            node.context(&status)?.clone(),
            s.view(owner, 1)?.context(&status)?.clone(),
        ];
        s.original[owner]
            .as_ref()
            .context("original identity")?
            .identity(&s.origin.pair.nodes[owner], &rows)?;
        pressure::resources(&status, node.slot as usize, false)?;
        let run = s.running[owner].as_ref().context("same owned runtime")?;
        ensure!(
            run.child.0.id() == key.pid && run.decoder.incarnation()? == *key,
            "same current PID/source/nonce"
        );
        for (context, row) in rows.iter().enumerate() {
            let slot = s.view(owner, context)?.slot as usize;
            ensure!(
                row["context"] == s.origin.contexts[context]
                    && row["role"] == owner as u64
                    && row["slot"] == slot
                    && status["routes"][slot]["slot"] == slot
                    && status["routes"][slot]["policy"]
                        == if relay { "relay-only" } else { "direct-only" },
                "actual logical/physical/current configured route binding"
            );
            ensure!(
                status["routes"][slot]["selected_path"].is_null()
                    || status["routes"][slot]["selected_path"]
                        == if relay { "relay" } else { "direct" },
                "no contradictory selected path during admission"
            );
            proofs::same(row, &base.rows[owner][context], owner, context, stage)?;
        }
        let observed = observations::stats(&status["stats"])?;
        proofs::work(observed, owner, stage, false)?;
        proofs::work(run.decoder.observed_counts()?, owner, stage, false)?;
        proofs::packet(run.decoder.packet()?, owner, false)?;
        for p in run.decoder.pending_rows.values() {
            ensure!(
                owner == 0
                    && matches!(stage, proofs::Stage::Released | proofs::Stage::Final)
                    && p.context == 0
                    && p.role == 1
                    && p.ordinal == 640
                    && p.event == held,
                "only actual original B640 pending in fresh relay incarnation"
            );
        }
        s.counters[owner] = observed;
        raw.push(status);
        all.push(rows);
    }
    s.check()?;
    for owner in 0..2 {
        let decoder = &s.running[owner].as_ref().context("same runtime")?.decoder;
        proofs::work(decoder.observed_counts()?, owner, stage, false)?;
        proofs::packet(decoder.packet()?, owner, false)?;
    }
    Ok(Sample {
        raw: raw.try_into().map_err(|_| anyhow::anyhow!("actual pair"))?,
        rows: all
            .try_into()
            .map_err(|_| anyhow::anyhow!("actual context rows"))?,
    })
}
pub(super) fn ready(
    s: &control::Controller,
    facts: &Sample,
    pubs: &locators::Publications,
    relay: bool,
) -> Result<bool> {
    let mut ready = true;
    for (owner, rows) in facts.rows.iter().enumerate() {
        for (context, row) in rows.iter().enumerate() {
            let slot = s.view(owner, context)?.slot as usize;
            ready &= locators::admitted(
                row,
                &facts.raw[owner]["routes"][slot],
                &pubs.rows[owner][context],
                &pubs.rows[1 - owner][context],
                s.running[owner]
                    .as_ref()
                    .context("actual owner")?
                    .decoder
                    .current_head(context as u8),
                relay,
            )?;
        }
    }
    Ok(ready)
}
pub(super) fn wait_relay(
    s: &mut control::Controller,
    base: &Baseline,
    keys: [oracle::Incarnation; 2],
    stage: proofs::Stage,
    pubs: &locators::Publications,
    event: old::Id,
) -> Result<Sample> {
    s.wait(45, |s| {
        let facts = sample(s, base, keys, stage, true, event)?;
        Ok(ready(s, &facts, pubs, true)?.then_some(facts))
    })
}
pub(super) fn apply(
    s: &mut control::Controller,
    base: &Baseline,
    stage: proofs::Stage,
) -> Result<()> {
    let desired = proofs::schedules(&base.controls, stage);
    for (owner, c) in desired.into_iter().enumerate() {
        s.check()?;
        s.controls[owner].old = c[0].clone();
        s.controls[owner].current = c[1].clone();
        s.controls[owner].write()?;
        s.check()?;
    }
    proofs::controls(&controls(s)?, &base.controls, stage)
}
pub(super) fn request(s: &mut control::Controller, expected: old::Id) -> Result<Value> {
    s.check()?;
    let node = s.view(1, 0)?;
    let request = oracle::Request::new(0, 1, 640)?;
    let row = json_cli(
        &current(),
        &[
            "ordered".into(),
            "request-status3".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--slot".into(),
            node.slot.to_string(),
            "--request-id".into(),
            source::request_id(request),
        ],
    )?;
    proofs::request(&row, expected)?;
    s.check()?;
    Ok(row)
}
pub(super) fn idle(
    s: &mut control::Controller,
    base: &Baseline,
    keys: [oracle::Incarnation; 2],
    stage: proofs::Stage,
    relay: bool,
    held: old::Id,
) -> Result<Sample> {
    s.wait(30, |s| {
        let facts = sample(s, base, keys, stage, relay, held)?;
        let mut idle = true;
        for owner in 0..2 {
            for context in 0..2 {
                idle &= facts.raw[owner]["window"][1][s.view(owner, context)?.slot as usize] == 0;
            }
        }
        Ok(idle.then_some(facts))
    })
}
pub(super) fn histories(
    s: &mut control::Controller,
    base: &Baseline,
    keys: [oracle::Incarnation; 2],
    stage: proofs::Stage,
    held: old::Id,
    expected: &[[BTreeMap<String, Value>; 2]; 2],
) -> Result<Value> {
    let mut observed = Vec::new();
    for (owner, contexts) in expected.iter().enumerate() {
        for (context, map) in contexts.iter().enumerate() {
            idle(
                s,
                base,
                keys,
                stage,
                stage != proofs::Stage::Held || s.relay.is_some(),
                held,
            )?;
            let rows = early::history(&s.view(owner, context)?)?;
            let frontier = if context == 1 {
                [33, 33]
            } else if owner == 0 && stage == proofs::Stage::Held {
                [641, 640]
            } else {
                [641, 641]
            };
            proofs::history(&rows, map, frontier)?;
            observed.push(json!({"owner":owner,"context":context,"rows":rows.len(),"digest":blake3::hash(&serde_json::to_vec(&source::history_map(&rows)?)?).to_hex().to_string()}));
        }
    }
    idle(s, base, keys, stage, s.relay.is_some(), held)?;
    s.frozen
        .as_ref()
        .context("original frozen23")?
        .verify(&s.view(0, 0)?)?;
    sample(s, base, keys, stage, s.relay.is_some(), held)?;
    Ok(json!(observed))
}
