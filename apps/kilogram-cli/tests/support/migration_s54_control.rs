//! Per-child controls and lifetime ownership for the same two original directories.
use super::*;
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
    pending_stops: [Option<Stopped>; 2],
    pub operations: oracle::Operations,
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
}
impl Drop for Controller {
    fn drop(&mut self) {
        // Error/cancellation cleanup must also work after an expired deadline.
        // It uses known owned handles only, never enumerates or kills by name.
        for (owner, run) in self.running.iter_mut().enumerate() {
            if let Some(mut owned) = run.take() {
                let pid = owned.child.0.id();
                let _ = owned.child.0.kill();
                let outcome = match owned.child.0.wait() {
                    Ok(exit) => {
                        json!({"owner":owner,"pid":pid,"waited":true,"success":exit.success(),"code":exit.code(),"counter_outcome":"unresolved until stopped-source review"})
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
    pub fn new(parent: &Path, until: Instant) -> Result<Self> {
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
        let origin = OldOrigin::create(parent)?;
        let controls = [
            Controls::new(&origin.pair.base, &origin.contexts[0], 0)?,
            Controls::new(&origin.pair.base, &origin.contexts[0], 1)?,
        ];
        Ok(Self {
            origin,
            controls,
            running: [None, None],
            pending_stops: [None, None],
            operations: Default::default(),
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
        })
    }
    pub fn check(&mut self) -> Result<()> {
        s53_deadline::check()?;
        ensure!(Instant::now() < self.until, "S54 same absolute deadline");
        for owner in 0..2 {
            if let Some(run) = self.running[owner].as_mut() {
                run.child.alive()?;
                run.decoder.poll(
                    &run.log,
                    &mut self.operations,
                    &mut self.pages,
                    &mut self.pre_q_heads[owner],
                )?;
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
        self.stopped(owner)?;
        ensure!(
            self.pending_stops[owner].is_none(),
            "previous incarnation reconciled before reopen"
        );
        self.controls[owner].write()?;
        let n = &self.origin.pair.nodes[owner];
        let (source_id, start_sources) = if n.indexed {
            source::point(n, &self.origin.contexts)?
        } else {
            ([0; 32], (0, 0))
        };
        let count = self.origin.pair.starts[owner] + 1;
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
        let child = OwnedChild(cmd.spawn()?);
        let pid = child.0.id();
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
        });
        self.origin.pair.starts[owner] = count;
        self.wait(15, |s| {
            Ok(s.origin.pair.nodes[owner].ipc.is_file().then_some(()))
        })
    }
    pub fn stop(&mut self, owner: usize) -> Result<()> {
        self.kill_wait(owner)?;
        self.reconcile_stop(owner)
    }
    pub fn kill_wait(&mut self, owner: usize) -> Result<()> {
        self.check()?;
        ensure!(
            self.pending_stops[owner].is_none() && self.outcomes.len() < 32,
            "bounded unique outstanding stop"
        );
        let n = &self.origin.pair.nodes[owner];
        if n.indexed {
            // The last actual status is an outcome candidate, never an expected total.
            self.counters[owner] = observations::stats(&n.status()?["stats"])?;
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
        self.stopped(owner)?;
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
                stopped,
                pending.counts,
            )?;
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
        self.wait(15, |s| {
            Ok(s.origin
                .pair
                .nodes
                .iter()
                .all(|n| {
                    n.locators
                        .join(format!("{}.locator2", s.origin.contexts[0]))
                        .is_file()
                })
                .then_some(()))
        })?;
        self.origin.pair.exchange2()?;
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
