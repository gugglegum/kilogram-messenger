//! P10A uses its own phase witness; earlier C2-L1 guards stay unchanged.
use super::*;
pub(super) struct Sample {
    pub raw: [Value; 2],
    pub rows: [[Value; 2]; 2],
}
pub(super) struct Baseline {
    pub entry: Sample,
    pub keys: [oracle::Incarnation; 2],
    pub counters: [oracle::Counts; 2],
    pub commits: [oracle::Counts; 2],
    pub controls: [[Value; 2]; 2],
    pub requests: old::Id,
}
pub(super) fn counts(s: &control::Controller) -> Result<[oracle::Counts; 2]> {
    Ok([
        s.running[0]
            .as_ref()
            .context("actual A stream")?
            .decoder
            .observed_counts()?,
        s.running[1]
            .as_ref()
            .context("actual B stream")?
            .decoder
            .observed_counts()?,
    ])
}
fn read_controls(s: &control::Controller) -> Result<[[Value; 2]; 2]> {
    let mut all = Vec::new();
    for c in &s.controls {
        let mut both = Vec::new();
        for (path, cached) in [(&c.od1, &c.old), (&c.s54, &c.current)] {
            let raw: Value = serde_json::from_slice(
                &kilogram_cli::ordered_dialog::admin::read_public(path, 16 * 1024)?,
            )?;
            ensure!(
                raw == *cached,
                "actual published control equals retained controller control"
            );
            both.push(raw);
        }
        all.push(
            both.try_into()
                .map_err(|_| anyhow::anyhow!("two actual control files"))?,
        );
    }
    all.try_into()
        .map_err(|_| anyhow::anyhow!("same two controlled owners"))
}
pub(super) fn entry(s: &mut control::Controller) -> Result<Baseline> {
    proofs::owned(s.phases.next, s.running.each_ref().map(Option::is_some))?;
    let (a, ar) = pressure::full(s, 0)?;
    let (b, br) = pressure::full(s, 1)?;
    let keys = [
        s.running[0]
            .as_ref()
            .context("actual A")?
            .decoder
            .incarnation()?,
        s.running[1]
            .as_ref()
            .context("actual B")?
            .decoder
            .incarnation()?,
    ];
    let controls = read_controls(s)?;
    proofs::held(&controls)?;
    let requests = proofs::request_digest(&s.requests)?;
    s.check()?;
    let commits = counts(s)?;
    ensure!(
        s.counters == [oracle::Counts::default(); 2] && commits == s.counters,
        "actual fresh P9 incarnations have zero readable and committed packet work"
    );
    let baseline = Baseline {
        entry: Sample {
            raw: [a, b],
            rows: [ar, br],
        },
        keys,
        counters: s.counters,
        commits,
        controls,
        requests,
    };
    for owner in 0..2 {
        let rows = &baseline.entry.rows[owner];
        proofs::c1(&rows[0], &rows[0], owner)?;
        proofs::c2(&rows[1], &rows[1], None, false)?;
        for (context, row) in rows.iter().enumerate() {
            let slot = s.view(owner, context)?.slot as usize;
            ensure!(
                proofs::admission(
                    row,
                    &baseline.entry.raw[owner]["routes"][slot],
                    s.running[owner]
                        .as_ref()
                        .context("same owner")?
                        .decoder
                        .current_head(context as u8),
                    context == 0
                )?,
                "stable actual P9 direct admission at entry"
            );
        }
    }
    Ok(baseline)
}
pub(super) fn sample(
    s: &mut control::Controller,
    base: &Baseline,
    consent: Option<old::Id>,
    upgrading: bool,
    selected: &mut proofs::Selection,
) -> Result<Sample> {
    s.check()?;
    proofs::controls(&read_controls(s)?, &base.controls)?;
    ensure!(
        proofs::request_digest(&s.requests)? == base.requests,
        "same original actual Text request map"
    );
    let mut raw = Vec::new();
    let mut all = Vec::new();
    for owner in 0..2 {
        let node = s.view(owner, 0)?;
        let status = node.status()?;
        let rows = [
            node.context(&status)?.clone(),
            s.view(owner, 1)?.context(&status)?.clone(),
        ];
        s.original[owner]
            .as_ref()
            .context("original two-context identity")?
            .identity(&s.origin.pair.nodes[owner], &rows)?;
        pressure::resources(&status, node.slot as usize, false)?;
        let run = s.running[owner]
            .as_ref()
            .context("same live owned process")?;
        ensure!(
            run.decoder.incarnation()? == base.keys[owner]
                && run.child.0.id() == base.keys[owner].pid,
            "no replacement process/source/nonce during C2 transition"
        );
        for (context, row) in rows.iter().enumerate() {
            let slot = s.view(owner, context)?.slot as usize;
            ensure!(
                row["context"] == s.origin.contexts[context]
                    && row["role"] == owner as u64
                    && row["slot"] == slot,
                "actual C/role/Ready physical slot mapping"
            );
            proofs::admission(
                row,
                &status["routes"][slot],
                run.decoder.current_head(context as u8),
                false,
            )?;
        }
        proofs::c1(&rows[0], &base.entry.rows[owner][0], owner)?;
        proofs::c2(&rows[1], &base.entry.rows[owner][1], consent, upgrading)?;
        if upgrading {
            selected.observe(
                &rows[1],
                consent.context("actual imported ceremony consent")?,
            )?;
        }
        let observed = observations::stats(&status["stats"])?;
        proofs::zero(observed, base.counters[owner])?;
        s.counters[owner] = observed;
        raw.push(status);
        all.push(rows);
    }
    s.check()?;
    for (owner, actual) in counts(s)?.into_iter().enumerate() {
        proofs::zero(actual, base.commits[owner])?;
    }
    Ok(Sample {
        raw: raw.try_into().map_err(|_| anyhow::anyhow!("actual pair"))?,
        rows: all
            .try_into()
            .map_err(|_| anyhow::anyhow!("actual pair rows"))?,
    })
}
pub(super) fn ready(
    s: &control::Controller,
    facts: &Sample,
    consent: old::Id,
    selected: &proofs::Selection,
) -> Result<bool> {
    let mut ready = true;
    for (owner, rows) in facts.rows.iter().enumerate() {
        ready &= selected.active(&rows[1], consent)?;
        for (context, row) in rows.iter().enumerate() {
            let slot = s.view(owner, context)?.slot as usize;
            ready &= proofs::admission(
                row,
                &facts.raw[owner]["routes"][slot],
                s.running[owner]
                    .as_ref()
                    .context("same owner")?
                    .decoder
                    .current_head(context as u8),
                true,
            )?;
        }
    }
    Ok(ready)
}
pub(super) fn histories(
    s: &mut control::Controller,
    base: &Baseline,
    consent: Option<old::Id>,
    upgrading: bool,
    selected: &mut proofs::Selection,
    expected: &[[BTreeMap<String, Value>; 2]; 2],
) -> Result<()> {
    idle(s, base, consent, upgrading, selected)?;
    for (owner, contexts) in expected.iter().enumerate() {
        for (context, map) in contexts.iter().enumerate() {
            idle(s, base, consent, upgrading, selected)?;
            let frontier = if context == 1 {
                [33, 33]
            } else if owner == 0 {
                [641, 640]
            } else {
                [641, 641]
            };
            proofs::history(&early::history(&s.view(owner, context)?)?, map, frontier)?;
        }
    }
    idle(s, base, consent, upgrading, selected)?;
    s.frozen
        .as_ref()
        .context("same original frozen23")?
        .verify(&s.view(0, 0)?)?;
    sample(s, base, consent, upgrading, selected)?;
    Ok(())
}
fn idle(
    s: &mut control::Controller,
    base: &Baseline,
    consent: Option<old::Id>,
    upgrading: bool,
    selected: &mut proofs::Selection,
) -> Result<()> {
    s.wait(30, |s| {
        let facts = sample(s, base, consent, upgrading, selected)?;
        Ok((0..2)
            .all(|owner| {
                (0..2).all(|context| {
                    s.view(owner, context)
                        .ok()
                        .is_some_and(|n| facts.raw[owner]["window"][1][n.slot as usize] == 0)
                })
            })
            .then_some(()))
    })?;
    Ok(())
}
