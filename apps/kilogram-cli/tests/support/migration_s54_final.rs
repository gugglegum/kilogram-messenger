//! Dormant P11 composition. Consumes the real continuation, never a whole PASS.
use super::*;
#[path = "migration_s54_final_live.rs"]
mod live;
#[cfg(test)]
#[path = "migration_s54_final_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Entry,
    ObserveFirst,
    RequestFirstA,
    RequestFirstB,
    WaitFirst,
    ReconcileFirst,
    ReadFirst,
    Reopen,
    ObserveRestart,
    RequestSecondA,
    RequestSecondB,
    WaitSecond,
    ReconcileSecond,
    ReadSecond,
    Compare,
    Final,
    RelayWait,
    Memory,
}
const STEPS: [Step; 18] = [
    Step::Entry,
    Step::ObserveFirst,
    Step::RequestFirstA,
    Step::RequestFirstB,
    Step::WaitFirst,
    Step::ReconcileFirst,
    Step::ReadFirst,
    Step::Reopen,
    Step::ObserveRestart,
    Step::RequestSecondA,
    Step::RequestSecondB,
    Step::WaitSecond,
    Step::ReconcileSecond,
    Step::ReadSecond,
    Step::Compare,
    Step::Final,
    Step::RelayWait,
    Step::Memory,
];
/// The tests inject operations into this exact decision code, not another lifecycle.
trait Adapter {
    type Output;
    fn perform(&mut self, step: Step) -> Result<()>;
    fn finish(self) -> Result<Self::Output>;
}
fn compose<A: Adapter>(mut adapter: A) -> Result<A::Output> {
    for step in STEPS {
        adapter.perform(step)?;
    }
    adapter.finish()
}
pub(super) struct Completed {
    facts: Value,
}
impl Completed {
    pub fn facts(&self) -> &Value {
        &self.facts
    }
}
fn bounded(mut facts: Value) -> Result<Completed> {
    public_fingerprints(&mut facts)?;
    ensure!(
        serde_json::to_vec(&facts)?.len() <= 256 * 1024,
        "bounded sanitized functional result"
    );
    Ok(Completed { facts })
}
struct State {
    base: live::Baseline,
    pubs: Option<relay::locators::Publications>,
    new_pubs: Option<Value>,
    current: Option<reader::Current>,
    associations: Vec<Value>,
    first: Option<reader::Cut>,
    second: Option<reader::Cut>,
    keys: [oracle::Incarnation; 2],
    outcomes: usize,
    starts: usize,
    attempt: String,
    relay: Option<Value>,
    memory: Option<Value>,
}
struct Actual {
    previous: relay::Continuation,
    state: Option<State>,
}
fn memory_scope_required(value: &Value, finished: bool) -> Result<()> {
    ensure!(
        value["synthetic_fixture"] == false && value["finished"] == finished,
        "active mandatory real memory scope at the required lifecycle boundary"
    );
    if finished {
        ensure!(
            value["original_lifetime_coverage_complete"] == true
                && value["coverage"]["coverage_valid"] == true,
            "actual complete original memory coverage"
        );
    }
    Ok(())
}
fn relay_waited(value: &Value, pid: u32, url: &str) -> Result<()> {
    ensure!(
        value["pid"] == pid
            && value["url"] == url
            && value["stop_published"] == true
            && value["waited"] == true
            && value["success"] == true
            && value["code"] == 0
            && value["forced"] == false,
        "same actual normal relay exit0/wait, not a stop file"
    );
    Ok(())
}
fn cardinality(before: [usize; 2], after: [usize; 2]) -> Result<()> {
    ensure!(
        after
            == [
                before[0].checked_add(4).context("normal outcomes bound")?,
                before[1].checked_add(2).context("restart starts bound")?
            ],
        "exact four new normal node outcomes and two new restart lifetimes"
    );
    Ok(())
}
fn counts(c: oracle::Counts) -> Value {
    json!({"encrypt":c.encrypt,"decrypt":c.decrypt,"attempts":c.attempts,
        "aborted":c.aborted,"ambiguous":c.ambiguous,"accounts":c.accounts,
        "outbound":c.outbound,"inbound":c.inbound})
}
fn cut_facts(cut: &reader::Cut) -> Value {
    json!(cut.owners.each_ref().map(|o| json!({
        "source":state::hex(&o.closure.key.owner),"pid":o.closure.key.pid,
        "incarnation":state::hex(&o.closure.key.nonce),"sequence":o.closure.sequence,
        "counts":counts(o.closure.counts),"outcome_index":o.closure.outcome,
        "eof":o.closure.eof,"root_generation":o.generation,"last_commit":state::hex(&o.last_commit),
        "source_point":o.closure.point,"histories":o.history.map(|h|state::hex(&h)),
        "witness_fingerprint":state::hex(&o.witnesses),"texts":o.graph.texts,"genesis":o.graph.genesis,
        "graph":o.graph.contexts.each_ref().map(|c|json!({"context":state::hex(&c.context),
            "scope":state::hex(&c.scope_commitment),"count":c.count,"next":c.next,
            "records_fingerprint":state::hex(&c.records_commitment)})),
        "native":o.native.each_ref().map(|n|n.public_facts())
    })))
}
impl Adapter for Actual {
    type Output = Completed;
    fn perform(&mut self, step: Step) -> Result<()> {
        if step == Step::Entry {
            ensure!(
                self.state.is_none() && memory_scope::active(),
                "one consuming entry with mandatory memory"
            );
            let attempt = s54_capture::admitted_attempt()?;
            memory_scope_required(&memory_scope::diagnostic()?, false)?;
            let base = live::entry(&mut self.previous)?;
            let s = live::controller(&mut self.previous);
            self.state = Some(State {
                keys: base.old_keys,
                base,
                pubs: None,
                new_pubs: None,
                current: None,
                associations: Vec::new(),
                first: None,
                second: None,
                outcomes: s.outcomes.len(),
                starts: s.intervals.starts.len(),
                attempt,
                relay: None,
                memory: None,
            });
            return Ok(());
        }
        let st = self
            .state
            .as_mut()
            .context("original admitted composition state")?;
        let s = live::controller(&mut self.previous);
        match step {
            Step::Entry => anyhow::bail!("entry already consumed"),
            Step::ObserveFirst => {
                let pubs = live::publications(s, &st.base, None)?;
                st.current = Some(live::observe(s, &st.base, st.keys, false, &pubs)?);
                st.associations.push(
                    st.current
                        .as_ref()
                        .context("first live cut")?
                        .associations
                        .clone(),
                );
                st.pubs = Some(pubs);
            }
            Step::RequestFirstA | Step::RequestSecondA => {
                s.request_normal(0)?;
            }
            Step::RequestFirstB | Step::RequestSecondB => {
                s.request_normal(1)?;
            }
            Step::WaitFirst | Step::WaitSecond => {
                s.wait_normal_both()?;
            }
            Step::ReconcileFirst | Step::ReconcileSecond => {
                s.reconcile_normal_both()?;
                let pair = s.normal_pair()?;
                ensure!(
                    pair.each_ref().map(|c| c.key) == st.keys,
                    "actual normally closed current pair"
                );
                for (o, c) in pair.iter().enumerate() {
                    ensure!(
                        c.counts
                            == if step == Step::ReconcileFirst {
                                st.base.old_counts[o]
                            } else {
                                Default::default()
                            },
                        "exact terminal counts through final EOF, not just last-live Stats"
                    );
                }
            }
            Step::ReadFirst => {
                ensure!(st.first.is_none(), "exactly one first cut");
                st.first = Some(reader::read_current(
                    &mut self.previous,
                    st.current.as_ref().context("fresh Cut1 associations")?,
                )?);
            }
            Step::Reopen => {
                ensure!(
                    st.first.is_some() && st.second.is_none(),
                    "Cut1 returned with released reader handles"
                );
                s.stopped_reconciled()?;
                live::unchanged(s, &st.base)?;
                // start's exclusive point read would reject any retained reader lock.
                s.start(0)?;
                s.start(1)?;
                st.keys = relay::live::keys(s)?;
                live::fresh(st.base.old_keys, st.keys)?;
                live::sample(s, &st.base, st.keys, true)?;
                st.current = None;
            }
            Step::ObserveRestart => {
                let pubs = live::publications(
                    s,
                    &st.base,
                    Some(st.pubs.as_ref().context("signed same-route baseline")?),
                )?;
                live::sample(s, &st.base, st.keys, true)?;
                for c in 0..2 {
                    s.import2_for(c)?;
                    live::sample(s, &st.base, st.keys, true)?;
                }
                st.current = Some(live::observe(s, &st.base, st.keys, true, &pubs)?);
                st.associations.push(
                    st.current
                        .as_ref()
                        .context("restart live cut")?
                        .associations
                        .clone(),
                );
                st.new_pubs = Some(pubs.facts());
            }
            Step::ReadSecond => {
                ensure!(
                    st.first.is_some() && st.second.is_none(),
                    "exactly second cut"
                );
                st.second = Some(reader::read_current(
                    &mut self.previous,
                    st.current.as_ref().context("fresh Cut2 associations")?,
                )?);
            }
            Step::Compare => {
                st.first
                    .as_ref()
                    .context("actual Cut1")?
                    .after_zero_restart(st.second.as_ref().context("actual Cut2")?)?;
                cardinality(
                    [st.outcomes, st.starts],
                    [s.outcomes.len(), s.intervals.starts.len()],
                )?;
                s.current_intervals()?;
                live::unchanged(s, &st.base)?;
            }
            Step::Final => {
                s.phases.accept(oracle::Phase::Final)?;
            }
            Step::RelayWait => {
                let outcome = s.finish_normal_relay()?;
                relay_waited(&outcome, st.base.relay_pid, &st.base.url)?;
                st.relay = Some(outcome);
            }
            Step::Memory => {
                s.check()?;
                ensure!(
                    s54_capture::admitted_attempt()? == st.attempt,
                    "same healthy mandatory capture scope"
                );
                let memory = s.finish_memory_coverage()?;
                memory_scope_required(&memory, true)?;
                st.memory = Some(memory);
            }
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Completed> {
        let st = self.state.take().context("completed functional state")?;
        let s = live::controller(&mut self.previous);
        // A8 finish seals the observer; Controller::check deliberately rejects
        // a finished scope. Only pure stopped/deadline checks may follow it.
        s53_deadline::check()?;
        ensure!(
            Instant::now() < s.until,
            "original deadline at functional return"
        );
        ensure!(
            s.phases.complete()
                && s.relay.is_none()
                && s.relay_outcome.as_ref() == st.relay.as_ref(),
            "completed functional phases with actual finalized relay"
        );
        s.stopped_reconciled()?;
        ensure!(
            s54_capture::admitted_attempt()? == st.attempt,
            "healthy original capture at return"
        );
        let verified = s.current_intervals()?;
        let memory = st.memory.context("actual memory final coverage")?;
        ensure!(
            memory_scope::diagnostic()? == memory,
            "same actual sealed memory report at return"
        );
        memory_scope_required(&memory, true)?;
        let a = st.first.as_ref().context("first actual cut")?;
        let b = st.second.as_ref().context("second actual cut")?;
        let totals = &b.accounting.totals;
        ensure!(
            st.associations.len() == 2,
            "exact two cut-local live associations"
        );
        let facts = json!({"classification":"P11 functional phase only",
            "whole_attempt_verdict":"not_established",
            "pending":["whole inventory schema and admission","independent auxiliary cleanup","actual outer harness return"],
            "capture_attempt":st.attempt,"cuts":[cut_facts(a),cut_facts(b)],
            "live_cut_associations":st.associations,
            "publications":{"before":st.pubs.context("baseline publications")?.facts(),"restart":st.new_pubs.context("restart publications")?},
            "normal_node_outcomes":&s.outcomes[st.outcomes..],"relay_outcome":st.relay.context("actual relay wait")?,
            "accounting":{"streams":verified.streams,"observed_operations":verified.observed_operations,
                "reconciled_entries":verified.reconciled_entries,"current":counts(verified.counts),
                "old":[totals.old.encrypt,totals.old.decrypt],"overlap":[totals.overlap.encrypt,totals.overlap.decrypt],
                "union":[totals.union.encrypt,totals.union.decrypt],"directed":totals.directed,
                "original_accounts":totals.accounts,"original_outbound":totals.outbound,"original_inbound":totals.inbound},
            "requests_fingerprint":state::hex(&st.base.requests),
            "memory":{"inventory_binding":memory["inventory"],"coverage":memory["coverage"],
                "resident_spawns":memory["actual_resident_spawns"],"terminal_outcomes":memory["actual_terminal_outcomes"],
                "query_attempts":memory["query_attempts"],"report_fingerprint":blake3::hash(&serde_json::to_vec(&memory)?).to_hex().to_string()}});
        s53_deadline::check()?;
        let completed = bounded(facts)?;
        s53_deadline::check()?;
        ensure!(
            Instant::now() < s.until,
            "original deadline after result assembly"
        );
        Ok(completed)
    }
}
/// No selector invokes this. No authoritative whole-result write occurs here.
pub(super) fn p11(previous: relay::Continuation) -> Result<Completed> {
    let until = previous
        .previous
        .previous
        .previous
        .previous
        .controller
        .until;
    let _phase = s53_deadline::Phase::until(until)?;
    compose(Actual {
        previous,
        state: None,
    })
}
