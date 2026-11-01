//! Inactive P10A: same P9 ownership, C2 conversion only, no relay or B640 release.
use super::*;
#[path = "migration_s54_handoff_live.rs"]
mod live;
#[path = "migration_s54_handoff_proofs.rs"]
mod proofs;
#[cfg(test)]
#[path = "migration_s54_handoff_tests.rs"]
mod tests;

pub(super) struct Continuation {
    pub previous: catchup::Continuation,
    pub baseline: [oracle::Counts; 2],
    pub observed: [oracle::Counts; 2],
    pub packet_baseline: [oracle::Counts; 2],
    pub packet_observed: [oracle::Counts; 2],
    pub incarnations: [oracle::Incarnation; 2],
    pub consent: old::Id,
    pub intents: [[u8; 16]; 2],
    pub selection: Value,
    pub before: Value,
    pub after: Value,
}
pub(super) fn p10a(mut previous: catchup::Continuation) -> Result<Continuation> {
    let c2 = previous.previous.c2_history.clone();
    let histories = [
        [previous.histories[0].clone(), c2[0].clone()],
        [previous.histories[1].clone(), c2[1].clone()],
    ];
    let s = &mut previous.previous.previous.controller;
    let _phase = s53_deadline::Phase::until(s.until)?;
    let base = live::entry(s)?;
    let mut selected = proofs::Selection::default();
    live::histories(s, &base, None, false, &mut selected, &histories)?;
    let text_ids = s
        .requests
        .keys()
        .map(|r| source::parse_request_id(&source::request_id(*r)))
        .collect::<Result<Vec<_>>>()?;
    let intents = proofs::intents(
        s.rows
            .get("c1-upgrade-requests")
            .context("actual original issued C1 upgrade IDs")?,
        &text_ids,
    )?;
    let context = s
        .origin
        .pair
        .borrowed_for([s.view(0, 1)?, s.view(1, 1)?], &s.origin.contexts[1])?;
    // Context contains only metadata copied from original views/roots/base, so
    // every subprocess can check the SAME controller without borrowing handles.
    let consent = context.ceremony("p10-c2", "absent", 30 * 86400, |candidate| {
        live::sample(s, &base, candidate, false, &mut selected).map(|_| ())
    })?;
    s.wait(15, |s| {
        let facts = live::sample(s, &base, Some(consent), false, &mut selected)?;
        ensure!(
            facts
                .rows
                .iter()
                .all(|r| r[1]["current_consent2"] == state::hex(&consent)),
            "both actual ceremony imports acknowledged before C2 intent"
        );
        Ok(s.origin
            .pair
            .nodes
            .iter()
            .all(|n| {
                n.locators
                    .join(format!("{}.locator2", s.origin.contexts[1]))
                    .is_file()
            })
            .then_some(()))
    })?;
    s.import2_for(1)?;
    live::sample(s, &base, Some(consent), false, &mut selected)?;
    let mut replies = Vec::new();
    for (index, owner) in [1, 0].into_iter().enumerate() {
        let reply = context.upgrade(owner, intents[index], || {
            live::sample(s, &base, Some(consent), true, &mut selected).map(|_| ())
        })?;
        replies.push(
            json!({"owner":owner,"request":state::hex(&intents[index]),"actual_reply":reply}),
        );
    }
    let stable = s.wait(60, |s| {
        let facts = live::sample(s, &base, Some(consent), true, &mut selected)?;
        Ok(live::ready(s, &facts, consent, &selected)?.then_some(facts))
    })?;
    live::histories(s, &base, Some(consent), true, &mut selected, &histories)?;
    let after = live::sample(s, &base, Some(consent), true, &mut selected)?;
    ensure!(
        live::ready(s, &after, consent, &selected)? && s.phases.next == 10 && !s.phases.complete(),
        "both original contexts Active/direct; same incomplete10/12, no P10 completion yet"
    );
    let observed = s.counters;
    let packet_observed = live::counts(s)?;
    let selection = selected.facts();
    let before = json!({"status":base.entry.rows,"stats":base.entry.raw.each_ref().map(|r|r["stats"].clone()),"requests_digest":state::hex(&base.requests)});
    let after = json!({"stable":stable.rows,"status":after.rows,"stats":after.raw.each_ref().map(|r|r["stats"].clone()),"consent":state::hex(&consent),"intents":replies,"selection":selection,"classification":"live observed zero delta; incarnations not closed/reconciled"});
    s.remember("p10a", json!({"before":before,"after":after}))?;
    Ok(Continuation {
        previous,
        baseline: base.counters,
        observed,
        packet_baseline: base.commits,
        packet_observed,
        incarnations: base.keys,
        consent,
        intents,
        selection,
        before,
        after,
    })
}
