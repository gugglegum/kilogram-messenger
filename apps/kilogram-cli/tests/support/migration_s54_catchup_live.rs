//! Bounded observations and existing owned lifecycle operations; no test entry point.
use super::*;

pub(super) struct Live {
    pub raw: [Value; 2],
    pub rows: [[Value; 2]; 2],
    pub keys: [oracle::Incarnation; 2],
}
pub(super) fn live(s: &mut control::Controller) -> Result<Live> {
    s.wait(30, |s| {
        let rows = [
            [s.status(0, 0)?, s.status(0, 1)?],
            [s.status(1, 0)?, s.status(1, 1)?],
        ];
        if !rows.iter().all(|r| {
            r[0]["mode"] == "Active"
                && r[0]["lease_current"] == true
                && r[0]["lease_family"] == 2
                && r[0]["policy_error"].is_null()
                && r[0]["handoff_policy_error"].is_null()
                && r[1]["mode"] == "L1"
                && r[1]["lease_current"] == true
                && r[1]["lease_family"] == 1
                && r[1]["policy_error"].is_null()
        }) {
            return Ok(None);
        }
        for (owner, row) in rows.iter().enumerate() {
            let run = s.running[owner].as_ref().context("owned live process")?;
            if !run
                .decoder
                .current_head(0)
                .is_some_and(|(_, g, _)| row[0]["lease_generation"] == g)
            {
                return Ok(None);
            }
        }
        let (a, ar) = pressure::full(s, 0)?;
        let (b, br) = pressure::full(s, 1)?;
        pressure::resources(&a, s.view(0, 0)?.slot as usize, false)?;
        pressure::resources(&b, s.view(1, 0)?.slot as usize, false)?;
        transition::lease_rows(&[ar[0].clone(), br[0].clone()], ["Active", "Active"], None)?;
        let keys = [
            s.running[0].as_ref().context("A")?.decoder.incarnation()?,
            s.running[1].as_ref().context("B")?.decoder.incarnation()?,
        ];
        Ok(Some(Live {
            raw: [a, b],
            rows: [ar, br],
            keys,
        }))
    })
}
pub(super) fn reopen(
    s: &mut control::Controller,
    owners: &[usize],
    work: &mut proofs::Work,
) -> Result<Live> {
    for owner in owners {
        s.start(*owner)?;
    }
    s.import2()?;
    s.import_legacy(1)?;
    let facts = live(s)?;
    for owner in owners {
        work.reopen(*owner, s.counters[*owner])?;
    }
    Ok(facts)
}
pub(super) fn idle(s: &mut control::Controller) -> Result<Live> {
    s.wait(30, |s| {
        let facts = live(s)?;
        for owner in 0..2 {
            for context in 0..2 {
                let slot = s.view(owner, context)?.slot as usize;
                ensure!(
                    facts.raw[owner]["window"][0][slot] == 64,
                    "same real selected64 share"
                );
                if facts.raw[owner]["window"][1][slot] != 0 {
                    return Ok(None);
                }
            }
        }
        Ok(Some(facts))
    })
}
pub(super) fn prefix(s: &mut control::Controller, suspended: bool) -> Result<()> {
    s.controls[1].old["suspend_prefix_requests"] = suspended.into();
    s.controls[1].current["suspend_prefix"] = suspended.into();
    s.controls[1].write()
}
fn remove_marker(s: &control::Controller, tag: &str) -> Result<()> {
    let path = s.controls[1].s54.with_extension(format!("s54-{tag}.json"));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
fn marker(s: &control::Controller, tag: &str) -> Result<Option<Value>> {
    let path = s.controls[1].s54.with_extension(format!("s54-{tag}.json"));
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(
        &kilogram_cli::ordered_dialog::admin::read_public(&path, 4096)?,
    )?))
}
pub(super) fn page(
    s: &mut control::Controller,
    base: oracle::PageBaseline,
    target: u64,
) -> Result<observations::PageObservation> {
    ensure!(
        s.controls[1].old["disable_ack"] == true && s.controls[1].current["suspend_prefix"] == true,
        "arm page while competing ACK/prefix suppressed"
    );
    s.check()?;
    let key = s.running[1].as_ref().context("B")?.decoder.incarnation()?;
    let local = s.pages.phase_local_target(key, base, target)?;
    remove_marker(s, "page")?;
    s.controls[1].current["pause_after_normal_page"] = local.into();
    s.controls[1].write()?;
    prefix(s, false)?;
    let page = s.wait(60, |s| {
        let current = s.running[1].as_ref().context("same B")?;
        ensure!(
            current.decoder.incarnation()? == key,
            "same page arm process/source/nonce"
        );
        let progress = s.pages.phase_total(key, base)?;
        ensure!(progress <= target, "actual phase target overshoot");
        let Some(page) = current.decoder.paused_page().cloned() else {
            return Ok(None);
        };
        let Some(marker) = marker(s, "page")? else {
            return Ok(None);
        };
        current.decoder.page_marker(&marker)?;
        proofs::barrier(&page, key, local, progress, target)?;
        Ok(Some(page))
    })?;
    // Pause is after Owner reply, before next prefix. Owner may still drain the
    // accepted pending page in batches <=4, so the marker alone is insufficient.
    s.wait(30, |s| {
        let row = s.status(1, 0)?;
        Ok(proofs::progress(&row, page.end)?.then_some(()))
    })?;
    prefix(s, true)?; // retain the active page barrier through the waited kill
    let facts = idle(s)?;
    ensure!(
        facts.rows[0][0]["next"] == json!([641, 640])
            && facts.rows[0][0]["counts"]["debt"] == 256
            && facts.rows[0][0]["counts"]["queued"] == 0
            && facts.rows[0][0]["counts"]["pending"] == 0
            && facts.rows[0][0]["head_pending"].is_null(),
        "actual A offline debt/frontier preserved at page stop"
    );
    proofs::progress(&facts.rows[1][0], page.end)?
        .then_some(())
        .context("quiescent accepted page end")?;
    let current = s.running[1].as_ref().context("same paused B")?;
    current
        .decoder
        .page_marker(&marker(s, "page")?.context("current page marker")?)?;
    s.remember(&format!("p9-page-{target}"),json!({"pid":key.pid,"source":state::hex(&key.owner),"incarnation":state::hex(&key.nonce),"phase_pages":s.pages.phase_total(key,base)?,"page":page.facts(),"marker":marker(s,"page")?,"drained":facts.rows[1][0],"stats":facts.raw[1]["stats"]}))?;
    Ok(page)
}
pub(super) fn clear_page(s: &mut control::Controller) -> Result<()> {
    s.controls[1].current["pause_after_normal_page"] = Value::Null;
    s.controls[1].write()?;
    remove_marker(s, "page")
}
pub(super) fn arm_ack(s: &mut control::Controller) -> Result<()> {
    ensure!(
        s.controls.iter().all(|c| c.old["disable_ack"] == true),
        "ACK remains disabled during fresh admission and decoder arm"
    );
    let facts = live(s)?;
    let decoder = &mut s.running[1].as_mut().context("same B decoder")?.decoder;
    let (binding, generation, _) = decoder
        .current_head(0)
        .context("actual accepted current Head")?;
    ensure!(
        facts.rows[1][0]["lease_generation"] == generation,
        "arm current live B lease"
    );
    decoder.arm_ack(0, binding, generation, Some(385))?;
    remove_marker(s, "ack")?;
    s.controls[1].current["hold_ack"] = true.into();
    s.controls[1].current["hold_ack_from"] = 385.into();
    // Controls::write publishes OD1 before S54; use TWO publications so no ACK
    // becomes enabled before the targeted S54 hold is present on disk.
    s.controls[1].write()?;
    s.controls[1].old["disable_ack"] = false.into();
    s.controls[1].write()
}
pub(super) fn ack(
    s: &mut control::Controller,
    map: &BTreeMap<String, Value>,
    ids: &[(u64, old::Id)],
) -> Result<observations::AckObservation> {
    s.wait(30, |s| {
        let decoder = &s.running[1].as_ref().context("B")?.decoder;
        let Some(row) = decoder.paused_ack().cloned() else {
            return Ok(None);
        };
        let Some(marker) = marker(s, "ack")? else {
            return Ok(None);
        };
        decoder.ack_marker(&marker)?;
        proofs::ack_sources(&row, map, ids)?;
        let a = s.status(0, 0)?;
        ensure!(
            a["counts"]["debt"] == 256,
            "actual offline debt remains outstanding while push is held"
        );
        let (b, rows) = pressure::full(s, 1)?;
        ensure!(
            b["window"][1][s.view(1, 0)?.slot as usize] == 32
                && rows[0]["counts"]["debt"] == 1
                && rows[0]["next"] == json!([641, 641]),
            "actual held saved-ACK reserve32 and original B640"
        );
        Ok(Some(row))
    })
}
pub(super) fn histories(
    s: &mut control::Controller,
    ids: &[(u64, old::Id)],
    held: &Value,
    c2: &[BTreeMap<String, Value>; 2],
) -> Result<[BTreeMap<String, Value>; 2]> {
    idle(s)?;
    let mut maps = Vec::new();
    for (owner, expected_c2) in c2.iter().enumerate() {
        maps.push(proofs::ordered_history(
            &early::history(&s.view(owner, 0)?)?,
            owner,
            ids,
            held,
        )?);
        ensure!(
            source::history_map(&early::history(&s.view(owner, 1)?)?)? == *expected_c2,
            "unchanged exact64 C2 Text history"
        );
    }
    let maps: [BTreeMap<String, Value>; 2] = maps
        .try_into()
        .map_err(|_| anyhow::anyhow!("actual pair"))?;
    proofs::history_pair(&maps[0], &maps[1], held)?;
    s.frozen
        .as_ref()
        .context("same original frozen23 cursor")?
        .verify(&s.view(0, 0)?)?;
    idle(s)?;
    Ok(maps)
}
