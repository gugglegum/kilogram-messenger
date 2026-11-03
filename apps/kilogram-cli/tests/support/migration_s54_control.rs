//! Per-child controls and lifetime ownership for the same two original directories.
use super::*;
#[path = "migration_s54_normal.rs"]
mod normal;
pub(super) use normal::Closure;
#[cfg(test)]
#[path = "migration_s54_migration_source_tests.rs"]
mod migration_source_tests;

/// Retained migration facts only. Live frontiers and genesis witnesses are not
/// substitutes for this baseline; resolving it performs no repository I/O.
pub(super) struct MigrationSources<'a> {
    contexts: &'a [String; 2],
    witnesses: &'a [Option<source::Legacy>; 2],
    rows: &'a BTreeMap<String, Value>,
    endpoints: &'a [Option<intervals::Endpoint>; 2],
}
impl<'a> MigrationSources<'a> {
    pub(super) fn new(
        contexts: &'a [String; 2],
        witnesses: &'a [Option<source::Legacy>; 2],
        rows: &'a BTreeMap<String, Value>,
        endpoints: &'a [Option<intervals::Endpoint>; 2],
    ) -> Self {
        Self {
            contexts,
            witnesses,
            rows,
            endpoints,
        }
    }
    pub(super) fn source(&self, owner: usize) -> Result<old::Id> {
        ensure!(owner < 2, "verified migration owner range");
        let witness = self.witnesses[owner]
            .as_ref()
            .context("verified migration witness")?;
        let ready = self
            .rows
            .get(&format!("ready-{owner}"))
            .context("retained migration Ready")?;
        let endpoint = self.endpoints[owner]
            .as_ref()
            .context("registered migration endpoint")?;
        endpoint.validate(owner as u8)?;
        let expected = witness.migration_endpoint(ready)?;
        ensure!(
            expected == *endpoint
                && expected
                    .lanes
                    .iter()
                    .enumerate()
                    .all(|(c, lane)| lane.role == owner as u8
                        && state::hex(&lane.context) == self.contexts[c]),
            "retained migration source/context/slot/frontier agreement"
        );
        Ok(expected.source)
    }
    pub(super) fn pair(&self) -> Result<[old::Id; 2]> {
        let pair = [self.source(0)?, self.source(1)?];
        ensure!(pair[0] != pair[1], "distinct verified migration owners");
        Ok(pair)
    }
    pub(super) fn bind_memory(&self, owner: usize) -> Result<()> {
        let source = self.source(owner)?;
        memory_scope::bind_legacy(owner as u8, source)
    }
    pub(super) fn normal_pair(
        &self,
        pair: Option<&[Closure; 2]>,
        outcomes: &[Value],
    ) -> Result<[Closure; 2]> {
        normal::closed_pair(pair, outcomes, self.pair()?)
    }
}
pub(super) fn runtime_workers(available: usize, inherited: Option<&str>) -> Result<usize> {
    let workers = inherited
        .map(str::parse::<usize>)
        .transpose()?
        .unwrap_or(available);
    ensure!(
        workers >= 2,
        "existing synchronous publication hook requires at least two Tokio workers"
    );
    Ok(workers)
}

pub(super) struct Controls {
    pub od1: PathBuf,
    pub s54: PathBuf,
    pub old: Value,
    pub current: Value,
}
impl Controls {
    pub fn new(base: &Path, context: &str, role: usize) -> Result<Self> {
        let value = Self {
            od1: base.join(format!("owner-{role}.od1.json")),
            s54: base.join(format!("owner-{role}.s54.json")),
            old: json!({"context":context,"hold_lane":role,"hold_ordinals":if role==1{vec![16]}else{vec![]},"suspend_prefix_requests":role==0,"trace_rpc":true}),
            current: json!({"context":context,"lane":role,"phase":"observe","max_pause_ms":120000}),
        };
        value.write()?;
        Ok(value)
    }
    pub fn write(&self) -> Result<()> {
        ensure!(
            self.old.get("phase").is_none() && self.old.get("hold_source64").is_none(),
            "frozen control schema remains separate"
        );
        schedule_file(&self.od1, &self.old)?;
        schedule_file(&self.s54, &self.current)
    }
    pub fn arm(&mut self, name: Option<&str>) -> Result<()> {
        // A marker is per publication interval; the signed facts stay immutable.
        let hit = self.od1.with_extension("hit");
        if hit.exists() {
            std::fs::remove_file(hit)?;
        }
        self.old["pause_point"] = name.map(Value::from).unwrap_or(Value::Null);
        self.write()
    }
    pub fn hit(&self, name: &str) -> Result<bool> {
        match std::fs::read_to_string(self.od1.with_extension("hit")) {
            Ok(raw) => Ok(raw == name),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    pub fn facts(&self, name: &str, reference: &Value) -> Result<Value> {
        let path = self
            .od1
            .with_extension(format!("s53-{name}-before-publication.facts.json"));
        let raw = kilogram_cli::ordered_dialog::admin::read_public(&path, 4096)?;
        let row: Value = serde_json::from_slice(&raw)?;
        ensure!(
            row["control"] == name
                && row["control_ref"] == *reference
                && row["signed_control_blake3"]
                    .as_str()
                    .is_some_and(|s| s.len() == 64),
            "actual saved control publication identity"
        );
        Ok(row)
    }
    pub fn attach(&self, cmd: &mut Command, indexed: bool) {
        cmd.env_remove("KILOGRAM_TEST_S54_SCHEDULE");
        cmd.env("KILOGRAM_TEST_OD1_SCHEDULE", &self.od1);
        if indexed {
            cmd.env("KILOGRAM_TEST_S54_SCHEDULE", &self.s54);
        }
    }
}

pub(super) struct Running {
    pub child: OwnedChild,
    pub log: PathBuf,
    pub decoder: observations::Decoder,
    pub start_sources: (u64, u64),
    pub interval_start: usize,
    normal: Option<normal::Stop>,
}
pub(super) struct Stopped {
    run: Running,
    counts: oracle::Counts,
    outcome: usize,
}
pub(super) fn waited_pair(running: [bool; 2], stopped: [bool; 2]) -> Result<()> {
    ensure!(
        running == [false; 2] && stopped == [true; 2],
        "both owned kill/waits precede source reconciliation"
    );
    Ok(())
}
pub(super) fn reconciled_pair(running: [bool; 2], pending: [bool; 2]) -> Result<()> {
    ensure!(
        running == [false; 2] && pending == [false; 2],
        "both owners stopped and reconciled before profile edit/relay start"
    );
    Ok(())
}
pub(super) fn interval10(start: Instant, whole: Instant) -> Instant {
    (start + Duration::from_secs(10)).min(whole)
}
pub(super) fn request(owner: usize, context: usize, number: u16) -> Result<oracle::Request> {
    oracle::Request::new(context.try_into()?, owner.try_into()?, number)
}
pub(super) struct Controller {
    pub origin: OldOrigin,
    pub controls: [Controls; 2],
    pub running: [Option<Running>; 2],
    pub relay: Option<relay_owner::Relay>,
    pub relay_outcome: Option<Value>,
    pending_stops: [Option<Stopped>; 2],
    last_normal: Option<[Closure; 2]>,
    pub operations: oracle::Operations,
    pub intervals: intervals::Ledger,
    pub accounting: accounting::Early,
    pub pages: oracle::Pages,
    pub phases: oracle::Phases,
    pub until: Instant,
    pub requests: BTreeMap<oracle::Request, Value>,
    pub original: [Option<source::Legacy>; 2],
    pub migration: [Option<source::Legacy>; 2],
    pub rows: BTreeMap<String, Value>,
    pub frozen: Option<early::Frozen>,
    pub counters: [oracle::Counts; 2],
    pub pre_q_heads: [u64; 2],
    pub outcomes: Vec<Value>,
    pub old_overlap: Vec<Value>,
    _memory: memory_scope::Guard,
    // Last field: retained through observer/profile destruction.
    _capture: s54_capture::Guard,
}
impl Drop for Controller {
    fn drop(&mut self) {
        // Error/cancellation cleanup must also work after an expired deadline.
        // It uses known owned handles only, never enumerates or kills by name.
        for (owner, run) in self.running.iter_mut().enumerate() {
            if let Some(mut owned) = run.take() {
                memory_scope::dropping(&owned.child.0);
                let pid = owned.child.0.id();
                let _ = owned.child.0.kill();
                let outcome = match owned.child.0.wait() {
                    Ok(exit) => {
                        let _ = memory_scope::terminal(&owned.child.0, true, exit.code(), true);
                        json!({"owner":owner,"pid":pid,"waited":true,"success":exit.success(),"code":exit.code(),"normal_stop":owned.normal.as_ref().map(normal::Stop::facts),"counter_outcome":"unresolved until stopped-source review"})
                    }
                    Err(error) => {
                        json!({"owner":owner,"pid":pid,"waited":false,"error":error.to_string(),"counter_outcome":"unresolved"})
                    }
                };
                self.outcomes.push(outcome);
            }
        }
        for stopped in self.pending_stops.iter().flatten() {
            if let Some(outcome) = self.outcomes.get_mut(stopped.outcome) {
                outcome["counter_outcome"] = "unresolved stopped-source reconciliation".into();
            }
        }
        // A failure cannot become PASS through Drop. Keep the actual cleanup
        // results for the future whole-run finalizer and independent review.
        if let Ok(raw) = serde_json::to_vec(
            &json!({"classification":"S54 owned cleanup; not whole PASS","outcomes":self.outcomes}),
        ) {
            let _ = std::fs::write(self.origin.pair.base.join("owner-cleanup.json"), raw);
        }
    }
}
impl Controller {
    pub(super) fn migration_sources(&self) -> MigrationSources<'_> {
        MigrationSources::new(
            &self.origin.contexts,
            &self.migration,
            &self.rows,
            &self.intervals.migrations,
        )
    }
    pub fn current_intervals(&self) -> Result<intervals::Verified> {
        self.stopped_reconciled()?;
        self.intervals
            .verify(&self.operations.summary()?, &self.outcomes)
    }
    pub fn memory_diagnostics(&self) -> Result<Value> {
        memory_scope::diagnostic()
    }
    pub fn finish_memory_coverage(&mut self) -> Result<Value> {
        ensure!(
            self.phases.complete() && self.relay_outcome.is_some(),
            "memory final lifecycle boundary required"
        );
        self.stopped_reconciled()?;
        // The live-relay gate already admitted this pair before relay closure.
        // Recheck retained outcomes without reopening that lifecycle boundary.
        self.migration_sources()
            .normal_pair(self.last_normal.as_ref(), &self.outcomes)?;
        memory_scope::finish_coverage()
    }
    pub fn feasibility_normal_pair(&mut self) -> Result<[Closure; 2]> {
        self.stopped_reconciled()?;
        self.check()?;
        ensure!(
            self.origin
                .pair
                .base
                .file_name()
                .is_some_and(|n| n == "f-origin")
                && self.relay.is_none()
                && self.relay_outcome.is_none()
                && !self.phases.complete(),
            "F original namespace and actual normal closures, no P/relay claim"
        );
        let pair = self
            .migration_sources()
            .normal_pair(self.last_normal.as_ref(), &self.outcomes)?;
        for c in &pair {
            self.operations.closed(c.key, c.sequence, c.counts)?;
        }
        self.operations.summary()?;
        Ok(pair)
    }
    pub fn finish_feasibility_memory(&mut self) -> Result<Value> {
        let _measurement = s53_deadline::measure("memory_finish");
        self.feasibility_normal_pair()?;
        self.current_intervals()?;
        memory_scope::finish_coverage()
    }
    pub fn new(parent: &Path, until: Instant) -> Result<Self> {
        Self::new_for(parent, until, Purpose::WholeP)
    }
    pub fn new_feasibility(parent: &Path, until: Instant) -> Result<Self> {
        Self::new_for(parent, until, Purpose::FeasibilityF)
    }
    fn new_for(parent: &Path, until: Instant, purpose: Purpose) -> Result<Self> {
        s53_deadline::check()?;
        let available = std::thread::available_parallelism()?.get();
        let inherited =
            std::env::var("TOKIO_WORKER_THREADS")
                .map(Some)
                .or_else(|error| match error {
                    std::env::VarError::NotPresent => Ok(None),
                    other => Err(other),
                })?;
        runtime_workers(available, inherited.as_deref())?;
        ensure!(
            Instant::now() < until,
            "future continuous absolute deadline"
        );
        let capture = s54_capture::Guard::enter(until)?;
        let memory = memory_scope::Guard::enter(parent, until)?;
        let origin = match purpose {
            Purpose::WholeP => OldOrigin::create(parent)?,
            Purpose::FeasibilityF => OldOrigin::create_for(parent, purpose)?,
        };
        let controls = [
            Controls::new(&origin.pair.base, &origin.contexts[0], 0)?,
            Controls::new(&origin.pair.base, &origin.contexts[0], 1)?,
        ];
        Ok(Self {
            origin,
            controls,
            running: [None, None],
            relay: None,
            relay_outcome: None,
            pending_stops: [None, None],
            last_normal: None,
            operations: Default::default(),
            intervals: Default::default(),
            accounting: Default::default(),
            pages: Default::default(),
            phases: oracle::Phases { next: 0 },
            until,
            requests: BTreeMap::new(),
            original: [None, None],
            migration: [None, None],
            rows: BTreeMap::new(),
            frozen: None,
            counters: Default::default(),
            pre_q_heads: [0; 2],
            outcomes: vec![],
            old_overlap: vec![],
            _memory: memory,
            _capture: capture,
        })
    }
    pub fn check(&mut self) -> Result<()> {
        memory_scope::phase(self.phases.next.try_into()?)?;
        s53_deadline::check()?;
        ensure!(Instant::now() < self.until, "S54 same absolute deadline");
        if let Some(relay) = self.relay.as_mut() {
            relay.check()?;
        }
        for owner in 0..2 {
            if let Some(run) = self.running[owner].as_mut() {
                if let Some(stop) = run.normal.as_ref() {
                    stop.check(
                        run.decoder.normal_identity()?,
                        run.child.0.id(),
                        Instant::now(),
                    )?;
                }
                normal::observed_exit(
                    run.normal.as_ref(),
                    run.child.0.try_wait()?.map(|s| s.code()),
                )?;
                run.decoder.poll(
                    &run.log,
                    &mut self.operations,
                    &mut self.pages,
                    &mut self.pre_q_heads[owner],
                )?;
                if let Ok(key) = run.decoder.normal_identity() {
                    memory_scope::bind_current(&run.child.0, key.owner, key.nonce)?;
                }
                memory_scope::progress(&mut run.child.0)?;
            }
        }
        Ok(())
    }
    pub fn wait<T>(
        &mut self,
        seconds: u64,
        mut f: impl FnMut(&mut Self) -> Result<Option<T>>,
    ) -> Result<T> {
        let until = (Instant::now() + Duration::from_secs(seconds)).min(self.until);
        let _phase = s53_deadline::Phase::until(until)?;
        loop {
            self.check()?;
            if let Some(value) = f(self)? {
                self.check()?;
                return Ok(value);
            }
            ensure!(Instant::now() < until, "bounded S54 phase observation");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    pub fn stopped(&self, owner: usize) -> Result<()> {
        ensure!(
            owner < 2
                && self.running[owner].is_none()
                && self.origin.pair.children[owner].is_none(),
            "exclusive owned state directory"
        );
        Ok(())
    }
    pub fn stopped_reconciled(&self) -> Result<()> {
        for owner in 0..2 {
            self.stopped(owner)?;
        }
        reconciled_pair(
            self.running.each_ref().map(Option::is_some),
            self.pending_stops.each_ref().map(Option::is_some),
        )
    }
    pub fn view(&self, owner: usize, context: usize) -> Result<Node> {
        let n = &self.origin.pair.nodes[owner];
        let slot = if n.indexed {
            let ready = &self.rows[&format!("ready-{owner}")];
            oracle::ready_slot(ready, &self.origin.contexts[context])?
        } else {
            0
        };
        Ok(n.view(&self.origin.contexts[context], slot))
    }
    pub fn status(&mut self, owner: usize, context: usize) -> Result<Value> {
        self.check()?;
        let view = self.view(owner, context)?;
        let status = view.status()?;
        if view.indexed {
            self.counters[owner] = observations::stats(&status["stats"])?;
        }
        let row = view.context(&status)?.clone();
        ensure!(
            (!view.indexed || row["role"] == owner as u64)
                && row["context"] == self.origin.contexts[context],
            "observed role/C mapping"
        );
        Ok(row)
    }
    pub fn start(&mut self, owner: usize) -> Result<()> {
        let _measurement = s53_deadline::measure("runtime_start");
        self.stopped(owner)?;
        self.last_normal = None;
        normal::reopen(
            self.pending_stops[owner].is_some(),
            self.relay_outcome.is_some(),
        )?;
        self.controls[owner].write()?;
        let n = &self.origin.pair.nodes[owner];
        let (source_id, start_sources, endpoint) = if n.indexed {
            let phase = self.intervals.expected_current_start(owner.try_into()?)?;
            let (source, point, endpoint) = source::point(n, &self.origin.contexts, phase)?;
            (source, point, Some(endpoint))
        } else {
            ([0; 32], (0, 0), None)
        };
        let count = self.origin.pair.starts[owner]
            .checked_add(1)
            .context("owned lifetime ordinal overflow")?;
        let name = format!("s54-{owner}-{count}");
        let log = n.branch.base.join(format!("{name}.stderr.log"));
        for context in &self.origin.contexts {
            for extension in ["locator", "locator2"] {
                let path = n.locators.join(format!("{context}.{extension}"));
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
            }
        }
        if n.ipc.exists() {
            std::fs::remove_file(&n.ipc)?;
        }
        // command() removes all inherited test env. Set only this child's files;
        // never mutate the controller's process-wide environment.
        let mut cmd = command(&n.exe());
        cmd.args(["runtime-from-profile", "--profile-file"])
            .arg(&n.profile)
            .stdout(Stdio::from(std::fs::File::create(
                n.branch.base.join(format!("{name}.stdout.log")),
            )?))
            .stderr(Stdio::from(std::fs::File::create(&log)?));
        self.controls[owner].attach(&mut cmd, n.indexed);
        memory_scope::alias(&n.branch.state, &n.profile, &n.ipc, n.indexed)?;
        let child = OwnedChild::spawn(&mut cmd)?;
        let pid = child.0.id();
        let interval_start = self.intervals.started(intervals::Started {
            owner: owner as u8,
            lifetime: count,
            pid,
            endpoint,
        })?;
        self.running[owner] = Some(Running {
            child,
            log,
            decoder: observations::Decoder::new(
                pid,
                source_id,
                self.origin.contexts.clone(),
                owner as u8,
            ),
            start_sources,
            interval_start,
            normal: None,
        });
        self.origin.pair.starts[owner] = count;
        self.wait(15, |s| {
            Ok(s.origin.pair.nodes[owner].ipc.is_file().then_some(()))
        })
    }
    pub fn stop(&mut self, owner: usize) -> Result<()> {
        let _measurement = s53_deadline::measure("kill_wait_reconcile");
        self.kill_wait(owner)?;
        self.reconcile_stop(owner)
    }
    pub fn kill_wait(&mut self, owner: usize) -> Result<()> {
        if let Err(error) = self.check() {
            if let Some(run) = self.running.get_mut(owner).and_then(Option::as_mut)
                && run.normal.is_none()
                && memory_scope::observed(&run.child.0)
            {
                let _ = run.child.kill_wait();
            }
            return Err(error);
        }
        self.last_normal = None;
        ensure!(
            self.running
                .get(owner)
                .and_then(Option::as_ref)
                .is_some_and(|r| r.normal.is_none()),
            "normal stop cannot be accepted as a killed outcome"
        );
        ensure!(
            self.pending_stops[owner].is_none() && self.outcomes.len() < 32,
            "bounded unique outstanding stop"
        );
        let n = &self.origin.pair.nodes[owner];
        if n.indexed {
            // The last actual status is an outcome candidate, never an expected total.
            match n.status().and_then(|v| observations::stats(&v["stats"])) {
                Ok(counts) => self.counters[owner] = counts,
                Err(error) => {
                    if let Some(run) = self.running[owner].as_mut()
                        && memory_scope::observed(&run.child.0)
                    {
                        let _ = run.child.kill_wait();
                    }
                    return Err(error);
                }
            }
        }
        let run = self.running[owner]
            .as_mut()
            .context("known owned process")?;
        let pid = run.child.0.id();
        run.child.kill_wait()?;
        let exit = run
            .child
            .0
            .try_wait()?
            .context("waited actual child outcome")?;
        ensure!(!exit.success(), "intentional killed outcome");
        self.outcomes.push(json!({"owner":owner,"pid":pid,"waited":true,"success":exit.success(),"code":exit.code(),"counter_outcome":"pending stopped-source reconciliation"}));
        let current = self.intervals.starts[run.interval_start].endpoint.is_some();
        self.intervals.waited(
            run.interval_start,
            pid,
            if current {
                Some(run.decoder.normal_identity()?)
            } else {
                None
            },
            self.outcomes.len() - 1,
            exit.code(),
            intervals::WaitKind::Killed,
        )?;
        ensure!(self.outcomes.len() <= 32, "bounded process outcomes");
        let run = self.running[owner]
            .take()
            .context("retained known waited child")?;
        self.pending_stops[owner] = Some(Stopped {
            run,
            counts: self.counters[owner],
            outcome: self.outcomes.len() - 1,
        });
        s53_deadline::check()?;
        Ok(())
    }
    pub fn reconcile_both(&mut self) -> Result<()> {
        waited_pair(
            self.running.each_ref().map(Option::is_some),
            self.pending_stops.each_ref().map(Option::is_some),
        )?;
        self.reconcile_stop(0)?;
        self.reconcile_stop(1)
    }
    fn reconcile_stop(&mut self, owner: usize) -> Result<()> {
        let _measurement = s53_deadline::measure("stopped_source_reconcile");
        self.stopped(owner)?;
        ensure!(
            self.pending_stops[owner]
                .as_ref()
                .is_some_and(|p| p.run.normal.is_none()),
            "normal final requires read-only normal reconciliation"
        );
        let n = &self.origin.pair.nodes[owner];
        let pending = self.pending_stops[owner]
            .as_mut()
            .context("known stopped outcome to reconcile")?;
        let run = &mut pending.run;
        run.decoder.poll(
            &run.log,
            &mut self.operations,
            &mut self.pages,
            &mut self.pre_q_heads[owner],
        )?;
        if n.indexed {
            let stopped = source::end(n, &self.origin.contexts)?;
            run.decoder.close_killed(
                &mut self.operations,
                run.start_sources,
                stopped.clone(),
                pending.counts,
            )?;
            let prefix = run.decoder.interval_prefix()?;
            let gap = pending.counts.encrypt != prefix.counts.encrypt
                || pending.counts.decrypt != prefix.counts.decrypt;
            let sequence = prefix
                .sequence
                .checked_add(u64::from(gap))
                .context("killed closed sequence overflow")?;
            let closed = self.intervals.build(
                intervals::CloseInput {
                    start: run.interval_start,
                    prefix,
                    counts: pending.counts,
                    sequence,
                    end: &stopped,
                    kind: intervals::ClosureKind::Killed,
                },
                &self.operations,
            )?;
            self.intervals.retain(vec![closed])?;
        }
        if let Some(outcome) = self.outcomes.get_mut(pending.outcome) {
            outcome["counter_outcome"] = if n.indexed {
                "source/counter resolved"
            } else {
                "frozen process; outside current ledger"
            }
            .into();
        }
        self.pending_stops[owner] = None;
        Ok(())
    }
    pub fn import1(&mut self) -> Result<()> {
        for context in 0..2 {
            self.import_legacy(context)?;
        }
        Ok(())
    }
    pub fn import_legacy(&mut self, context: usize) -> Result<()> {
        self.wait(15, |s| {
            Ok(s.origin
                .pair
                .nodes
                .iter()
                .all(|n| {
                    n.locators
                        .join(format!("{}.locator", s.origin.contexts[context]))
                        .is_file()
                })
                .then_some(()))
        })?;
        for owner in 0..2 {
            self.check()?;
            self.view(owner, context)?
                .import(&self.view(1 - owner, context)?)?;
        }
        Ok(())
    }
    pub fn import2(&mut self) -> Result<()> {
        self.import2_for(0)
    }
    pub fn import2_for(&mut self, context: usize) -> Result<()> {
        ensure!(context < 2, "bounded logical Locator2 context");
        self.wait(15, |s| {
            Ok(s.origin
                .pair
                .nodes
                .iter()
                .all(|n| {
                    n.locators
                        .join(format!("{}.locator2", s.origin.contexts[context]))
                        .is_file()
                })
                .then_some(()))
        })?;
        for owner in 0..2 {
            self.check()?;
            let node = self.view(owner, context)?;
            let peer = self.view(1 - owner, context)?;
            cli(&current(), &borrowed::locator_arguments(&node, &peer)?)?;
            self.check()?;
        }
        self.check()
    }
    pub fn send(&mut self, owner: usize, number: u16) -> Result<Value> {
        self.send_context(owner, 0, number)
    }
    pub fn send_context(&mut self, owner: usize, context: usize, number: u16) -> Result<Value> {
        self.check()?;
        let request = request(owner, context, number)?;
        let raw = self
            .view(owner, context)?
            .send(request.numeric(), &request.body())?;
        let result: Value = serde_json::from_str(&raw)?;
        if let Some(previous) = self.requests.get(&request) {
            // Queued versus materialized replies may differ; the final request
            // association and exact source/body, rather than reply bytes, bind retry.
            ensure!(!previous.is_null(), "original accepted request");
        } else {
            self.requests.insert(request, result.clone());
        }
        Ok(result)
    }
    pub fn remember(&mut self, name: &str, mut facts: Value) -> Result<()> {
        ensure!(
            self.rows.len() < 64 && !self.rows.contains_key(name),
            "bounded unique phase witness"
        );
        public_fingerprints(&mut facts)?;
        self.rows.insert(name.into(), facts);
        Ok(())
    }
}
