//! Dormant consuming P10B. One relay, original B640, same owned incomplete11/12.
use super::*;
#[path = "migration_s54_relay_live.rs"]
mod live;
#[cfg(test)]
#[path = "migration_s54_relay_locator_tests.rs"]
mod locator_tests;
#[path = "migration_s54_relay_locators.rs"]
mod locators;
#[path = "migration_s54_relay_proofs.rs"]
mod proofs;
#[cfg(test)]
#[path = "migration_s54_relay_tests.rs"]
mod tests;

pub(super) struct Continuation {
    pub previous: handoff::Continuation,
    pub direct: Value,
    pub relay: Value,
    pub closed: Value,
    pub delivery: Value,
    pub histories: Value,
    pub work: Value,
    pub incarnations: [oracle::Incarnation; 2],
    pub observed: [oracle::Counts; 2],
    pub packets: [Option<observations::PacketObservation>; 2],
    pub relay_pid: u32,
}
fn profiles(s: &mut control::Controller, url: &str) -> Result<Value> {
    s.stopped_reconciled()?;
    s.check()?;
    let mut facts = Vec::new();
    for owner in 0..2 {
        s.stopped_reconciled()?;
        let node = s.view(owner, 0)?;
        let before: Value = serde_json::from_slice(
            &kilogram_cli::ordered_dialog::admin::read_public(&node.profile, 64 * 1024)?,
        )?;
        super::super::route(&node, "relay-only", Some(url))?;
        let actual: Value = serde_json::from_slice(
            &kilogram_cli::ordered_dialog::admin::read_public(&node.profile, 64 * 1024)?,
        )?;
        proofs::profile(&before, &actual, url)?;
        ensure!(
            Profile3::load(&node.profile)?
                .route_config()?
                .effective()?
                .as_str()
                == "relay-only",
            "actual effective relay-only profile"
        );
        facts.push(json!({"owner":owner,"profile":node.profile,"before_blake3":blake3::hash(&serde_json::to_vec(&before)?).to_hex().to_string(),"after_blake3":blake3::hash(&serde_json::to_vec(&actual)?).to_hex().to_string(),"route":actual["route"]}));
        s.check()?;
    }
    Ok(json!(facts))
}
pub(super) fn p10b(mut previous: handoff::Continuation) -> Result<Continuation> {
    let held = previous.previous.previous.previous.held_b640.clone();
    let event = state::parse_id(held["id"].as_str().context("original B640 ID")?)?;
    let c2 = previous.previous.previous.c2_history.clone();
    let before_maps = [
        [previous.previous.histories[0].clone(), c2[0].clone()],
        [previous.previous.histories[1].clone(), c2[1].clone()],
    ];
    let mut final_maps = before_maps.clone();
    let original = final_maps[1][0]
        .get(&state::hex(&event))
        .context("original B640 public source")?
        .clone();
    ensure!(
        original["request"] == held["request"]
            && original["ordinal"] == 640
            && original["role"] == 1
            && original["format"] == 2
            && final_maps[0][0]
                .insert(state::hex(&event), original)
                .is_none()
            && final_maps[0][0] == final_maps[1][0],
        "A gains only original B640, B/C2 maps unchanged"
    );
    let s = &mut previous.previous.previous.previous.controller;
    let _phase = s53_deadline::Phase::until(s.until)?;
    let base = live::baseline(s, &previous.after, &previous.selection, previous.consent)?;
    ensure!(
        state::hex(&base.requests) == previous.before["requests_digest"],
        "same actual P10A Text request/reply associations"
    );
    let old_keys = live::keys(s)?;
    ensure!(
        old_keys == previous.incarnations,
        "same P10A live incarnations"
    );
    for owner in 0..2 {
        ensure!(
            previous.observed[owner] == previous.baseline[owner]
                && previous.packet_observed[owner] == previous.packet_baseline[owner],
            "retained actual P10A zero-delta observations"
        );
    }
    let direct = locators::read(s, None, None)?;
    s.wait(30, |s| {
        let facts = live::sample(s, &base, old_keys, proofs::Stage::Held, false, event)?;
        Ok(live::ready(s, &facts, &direct, false)?.then_some(()))
    })?;
    let direct_history =
        live::histories(s, &base, old_keys, proofs::Stage::Held, event, &before_maps)?;
    let stop_start = s.outcomes.len();
    s.kill_wait(0)?;
    s.kill_wait(1)?;
    s.reconcile_both()?;
    s.stopped_reconciled()?;
    for owner in 0..2 {
        proofs::work(s.counters[owner], owner, proofs::Stage::Held, false)?;
    }
    let stopped = s
        .outcomes
        .get(stop_start..)
        .context("actual waited outcomes")?;
    proofs::closed(stopped, old_keys)?;
    let actual_held = source::debt(&s.view(1, 0)?, oracle::Request::new(0, 1, 640)?)?;
    ensure!(
        actual_held == held,
        "original stopped B640 request/EventID/raw-source/cipher/debt1/no receipt"
    );
    let closed = json!({"outcomes":stopped,"held_b640":actual_held,"classification":"both waited/stopped sources reconciled; P10A interval zero"});
    let relay_start = relay_owner::start(s)?;
    let url = s
        .relay
        .as_ref()
        .context("same owned relay")?
        .url
        .clone()
        .context("actual canonical ready URL")?;
    let profile_facts = profiles(s, &url)?;
    s.start(0)?;
    s.start(1)?;
    let keys = live::keys(s)?;
    ensure!(
        keys.iter()
            .zip(&old_keys)
            .all(|(new, old)| new.owner == old.owner && new != old),
        "same original sources, genuine new route incarnations"
    );
    s.wait(15, |s| {
        live::sample(s, &base, keys, proofs::Stage::Held, true, event)?;
        Ok((0..2)
            .all(|owner| {
                (0..2).all(|context| {
                    s.view(owner, context).ok().is_some_and(|n| {
                        n.locators
                            .join(format!("{}.locator2", s.origin.contexts[context]))
                            .is_file()
                    })
                })
            })
            .then_some(()))
    })?;
    let publications = locators::read(s, Some(&url), Some(&direct))?;
    for context in 0..2 {
        s.import2_for(context)?;
        live::sample(s, &base, keys, proofs::Stage::Held, true, event)?;
    }
    let admitted = live::wait_relay(s, &base, keys, proofs::Stage::Held, &publications, event)?;
    let request_before = live::request(s, event)?;
    ensure!(
        !proofs::request(&request_before, event)?,
        "original B640 still has no receipt before release"
    );
    live::apply(s, &base, proofs::Stage::Ack)?;
    live::wait_relay(s, &base, keys, proofs::Stage::Ack, &publications, event)?;
    live::apply(s, &base, proofs::Stage::Released)?;
    let request_after = s.wait(60, |s| {
        let facts = live::sample(s, &base, keys, proofs::Stage::Released, true, event)?;
        ensure!(
            live::ready(s, &facts, &publications, true)?,
            "same current relay admission through autonomous delivery"
        );
        let row = live::request(s, event)?;
        Ok((proofs::request(&row, event)?
            && facts.rows[0][0]["next"] == json!([641, 641])
            && facts.rows[0][0]["counts"]["pending"] == 0
            && facts.rows[1][0]["counts"]["debt"] == 0)
            .then_some(row))
    })?;
    proofs::acknowledged(&request_before, &request_after, event)?;
    live::idle(s, &base, keys, proofs::Stage::Released, true, event)?;
    let materialized = early::source_row(&s.view(0, 0)?, 1, 640)?;
    ensure!(
        source::history_map(std::slice::from_ref(&materialized))?.get(&state::hex(&event))
            == final_maps[0][0].get(&state::hex(&event)),
        "actual A materialized original B640 request/EventID/body/format before clearing A prefix"
    );
    live::apply(s, &base, proofs::Stage::Final)?;
    let histories = live::histories(s, &base, keys, proofs::Stage::Final, event, &final_maps)?;
    let final_sample = live::sample(s, &base, keys, proofs::Stage::Final, true, event)?;
    ensure!(
        live::ready(s, &final_sample, &publications, true)?,
        "all four current actual relay paths at P10 end"
    );
    let packets = [
        s.running[0].as_ref().context("same A")?.decoder.packet()?,
        s.running[1].as_ref().context("same B")?.decoder.packet()?,
    ];
    for (owner, packet) in packets.iter().enumerate() {
        proofs::work(s.counters[owner], owner, proofs::Stage::Final, true)?;
        proofs::work(
            s.running[owner]
                .as_ref()
                .context("same stream")?
                .decoder
                .observed_counts()?,
            owner,
            proofs::Stage::Final,
            true,
        )?;
        proofs::packet(*packet, owner, true)?;
    }
    let relay_pid = s
        .relay
        .as_ref()
        .context("same relay retained into P11")?
        .child
        .0
        .id();
    let direct_facts = json!({"locators":direct.facts(),"histories":direct_history});
    let relay_facts = json!({"startup":relay_start,"profiles":profile_facts,"locators":publications.facts(),"admitted":admitted.rows,"final":final_sample.rows});
    let work = json!({"stats":final_sample.raw.each_ref().map(|v|v["stats"].clone()),"packets":packets.map(|p|p.map(|p|json!({"context":p.context,"encrypt":p.counts.encrypt,"decrypt":p.counts.decrypt,"attempts":p.counts.attempts,"aborted":p.counts.aborted,"ambiguous":p.counts.ambiguous,"accounts":p.counts.accounts,"outbound":p.counts.outbound,"inbound":p.counts.inbound,"operation":state::hex(&p.operation),"generation":p.generation,"sequence":p.sequence}))),"classification":"actual fresh relay live observations, streams remain open for P11"});
    let delivery = json!({"original_commitment":held,"request_before":request_before,"request_after":request_after,"materialized":materialized,"raw_cipher_rechecked_at_stopped_route_cut":true,"live_history_is_not_ciphertext_export":true,"final_signed_receipt_bytes_for_P11":true});
    s.remember("p10b",json!({"direct":direct_facts,"relay":relay_facts,"closed":closed,"delivery":delivery,"histories":histories,"work":work,"classification":"owned incomplete11/12, no whole PASS"}))?;
    s.phases.accept(oracle::Phase::Relay)?;
    ensure!(
        s.phases.next == 11
            && !s.phases.complete()
            && s.running.iter().all(Option::is_some)
            && s.relay.is_some(),
        "same owned runtimes and live relay for P11"
    );
    let observed = s.counters;
    Ok(Continuation {
        previous,
        direct: direct_facts,
        relay: relay_facts,
        closed,
        delivery,
        histories,
        work,
        incarnations: keys,
        observed,
        packets,
        relay_pid,
    })
}
