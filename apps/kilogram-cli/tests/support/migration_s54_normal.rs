//! Dormant normal lifecycle on the original Controller slots. No P11 composition.
use super::*;
#[cfg(test)]
#[path = "migration_s54_normal_tests.rs"]
mod tests;

/// Retained only after both real normal waits, final EOF and ledger/source reconciliation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct Closure {
    pub key: oracle::Incarnation,
    pub sequence: u64,
    pub counts: oracle::Counts,
    pub eof: u64,
    pub generation: u64,
    pub operation: old::Id,
    pub point: [u64; 2],
    outcome: usize,
}
fn closed_outcome(c: &Closure, row: &Value, owner: usize) -> Result<()> {
    ensure!(
        row["owner"] == owner
            && row["pid"] == c.key.pid
            && row["code"] == 0
            && row["success"] == true
            && row["waited"] == true
            && row["counter_outcome"] == "normal exit0/final/EOF/source resolved"
            && row["normal_stop"]["source"] == state::hex(&c.key.owner)
            && row["normal_stop"]["incarnation"] == state::hex(&c.key.nonce)
            && row["normal_stop"]["acknowledged"] == true
            && row["normal_stop"]["waited"] == true
            && row["normal_closure"]["source"] == state::hex(&c.key.owner)
            && row["normal_closure"]["pid"] == c.key.pid
            && row["normal_closure"]["incarnation"] == state::hex(&c.key.nonce)
            && row["normal_closure"]["eof"] == c.eof
            && row["normal_closure"]["sequence"] == c.sequence
            && row["normal_closure"]["root_generation"] == c.generation
            && row["normal_closure"]["last_commit"] == state::hex(&c.operation)
            && row["normal_closure"]["source_point"] == json!(c.point),
        "retained actual normal closure/outcome binding"
    );
    Ok(())
}
pub(super) fn closed_pair(
    pair: Option<&[Closure; 2]>,
    outcomes: &[Value],
    sources: [old::Id; 2],
) -> Result<[Closure; 2]> {
    let pair = pair.context("actual last normal pair, not an earlier killed stop")?;
    ensure!(
        pair[0].key.owner != pair[1].key.owner && pair[0].key.pid != pair[1].key.pid,
        "two original distinct normally closed owners"
    );
    for (owner, c) in pair.iter().enumerate() {
        ensure!(
            c.key.owner == sources[owner]
                && c.key.pid > 0
                && c.key.nonce != [0; 32]
                && outcomes
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, r)| r["owner"] == owner)
                    .is_some_and(|(i, _)| i == c.outcome),
            "latest actual original owner closure"
        );
        closed_outcome(
            c,
            outcomes
                .get(c.outcome)
                .context("nonempty actual normal outcome")?,
            owner,
        )?;
    }
    Ok(pair.clone())
}

pub(super) fn exit_zero(code: Option<i32>) -> Result<()> {
    ensure!(
        code == Some(0),
        "actual requested normal exit must be code0"
    );
    Ok(())
}
pub(super) fn observed_exit(stop: Option<&Stop>, code: Option<Option<i32>>) -> Result<()> {
    if let Some(code) = code {
        ensure!(
            stop.is_some(),
            "unexpected owned child exit without normal request"
        );
        exit_zero(code)?;
    }
    Ok(())
}
pub(super) fn reopen(pending: bool, relay_finished: bool) -> Result<()> {
    ensure!(
        !pending && !relay_finished,
        "previous incarnation reconciled and original relay not finalized before reopen"
    );
    Ok(())
}
pub(super) struct Stop {
    key: oracle::Incarnation,
    started: Instant,
    until: Instant,
    acknowledged: bool,
    waited: bool,
}
impl Stop {
    fn new(key: oracle::Incarnation, now: Instant, whole: Instant) -> Result<Self> {
        ensure!(
            now < whole && key.pid > 0 && key.nonce != [0; 32],
            "live original normal request/deadline"
        );
        Ok(Self {
            key,
            started: now,
            until: (now + Duration::from_secs(15)).min(whole),
            acknowledged: false,
            waited: false,
        })
    }
    pub(super) fn check(&self, key: oracle::Incarnation, pid: u32, now: Instant) -> Result<()> {
        ensure!(
            self.key == key && key.pid == pid && (self.waited || now < self.until),
            "same requested source/PID/nonce and unextended normal deadline"
        );
        Ok(())
    }
    fn acknowledge(&mut self, output: &str) -> Result<()> {
        ensure!(
            !self.acknowledged && !self.waited && output.trim() == "status=shutdown-accepted",
            "one actual ordered shutdown acknowledgement"
        );
        self.acknowledged = true;
        Ok(())
    }
    fn wait(&mut self, code: Option<i32>, now: Instant) -> Result<()> {
        self.check(self.key, self.key.pid, now)?;
        ensure!(
            self.acknowledged && !self.waited,
            "acknowledgement is required but not a wait"
        );
        exit_zero(code)?;
        self.waited = true;
        Ok(())
    }
    fn complete(&self) -> Result<()> {
        ensure!(
            self.acknowledged && self.waited,
            "real normal acknowledgement and wait required before source read"
        );
        Ok(())
    }
    pub(super) fn facts(&self) -> Value {
        json!({"pid":self.key.pid,"source":state::hex(&self.key.owner),"incarnation":state::hex(&self.key.nonce),"acknowledged":self.acknowledged,"waited":self.waited,"limit_ms":self.until.duration_since(self.started).as_millis()})
    }
}
fn ready_to_read(running: [bool; 2], stops: [Option<&Stop>; 2]) -> Result<()> {
    waited_pair(
        running,
        stops.map(|s| s.is_some_and(|s| s.complete().is_ok())),
    )
}
impl Controller {
    pub fn request_normal(&mut self, owner: usize) -> Result<()> {
        ensure!(
            owner < 2 && self.origin.pair.nodes[owner].indexed,
            "original current owner normal lifecycle"
        );
        self.check()?;
        let run = self.running[owner]
            .as_mut()
            .context("original live normal handle")?;
        ensure!(run.normal.is_none(), "one normal request per incarnation");
        run.child.alive()?;
        memory_scope::before_normal(&mut run.child.0)?;
        let key = run.decoder.incarnation()?;
        ensure!(
            key.pid == run.child.0.id()
                && self.original[owner]
                    .as_ref()
                    .is_some_and(|o| o.source_id == key.owner),
            "original handle/source/nonce before requesting shutdown"
        );
        let stop = Stop::new(key, Instant::now(), self.until)?;
        let until = stop.until;
        run.normal = Some(stop); // Own the unresolved request even if IPC fails.
        let _phase = s53_deadline::Phase::until(until)?;
        let node = self.view(owner, 0)?;
        let output = cli(
            &current(),
            &[
                "ordered".into(),
                "shutdown".into(),
                "--ipc-file".into(),
                text(&node.ipc),
            ],
        )?;
        self.running[owner]
            .as_mut()
            .context("same normal child")?
            .normal
            .as_mut()
            .context("retained request")?
            .acknowledge(&output)?;
        self.check()
    }
    pub fn wait_normal_both(&mut self) -> Result<()> {
        for run in &self.running {
            let stop = run
                .as_ref()
                .and_then(|r| r.normal.as_ref())
                .context("both normal requests remain owned")?;
            ensure!(
                stop.acknowledged && !stop.waited,
                "both acknowledged requests before normal waits"
            );
        }
        ensure!(
            self.pending_stops.iter().all(Option::is_none) && self.outcomes.len() <= 30,
            "bounded pair of unresolved normal outcomes"
        );
        loop {
            self.check()?;
            for owner in 0..2 {
                let Some(run) = self.running[owner].as_mut() else {
                    continue;
                };
                if run.child.0.try_wait()?.is_none() {
                    continue;
                }
                let exit = run.child.0.wait()?;
                memory_scope::terminal(&run.child.0, true, exit.code(), false)?;
                run.normal
                    .as_mut()
                    .context("requested normal child")?
                    .wait(exit.code(), Instant::now())?;
                self.outcomes.push(json!({"owner":owner,"pid":run.child.0.id(),"waited":true,"success":exit.success(),"code":exit.code(),"normal_stop":run.normal.as_ref().map(Stop::facts),"counter_outcome":"pending normal final/EOF/source reconciliation"}));
                let run = self.running[owner].take().context("same waited handle")?;
                // This field remains the actual last live sample for killed callers;
                // the normal path below uses only the retained final record.
                self.pending_stops[owner] = Some(Stopped {
                    run,
                    counts: self.counters[owner],
                    outcome: self.outcomes.len() - 1,
                });
            }
            if self.running.iter().all(Option::is_none) {
                ready_to_read(
                    self.running.each_ref().map(Option::is_some),
                    self.pending_stops
                        .each_ref()
                        .map(|p| p.as_ref().and_then(|p| p.run.normal.as_ref())),
                )?;
                return self.check();
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    pub fn reconcile_normal_both(&mut self) -> Result<Value> {
        ready_to_read(
            self.running.each_ref().map(Option::is_some),
            self.pending_stops
                .each_ref()
                .map(|p| p.as_ref().and_then(|p| p.run.normal.as_ref())),
        )?;
        self.check()?;
        let mut proofs = Vec::new();
        let mut closures = Vec::new();
        // Both actual waits are proved before either private source is opened.
        for owner in 0..2 {
            self.stopped(owner)?;
            let pending = self.pending_stops[owner]
                .as_mut()
                .context("retained unresolved normal stop")?;
            let run = &mut pending.run;
            run.decoder.poll(
                &run.log,
                &mut self.operations,
                &mut self.pages,
                &mut self.pre_q_heads[owner],
            )?;
            let eof = std::fs::metadata(&run.log)?.len();
            let stop = run.normal.as_ref().context("actual normal request")?;
            stop.complete()?;
            run.decoder.normal_final(&self.operations, stop.key, eof)?;
            let end = source::normal_end(
                &self.origin.pair.nodes[owner],
                &self.origin.contexts,
                stop.key.owner,
            )?;
            let facts = run.decoder.close_normal(
                &self.operations,
                stop.key,
                run.start_sources,
                &end,
                eof,
            )?;
            let final_record = run.decoder.normal_final(&self.operations, stop.key, eof)?;
            closures.push(Closure {
                key: stop.key,
                sequence: final_record.sequence,
                counts: final_record.counts,
                eof,
                generation: end.generation,
                operation: end.operation,
                point: [end.point.0, end.point.1],
                outcome: pending.outcome,
            });
            proofs.push((facts, final_record.counts, pending.outcome));
        }
        for (owner, (facts, counts, outcome)) in proofs.iter().enumerate() {
            self.counters[owner] = *counts;
            self.outcomes[*outcome]["counter_outcome"] =
                "normal exit0/final/EOF/source resolved".into();
            self.outcomes[*outcome]["normal_closure"] = facts.clone();
        }
        // Preserve both unresolved records if either validation failed.
        self.pending_stops = [None, None];
        self.stopped_reconciled()?;
        self.check()?;
        self.last_normal = Some(
            closures
                .try_into()
                .map_err(|_| anyhow::anyhow!("two actual normal closures"))?,
        );
        Ok(json!(
            proofs
                .into_iter()
                .map(|(facts, _, _)| facts)
                .collect::<Vec<_>>()
        ))
    }
    /// No source opens here. The retained pair is invalidated on any start/kill.
    pub fn normal_pair(&mut self) -> Result<[Closure; 2]> {
        self.stopped_reconciled()?;
        self.check()?;
        ensure!(
            self.relay.is_some() && self.relay_outcome.is_none(),
            "same owned relay still live"
        );
        self.relay
            .as_mut()
            .context("retained original relay")?
            .child
            .alive()?;
        let sources = [
            self.original[0]
                .as_ref()
                .context("actual original A")?
                .source_id,
            self.original[1]
                .as_ref()
                .context("actual original B")?
                .source_id,
        ];
        let pair = closed_pair(self.last_normal.as_ref(), &self.outcomes, sources)?;
        for c in &pair {
            self.operations.closed(c.key, c.sequence, c.counts)?;
        }
        self.operations.summary()?;
        Ok(pair)
    }
    pub fn finish_normal_relay(&mut self) -> Result<Value> {
        self.stopped_reconciled()?;
        self.operations.summary()?;
        ensure!(
            self.relay_outcome.is_none(),
            "one normal relay finalization"
        );
        self.check()?;
        // Exclusive original locks also reject an accidentally retained reader.
        for node in &self.origin.pair.nodes {
            source::owned(node)?;
        }
        let [a, b] = self
            .origin
            .pair
            .nodes
            .each_ref()
            .map(|n| kilogram_state::StateDirectoryLock::acquire(&n.branch.state));
        let (_a, _b) = (a?, b?);
        let facts = self
            .relay
            .as_mut()
            .context("original owned relay")?
            .finish_normal(self.until)?;
        self.relay_outcome = Some(facts.clone());
        self.relay.take();
        s53_deadline::check()?;
        Ok(facts)
    }
}
