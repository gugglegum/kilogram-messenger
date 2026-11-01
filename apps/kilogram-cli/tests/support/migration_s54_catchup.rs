//! Inactive consuming P9. Same owners/ledger/deadline return incomplete10/12.
use super::*;
#[path = "migration_s54_catchup_live.rs"]
mod live;
#[path = "migration_s54_catchup_proofs.rs"]
mod proofs;
#[cfg(test)]
#[path = "migration_s54_catchup_tests.rs"]
mod tests;

pub(super) struct Continuation {
    pub previous: pressure::Continuation,
    pub baseline: oracle::PageBaseline,
    pub pages: Vec<observations::PageObservation>,
    pub head: observations::PageObservation,
    pub first_tail: observations::PageObservation,
    pub ack_lost: observations::AckObservation,
    pub ack_replayed: observations::AckObservation,
    pub histories: [BTreeMap<String, Value>; 2],
    pub terminal: Value,
    pub work: Value,
}
pub(super) fn p9(mut previous: pressure::Continuation) -> Result<Continuation> {
    let immutable = previous.previous.immutable()?;
    let ids = immutable.ids();
    let held = previous.previous.held_b640.clone();
    let c2 = previous.c2_history.clone();
    let s = &mut previous.previous.controller;
    let _phase = s53_deadline::Phase::until(s.until)?;
    proofs::prerequisites(
        s.phases.next,
        s.running.each_ref().map(Option::is_some),
        s.requests.len(),
        &previous.pending,
        &ids,
    )?;
    pressure::bracket(s, &previous.hold, &previous.successors)?;
    let mut work = proofs::Work::new(s.counters);
    {
        let _old_bound = s53_deadline::Phase::until(previous.hold.until)?;
        for owner in 0..2 {
            let c = &mut s.controls[owner];
            c.old["disable_ack"] = true.into();
            c.old["suspend_prefix_requests"] = true.into();
            c.current["phase"] = "catch_up".into();
            c.current["suspend_live"] = true.into();
            c.current["suspend_prefix"] = true.into();
            c.current["hold_source64"] = false.into();
            c.current["source64_after"] = Value::Null;
            c.current["hold_ack"] = false.into();
            c.current["hold_ack_from"] = Value::Null;
            c.current["pause_after_normal_page"] = Value::Null;
            proofs::held_schedule(c, owner)?;
            c.write()?;
        }
        let released = s.wait(30, |s| {
            let (raw, rows) = pressure::full(s, 0)?;
            let run = s.running[0].as_ref().context("same held A")?;
            ensure!(
                run.child.0.id() == previous.hold.pids[0]
                    && rows[0]["lease_generation"] == previous.hold.generations[0],
                "real64 released under original PID/lease"
            );
            Ok((raw["window"][1][s.view(0, 0)?.slot as usize] == 0).then_some(raw))
        })?;
        s.remember("p9-release64", released)?;
    }
    let initial = live::live(s)?;
    let baseline = s.pages.baseline(initial.keys[1])?;
    let owners = initial.keys.map(|k| k.owner);
    let page2 = live::page(s, baseline, 2)?;
    let decoder = &s.running[1].as_ref().context("original B decoder")?.decoder;
    let head = decoder
        .accepted_head_page()
        .cloned()
        .context("actual funded head385")?;
    let first_tail = decoder
        .first_tail_after_head()
        .cloned()
        .context("actual first tail after head/pending drain")?;
    ensure!(
        head.context == 0
            && head.head
            && head.start == 385
            && head.end == 386
            && head.new_sources == 1
            && first_tail.start == 450
            && first_tail.stamp.line > head.stamp.line
            && first_tail.binding == head.binding
            && first_tail.lease_generation == head.lease_generation,
        "ordered actual head385/first tail450, no transient frontier polling"
    );
    let row = early::source_row(&s.view(1, 0)?, 0, 385)?;
    ensure!(
        row["id"] == state::hex(&ids[0].1),
        "actual funded head source equals original P7 A385"
    );
    s.remember("p9-head-tail",json!({"head":head.facts(),"first_tail":first_tail.facts(),"head_source":row,"original_pending":previous.pending.iter().map(|p|json!([p.ordinal,state::hex(&p.event),p.generation,state::hex(&p.operation)])).collect::<Vec<_>>()}))?;
    s.stop(1)?;
    work.close(1, s.counters[1])?;
    live::clear_page(s)?;
    let second = live::reopen(s, &[1], &mut work)?;
    ensure!(
        second.keys[1].owner == owners[1]
            && second.keys[1] != initial.keys[1]
            && s.pages.phase_total(second.keys[1], baseline)? == 2,
        "same B source, actual new incarnation at phase2"
    );
    let target = s.pages.phase_local_target(second.keys[1], baseline, 5)?;
    ensure!(
        target == 3,
        "derived new-local target after observed phase2/new local0"
    );
    let page5 = live::page(s, baseline, 5)?;
    s.stop(1)?;
    work.close(1, s.counters[1])?;
    live::clear_page(s)?;
    let third = live::reopen(s, &[1], &mut work)?;
    ensure!(
        third.keys[1].owner == owners[1] && third.keys[1] != second.keys[1],
        "same actual third B incarnation"
    );
    live::prefix(s, false)?;
    s.wait(60, |s| {
        let b = s.status(1, 0)?;
        Ok(proofs::progress(&b, 641)?.then_some(()))
    })?;
    live::prefix(s, true)?;
    let caught = live::idle(s)?;
    proofs::final_c1(&[caught.rows[0][0].clone(), caught.rows[1][0].clone()], 256)?;
    let actual_pages = s.pages.phase_total(caught.keys[1], baseline)?;
    ensure!(
        actual_pages >= 8,
        "at least eight actual P9 forward normal pages"
    );
    let histories = live::histories(s, &ids, &held, &c2)?;
    live::arm_ack(s)?;
    let ack_lost = live::ack(s, &histories[0], &ids)?;
    s.remember("p9-ack-lost",json!({"facts":ack_lost.facts(),"pid":ack_lost.stamp.incarnation.pid,"sequence":ack_lost.stamp.sequence,"line":ack_lost.stamp.line}))?;
    // Keep the old held push armed, but suppress future ACK work before A's
    // fresh admission. Closing A cancels the old dispatch and releases reserve32.
    s.controls[1].old["disable_ack"] = true.into();
    s.controls[1].write()?;
    s.stop(0)?;
    work.close(0, s.counters[0])?;
    let lost=s.wait(30,|s| {
        let b=s.status(1,0)?;
        let raw=s.view(1,0)?.status()?;
        Ok((b["lease_current"]==false && b["lease_generation"].is_null() && raw["window"][1][s.view(1,0)?.slot as usize]==0).then_some(json!({"status":b,"window":raw["window"],"old_binding":state::hex(&ack_lost.binding)})))
    })?;
    s.remember("p9-old-binding-released", lost)?;
    let admitted = live::reopen(s, &[0], &mut work)?;
    ensure!(
        admitted.keys[0].owner == owners[0] && admitted.keys[0] != initial.keys[0],
        "same A source actual new process"
    );
    live::arm_ack(s)?;
    let ack_replayed = live::ack(s, &histories[0], &ids)?;
    proofs::replay(&ack_lost, &ack_replayed)?;
    s.remember("p9-ack-replayed",json!({"facts":ack_replayed.facts(),"pid":ack_replayed.stamp.incarnation.pid,"sequence":ack_replayed.stamp.sequence,"line":ack_replayed.stamp.line}))?;
    s.controls[1].current["hold_ack"] = false.into();
    s.controls[1].current["hold_ack_from"] = Value::Null;
    s.controls[1].write()?;
    s.wait(60, |s| {
        let a = s.status(0, 0)?;
        Ok((a["counts"]["debt"] == 0).then_some(()))
    })?;
    s.controls[1].old["disable_ack"] = true.into();
    s.controls[1].write()?;
    let final_live = live::idle(s)?;
    proofs::final_c1(
        &[final_live.rows[0][0].clone(), final_live.rows[1][0].clone()],
        0,
    )?;
    ensure!(
        live::histories(s, &ids, &held, &c2)? == histories,
        "receipt retirement changes no source/body history"
    );
    // Accepted terminal cut: BOTH waits first, then actual ledger/source close,
    // then immutable-only reads. No mutable P7 snapshot comparison here.
    s.kill_wait(0)?;
    s.kill_wait(1)?;
    s.reconcile_both()?;
    for owner in 0..2 {
        work.close(owner, s.counters[owner])?;
    }
    let work_facts = work.terminal()?;
    let terminal = immutable.read(s, &final_live.rows, owners, &c2, &held, &ack_replayed)?;
    let reopened = live::reopen(s, &[0, 1], &mut work)?;
    ensure!(
        reopened.keys.map(|k| k.owner) == owners
            && reopened
                .keys
                .iter()
                .zip(&final_live.keys)
                .all(|(new, old)| new != old),
        "same original sources, actual final new incarnations"
    );
    proofs::final_c1(
        &[reopened.rows[0][0].clone(), reopened.rows[1][0].clone()],
        0,
    )?;
    ensure!(
        s.counters == [oracle::Counts::default(); 2]
            && live::histories(s, &ids, &held, &c2)? == histories
            && s.counters == [oracle::Counts::default(); 2],
        "actual reopened zero effects with original public/frozen/C2 witnesses"
    );
    s.remember("p9",json!({"classification":"owned incomplete10/12; not whole PASS","normal_pages":actual_pages,"terminal":terminal,"work":work_facts,"reopened":reopened.rows,"outcomes":s.outcomes}))?;
    s.phases.accept(oracle::Phase::CatchUp)?;
    ensure!(
        s.phases.next == 10 && !s.phases.complete() && s.running.iter().all(Option::is_some),
        "same owned incomplete10/12, both live for P10"
    );
    Ok(Continuation {
        previous,
        baseline,
        pages: vec![page2, page5],
        head,
        first_tail,
        ack_lost,
        ack_replayed,
        histories,
        terminal,
        work: work_facts,
    })
}
