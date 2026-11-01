//! Inactive until an exact managed case is allocated. Real public CLI children,
//! fixed images, one absolute deadline, and genuine frozen Engine origins only.
use super::*;
#[path = "migration_s53_capacity_review2.rs"]
mod capacity_review2;
#[allow(dead_code)] // Block1 helpers are not process-activated yet.
#[path = "migration_s54_p5.rs"]
mod final_p5;
#[cfg(windows)]
#[path = "migration_s53_g1_admission.rs"]
mod g1_admission;
#[cfg(windows)]
#[path = "migration_s53_g1_continue.rs"]
mod g1_continue;
#[cfg(windows)]
#[path = "migration_s53_g1_data.rs"]
mod g1_data;
#[path = "migration_s53_g1_prerequisite.rs"]
mod g1_prerequisite;
#[cfg(windows)]
// The pinned G1b1 verifier modules are private. Reuse their pure source in C1
// without editing that accepted helper or widening its visibility.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "migration_s53_g1_replay.rs"]
mod g1_replay;
#[cfg(windows)]
#[path = "migration_s53_g1_witness.rs"]
mod g1_witness;
#[path = "migration_s53_metadata_review2.rs"]
mod metadata_review2;
#[path = "migration_s53_two_context.rs"]
mod two_context;

// r29a inventory-reader additions BEGIN
const MAX_MANAGED_INVENTORY_BYTES: u64 = 2 * 1024 * 1024;

fn read_bounded_managed_inventory(reader: impl std::io::Read) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(reader, MAX_MANAGED_INVENTORY_BYTES + 1),
        &mut raw,
    )?;
    ensure!(
        raw.len() as u64 <= MAX_MANAGED_INVENTORY_BYTES,
        "bounded inventory"
    );
    Ok(raw)
}

fn read_managed_inventory(path: &Path) -> Result<Vec<u8>> {
    ensure!(path.is_absolute(), "bounded inventory");
    let file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= MAX_MANAGED_INVENTORY_BYTES,
        "bounded inventory"
    );
    read_bounded_managed_inventory(file)
}

#[cfg(test)]
mod inventory_envelope_tests {
    use super::*;

    #[test]
    fn file_reader_preserves_bytes_at_inclusive_boundaries() -> Result<()> {
        let parent = workspace().join(".tmp");
        std::fs::create_dir_all(&parent)?;
        let directory = tempfile::Builder::new()
            .prefix("r29a-inventory-")
            .tempdir_in(&parent)?;
        let limit = usize::try_from(MAX_MANAGED_INVENTORY_BYTES)?;
        for (index, size) in [limit - 1, limit, limit + 1].into_iter().enumerate() {
            let path = directory.path().join(format!("boundary-{index}.json"));
            let expected = vec![b'x'; size];
            std::fs::write(&path, &expected)?;
            let actual = read_managed_inventory(&path);
            if size <= limit {
                ensure!(actual? == expected, "exact file bytes at boundary {size}");
            } else {
                ensure!(actual.is_err(), "reject oversized file metadata");
            }
        }
        let path = directory.path().join("opaque-bytes.json");
        let expected: [u8; 5] = [0xef, 0xbb, 0xbf, 0xff, 0x00];
        std::fs::write(&path, expected)?;
        ensure!(
            read_managed_inventory(&path)? == expected,
            "reader preserves raw bytes; JSON and BOM policy are separate"
        );
        Ok(())
    }

    #[test]
    fn file_reader_rejects_relative_and_missing_paths() -> Result<()> {
        ensure!(
            read_managed_inventory(Path::new("r29a-relative-inventory.json")).is_err(),
            "reject relative inventory before opening"
        );
        let parent = workspace().join(".tmp");
        std::fs::create_dir_all(&parent)?;
        let directory = tempfile::Builder::new()
            .prefix("r29a-inventory-missing-")
            .tempdir_in(&parent)?;
        let missing = directory.path().join("missing.json");
        ensure!(
            read_managed_inventory(&missing).is_err() && !missing.exists(),
            "missing absolute inventory fails without creating it"
        );
        Ok(())
    }

    #[test]
    fn bounded_reader_rejects_growth_after_metadata_at_limit_plus_one() -> Result<()> {
        struct ControlledRead {
            remaining: u64,
            consumed: u64,
        }
        impl std::io::Read for ControlledRead {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let count = usize::try_from(self.remaining.min(buffer.len() as u64))
                    .map_err(std::io::Error::other)?;
                buffer[..count].fill(b'x');
                self.remaining -= count as u64;
                self.consumed += count as u64;
                Ok(count)
            }
        }
        let mut growing = ControlledRead {
            remaining: MAX_MANAGED_INVENTORY_BYTES + 4096,
            consumed: 0,
        };
        ensure!(
            read_bounded_managed_inventory(&mut growing).is_err(),
            "actual post-read size exceeds limit despite earlier bounded metadata"
        );
        ensure!(
            growing.consumed == MAX_MANAGED_INVENTORY_BYTES + 1 && growing.remaining == 4095,
            "real bounded helper reads exactly one sentinel byte, never the rest"
        );
        let mut exact = ControlledRead {
            remaining: MAX_MANAGED_INVENTORY_BYTES,
            consumed: 0,
        };
        ensure!(
            read_bounded_managed_inventory(&mut exact)?.len() as u64 == MAX_MANAGED_INVENTORY_BYTES
                && exact.consumed == MAX_MANAGED_INVENTORY_BYTES,
            "real bounded helper accepts exact limit"
        );
        Ok(())
    }

    #[test]
    fn bounded_reader_propagates_read_errors() {
        struct FailedRead;
        impl std::io::Read for FailedRead {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("r29a controlled read failure"))
            }
        }
        assert!(
            matches!(
                read_bounded_managed_inventory(FailedRead),
                Err(error) if error.to_string().contains("r29a controlled read failure")
            ),
            "IO error must not return partial accepted bytes"
        );
    }
}
// r29a inventory-reader additions END

struct Managed {
    case: String,
    run: PathBuf,
    provenance: attempt_result::Provenance,
    _deadline: s53_deadline::Guard,
}
impl Managed {
    fn new(case: &str, seconds: u64) -> Result<Self> {
        ensure!(
            std::env::var("KILOGRAM_S53_CASE")? == case,
            "managed exact case"
        );
        let path = PathBuf::from(
            std::env::var_os("KILOGRAM_S53_INVENTORY").context("bound input inventory")?,
        );
        let raw = read_managed_inventory(&path)?;
        let input: Value = serde_json::from_slice(&raw)?;
        let identity = std::env::var("KILOGRAM_S53_INVENTORY_ID")?;
        ensure!(
            input["classification"] == "bound_pre_run_inventory"
                && input["case"] == case
                && input["inventory_identity"] == identity
                && input["baseline"] == "64c2359788ead360314793e47741adacb83b8d4f"
                && input["sources"].as_array().is_some_and(|v| !v.is_empty())
                && input["images"].as_array().is_some_and(|v| !v.is_empty()),
            "real managed binding"
        );
        let run = PathBuf::from(input["run"].as_str().context("run directory")?);
        ensure!(
            run.is_absolute() && run.is_dir(),
            "absolute existing managed run directory"
        );
        let attempt = input["attempt"]
            .as_str()
            .context("managed attempt identity")?;
        let (attempt_case, ordinal) = attempt
            .rsplit_once("-attempt-")
            .context("managed case/attempt shape")?;
        ensure!(
            attempt_case == case
                && ordinal.len() == 2
                && ordinal.bytes().all(|b| b.is_ascii_digit()),
            "exact managed case/attempt identity"
        );
        // Compare physical path components in the same Windows path form. Keep
        // the original inventory path for provenance/outputs, not a stripped or
        // rewritten verbatim prefix. The intended root may not escape workspace.
        let workspace_root = workspace().canonicalize()?;
        let owned_root = workspace_root.join(".tmp").canonicalize()?;
        ensure!(
            owned_root.is_dir()
                && owned_root.starts_with(&workspace_root)
                && owned_root != workspace_root,
            "intended owned root remains inside workspace"
        );
        let canonical_run = run.canonicalize()?;
        let expected_run = owned_root
            .join("migration-s53-integrated")
            .join(attempt)
            .canonicalize()?;
        ensure!(
            canonical_run.is_dir()
                && canonical_run.starts_with(&owned_root)
                && canonical_run != owned_root
                && canonical_run == expected_run,
            "fresh owned test directory"
        );
        let deadline = s53_deadline::Guard::enter(&run, seconds)?;
        Ok(Self {
            case: case.into(),
            run,
            provenance: attempt_result::Provenance::BoundPreRunInventory {
                path,
                blake3: blake3::hash(&raw).to_hex().to_string(),
                identity,
            },
            _deadline: deadline,
        })
    }
    fn finish(self, mut observations: Value) -> Result<()> {
        s53_deadline::check()?;
        public_fingerprints(&mut observations)?;
        let output = self.run.join("assertions.json");
        attempt_result::persist_before_tail(
            &output,
            &attempt_result::Envelope {
                case: &self.case,
                run: &self.run,
                output: &output,
                provenance: &self.provenance,
                observations: &observations,
            },
            s53_deadline::check,
        )
    }
}
fn public_fingerprints(value: &mut Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            if let Some(keys) = object.remove("keys") {
                object.insert(
                    "keys_fingerprint".into(),
                    blake3::hash(&serde_json::to_vec(&keys)?)
                        .to_hex()
                        .to_string()
                        .into(),
                );
            }
            for child in object.values_mut() {
                public_fingerprints(child)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                public_fingerprints(child)?;
            }
        }
        _ => (),
    }
    Ok(())
}
struct Pair {
    nodes: [Node; 2],
    roots: [PathBuf; 2],
    children: [Option<OwnedChild>; 2],
    base: PathBuf,
    c: String,
    starts: [u64; 2],
}
impl Drop for Pair {
    fn drop(&mut self) {
        self.children = [None, None];
    }
}
impl Pair {
    fn fresh(parent: &Path, name: &str) -> Result<Self> {
        let base = parent.join(name);
        std::fs::create_dir(&base)?;
        let (old, c) = old_pair(&base)?;
        let mut a = spawn(&frozen(), &old[0].runtime(), &base, "old-a", None, None)?;
        let mut b = spawn(&frozen(), &old[1].runtime(), &base, "old-b", None, None)?;
        wait_file(&mut a, &old[0].ipc, 15)?;
        wait_file(&mut b, &old[1].ipc, 15)?;
        import_peer(&old[0], &old[1], &c)?;
        import_peer(&old[1], &old[0], &c)?;
        isolation::old_queue(&old[0], &c, 1)?;
        isolation::old_queue(&old[1], &c, 2)?;
        wait_old(&old[0], &mut a, &mut b, |s| {
            s["phase"] == 3 && s["send_next"] == 2 && s["receive_next"] == 2 && s["unacked"] == 0
        })?;
        wait_old(&old[1], &mut b, &mut a, |s| {
            s["phase"] == 3 && s["send_next"] == 2 && s["receive_next"] == 2 && s["unacked"] == 0
        })?;
        a.kill_wait()?;
        b.kill_wait()?;
        let mut nodes = [Node::from_old(&old[0], &c)?, Node::from_old(&old[1], &c)?];
        for node in &mut nodes {
            node.migrate()?;
        }
        let roots = old.map(|node| node.root);
        let mut pair = Self {
            nodes,
            roots,
            children: [None, None],
            base,
            c,
            starts: [0, 0],
        };
        pair.start(0, None)?;
        pair.start(1, None)?;
        pair.ceremony("initial", "absent", 30 * 86400)?;
        Ok(pair)
    }
    fn start(&mut self, role: usize, schedule: Option<&Path>) -> Result<()> {
        ensure!(
            self.children[role].is_none(),
            "no overlapping Native runtime"
        );
        self.starts[role] += 1;
        self.children[role] = Some(self.nodes[role].start_controlled(
            &format!("live-{role}-{}", self.starts[role]),
            schedule,
            None,
        )?);
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        for child in &mut self.children {
            if let Some(mut owned) = child.take() {
                owned.kill_wait()?;
            }
        }
        Ok(())
    }
    fn row(&self, role: usize) -> Result<Value> {
        let status = self.nodes[role].status()?;
        Ok(self.nodes[role].context(&status)?.clone())
    }
    fn wait(
        &mut self,
        role: usize,
        seconds: u64,
        condition: impl Fn(&Value) -> bool,
    ) -> Result<Value> {
        let limit = Instant::now() + Duration::from_secs(seconds);
        loop {
            s53_deadline::check()?;
            for child in self.children.iter_mut().flatten() {
                child.alive()?;
            }
            let value = self.row(role)?;
            if condition(&value) {
                return Ok(value);
            }
            ensure!(
                Instant::now() < limit,
                "bounded real process observation {value}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn exchange2(&mut self) -> Result<()> {
        for role in 0..2 {
            let node = &self.nodes[role];
            let peer = &self.nodes[1 - role];
            let locator = peer.locators.join(format!("{}.locator2", self.c));
            if let Some(child) = self.children[1 - role].as_mut() {
                wait_file(child, &locator, 15)?;
            } else {
                ensure!(
                    locator.is_file(),
                    "externally owned cluster locator publication"
                );
            }
            cli(
                &current(),
                &[
                    "ordered".into(),
                    "locator2-import".into(),
                    "--existing".into(),
                    "--ipc-file".into(),
                    text(&node.ipc),
                    "--slot".into(),
                    node.slot.to_string(),
                    "--locator-file".into(),
                    text(&locator),
                ],
            )?;
        }
        Ok(())
    }
    fn command(&self, role: usize, name: &str, extra: &[String]) -> Result<String> {
        let node = &self.nodes[role];
        let mut args = vec![
            "ordered".into(),
            name.into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--slot".into(),
            node.slot.to_string(),
        ];
        args.extend_from_slice(extra);
        cli(&current(), &args)
    }
    fn upgrade(&self, role: usize, id: u8) -> Result<()> {
        self.command(
            role,
            "upgrade",
            &["--request-id".into(), format!("{id:02x}").repeat(16)],
        )?;
        Ok(())
    }
    fn ceremony(&self, name: &str, expected: &str, ttl: u64) -> Result<()> {
        let origin = self.base.join(format!("{name}.origin.cbor"));
        cli(
            &current(),
            &[
                "ordered".into(),
                "existing-origin-export".into(),
                "--profile".into(),
                text(&self.nodes[0].profile),
                "--context".into(),
                self.c.clone(),
                "--output-file".into(),
                text(&origin),
            ],
        )?;
        let authorities = [
            policy::snapshot(&self.base, &self.roots[0], &format!("{name}.a.authority"))?,
            policy::snapshot(&self.base, &self.roots[1], &format!("{name}.b.authority"))?,
        ];
        let proposal = self.base.join(format!("{name}.proposal"));
        let consent = self.base.join(format!("{name}.consent"));
        for role in 0..2 {
            let mut args = vec![
                "ordered".into(),
                "consent2-create".into(),
                "--origin".into(),
                "existing".into(),
                "--step".into(),
                if role == 0 { "propose" } else { "cosign" }.into(),
                "--context".into(),
                self.c.clone(),
                "--existing-origin-file".into(),
                text(&origin),
                "--account-dir".into(),
                text(&self.roots[role]),
                "--expect-account".into(),
                self.nodes[role].branch.account.clone(),
                "--expect-peer-account".into(),
                self.nodes[1 - role].branch.account.clone(),
                "--authority-a-file".into(),
                text(&authorities[0]),
                "--authority-b-file".into(),
                text(&authorities[1]),
                "--minimum-authority-a-file".into(),
                text(&authorities[0]),
                "--minimum-authority-b-file".into(),
                text(&authorities[1]),
                "--valid-for-seconds".into(),
                ttl.to_string(),
                "--output-file".into(),
                text(if role == 0 { &proposal } else { &consent }),
            ];
            if role == 1 {
                args.extend(["--proposal-file".into(), text(&proposal)]);
            }
            policy::audit(&self.base, &current(), &args)?;
        }
        for role in 0..2 {
            self.command(
                role,
                "consent2-import",
                &[
                    "--existing".into(),
                    "--expected-current".into(),
                    expected.into(),
                    "--consent-file".into(),
                    text(&consent),
                ],
            )?;
        }
        Ok(())
    }
    fn converge(&mut self) -> Result<[Value; 2]> {
        self.wait(0, 45, |r| r["mode"] == "Active")?;
        self.wait(1, 45, |r| r["mode"] == "Active")?;
        for role in 0..2 {
            self.wait(role, 30, |r| {
                r["counts"]["queued"] == 0
                    && r["counts"]["pending"] == 0
                    && r["counts"]["debt"] == 0
                    && r["control_debt"] == false
            })?;
        }
        Ok([self.row(0)?, self.row(1)?])
    }
}
fn same_origin(before: &Value, after: &Value) -> Result<()> {
    for field in ["context", "d", "e", "sid"] {
        ensure!(before[field] == after[field], "same original {field}");
    }
    ensure!(
        before["native"]["keys"] == after["native"]["keys"],
        "same original public Native key identifiers"
    );
    Ok(())
}

#[test]
fn s53_control_loss_reopen_public() -> Result<()> {
    let name = std::env::var("KILOGRAM_S53_LOSS")?;
    let valid = ["P", "A", "K", "Committed"]
        .into_iter()
        .flat_map(|control| {
            ["before-commit", "after-commit", "before-publication"]
                .map(move |cut| format!("s53-{control}-{cut}"))
        })
        .collect::<Vec<_>>();
    ensure!(valid.contains(&name), "one exact real loss boundary");
    // r17 P-after-commit target-only BEGIN
    if matches!(
        name.as_str(),
        "s53-P-before-commit"
            | "s53-P-after-commit"
            | "s53-P-before-publication"
            | "s53-A-after-commit"
            | "s53-K-after-commit"
            | "s53-Committed-after-commit"
            | "s53-A-before-commit"
            | "s53-A-before-publication"
            | "s53-K-before-commit"
            | "s53-K-before-publication"
            | "s53-Committed-before-commit"
            | "s53-Committed-before-publication"
    ) {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum CutPolicy {
            BeforeCommit,
            AfterCommit,
            BeforePublication,
            AcceptAfterCommit,
            SelectAfterCommit,
            CommittedAfterCommit,
            AcceptBeforeCommit,
            AcceptBeforePublication,
            SelectBeforeCommit,
            SelectBeforePublication,
            CommittedBeforeCommit,
            CommittedBeforePublication,
        }
        impl CutPolicy {
            fn case(self) -> &'static str {
                match self {
                    Self::BeforeCommit => "F-s53-p-before-commit-01",
                    Self::AfterCommit => "F-s53-p-after-commit-01",
                    Self::BeforePublication => "F-s53-p-before-publication-01",
                    Self::AcceptAfterCommit => "F-s53-a-after-commit-01",
                    Self::SelectAfterCommit => "F-s53-k-after-commit-01",
                    Self::CommittedAfterCommit => "F-s53-committed-after-commit-01",
                    Self::AcceptBeforeCommit => "F-s53-a-before-commit-01",
                    Self::AcceptBeforePublication => "F-s53-a-before-publication-01",
                    Self::SelectBeforeCommit => "F-s53-k-before-commit-01",
                    Self::SelectBeforePublication => "F-s53-k-before-publication-01",
                    Self::CommittedBeforeCommit => "F-s53-committed-before-commit-01",
                    Self::CommittedBeforePublication => "F-s53-committed-before-publication-01",
                }
            }
            fn recovery_seconds(self) -> u64 {
                match self {
                    Self::BeforeCommit => 90,
                    Self::AfterCommit => 45,
                    Self::BeforePublication => 45,
                    Self::AcceptAfterCommit => 45,
                    Self::SelectAfterCommit => 45,
                    Self::CommittedAfterCommit => 45,
                    Self::AcceptBeforeCommit => 45,
                    Self::AcceptBeforePublication => 45,
                    Self::SelectBeforeCommit => 45,
                    Self::SelectBeforePublication => 45,
                    Self::CommittedBeforeCommit => 45,
                    Self::CommittedBeforePublication => 45,
                }
            }
        }
        let policy = match name.as_str() {
            "s53-P-before-commit" => CutPolicy::BeforeCommit,
            "s53-P-after-commit" => CutPolicy::AfterCommit,
            "s53-P-before-publication" => CutPolicy::BeforePublication,
            "s53-A-after-commit" => CutPolicy::AcceptAfterCommit,
            "s53-K-after-commit" => CutPolicy::SelectAfterCommit,
            "s53-Committed-after-commit" => CutPolicy::CommittedAfterCommit,
            "s53-A-before-commit" => CutPolicy::AcceptBeforeCommit,
            "s53-A-before-publication" => CutPolicy::AcceptBeforePublication,
            "s53-K-before-commit" => CutPolicy::SelectBeforeCommit,
            "s53-K-before-publication" => CutPolicy::SelectBeforePublication,
            "s53-Committed-before-commit" => CutPolicy::CommittedBeforeCommit,
            "s53-Committed-before-publication" => CutPolicy::CommittedBeforePublication,
            _ => anyhow::bail!("explicit control cut policy"),
        };
        let accept_after_commit = policy == CutPolicy::AcceptAfterCommit;
        let select_after_commit = policy == CutPolicy::SelectAfterCommit;
        let committed_after_commit = policy == CutPolicy::CommittedAfterCommit;
        let accept_before_commit = policy == CutPolicy::AcceptBeforeCommit;
        let accept_before_publication = policy == CutPolicy::AcceptBeforePublication;
        let select_before_commit = policy == CutPolicy::SelectBeforeCommit;
        let select_before_publication = policy == CutPolicy::SelectBeforePublication;
        let committed_before_commit = policy == CutPolicy::CommittedBeforeCommit;
        let committed_before_publication = policy == CutPolicy::CommittedBeforePublication;
        // Declared before Pair: its children drop/kill/wait before this report.
        // This is failure evidence only, never a functional assertion snapshot.
        struct PDiagnostics {
            started: Instant,
            data: Value,
            complete: bool,
        }
        impl Drop for PDiagnostics {
            fn drop(&mut self) {
                if self.complete {
                    return;
                }
                let mut snapshot = self.data.clone();
                snapshot["trace_files"] = trace_files(&self.data);
                if let Err(error) = public_fingerprints(&mut snapshot) {
                    eprintln!("s53_p_incomplete_diagnostics serialization={error:#}");
                    return;
                }
                match serde_json::to_string(&snapshot) {
                    Ok(raw) if raw.len() <= 256 * 1024 => {
                        eprintln!("s53_p_incomplete_public_diagnostics={raw}");
                    }
                    Ok(_) => eprintln!(
                        "s53_p_incomplete_diagnostics capped phase={} processes={}",
                        self.data["phase"], self.data["processes"]
                    ),
                    Err(error) => {
                        eprintln!("s53_p_incomplete_diagnostics serialization={error:#}");
                    }
                }
            }
        }
        fn note(diagnostics: &mut PDiagnostics, phase: &str, facts: Value) -> Result<()> {
            let elapsed = diagnostics.started.elapsed().as_secs_f64();
            let events = diagnostics.data["events"]
                .as_array_mut()
                .context("P phase events")?;
            ensure!(events.len() < 64, "bounded P phase events");
            events.push(json!({"phase":phase,"elapsed_seconds":elapsed,"facts":facts}));
            diagnostics.data["phase"] = phase.into();
            Ok(())
        }
        fn started(
            pair: &Pair,
            role: usize,
            phase: &str,
            diagnostics: &mut PDiagnostics,
        ) -> Result<()> {
            let pid = pair.children[role]
                .as_ref()
                .context("started child")?
                .0
                .id();
            let stem = format!("live-{role}-{}", pair.starts[role]);
            let record = json!({"role":role,"pid":pid,"phase":phase,
                "stdout":pair.nodes[role].branch.base.join(format!("{stem}.stdout.log")),
                "stderr":pair.nodes[role].branch.base.join(format!("{stem}.stderr.log"))});
            let processes = diagnostics.data["processes"]
                .as_array_mut()
                .context("P owned process records")?;
            ensure!(processes.len() < 8, "bounded owned P processes");
            processes.push(record.clone());
            note(diagnostics, phase, record)
        }
        fn trace_files(diagnostics: &Value) -> Value {
            let mut files = Vec::new();
            for process in diagnostics["processes"].as_array().into_iter().flatten() {
                for stream in ["stdout", "stderr"] {
                    let Some(path) = process[stream].as_str() else {
                        continue;
                    };
                    let observation = (|| -> Result<Value> {
                        let length = std::fs::metadata(path)?.len();
                        ensure!(length <= 1024 * 1024, "owned trace exceeds read cap");
                        let raw = std::fs::read(path)?;
                        ensure!(raw.len() <= 1024 * 1024, "owned trace grew beyond cap");
                        let tail = &raw[raw.len().saturating_sub(4096)..];
                        Ok(
                            json!({"bytes":raw.len(),"blake3":blake3::hash(&raw).to_hex().to_string(),
                            "tail":String::from_utf8_lossy(tail)}),
                        )
                    })();
                    files.push(match observation {
                        Ok(value) => json!({"path":path,"observation":value}),
                        Err(error) => json!({"path":path,"read_error":format!("{error:#}")}),
                    });
                }
            }
            json!(files)
        }
        fn public_document(path: &Path) -> Result<(Value, Value)> {
            ensure!(
                std::fs::metadata(path)?.len() <= 64 * 1024,
                "bounded public document"
            );
            let raw = std::fs::read(path)?;
            ensure!(raw.len() <= 64 * 1024, "public document read cap");
            let value = serde_json::from_slice(&raw)?;
            Ok((
                value,
                json!({"path":path,"bytes":raw.len(),
                "blake3":blake3::hash(&raw).to_hex().to_string()}),
            ))
        }
        fn publication_scope(pair: &Pair) -> Result<Value> {
            let mut profiles = Vec::new();
            for role in 0..2 {
                let (config, evidence) = public_document(&pair.nodes[role].profile)?;
                ensure!(
                    config["version"] == 3
                        && config["profile_mode"] == "ordered-dialog-indexed"
                        && config["account"] == pair.nodes[role].branch.account
                        && config["device"] == pair.nodes[role].branch.device
                        && config["enabled_contexts"] == json!([pair.c]),
                    "actual public profile enables only the original C, role{role}"
                );
                profiles.push(json!({"role":role,"slot":pair.nodes[role].slot,"document":evidence,
                    "version":config["version"],"profile_mode":config["profile_mode"],
                    "account":config["account"],"device":config["device"],"enabled_contexts":config["enabled_contexts"]}));
            }
            Ok(json!(profiles))
        }
        fn publication_trace(path: &Path, slot: u8) -> Result<Value> {
            ensure!(
                std::fs::metadata(path)?.len() <= 1024 * 1024,
                "complete publication trace cap"
            );
            let raw = std::fs::read_to_string(path)?;
            ensure!(
                raw.len() <= 1024 * 1024,
                "complete publication trace read cap"
            );
            let complete_bytes = raw.rfind('\n').map_or(0, |end| end + 1);
            ensure!(complete_bytes > 0, "nonempty complete owned trace prefix");
            let prefix = &raw[..complete_bytes];
            let mut rpc_records = 0;
            let mut proposes = 0;
            for line in prefix.lines() {
                let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
                if fields.first().copied() != Some("s53_rpc") {
                    continue;
                }
                ensure!(fields.len() == 3, "complete exact s53_rpc record");
                let actual_slot: u8 = fields[1]
                    .strip_prefix("slot=")
                    .context("RPC trace slot")?
                    .parse()?;
                let kind = fields[2].strip_prefix("kind=").context("RPC trace kind")?;
                ensure!(
                    actual_slot == slot && !kind.is_empty(),
                    "single actual C slot in owned RPC trace"
                );
                rpc_records += 1;
                if kind == "Propose" {
                    proposes += 1;
                }
            }
            ensure!(
                proposes == 0,
                "no completed Propose trace record before first held publication"
            );
            Ok(
                json!({"path":path,"actual_slot":slot,"total_read_bytes":raw.len(),
                "complete_prefix_bytes":complete_bytes,"partial_suffix_bytes":raw.len()-complete_bytes,
                "complete_prefix_blake3":blake3::hash(prefix.as_bytes()).to_hex().to_string(),
                "complete_records":prefix.lines().count(),"rpc_records":rpc_records,"propose_records":proposes,
                "claim":"supporting complete-prefix evidence before this held write; not global packet silence"}),
            )
        }
        fn saved_offering_state(
            row: &Value,
            original: &Value,
            c: &str,
            slot: u8,
            p: &Value,
        ) -> Result<()> {
            origin(row, original, 0, c)?;
            controls(row, false)?;
            ensure!(
                row["slot"] == slot
                    && row["mode"] == "Offering"
                    && row["wire_stage"] == 2
                    && row["freeze"] == true
                    && row["control_debt"] == true
                    && row["native_encoding"] == "Legacy7"
                    && row["alpn1_allowed"] == false
                    && row["lease_current"] == false
                    && row["next"] == json!([2, 2])
                    && empty(row, 1)
                    && hex(p, 32)
                    && row["controls"]["p"] == *p
                    && ["a", "k", "committed"]
                        .iter()
                        .all(|key| row["controls"][*key].is_null())
                    && row.get("q").is_some_and(Value::is_null)
                    && row["cuts"] == json!([2, null])
                    && row["native_hash"] == original["native_hash"]
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "independent isolated saved publication P/permission/Native witness: {row}"
            );
            Ok(())
        }
        fn coalesced_p(status: &Value, p: &Value) -> Result<()> {
            ensure!(
                status.as_object().is_some_and(|fields| fields.len() == 1),
                "exact externally tagged CoalescedWithP"
            );
            let bytes = status["CoalescedWithP"]
                .as_array()
                .context("CoalescedWithP numeric bytes")?;
            ensure!(bytes.len() == 32, "CoalescedWithP ID length");
            let mut reference = String::with_capacity(64);
            for byte in bytes {
                let byte = byte
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .context("CoalescedWithP byte range")?;
                reference.push_str(&format!("{byte:02x}"));
            }
            ensure!(
                hex(p, 32) && p.as_str() == Some(reference.as_str()),
                "actual CoalescedWithP equals saved P"
            );
            Ok(())
        }
        fn saved_prepared_state(
            row: &Value,
            original: &Value,
            c: &str,
            slot: u8,
            boundary: &Value,
        ) -> Result<()> {
            origin(row, original, 1, c)?;
            controls(row, false)?;
            ensure!(
                row["slot"] == slot
                    && row["mode"] == "Prepared"
                    && row["wire_stage"] == 3
                    && row["freeze"] == true
                    && row["control_debt"] == true
                    && row["native_encoding"] == "Legacy7"
                    && row["native_hash"] == original["native_hash"]
                    && row["alpn1_allowed"] == false
                    && row["lease_current"] == false
                    && row["next"] == json!([2, 2])
                    && empty(row, 1)
                    && hex(&boundary["controls"]["p"], 32)
                    && hex(&boundary["controls"]["a"], 32)
                    && row["controls"] == boundary["controls"]
                    && row.get("q").is_some_and(Value::is_null)
                    && row["cuts"] == json!([2, 2])
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "independent isolated saved Prepared P/Accept/permission/Native witness: {row}"
            );
            Ok(())
        }
        fn accept_cut_witness(
            pair: &mut Pair,
            role: usize,
            original: &Value,
            original_history: &Value,
            boundary: &Value,
        ) -> Result<Value> {
            ensure!(role < 2, "original Accept-cut witness role");
            ensure!(
                pair.children[1 - role].is_none(),
                "peer absent before Accept-cut witness"
            );
            let pid = pair.children[role]
                .as_ref()
                .context("isolated Accept-cut child")?
                .0
                .id();
            let before = op(
                None,
                "isolated Accept-cut row before status/history",
                || pair.row(role),
            )?;
            let raw = op(None, "isolated Accept-cut public upgrade-status", || {
                pair.command(role, "upgrade-status", &[])
            })?;
            ensure!(raw.len() <= 48 * 1024, "bounded Accept-cut upgrade-status");
            let status: Value =
                serde_json::from_str(&raw).context("actual Accept-cut status JSON")?;
            let history = page(pair, role, None)?;
            let after = op(None, "isolated Accept-cut row after history", || {
                pair.row(role)
            })?;
            for row in [&before, &after] {
                if role == 1 {
                    saved_prepared_state(row, original, &pair.c, pair.nodes[role].slot, boundary)?;
                } else {
                    saved_offering_state(
                        row,
                        original,
                        &pair.c,
                        pair.nodes[role].slot,
                        &boundary["controls"]["p"],
                    )?;
                }
                coalesced_p(&row["upgrade"], &boundary["controls"]["p"])?;
            }
            controls(&status, false)?;
            coalesced_p(&status["status"], &boundary["controls"]["p"])?;
            ensure!(
                status["version"] == 3
                    && status["slot"] == pair.nodes[role].slot
                    && status["context"] == pair.c
                    && status["mode"] == after["mode"]
                    && status["wire_stage"] == after["wire_stage"]
                    && status["controls"] == after["controls"]
                    && status.get("q").is_some_and(Value::is_null)
                    && status["cuts"] == after["cuts"]
                    && status["freeze"] == true
                    && status["control_debt"] == true
                    && status["native_encoding"] == "Legacy7"
                    && status["lease_current"] == false
                    && status.get("policy_error").is_some_and(Value::is_null)
                    && status["policy_paused"] == false
                    && status["receive_event2"] == false
                    && status["send_event2"] == false
                    && status["coordinator"] == (role == 0)
                    && history["F"] == json!([2, 2])
                    && history["items"] == original_history["items"],
                "actual isolated Accept-cut status/authority/history role{role}: {status}"
            );
            ensure!(
                pair.children[1 - role].is_none(),
                "peer absent through Accept-cut status/history witness"
            );
            op(None, "isolated Accept-cut child still alive", || {
                pair.children[role]
                    .as_mut()
                    .context("isolated Accept-cut child")?
                    .alive()
            })?;
            Ok(
                json!({"role":role,"pid":pid,"peer_absent":true,"before":before,
                "upgrade_status_raw":raw,"upgrade_status":status,"history":history,"after":after}),
            )
        }
        fn accept_cut_deaths(pair: &mut Pair) -> Result<Value> {
            let mut deaths = Vec::new();
            for role in [1, 0] {
                let mut child = pair.children[role]
                    .take()
                    .context("owned Accept-cut child before kill")?;
                let pid = child.0.id();
                op(None, "actual Accept-cut B then A kill/wait", || {
                    child.kill_wait()
                })?;
                deaths.push(json!({"role":role,"pid":pid,"kill_wait_completed":true}));
            }
            ensure!(
                pair.children.iter().all(Option::is_none),
                "Accept-cut B and A death waits complete"
            );
            Ok(json!(deaths))
        }
        fn accept_saved_refs(row: &Value, role: usize, boundary: &Value) -> Result<()> {
            ensure!(role < 2, "original saved Accept reference role");
            controls(row, false)?;
            ensure!(
                row["controls"]["p"] == boundary["controls"]["p"]
                    && (row["controls"]["a"] == boundary["controls"]["a"]
                        || role == 0 && row["controls"]["a"].is_null()),
                "saved P/Accept refs across original role{role} incarnations: {row}"
            );
            Ok(())
        }
        fn select_history_view(history: &Value, original: &Value) -> Result<()> {
            ensure!(
                history["version"] == 3
                    && original["version"] == 3
                    && hex(&original["view"], 32)
                    && hex(&history["view"], 32)
                    && history["view"] == original["view"],
                "Select cut retains this peer's actual public history view"
            );
            Ok(())
        }
        fn select_native(
            row: &Value,
            original: &Value,
            role: usize,
            boundary: &Value,
            converted: &mut Value,
        ) -> Result<()> {
            ensure!(role < 2, "original Select Native role");
            let old_generation = original["native_generation"]
                .as_u64()
                .context("original Native generation u64")?;
            let converted_generation = old_generation
                .checked_add(1)
                .context("Select Native generation overflow")?;
            let generation = row["native_generation"]
                .as_u64()
                .context("observed Native generation u64")?;
            ensure!(
                hex(&row["native_hash"], 32)
                    && hex(&original["native_hash"], 32)
                    && row["next"] == json!([2, 2])
                    && row["native"]["own_next"] == 2
                    && row["native"]["peer_next"] == 2
                    && row["cuts"] == json!([2, 2])
                    && empty(row, 1)
                    && row["alpn1_allowed"] == false,
                "Select pre-data Native/frontiers/empty ledgers role{role}: {row}"
            );
            if role == 1 && row["mode"] == "Prepared" {
                ensure!(
                    converted.is_null()
                        && row["wire_stage"] == 3
                        && row["freeze"] == true
                        && row["control_debt"] == true
                        && row["native_encoding"] == "Legacy7"
                        && generation == old_generation
                        && row["native_hash"] == original["native_hash"],
                    "B Prepared retains original Native before its sole conversion"
                );
                return Ok(());
            }
            ensure!(
                (row["mode"] == "Active" && row["wire_stage"] == 5 && row["freeze"] == false
                    || role == 0
                        && row["mode"] == "Selected"
                        && row["wire_stage"] == 4
                        && row["freeze"] == true
                        && row["control_debt"] == true)
                    && row["native_encoding"] == "Mixed7"
                    && generation == converted_generation
                    && row["native_hash"] != original["native_hash"]
                    && (role == 1 || row["native_hash"] == boundary["native_hash"]),
                "Select conversion once, with held A Native or first Active B Native: {row}"
            );
            if converted.is_null() {
                *converted = json!({"native_hash":row["native_hash"],
                    "native_generation":generation});
            } else {
                ensure!(
                    row["native_hash"] == converted["native_hash"]
                        && row["native_generation"] == converted["native_generation"],
                    "observed converted Native never changes before original400/401"
                );
            }
            Ok(())
        }
        fn select_refs(
            row: &Value,
            role: usize,
            boundary: &Value,
            saved_q: &mut Value,
        ) -> Result<()> {
            accept_saved_refs(row, role, boundary)?;
            let q = row.get("q").context("present Select Q field")?;
            let prepared_b = role == 1 && row["mode"] == "Prepared";
            ensure!(
                row["controls"]["a"] == boundary["controls"]["a"]
                    && (prepared_b
                        && row["controls"]["k"].is_null()
                        && row["controls"]["committed"].is_null()
                        && q.is_null()
                        || !prepared_b
                            && row["controls"]["k"] == boundary["controls"]["k"]
                            && q == &boundary["q"]
                            && hex(q, 32))
                    && (prepared_b
                        || row["mode"] == "Selected" && row["controls"]["committed"].is_null()
                        || row["mode"] == "Active" && hex(&row["controls"]["committed"], 32)),
                "held P/Accept/K/Q across original Select-cut role{role}: {row}"
            );
            if !saved_q.is_null() {
                ensure!(
                    q == &*saved_q,
                    "observed Q changed or disappeared role{role}"
                );
            } else if !q.is_null() {
                *saved_q = q.clone();
            }
            Ok(())
        }
        fn select_prepared_state(
            row: &Value,
            original: &Value,
            c: &str,
            slot: u8,
            boundary: &Value,
        ) -> Result<()> {
            origin(row, original, 1, c)?;
            accept_saved_refs(row, 1, boundary)?;
            let mut converted = Value::Null;
            select_native(row, original, 1, boundary, &mut converted)?;
            ensure!(
                row["slot"] == slot
                    && row["enabled"] == true
                    && row["mode"] == "Prepared"
                    && row["lease_current"] == false
                    && ["k", "committed"]
                        .iter()
                        .all(|key| row["controls"][*key].is_null())
                    && row.get("q").is_some_and(Value::is_null)
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "isolated pre-Select B Prepared/Legacy7/current authority: {row}"
            );
            Ok(())
        }
        fn select_selected_state(
            row: &Value,
            original: &Value,
            c: &str,
            slot: u8,
            boundary: &Value,
        ) -> Result<()> {
            origin(row, original, 0, c)?;
            controls(row, false)?;
            let mut converted = Value::Null;
            select_native(row, original, 0, boundary, &mut converted)?;
            ensure!(
                row["slot"] == slot
                    && row["enabled"] == true
                    && row["mode"] == "Selected"
                    && row["lease_current"] == false
                    && row["controls"] == boundary["controls"]
                    && row["q"] == boundary["q"]
                    && row["controls"].get("committed").is_some_and(Value::is_null)
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "isolated saved Select/Mixed7/Q/current authority on A: {row}"
            );
            Ok(())
        }
        fn select_cut_witness(
            pair: &mut Pair,
            role: usize,
            original: &Value,
            original_history: &Value,
            boundary: &Value,
            diagnostics: &mut PDiagnostics,
        ) -> Result<Value> {
            ensure!(role < 2, "original Select-cut witness role");
            ensure!(
                pair.children[1 - role].is_none(),
                "Select-cut peer absent before witness"
            );
            let pid = pair.children[role]
                .as_ref()
                .context("isolated Select-cut child")?
                .0
                .id();
            let before = op(
                None,
                "isolated Select-cut row before status/history",
                || pair.row(role),
            )?;
            diagnostics.data["select_cold_partial"][role] = json!({"role":role,"pid":pid,
                "peer_absent_before":true,"before":before});
            let raw = op(None, "isolated Select-cut public upgrade-status", || {
                pair.command(role, "upgrade-status", &[])
            })?;
            diagnostics.data["select_cold_partial"][role]["upgrade_status_raw"] =
                raw.clone().into();
            ensure!(raw.len() <= 48 * 1024, "bounded Select-cut upgrade-status");
            let status: Value =
                serde_json::from_str(&raw).context("actual Select-cut status JSON")?;
            diagnostics.data["select_cold_partial"][role]["upgrade_status"] = status.clone();
            let history = page(pair, role, None)?;
            diagnostics.data["select_cold_partial"][role]["history"] = history.clone();
            let after = op(None, "isolated Select-cut row after history", || {
                pair.row(role)
            })?;
            diagnostics.data["select_cold_partial"][role]["after"] = after.clone();
            for row in [&before, &after] {
                if role == 1 {
                    select_prepared_state(row, original, &pair.c, pair.nodes[role].slot, boundary)?;
                } else {
                    select_selected_state(row, original, &pair.c, pair.nodes[role].slot, boundary)?;
                }
                coalesced_p(&row["upgrade"], &boundary["controls"]["p"])?;
            }
            controls(&status, false)?;
            coalesced_p(&status["status"], &boundary["controls"]["p"])?;
            select_history_view(&history, original_history)?;
            ensure!(
                status["version"] == 3
                    && status["slot"] == pair.nodes[role].slot
                    && status["context"] == pair.c
                    && status["mode"] == after["mode"]
                    && status["wire_stage"] == after["wire_stage"]
                    && status["controls"] == after["controls"]
                    && status.get("q") == after.get("q")
                    && status["cuts"] == after["cuts"]
                    && status["freeze"] == true
                    && status["control_debt"] == true
                    && status["native_encoding"] == after["native_encoding"]
                    && status["lease_current"] == false
                    && status.get("policy_error").is_some_and(Value::is_null)
                    && status["policy_paused"] == false
                    && status["receive_event2"] == false
                    && status["send_event2"] == false
                    && status["coordinator"] == (role == 0)
                    && history["F"] == json!([2, 2])
                    && history["items"] == original_history["items"],
                "actual isolated Select-cut status/authority/full old history role{role}: {status}"
            );
            ensure!(
                pair.children[1 - role].is_none(),
                "Select-cut peer absent through witness"
            );
            op(None, "isolated Select-cut child still alive", || {
                pair.children[role]
                    .as_mut()
                    .context("isolated Select-cut child")?
                    .alive()
            })?;
            diagnostics.data["select_cold_partial"][role]["peer_absent_through"] = true.into();
            diagnostics.data["select_cold_partial"][role]["child_alive_after"] = true.into();
            Ok(
                json!({"role":role,"pid":pid,"peer_absent":true,"child_alive_after":true,
                "before":before,"upgrade_status_raw":raw,"upgrade_status":status,"history":history,"after":after}),
            )
        }
        fn committed_native(
            row: &Value,
            original: &Value,
            role: usize,
            boundary: &Value,
            saved: &mut Value,
        ) -> Result<()> {
            ensure!(role < 2, "original Committed Native role");
            origin(
                row,
                original,
                role,
                original["context"]
                    .as_str()
                    .context("original Committed C")?,
            )?;
            let generation = original["native_generation"]
                .as_u64()
                .context("original Committed Native generation u64")?
                .checked_add(1)
                .context("Committed Native generation overflow")?;
            let actual = row["native_generation"]
                .as_u64()
                .context("observed Committed Native generation u64")?;
            let debt = row["control_debt"]
                .as_bool()
                .context("observed boolean Committed control debt")?;
            ensure!(
                row["role"] == role as u64
                    && original["role"] == role as u64
                    && (row["mode"] == "Active"
                        && row["wire_stage"] == 5
                        && row["freeze"] == false
                        && (role == 1 || !debt)
                        || role == 0
                            && row["mode"] == "Selected"
                            && row["wire_stage"] == 4
                            && row["freeze"] == true
                            && debt)
                    && row["native_encoding"] == "Mixed7"
                    && actual == generation
                    && hex(&row["native_hash"], 32)
                    && hex(&original["native_hash"], 32)
                    && row["native_hash"] != original["native_hash"]
                    && (role == 0 || row["native_hash"] == boundary["native_hash"])
                    && row["next"] == json!([2, 2])
                    && row["native"]["own_next"] == 2
                    && row["native"]["peer_next"] == 2
                    && row["cuts"] == json!([2, 2])
                    && empty(row, 1)
                    && row["alpn1_allowed"] == false
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "Committed pre-data original+1 Mixed7/Native/ledgers role{role}: {row}"
            );
            if saved.is_null() {
                *saved = json!({"native_hash":row["native_hash"],
                    "native_generation":actual,"control_debt":debt});
            } else {
                let prior_debt = saved["control_debt"]
                    .as_bool()
                    .context("saved boolean Committed control debt")?;
                ensure!(
                    row["native_hash"] == saved["native_hash"]
                        && row["native_generation"] == saved["native_generation"]
                        && (prior_debt || !debt),
                    "Committed Native changed or cleared debt regressed before original400/401 role{role}"
                );
                saved["control_debt"] = debt.into();
            }
            Ok(())
        }
        fn committed_refs(
            row: &Value,
            role: usize,
            boundary: &Value,
            saved_q: &mut Value,
        ) -> Result<()> {
            select_refs(row, role, boundary, saved_q)?;
            let committed = row["controls"]
                .get("committed")
                .context("present Committed control field")?;
            ensure!(
                hex(&boundary["controls"]["committed"], 32)
                    && (committed == &boundary["controls"]["committed"]
                        || role == 0 && row["mode"] == "Selected" && committed.is_null())
                    && (role == 0 || row["mode"] == "Active"),
                "exact held Committed, or original A still Selected: {row}"
            );
            Ok(())
        }
        fn committed_active_state(
            row: &Value,
            original: &Value,
            c: &str,
            slot: u8,
            boundary: &Value,
        ) -> Result<()> {
            origin(row, original, 1, c)?;
            controls(row, true)?;
            let mut q = Value::Null;
            committed_refs(row, 1, boundary, &mut q)?;
            let mut native = Value::Null;
            committed_native(row, original, 1, boundary, &mut native)?;
            ensure!(
                row["slot"] == slot
                    && row["enabled"] == true
                    && row["mode"] == "Active"
                    && row["control_debt"] == true
                    && row["lease_current"] == false
                    && row["controls"] == boundary["controls"]
                    && row["q"] == boundary["q"]
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "isolated B Active/Mixed7/held Committed with outstanding control debt: {row}"
            );
            Ok(())
        }
        fn committed_selected_state(
            row: &Value,
            original: &Value,
            c: &str,
            slot: u8,
            boundary: &Value,
        ) -> Result<()> {
            origin(row, original, 0, c)?;
            let mut q = Value::Null;
            committed_refs(row, 0, boundary, &mut q)?;
            let mut native = Value::Null;
            committed_native(row, original, 0, boundary, &mut native)?;
            ensure!(
                row["slot"] == slot
                    && row["enabled"] == true
                    && row["mode"] == "Selected"
                    && row["lease_current"] == false
                    && row["controls"].get("committed").is_some_and(Value::is_null)
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "isolated A Selected/Mixed7 without the held B Committed: {row}"
            );
            Ok(())
        }
        fn committed_cut_witness(
            pair: &mut Pair,
            role: usize,
            original: &Value,
            original_history: &Value,
            boundary: &Value,
            diagnostics: &mut PDiagnostics,
        ) -> Result<Value> {
            ensure!(role < 2, "original Committed-cut witness role");
            ensure!(
                pair.children[1 - role].is_none(),
                "Committed-cut peer absent before witness"
            );
            let pid = pair.children[role]
                .as_ref()
                .context("isolated Committed-cut child")?
                .0
                .id();
            let before = op(
                None,
                "isolated Committed-cut row before status/history",
                || pair.row(role),
            )?;
            diagnostics.data["committed_cold_partial"][role] = json!({"role":role,"pid":pid,
                "peer_absent_before":true,"before":before});
            let raw = op(None, "isolated Committed-cut public upgrade-status", || {
                pair.command(role, "upgrade-status", &[])
            })?;
            diagnostics.data["committed_cold_partial"][role]["upgrade_status_raw"] =
                raw.clone().into();
            ensure!(
                raw.len() <= 48 * 1024,
                "bounded Committed-cut upgrade-status"
            );
            let status: Value =
                serde_json::from_str(&raw).context("actual Committed-cut status JSON")?;
            diagnostics.data["committed_cold_partial"][role]["upgrade_status"] = status.clone();
            let history = page(pair, role, None)?;
            diagnostics.data["committed_cold_partial"][role]["history"] = history.clone();
            let after = op(None, "isolated Committed-cut row after history", || {
                pair.row(role)
            })?;
            diagnostics.data["committed_cold_partial"][role]["after"] = after.clone();
            for row in [&before, &after] {
                if role == 1 {
                    committed_active_state(
                        row,
                        original,
                        &pair.c,
                        pair.nodes[role].slot,
                        boundary,
                    )?;
                } else {
                    committed_selected_state(
                        row,
                        original,
                        &pair.c,
                        pair.nodes[role].slot,
                        boundary,
                    )?;
                }
                coalesced_p(&row["upgrade"], &boundary["controls"]["p"])?;
            }
            controls(&status, false)?;
            coalesced_p(&status["status"], &boundary["controls"]["p"])?;
            select_history_view(&history, original_history)?;
            ensure!(
                status["version"] == 3
                    && status["slot"] == pair.nodes[role].slot
                    && status["context"] == pair.c
                    && status["mode"] == after["mode"]
                    && status["wire_stage"] == after["wire_stage"]
                    && status["controls"] == after["controls"]
                    && status.get("q") == after.get("q")
                    && status["cuts"] == after["cuts"]
                    && status["freeze"] == after["freeze"]
                    && status["control_debt"] == true
                    && status["native_encoding"] == "Mixed7"
                    && status["lease_current"] == false
                    && status.get("policy_error").is_some_and(Value::is_null)
                    && status["policy_paused"] == false
                    && status["receive_event2"] == false
                    && status["send_event2"] == false
                    && status["coordinator"] == (role == 0)
                    && history["F"] == json!([2, 2])
                    && history["items"] == original_history["items"],
                "actual isolated Committed-cut status/authority/full old history role{role}: {status}"
            );
            ensure!(
                pair.children[1 - role].is_none(),
                "Committed-cut peer absent through witness"
            );
            op(None, "isolated Committed-cut child still alive", || {
                pair.children[role]
                    .as_mut()
                    .context("isolated Committed-cut child")?
                    .alive()
            })?;
            diagnostics.data["committed_cold_partial"][role]["peer_absent_through"] = true.into();
            diagnostics.data["committed_cold_partial"][role]["child_alive_after"] = true.into();
            Ok(
                json!({"role":role,"pid":pid,"peer_absent":true,"child_alive_after":true,
                "before":before,"upgrade_status_raw":raw,"upgrade_status":status,"history":history,"after":after}),
            )
        }
        fn a_before_b_state(row: &Value, original: &Value, c: &str, slot: u8) -> Result<()> {
            origin(row, original, 1, c)?;
            controls(row, false)?;
            ensure!(
                row["slot"] == slot
                    && row["enabled"] == true
                    && row["mode"] == "L1"
                    && row["wire_stage"] == 1
                    && row["upgrade"] == "NotificationSentUnconfirmed"
                    && row["freeze"] == false
                    && row["control_debt"] == false
                    && row["native_encoding"] == "Legacy7"
                    && row["native_generation"].as_u64().is_some()
                    && row["native_generation"] == original["native_generation"]
                    && row["native_hash"] == original["native_hash"]
                    && row["native"]["own_next"] == 2
                    && row["native"]["peer_next"] == 2
                    && row["next"] == json!([2, 2])
                    && empty(row, 1)
                    && row["alpn1_allowed"] == true
                    && row["lease_current"] == false
                    && row.get("q").is_some_and(Value::is_null)
                    && row["cuts"] == json!([null, null])
                    && ["p", "a", "k", "committed"]
                        .iter()
                        .all(|key| row["controls"][*key].is_null())
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "isolated B before Accept storage retains its actual L1 intent/Native: {row}"
            );
            Ok(())
        }
        fn a_before_cut_witness(
            pair: &mut Pair,
            role: usize,
            original: &Value,
            original_history: &Value,
            diagnostics: &mut PDiagnostics,
        ) -> Result<Value> {
            ensure!(role < 2, "original A-before witness role");
            ensure!(
                pair.children[1 - role].is_none(),
                "A-before peer absent before cold witness"
            );
            let pid = pair.children[role]
                .as_ref()
                .context("isolated A-before child")?
                .0
                .id();
            let before = op(None, "A-before cold row before status/history", || {
                pair.row(role)
            })?;
            diagnostics.data["a_before_cold_partial"][role] = json!({
                "role":role,"pid":pid,"peer_absent_before":true,"before":before
            });
            let raw = op(None, "A-before cold public upgrade-status", || {
                pair.command(role, "upgrade-status", &[])
            })?;
            diagnostics.data["a_before_cold_partial"][role]["upgrade_status_raw"] =
                raw.clone().into();
            ensure!(raw.len() <= 48 * 1024, "bounded A-before upgrade-status");
            let status: Value =
                serde_json::from_str(&raw).context("actual A-before cold status JSON")?;
            diagnostics.data["a_before_cold_partial"][role]["upgrade_status"] = status.clone();
            let history = page(pair, role, None)?;
            diagnostics.data["a_before_cold_partial"][role]["history"] = history.clone();
            let after = op(None, "A-before cold row after history", || pair.row(role))?;
            diagnostics.data["a_before_cold_partial"][role]["after"] = after.clone();
            let p0 = if role == 0 {
                before["controls"]
                    .get("p")
                    .context("first actual cold A P field")?
                    .clone()
            } else {
                Value::Null
            };
            for row in [&before, &after] {
                if role == 1 {
                    a_before_b_state(row, original, &pair.c, pair.nodes[role].slot)?;
                } else {
                    saved_offering_state(row, original, &pair.c, pair.nodes[role].slot, &p0)?;
                    coalesced_p(&row["upgrade"], &p0)?;
                }
                ensure!(
                    row["enabled"] == true
                        && row["native_generation"].as_u64().is_some()
                        && row["native_generation"] == original["native_generation"]
                        && row["native"]["own_next"] == 2
                        && row["native"]["peer_next"] == 2,
                    "A-before cold original generation/cursors role{role}: {row}"
                );
            }
            controls(&status, false)?;
            if role == 0 {
                coalesced_p(&status["status"], &p0)?;
            } else {
                ensure!(
                    status["status"] == "NotificationSentUnconfirmed",
                    "actual cold B notification remains sent-unconfirmed"
                );
            }
            select_history_view(&history, original_history)?;
            ensure!(
                status["version"] == 3
                    && status["slot"] == pair.nodes[role].slot
                    && status["context"] == pair.c
                    && status["mode"] == after["mode"]
                    && status["wire_stage"] == after["wire_stage"]
                    && status["controls"] == after["controls"]
                    && status.get("q") == after.get("q")
                    && status["cuts"] == after["cuts"]
                    && status["freeze"] == after["freeze"]
                    && status["control_debt"] == after["control_debt"]
                    && status["native_encoding"] == "Legacy7"
                    && status["lease_current"] == false
                    && status.get("policy_error").is_some_and(Value::is_null)
                    && status["policy_paused"] == false
                    && status["receive_event2"] == false
                    && status["send_event2"] == false
                    && status["coordinator"] == (role == 0)
                    && history["F"] == json!([2, 2])
                    && history["items"] == original_history["items"],
                "actual A-before cold status/authority/history role{role}: {status}"
            );
            ensure!(
                pair.children[1 - role].is_none(),
                "A-before peer absent through cold status/history"
            );
            op(None, "A-before isolated child still alive", || {
                pair.children[role]
                    .as_mut()
                    .context("isolated A-before child")?
                    .alive()
            })?;
            diagnostics.data["a_before_cold_partial"][role]["peer_absent_through"] = true.into();
            diagnostics.data["a_before_cold_partial"][role]["child_alive_after"] = true.into();
            Ok(json!({
                "role":role,"pid":pid,"peer_absent":true,"child_alive_after":true,
                "before":before,"upgrade_status_raw":raw,"upgrade_status":status,
                "history":history,"after":after
            }))
        }
        fn a_before_progress(
            pair: &Pair,
            row: &Value,
            original: &Value,
            role: usize,
            p0: &Value,
            observed: &mut Value,
            shared: &mut Value,
        ) -> Result<()> {
            ensure!(role < 2, "original A-before progress role");
            origin(row, original, role, &pair.c)?;
            controls(row, false)?;
            let mode = row["mode"].as_str().context("A-before actual mode")?;
            let (stage, required, cuts, mixed, frozen, expected_debt) = match (role, mode) {
                (0, "Offering") => (
                    2,
                    [true, false, false, false],
                    json!([2, null]),
                    false,
                    true,
                    Some(true),
                ),
                (0, "Selected") => (
                    4,
                    [true, true, true, false],
                    json!([2, 2]),
                    true,
                    true,
                    Some(true),
                ),
                (0, "Active") => (5, [true; 4], json!([2, 2]), true, false, Some(false)),
                (1, "L1") => (
                    1,
                    [false; 4],
                    json!([null, null]),
                    false,
                    false,
                    Some(false),
                ),
                (1, "Prepared") => (
                    3,
                    [true, true, false, false],
                    json!([2, 2]),
                    false,
                    true,
                    Some(true),
                ),
                (1, "Active") => (5, [true; 4], json!([2, 2]), true, false, None),
                _ => anyhow::bail!("unreachable A-before role/stage role{role}: {row}"),
            };
            let debt = row["control_debt"]
                .as_bool()
                .context("A-before boolean control debt")?;
            ensure!(
                row["slot"] == pair.nodes[role].slot
                    && row["enabled"] == true
                    && row["wire_stage"] == stage
                    && row["freeze"].as_bool() == Some(frozen)
                    && expected_debt.is_none_or(|expected| debt == expected)
                    && row["cuts"] == cuts
                    && row["next"] == json!([2, 2])
                    && row["native"]["own_next"] == 2
                    && row["native"]["peer_next"] == 2
                    && empty(row, 1)
                    && row["alpn1_allowed"] == (role == 1 && stage == 1)
                    && ["policy_error", "handoff_policy_error"]
                        .iter()
                        .all(|key| row.get(*key).is_some_and(Value::is_null)),
                "A-before actual role/stage/authority/frontiers/debt role{role}: {row}"
            );
            for (key, present) in ["p", "a", "k", "committed"].into_iter().zip(required) {
                let value = row["controls"].get(key).context("A-before control field")?;
                ensure!(
                    if present {
                        hex(value, 32)
                    } else {
                        value.is_null()
                    },
                    "A-before explicit control shape role{role} {mode}/{key}: {row}"
                );
            }
            let q = row.get("q").context("A-before explicit Q field")?;
            ensure!(
                if mixed { hex(q, 32) } else { q.is_null() },
                "A-before actual stage/Q shape role{role}: {row}"
            );
            if !row["controls"]["p"].is_null() {
                ensure!(
                    hex(p0, 32) && row["controls"]["p"] == *p0,
                    "A-before retained first actual cold A P0 role{role}"
                );
                coalesced_p(&row["upgrade"], p0)?;
            } else {
                ensure!(
                    row["upgrade"] == "NotificationSentUnconfirmed",
                    "A-before B L1 retains its genuine notification"
                );
            }
            if observed.is_null() {
                *observed = json!({
                    "controls":{"p":null,"a":null,"k":null,"committed":null},
                    "q":null,"wire_stage":null,"converted_native":null,
                    "active_debt_cleared":false
                });
            }
            if shared.is_null() {
                *shared = json!({
                    "controls":{"p":null,"a":null,"k":null,"committed":null},"q":null
                });
            }
            if let Some(previous) = observed["wire_stage"].as_u64() {
                ensure!(
                    stage >= previous,
                    "A-before observed stage cannot regress role{role}"
                );
            }
            remember_controls(&mut observed["controls"], row)?;
            if !observed["q"].is_null() {
                ensure!(q == &observed["q"], "A-before saved Q changed role{role}");
            } else if !q.is_null() {
                observed["q"] = q.clone();
            }
            for key in ["p", "a", "k", "committed"] {
                let value = &row["controls"][key];
                if !value.is_null() {
                    if shared["controls"][key].is_null() {
                        shared["controls"][key] = value.clone();
                    } else {
                        ensure!(
                            value == &shared["controls"][key],
                            "A-before actually present cross-peer {key} conflict role{role}"
                        );
                    }
                }
            }
            if !q.is_null() {
                if shared["q"].is_null() {
                    shared["q"] = q.clone();
                } else {
                    ensure!(q == &shared["q"], "A-before present cross-peer Q conflict");
                }
            }
            let old_generation = original["native_generation"]
                .as_u64()
                .context("A-before original Native generation u64")?;
            let generation = row["native_generation"]
                .as_u64()
                .context("A-before observed Native generation u64")?;
            if mixed {
                ensure!(
                    row["native_encoding"] == "Mixed7"
                        && Some(generation) == old_generation.checked_add(1)
                        && row["native_hash"] != original["native_hash"],
                    "A-before sole conversion to own Mixed7/original+1 role{role}: {row}"
                );
                if observed["converted_native"].is_null() {
                    observed["converted_native"] = json!({
                        "native_hash":row["native_hash"],"native_generation":generation
                    });
                } else {
                    ensure!(
                        row["native_hash"] == observed["converted_native"]["native_hash"]
                            && row["native_generation"]
                                == observed["converted_native"]["native_generation"],
                        "A-before first own converted Native unchanged before400/401 role{role}"
                    );
                }
            } else {
                ensure!(
                    row["native_encoding"] == "Legacy7"
                        && generation == old_generation
                        && row["native_hash"] == original["native_hash"]
                        && observed["converted_native"].is_null(),
                    "A-before original Legacy Native until actual conversion role{role}: {row}"
                );
            }
            if role == 1 && stage == 5 {
                ensure!(
                    observed["active_debt_cleared"] != true || !debt,
                    "A-before B Active cleared debt cannot regress"
                );
                if !debt {
                    observed["active_debt_cleared"] = true.into();
                }
            }
            observed["wire_stage"] = stage.into();
            observed["mode"] = row["mode"].clone();
            observed["control_debt"] = debt.into();
            Ok(())
        }
        fn check(until: Option<Instant>, phase: &str) -> Result<()> {
            s53_deadline::check().with_context(|| phase.to_owned())?;
            ensure!(
                until.is_none_or(|end| Instant::now() < end),
                "{phase}: deadline"
            );
            Ok(())
        }
        fn op<T>(
            until: Option<Instant>,
            phase: &str,
            action: impl FnOnce() -> Result<T>,
        ) -> Result<T> {
            check(until, phase)?;
            let result = action().with_context(|| phase.to_owned());
            match (result, check(until, phase)) {
                (Ok(value), Ok(())) => Ok(value),
                (Err(error), Ok(())) => Err(error),
                (Ok(_), Err(deadline)) => Err(deadline),
                (Err(error), Err(deadline)) => {
                    Err(error.context(format!("post-operation deadline: {deadline:#}")))
                }
            }
        }
        fn hex(value: &Value, bytes: usize) -> bool {
            value
                .as_str()
                .is_some_and(|s| s.len() == bytes * 2 && s.bytes().all(|ch| ch.is_ascii_hexdigit()))
        }
        fn identity(row: &Value, role: usize, c: &str) -> Result<()> {
            ensure!(
                row["context"] == c
                    && hex(&row["context"], 32)
                    && row["role"] == role as u64
                    && row["phase"] == 3
                    && hex(&row["d"], 32)
                    && hex(&row["e"], 32)
                    && row["sid"].as_str().is_some_and(|s| !s.is_empty())
                    && row["native"]["sid"] == row["sid"]
                    && hex(&row["native_hash"], 32)
                    && row["native"]["keys"]
                        .as_array()
                        .is_some_and(|v| { v.len() == 3 && v.iter().all(|key| hex(key, 32)) })
                    && hex(&row["current_consent2"], 32),
                "present original identity/Native/permission role{role}: {row}"
            );
            Ok(())
        }
        fn origin(row: &Value, old: &Value, role: usize, c: &str) -> Result<()> {
            identity(row, role, c)?;
            same_origin(old, row)?;
            ensure!(
                row["current_consent2"] == old["current_consent2"],
                "original Consent2"
            );
            Ok(())
        }
        fn controls(row: &Value, complete: bool) -> Result<()> {
            for key in ["p", "a", "k", "committed"] {
                let value = row["controls"].get(key).context("present control field")?;
                ensure!(
                    hex(value, 32) || !complete && value.is_null(),
                    "control {key}"
                );
            }
            Ok(())
        }
        fn empty(row: &Value, receipts: u64) -> bool {
            row["counts"]["queued"] == 0
                && row["counts"]["pending"] == 0
                && row["counts"]["debt"] == 0
                && row["counts"]["receipts"] == json!([receipts, receipts])
        }
        fn live(row: &Value) -> bool {
            row["lease_current"] == true
                && row["lease_family"] == 2
                && row["lease_generation"].as_u64().is_some()
                && row["publication_ready"] == true
                && row["existing_origin_ready"] == true
                && [
                    "policy_error",
                    "handoff_policy_error",
                    "publication_error",
                    "origin_publication_error",
                ]
                .iter()
                .all(|key| row.get(*key).is_some_and(Value::is_null))
        }
        fn page(pair: &Pair, role: usize, until: Option<Instant>) -> Result<Value> {
            let page = op(until, "complete public history", || {
                pair.nodes[role].history(32, None)
            })?;
            ensure!(
                page["finished"] == true && page["items"].is_array(),
                "complete bounded history"
            );
            Ok(page)
        }
        fn alive(pair: &mut Pair, until: Instant) -> Result<()> {
            for child in &mut pair.children {
                op(Some(until), "owned setup child liveness", || {
                    child.as_mut().context("owned live child")?.alive()
                })?;
            }
            Ok(())
        }
        fn pause(until: Instant, phase: &str) -> Result<()> {
            check(Some(until), phase)?;
            let remaining = until
                .checked_duration_since(Instant::now())
                .context("remaining phase time")?;
            op(Some(until), phase, || {
                std::thread::sleep(remaining.min(Duration::from_millis(20)));
                Ok(())
            })
        }
        fn clean_setup(row: &Value, old: &Value, role: usize, c: &str, slot: u8) -> Result<()> {
            origin(row, old, role, c)?;
            controls(row, false)?;
            ensure!(
                row["slot"] == slot
                    && row["mode"] == "L1"
                    && row["upgrade"] == "NoLocalIntent"
                    && row["freeze"] == false
                    && row["native_encoding"] == "Legacy7"
                    && row["next"] == json!([2, 2])
                    && empty(row, 1)
                    && row["control_debt"] == false
                    && row.get("q").is_some_and(Value::is_null)
                    && row["cuts"] == json!([null, null])
                    && row["native_hash"] == old["native_hash"]
                    && ["p", "a", "k", "committed"]
                        .iter()
                        .all(|key| row["controls"][*key].is_null()),
                "original no-intent phase3 setup role{role}"
            );
            Ok(())
        }
        fn current_setup(row: &Value, family: u64) -> bool {
            row["lease_current"] == true
                && row["lease_family"] == family
                && row["lease_generation"].as_u64().is_some()
                && row["publication_ready"] == true
                && row["existing_origin_ready"] == true
                && [
                    "policy_error",
                    "handoff_policy_error",
                    "publication_error",
                    "origin_publication_error",
                ]
                .iter()
                .all(|key| row.get(*key).is_some_and(Value::is_null))
        }
        fn diagnostic(rows: &[Value; 2]) -> Value {
            json!(rows.iter().map(|row| json!({"slot":row["slot"],"mode":row["mode"],
                "upgrade":row["upgrade"],"family":row["lease_family"],"current":row["lease_current"],
                "local_generation":row["lease_generation"],"next":row["next"],"counts":row["counts"],
                "policy_error":row["policy_error"],"handoff_policy_error":row["handoff_policy_error"],
                "publication_error":row["publication_error"],"origin_publication_error":row["origin_publication_error"]}))
                .collect::<Vec<_>>())
        }
        fn warmup(
            pair: &mut Pair,
            old: &[Value; 2],
            old_history: &[Value; 2],
            family: u64,
            until: Instant,
        ) -> Result<Value> {
            let mut last = Value::Null;
            let result = (|| -> Result<Value> {
                loop {
                    alive(pair, until)?;
                    let before = [
                        op(Some(until), "setup A before history", || pair.row(0))?,
                        op(Some(until), "setup B before history", || pair.row(1))?,
                    ];
                    last = diagnostic(&before);
                    for (role, row) in before.iter().enumerate() {
                        clean_setup(row, &old[role], role, &pair.c, pair.nodes[role].slot)?;
                    }
                    if before.iter().all(|row| current_setup(row, family)) {
                        let history = [page(pair, 0, Some(until))?, page(pair, 1, Some(until))?];
                        for (role, now) in history.iter().enumerate() {
                            ensure!(
                                now["F"] == json!([2, 2])
                                    && now["items"] == old_history[role]["items"],
                                "complete unchanged original history family{family}/role{role}"
                            );
                        }
                        let after = [
                            op(Some(until), "setup A after history", || pair.row(0))?,
                            op(Some(until), "setup B after history", || pair.row(1))?,
                        ];
                        last = diagnostic(&after);
                        for (role, row) in after.iter().enumerate() {
                            clean_setup(row, &old[role], role, &pair.c, pair.nodes[role].slot)?;
                            ensure!(
                                current_setup(row, family)
                                    && row["lease_generation"] == before[role]["lease_generation"],
                                "same current LOCAL family{family} generation role{role} across history"
                            );
                        }
                        check(Some(until), "accept current original no-intent setup")?;
                        return Ok(
                            json!({"family":family,"before":before,"history":history,"after":after}),
                        );
                    }
                    pause(until, "remaining no-intent setup poll")?;
                }
            })();
            result.with_context(|| format!("family{family} setup; last safe public state={last}"))
        }
        fn probe(path: &Path, slot: u8, until: Instant) -> Result<(Option<u64>, Value)> {
            let raw = op(Some(until), "bounded owned A scheduler trace", || {
                ensure!(
                    std::fs::metadata(path)?.len() <= 1024 * 1024,
                    "scheduler trace cap"
                );
                let raw = std::fs::read_to_string(path)?;
                ensure!(raw.len() <= 1024 * 1024, "scheduler trace read cap");
                Ok(raw)
            })?;
            let complete = raw.rsplit_once('\n').map_or("", |(lines, _)| lines);
            let mut starts = Vec::new();
            let mut lines = Vec::new();
            for line in complete.lines() {
                let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
                if fields.first().copied() != Some("s53_scheduler") {
                    continue;
                }
                let actual: u8 = fields
                    .get(1)
                    .and_then(|f| f.strip_prefix("slot="))
                    .context("scheduler slot")?
                    .parse()?;
                if actual != slot {
                    continue;
                }
                let phase = fields
                    .get(2)
                    .and_then(|f| f.strip_prefix("phase="))
                    .context("scheduler phase")?;
                match phase {
                    "probe-start" | "ordinary-start" | "ordinary-finished" => {
                        ensure!(fields.len() == 4, "exact scheduler start/finish record");
                        let m: u64 = fields[3]
                            .strip_prefix("monotonic_ms=")
                            .context("scheduler stamp")?
                            .parse()?;
                        lines.push(line.to_owned());
                        if phase == "probe-start" {
                            starts.push(m);
                            ensure!(
                                starts.len() == 1,
                                "one first probe, no replacement opportunity: {line}"
                            );
                        }
                    }
                    "probe-resolved" => {
                        ensure!(
                            fields.len() == 5
                                && fields[3]
                                    .strip_prefix("mode=")
                                    .is_some_and(|s| !s.is_empty()),
                            "exact probe resolution record"
                        );
                        let _: u64 = fields[4]
                            .strip_prefix("monotonic_ms=")
                            .context("resolution stamp")?
                            .parse()?;
                        anyhow::bail!("first probe already resolved before trigger/hit: {line}");
                    }
                    _ => anyhow::bail!("unknown actual-slot scheduler record: {line}"),
                }
            }
            check(Some(until), "accept complete owned first-probe trace")?;
            let first = starts.first().copied();
            Ok((
                first,
                json!({"path":path,"slot":slot,"first_m":first,"lines":lines,
                "complete_prefix_blake3":blake3::hash(complete.as_bytes()).to_hex().to_string()}),
            ))
        }
        fn upgrade_reply(raw: &str) -> Result<Value> {
            ensure!(
                raw.len() <= 96 * 1024 + 3,
                "bounded upgrade JSON hex string"
            );
            let encoded: Value = serde_json::from_str(raw)?;
            let encoded = encoded.as_str().context("upgrade JSON hex string")?;
            ensure!(
                !encoded.is_empty()
                    && encoded.len() <= 96 * 1024
                    && encoded.len().is_multiple_of(2)
                    && encoded.bytes().all(|b| b.is_ascii_hexdigit()),
                "upgrade hex encoding"
            );
            let mut bytes = Vec::with_capacity(encoded.len() / 2);
            for pair in encoded.as_bytes().as_chunks::<2>().0 {
                bytes.push(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?);
            }
            let status = std::str::from_utf8(&bytes).context("upgrade status UTF8")?;
            Ok(serde_json::from_str(status)?)
        }
        fn source(page: &Value, role: usize, request: Option<u64>) -> Result<Option<Value>> {
            let request = request.map(|id| format!("fa00000000000000{id:016x}"));
            let found = page["items"]
                .as_array()
                .context("history items")?
                .iter()
                .filter(|row| {
                    row["role"] == role as u64
                        && request
                            .as_ref()
                            .map_or(row["ordinal"] == 1, |id| row["request"] == *id)
                })
                .collect::<Vec<_>>();
            ensure!(found.len() <= 1, "unique original source role{role}");
            Ok(found.first().map(|row| (*row).clone()))
        }
        fn event(row: &Value, format: u64, ordinal: u64) -> Result<()> {
            ensure!(
                hex(&row["id"], 32)
                    && hex(&row["request"], 16)
                    && row["format"] == format
                    && row["ordinal"] == ordinal
                    && row["text"].as_str().is_some_and(|s| !s.is_empty()),
                "real source shape: {row}"
            );
            Ok(())
        }
        fn isolated_intent(
            pair: &mut Pair,
            role: usize,
            original: &Value,
            original_history: &Value,
        ) -> Result<Value> {
            ensure!(
                pair.children[1 - role].is_none(),
                "peer dead before witness"
            );
            let pid = pair.children[role]
                .as_ref()
                .context("isolated child")?
                .0
                .id();
            let before = op(None, "isolated intent row before history", || {
                pair.row(role)
            })?;
            let raw = op(None, "isolated public upgrade-status", || {
                pair.command(role, "upgrade-status", &[])
            })?;
            ensure!(raw.len() <= 48 * 1024, "bounded isolated upgrade-status");
            let status: Value =
                serde_json::from_str(&raw).context("actual isolated status JSON")?;
            let history = page(pair, role, None)?;
            let after = op(None, "isolated intent row after history", || pair.row(role))?;
            let expected = if role == 1 {
                "NotificationSentUnconfirmed"
            } else {
                "PeerIntentAccepted"
            };
            for row in [&before, &after] {
                origin(row, original, role, &pair.c)?;
                controls(row, false)?;
                ensure!(
                    row["slot"] == pair.nodes[role].slot
                        && row["mode"] == "L1"
                        && row["wire_stage"] == 1
                        && row["upgrade"] == expected
                        && row["freeze"] == false
                        && row["native_encoding"] == "Legacy7"
                        && row["native_hash"] == original["native_hash"]
                        && row["alpn1_allowed"] == true
                        && row["control_debt"] == false
                        && row["next"] == json!([2, 2])
                        && empty(row, 1)
                        && row["lease_current"] == false
                        && row.get("q").is_some_and(Value::is_null)
                        && row["cuts"] == json!([null, null])
                        && ["p", "a", "k", "committed"]
                            .iter()
                            .all(|key| row["controls"][*key].is_null())
                        && ["policy_error", "handoff_policy_error"]
                            .iter()
                            .all(|key| row.get(*key).is_some_and(Value::is_null)),
                    "isolated durable intent/original permission role{role}: {row}"
                );
            }
            controls(&status, false)?;
            ensure!(
                status["version"] == 3
                    && status["slot"] == pair.nodes[role].slot
                    && status["context"] == pair.c
                    && status["status"] == expected
                    && status["mode"] == "L1"
                    && status["wire_stage"] == 1
                    && status["controls"] == after["controls"]
                    && status.get("q").is_some_and(Value::is_null)
                    && status["cuts"] == json!([null, null])
                    && status["freeze"] == false
                    && status["control_debt"] == false
                    && status["native_encoding"] == "Legacy7"
                    && status["lease_current"] == false
                    && status.get("policy_error").is_some_and(Value::is_null)
                    && status["policy_paused"] == false
                    && status["receive_event2"] == false
                    && status["send_event2"] == false
                    && status["coordinator"] == (role == 0)
                    && history["F"] == json!([2, 2])
                    && history["items"] == original_history["items"],
                "actual isolated status/permissions/full history role{role}: {status}"
            );
            ensure!(
                pair.children[1 - role].is_none(),
                "peer absent through witness"
            );
            op(None, "isolated witness child still alive", || {
                pair.children[role]
                    .as_mut()
                    .context("isolated child")?
                    .alive()
            })?;
            Ok(
                json!({"role":role,"pid":pid,"peer_absent":true,"before":before,
                "upgrade_status_raw":raw,"upgrade_status":status,"history":history,"after":after}),
            )
        }
        fn remember_controls(saved: &mut Value, row: &Value) -> Result<()> {
            controls(row, false)?;
            for key in ["p", "a", "k", "committed"] {
                if !saved[key].is_null() {
                    ensure!(
                        row["controls"][key] == saved[key],
                        "observed committed {key} changed"
                    );
                } else if !row["controls"][key].is_null() {
                    saved[key] = row["controls"][key].clone();
                }
            }
            Ok(())
        }
        fn stop(pair: &mut Pair) -> Result<Value> {
            let mut deaths = Vec::new();
            for role in 0..2 {
                let mut child = pair.children[role]
                    .take()
                    .context("owned child before kill")?;
                let pid = child.0.id();
                op(None, "actual ordered kill/wait", || child.kill_wait())?;
                deaths.push(json!({"role":role,"pid":pid,"kill_wait_completed":true}));
            }
            ensure!(
                pair.children.iter().all(Option::is_none),
                "both death waits complete"
            );
            Ok(json!(deaths))
        }
        fn request(
            pair: &Pair,
            role: usize,
            until: Option<Instant>,
        ) -> Result<(Option<String>, bool, Value)> {
            let id = 400 + role as u64;
            let raw = op(until, "original request-status3", || {
                pair.command(
                    role,
                    "request-status3",
                    &["--request-id".into(), format!("fa00000000000000{id:016x}")],
                )
            })?;
            let value: Value = serde_json::from_str(&raw)?;
            let outer = value.as_array().context("request option array")?;
            ensure!(outer.len() == 1, "original accepted request present");
            let fields = outer[0].as_array().context("request tuple")?;
            ensure!(fields.len() == 3, "request tuple shape");
            let queue = fields[0].as_array().context("queue option")?;
            let event = fields[1].as_array().context("event option")?;
            ensure!(
                queue.len() <= 1
                    && queue.first().is_none_or(|v| v.as_u64().is_some())
                    && event.len() <= 1
                    && event.first().is_none_or(|v| hex(v, 32))
                    && matches!(fields[2].as_u64(), Some(0 | 1)),
                "real request options"
            );
            let event = event.first().and_then(Value::as_str).map(str::to_owned);
            let delivered = fields[2] == 1;
            ensure!(
                (event.is_some() || queue.len() == 1) && (!delivered || event.is_some()),
                "accepted original event or queue, delivered requires event"
            );
            Ok((event, delivered, value))
        }
        let managed = Managed::new(policy.case(), 160)?;
        let mut diagnostics = PDiagnostics {
            started: Instant::now(),
            data: json!({"classification":if committed_before_publication {
                    "incomplete Committed-before-publication public diagnostics; no acceptance"
                } else if committed_before_commit {
                    "incomplete Committed-before-commit public diagnostics; no acceptance"
                } else if select_before_publication {
                    "incomplete Select-before-publication public diagnostics; no acceptance"
                } else if select_before_commit {
                    "incomplete Select-before-commit public diagnostics; no acceptance"
                } else if accept_before_publication {
                    "incomplete Accept-before-publication public diagnostics; no acceptance"
                } else if accept_before_commit {
                    "incomplete Accept-before-commit public diagnostics; no acceptance"
                } else if accept_after_commit {
                    "incomplete Accept controller public diagnostics; no acceptance"
                } else if select_after_commit {
                    "incomplete Select controller public diagnostics; no acceptance"
                } else if committed_after_commit {
                    "incomplete Committed controller public diagnostics; no acceptance"
                } else {
                    "incomplete P controller public diagnostics; no acceptance"
                },
                "case":policy.case(),"phase":"fresh pair","events":[],"processes":[]}),
            complete: false,
        };
        if select_after_commit {
            diagnostics.data["select_cold_partial"] = json!([null, null]);
        }
        if committed_after_commit {
            diagnostics.data["committed_cold_partial"] = json!([null, null]);
        }
        if accept_before_commit {
            diagnostics.data["a_before_cold_partial"] = json!([null, null]);
        }
        if accept_before_publication {
            diagnostics.data["select_cold_partial"] = json!([null, null]);
            diagnostics.data["a_before_cold_partial"] = json!([null, null]);
        }
        if select_before_commit {
            diagnostics.data["select_cold_partial"] = json!([null, null]);
            diagnostics.data["a_before_cold_partial"] = json!([null, null]);
        }
        if select_before_publication {
            diagnostics.data["select_cold_partial"] = json!([null, null]);
        }
        if committed_before_commit {
            diagnostics.data["select_cold_partial"] = json!([null, null]);
        }
        if committed_before_publication {
            diagnostics.data["committed_cold_partial"] = json!([null, null]);
        }
        let mut pair = op(None, "fresh original pair and public ceremony", || {
            Pair::fresh(&managed.run, "loss")
        })?;
        started(&pair, 0, "initial A ready", &mut diagnostics)?;
        started(&pair, 1, "initial B ready", &mut diagnostics)?;
        let original = [
            op(None, "original A", || pair.row(0))?,
            op(None, "original B", || pair.row(1))?,
        ];
        let original_history = [page(&pair, 0, None)?, page(&pair, 1, None)?];
        for role in 0..2 {
            identity(&original[role], role, &pair.c)?;
            controls(&original[role], false)?;
            ensure!(
                original[role]["mode"] == "L1"
                    && original[role]["next"] == json!([2, 2])
                    && empty(&original[role], 1)
                    && original[role]["upgrade"] == "NoLocalIntent"
                    && original[role].get("q").is_some_and(Value::is_null)
                    && original[role]["cuts"] == json!([null, null])
                    && original[role]["control_debt"] == false
                    && original[role]
                        .get("handoff_policy_error")
                        .is_some_and(Value::is_null)
                    && ["p", "a", "k", "committed"]
                        .iter()
                        .all(|key| original[role]["controls"][*key].is_null())
                    && original_history[role]["items"]
                        .as_array()
                        .is_some_and(|items| items.len() == 2),
                "real clean original pair"
            );
            if select_after_commit
                || committed_after_commit
                || accept_before_commit
                || accept_before_publication
                || select_before_commit
                || select_before_publication
                || committed_before_commit
                || committed_before_publication
            {
                select_history_view(&original_history[role], &original_history[role])?;
                ensure!(
                    original_history[role]["F"] == json!([2, 2])
                        && original[role]["native_generation"]
                            .as_u64()
                            .and_then(|generation| generation.checked_add(1))
                            .is_some(),
                    "Select baseline has an actual view and convertible Native generation"
                );
            }
            for lane in 0..2 {
                let old = source(&original_history[role], lane, None)?
                    .context("original readable Text")?;
                event(&old, 1, 1)?;
                ensure!(old["delivered"] == true, "original receipt");
                let peer = source(&original_history[1 - role], lane, None)?
                    .context("same old source at peer")?;
                ensure!(
                    old["id"] == peer["id"]
                        && old["request"] == peer["request"]
                        && old["text"] == peer["text"],
                    "original old history correspondence"
                );
            }
        }
        let publication_profiles = if policy == CutPolicy::BeforePublication {
            note(
                &mut diagnostics,
                "verify publication single enabled C",
                Value::Null,
            )?;
            let evidence = op(None, "bounded actual public publication profiles", || {
                publication_scope(&pair)
            })?;
            diagnostics.data["publication_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let accept_profiles = if accept_after_commit {
            note(
                &mut diagnostics,
                "verify Accept-cut single enabled C",
                Value::Null,
            )?;
            let evidence = op(None, "bounded actual Accept-cut public profiles", || {
                publication_scope(&pair)
            })?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Accept cut"
            );
            diagnostics.data["accept_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let select_profiles = if select_after_commit {
            note(
                &mut diagnostics,
                "verify Select-cut single enabled C",
                Value::Null,
            )?;
            let evidence = op(None, "bounded actual Select-cut public profiles", || {
                publication_scope(&pair)
            })?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Select cut"
            );
            diagnostics.data["select_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let committed_profiles = if committed_after_commit {
            note(
                &mut diagnostics,
                "verify Committed-cut single enabled C",
                Value::Null,
            )?;
            let evidence = op(None, "bounded actual Committed-cut public profiles", || {
                publication_scope(&pair)
            })?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Committed cut"
            );
            diagnostics.data["committed_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let a_before_profiles = if accept_before_commit {
            let evidence = op(None, "bounded actual A-before public profiles", || {
                publication_scope(&pair)
            })?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Accept storage"
            );
            diagnostics.data["a_before_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let a_publication_profiles = if accept_before_publication {
            let evidence = op(None, "bounded actual A-publication public profiles", || {
                publication_scope(&pair)
            })?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Accept publication"
            );
            diagnostics.data["a_publication_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let k_before_profiles = if select_before_commit {
            let evidence = op(None, "bounded actual K-before public profiles", || {
                publication_scope(&pair)
            })?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Select storage"
            );
            diagnostics.data["k_before_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let k_publication_profiles = if select_before_publication {
            let evidence = op(None, "bounded actual K-publication public profiles", || {
                publication_scope(&pair)
            })?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Select publication"
            );
            diagnostics.data["k_publication_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let committed_before_profiles = if committed_before_commit {
            let evidence = op(
                None,
                "bounded actual Committed-before public profiles",
                || publication_scope(&pair),
            )?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before CommitB storage"
            );
            diagnostics.data["committed_before_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let committed_publication_profiles = if committed_before_publication {
            let evidence = op(
                None,
                "bounded actual Committed-publication public profiles",
                || publication_scope(&pair),
            )?;
            ensure!(
                original[0]["current_consent2"] == original[1]["current_consent2"],
                "original shared current Consent2 before Committed publication"
            );
            diagnostics.data["committed_publication_profiles"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let initial_deaths = stop(&mut pair)?;
        note(
            &mut diagnostics,
            "initial deaths complete",
            initial_deaths.clone(),
        )?;
        let schedule = pair.base.join("loss.schedule.json");
        let b_schedule = pair.base.join("loss.b.trace.schedule.json");
        let held_schedule = if accept_after_commit
            || committed_after_commit
            || committed_before_commit
            || committed_before_publication
            || accept_before_commit
            || accept_before_publication
        {
            b_schedule.as_path()
        } else {
            schedule.as_path()
        };
        if accept_after_commit
            || committed_after_commit
            || committed_before_commit
            || committed_before_publication
            || accept_before_commit
            || accept_before_publication
        {
            op(
                None,
                if committed_after_commit || committed_before_commit || committed_before_publication
                {
                    "A trace-only schedule for Committed cut"
                } else {
                    "A trace-only schedule for Accept cut"
                },
                || schedule_file(&schedule, &json!({"context":pair.c,"trace_rpc":true})),
            )?;
            op(
                None,
                if committed_after_commit || committed_before_commit || committed_before_publication
                {
                    "arm exact B Committed control cut"
                } else {
                    "arm exact B Accept control cut"
                },
                || {
                    schedule_file(
                        &b_schedule,
                        &json!({"context":pair.c,"pause_point":name,"trace_rpc":true}),
                    )
                },
            )?;
        } else {
            op(
                None,
                if select_after_commit || select_before_commit || select_before_publication {
                    "arm exact A Select control cut"
                } else {
                    "arm exact P control cut"
                },
                || {
                    schedule_file(
                        &schedule,
                        &json!({"context":pair.c,"pause_point":name,"trace_rpc":true}),
                    )
                },
            )?;
            op(None, "B trace-only schedule", || {
                schedule_file(&b_schedule, &json!({"context":pair.c,"trace_rpc":true}))
            })?;
        }
        let publication_plan = if policy == CutPolicy::BeforePublication {
            let (plan, evidence) = op(None, "actual publication trace/hold plan", || {
                public_document(&schedule)
            })?;
            ensure!(
                plan["context"] == pair.c
                    && plan["pause_point"] == name
                    && plan["trace_rpc"] == true,
                "actual owned A plan arms publication and trace_rpc"
            );
            let observed = json!({"document":evidence,"context":plan["context"],
                "pause_point":plan["pause_point"],"trace_rpc":plan["trace_rpc"]});
            diagnostics.data["publication_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let accept_plan = if accept_after_commit {
            let (a_plan, a_document) = op(None, "actual Accept-cut A trace-only plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) = op(None, "actual Accept-cut B armed plan", || {
                public_document(&b_schedule)
            })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan.get("pause_point").is_none_or(Value::is_null)
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan["pause_point"] == name,
                "actual trace-only A and exact held-owner B plans"
            );
            let observed = json!({"victim_role":1,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}});
            diagnostics.data["accept_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let select_plan = if select_after_commit {
            let (a_plan, a_document) = op(None, "actual Select-cut A armed plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) = op(None, "actual Select-cut B trace-only plan", || {
                public_document(&b_schedule)
            })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan["pause_point"] == name
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan.get("pause_point").is_none_or(Value::is_null),
                "actual exact held-owner A and trace-only B plans"
            );
            let observed = json!({"victim_role":0,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}});
            diagnostics.data["select_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let committed_plan = if committed_after_commit {
            let (a_plan, a_document) = op(None, "actual Committed-cut A trace-only plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) = op(None, "actual Committed-cut B armed plan", || {
                public_document(&b_schedule)
            })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan.get("pause_point").is_none_or(Value::is_null)
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan["pause_point"] == name,
                "actual trace-only A and exact held-owner B Committed plans"
            );
            let observed = json!({"victim_role":1,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}});
            diagnostics.data["committed_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let a_before_plan = if accept_before_commit {
            let (a_plan, a_document) = op(None, "actual A-before A trace-only plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) = op(None, "actual A-before B armed plan", || {
                public_document(&b_schedule)
            })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan.get("pause_point").is_none_or(Value::is_null)
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan["pause_point"] == name,
                "actual A-before trace-only A and held B plans"
            );
            let observed = json!({
                "victim_role":1,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}
            });
            diagnostics.data["a_before_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let a_publication_plan = if accept_before_publication {
            let (a_plan, a_document) = op(None, "actual A-publication A trace-only plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) =
                op(None, "actual A-publication B held transport plan", || {
                    public_document(&b_schedule)
                })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan.get("pause_point").is_none_or(Value::is_null)
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan["pause_point"] == name,
                "actual A-publication trace-only A and held B transport plans"
            );
            let observed = json!({
                "victim_role":1,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}
            });
            diagnostics.data["a_publication_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let k_before_plan = if select_before_commit {
            let (a_plan, a_document) = op(None, "actual K-before A held plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) = op(None, "actual K-before B trace plan", || {
                public_document(&b_schedule)
            })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan["pause_point"] == name
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan.get("pause_point").is_none_or(Value::is_null),
                "actual K-before held-owner A and trace-only B plans"
            );
            let observed = json!({
                "victim_role":0,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}
            });
            diagnostics.data["k_before_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let k_publication_plan = if select_before_publication {
            let (a_plan, a_document) = op(None, "actual K-publication A held plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) = op(None, "actual K-publication B trace plan", || {
                public_document(&b_schedule)
            })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan["pause_point"] == name
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan.get("pause_point").is_none_or(Value::is_null),
                "actual K-publication held-transport A and trace-only B plans"
            );
            let observed = json!({
                "victim_role":0,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}
            });
            diagnostics.data["k_publication_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let committed_before_plan = if committed_before_commit {
            let (a_plan, a_document) = op(None, "actual Committed-before A trace plan", || {
                public_document(&schedule)
            })?;
            let (b_plan, b_document) = op(None, "actual Committed-before B held plan", || {
                public_document(&b_schedule)
            })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan.get("pause_point").is_none_or(Value::is_null)
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan["pause_point"] == name,
                "actual Committed-before trace-only A and held-owner B plans"
            );
            let observed = json!({
                "victim_role":1,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}
            });
            diagnostics.data["committed_before_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let committed_publication_plan = if committed_before_publication {
            let (a_plan, a_document) =
                op(None, "actual Committed-publication A trace plan", || {
                    public_document(&schedule)
                })?;
            let (b_plan, b_document) =
                op(None, "actual Committed-publication B held plan", || {
                    public_document(&b_schedule)
                })?;
            ensure!(
                a_plan["context"] == pair.c
                    && a_plan["trace_rpc"] == true
                    && a_plan.get("pause_point").is_none_or(Value::is_null)
                    && b_plan["context"] == pair.c
                    && b_plan["trace_rpc"] == true
                    && b_plan["pause_point"] == name,
                "actual trace-only A and exact B network-publication plans"
            );
            let observed = json!({
                "victim_role":1,
                "A":{"document":a_document,"context":a_plan["context"],
                    "trace_rpc":a_plan["trace_rpc"],"pause_point":a_plan["pause_point"]},
                "B":{"document":b_document,"context":b_plan["context"],
                    "trace_rpc":b_plan["trace_rpc"],"pause_point":b_plan["pause_point"]}
            });
            diagnostics.data["committed_publication_plan"] = observed.clone();
            observed
        } else {
            Value::Null
        };
        let a0 = Instant::now();
        op(
            None,
            if accept_after_commit
                || committed_after_commit
                || committed_before_commit
                || committed_before_publication
                || accept_before_commit
                || accept_before_publication
            {
                "start trace-only A"
            } else {
                "start armed A"
            },
            || pair.start(0, Some(&schedule)),
        )?;
        started(
            &pair,
            0,
            if accept_after_commit
                || committed_after_commit
                || committed_before_commit
                || committed_before_publication
                || accept_before_commit
                || accept_before_publication
            {
                "trace-only A ready"
            } else {
                "armed A ready"
            },
            &mut diagnostics,
        )?;
        op(
            None,
            if accept_after_commit
                || committed_after_commit
                || committed_before_commit
                || committed_before_publication
                || accept_before_commit
                || accept_before_publication
            {
                "start armed B"
            } else {
                "start B trace-only"
            },
            || pair.start(1, Some(&b_schedule)),
        )?;
        started(
            &pair,
            1,
            if accept_after_commit
                || committed_after_commit
                || committed_before_commit
                || committed_before_publication
                || accept_before_commit
                || accept_before_publication
            {
                "armed B ready"
            } else {
                "initial trigger B ready"
            },
            &mut diagnostics,
        )?;
        let a_trace = pair.nodes[0]
            .branch
            .base
            .join(format!("live-0-{}.stderr.log", pair.starts[0]));
        let b_trace = pair.nodes[1]
            .branch
            .base
            .join(format!("live-1-{}.stderr.log", pair.starts[1]));
        let a_slot = pair.nodes[0].slot;
        let setup_started = Instant::now();
        let shared_end = setup_started
            .checked_add(Duration::from_secs(45))
            .context("shared setup deadline overflow")?;
        op(Some(shared_end), "B imports current A Locator1", || {
            pair.nodes[1].import(&pair.nodes[0])
        })?;
        op(Some(shared_end), "A imports current B Locator1", || {
            pair.nodes[0].import(&pair.nodes[1])
        })?;
        let family1_setup = warmup(&mut pair, &original, &original_history, 1, shared_end)?;
        note(
            &mut diagnostics,
            "current family1 setup",
            family1_setup["after"].clone(),
        )?;
        op(
            Some(shared_end),
            "actual Locator2 exchange before intent",
            || pair.exchange2(),
        )?;
        let family2_setup = warmup(&mut pair, &original, &original_history, 2, shared_end)?;
        note(
            &mut diagnostics,
            "current family2 setup",
            family2_setup["after"].clone(),
        )?;
        let no_local = family2_setup["after"][0].clone();
        let (first_m, first_probe_before) = probe(&a_trace, a_slot, shared_end)?;
        let first_m = first_m.context("positive complete first A probe-start required")?;
        ensure!(
            first_m > 0 && u128::from(first_m) <= a0.elapsed().as_millis(),
            "owned A trace timestamp within parent lifetime"
        );
        let conservative_end = a0
            .checked_add(Duration::from_millis(first_m))
            .context("probe stamp overflow")?
            .checked_add(Duration::from_secs(15))
            .context("conservative quiet deadline overflow")?;
        let hit_end = shared_end.min(conservative_end);
        let marker_end = Some(hit_end);
        let shared_seconds_from_a0 = shared_end
            .checked_duration_since(a0)
            .context("shared deadline before A0")?
            .as_secs_f64();
        let hit_seconds_from_a0 = hit_end
            .checked_duration_since(a0)
            .context("hit deadline before A0")?
            .as_secs_f64();
        check(marker_end, "fresh first opportunity before sole B trigger")?;
        let trigger_elapsed_seconds = setup_started.elapsed().as_secs_f64();
        note(
            &mut diagnostics,
            "sole B request41 issuance",
            json!({"first_m":first_m}),
        )?;
        // After sole B issuance: no further owner IPC to either child through cut deaths.
        let upgrade_raw = op(marker_end, "one B-only upgrade41 transaction", || {
            pair.command(
                1,
                "upgrade",
                &["--request-id".into(), format!("{:02x}", 41_u8).repeat(16)],
            )
        })?;
        if committed_before_publication {
            diagnostics.data["committed_publication_upgrade_raw"] = upgrade_raw.clone().into();
        }
        let notification =
            upgrade_reply(&upgrade_raw).context("actual B upgrade transaction reply")?;
        if committed_before_publication {
            diagnostics.data["committed_publication_upgrade_reply"] = notification.clone();
        }
        controls(&notification, false)?;
        ensure!(
            notification["version"] == 3
                && notification["slot"] == pair.nodes[1].slot
                && notification["context"] == pair.c
                && notification["status"] == "NotificationPending"
                && notification["mode"] == "L1"
                && notification["wire_stage"] == 1
                && notification["freeze"] == false
                && notification["control_debt"] == false
                && notification.get("q").is_some_and(Value::is_null)
                && notification["cuts"] == json!([null, null])
                && ["p", "a", "k", "committed"]
                    .iter()
                    .all(|key| notification["controls"][*key].is_null())
                && notification.get("policy_error").is_some_and(Value::is_null)
                && notification["policy_paused"] == false
                && notification["lease_current"] == true
                && notification["native_encoding"] == "Legacy7"
                && notification["coordinator"] == false
                && notification["receive_event2"] == false
                && notification["send_event2"] == false
                && no_local["upgrade"] == "NoLocalIntent"
                && no_local["mode"] == "L1",
            "same real B-only trigger at its serialized transaction"
        );
        check(marker_end, "accept actual B upgrade transaction")?;
        note(
            &mut diagnostics,
            "actual B upgrade reply",
            notification.clone(),
        )?;
        let (after_m, first_probe_after) = probe(&a_trace, a_slot, hit_end)?;
        ensure!(
            after_m == Some(first_m),
            "same first probe through B trigger"
        );
        note(
            &mut diagnostics,
            if policy == CutPolicy::BeforePublication
                || accept_before_publication
                || select_before_publication
                || committed_before_publication
            {
                "await exact held-publication marker"
            } else {
                "await exact held-owner marker"
            },
            json!({"boundary":name}),
        )?;
        let marker = loop {
            alive(&mut pair, hit_end)?;
            let hit = op(
                marker_end,
                if accept_after_commit || accept_before_commit || accept_before_publication {
                    "remaining-deadline exact B Accept marker"
                } else if committed_after_commit
                    || committed_before_commit
                    || committed_before_publication
                {
                    "remaining-deadline exact B Committed marker"
                } else if select_after_commit || select_before_commit || select_before_publication {
                    "remaining-deadline exact A Select marker"
                } else {
                    "remaining-deadline exact P marker"
                },
                || {
                    let path = held_schedule.with_extension("hit");
                    match std::fs::metadata(&path) {
                        Ok(meta) => {
                            ensure!(meta.len() <= 256, "bounded exact marker");
                            let value = std::fs::read_to_string(path)?;
                            ensure!(value.len() <= 256, "bounded marker read");
                            Ok(Some(value))
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                        Err(error) => Err(error.into()),
                    }
                },
            )?;
            if let Some(marker) = hit {
                check(
                    marker_end,
                    "accept exact P marker within remaining opportunity",
                )?;
                break marker;
            }
            pause(hit_end, "remaining-deadline P marker poll")?;
        };
        if select_before_publication {
            diagnostics.data["k_publication_marker"] = marker.clone().into();
        }
        if committed_before_commit {
            diagnostics.data["committed_before_marker"] = marker.clone().into();
        }
        if committed_before_publication {
            diagnostics.data["committed_publication_marker"] = marker.clone().into();
        }
        ensure!(
            marker == name,
            if accept_after_commit || accept_before_commit || accept_before_publication {
                "exact B Accept marker contents"
            } else if committed_after_commit
                || committed_before_commit
                || committed_before_publication
            {
                "exact B Committed marker contents"
            } else if select_after_commit || select_before_commit || select_before_publication {
                "exact A Select marker contents"
            } else {
                "exact P marker contents"
            }
        );
        let (hit_m, first_probe_at_hit) = probe(&a_trace, a_slot, hit_end)?;
        ensure!(hit_m == Some(first_m), "same first probe at exact P marker");
        let publication_trace_observation = if policy == CutPolicy::BeforePublication {
            let evidence = op(
                marker_end,
                "complete owned publication trace prefix",
                || publication_trace(&a_trace, a_slot),
            )?;
            diagnostics.data["publication_trace"] = evidence.clone();
            evidence
        } else {
            Value::Null
        };
        let boundary: Value = op(marker_end, "bounded committed facts", || {
            let path = held_schedule.with_extension(format!("{name}.facts.json"));
            ensure!(std::fs::metadata(&path)?.len() <= 4096, "bounded facts");
            Ok(serde_json::from_slice(&std::fs::read(path)?)?)
        })?;
        if select_before_publication {
            diagnostics.data["k_publication_marker_facts"] = boundary.clone();
        }
        if committed_before_commit {
            diagnostics.data["committed_before_marker_facts"] = boundary.clone();
        }
        if committed_before_publication {
            diagnostics.data["committed_publication_marker_facts"] = boundary.clone();
        }
        if policy != CutPolicy::BeforePublication
            && !accept_before_publication
            && !select_before_publication
            && !committed_before_publication
        {
            controls(&boundary, false)?;
        }
        match policy {
            CutPolicy::AfterCommit => {
                ensure!(
                    boundary["context"] == pair.c
                        && boundary["mode"] == "Offering"
                        && hex(&boundary["controls"]["p"], 32)
                        && ["a", "k", "committed"]
                            .iter()
                            .all(|key| boundary["controls"][*key].is_null())
                        && boundary.get("q").is_some_and(Value::is_null)
                        && boundary["cuts"] == json!([2, null])
                        && hex(&boundary["native_hash"], 32)
                        && boundary["native_hash"] == original[0]["native_hash"],
                    "actual committed P boundary"
                );
            }
            CutPolicy::BeforeCommit => {
                ensure!(
                    boundary["context"] == pair.c
                        && boundary["mode"] == "L1"
                        && ["p", "a", "k", "committed"]
                            .iter()
                            .all(|key| boundary["controls"][*key].is_null())
                        && boundary.get("q").is_some_and(Value::is_null)
                        && boundary["cuts"] == json!([null, null])
                        && hex(&boundary["native_hash"], 32)
                        && boundary["native_hash"] == original[0]["native_hash"],
                    "actual before-commit state has no durable P"
                );
            }
            CutPolicy::BeforePublication => {
                ensure!(
                    boundary.as_object().is_some_and(|fields| fields.len() == 3)
                        && boundary["control"] == "P"
                        && hex(&boundary["control_ref"], 32)
                        && hex(&boundary["signed_control_blake3"], 32),
                    "actual three-field publication producer pair, not durable-state facts"
                );
            }
            CutPolicy::AcceptAfterCommit => {
                ensure!(
                    boundary["context"] == pair.c
                        && boundary["mode"] == "Prepared"
                        && hex(&boundary["controls"]["p"], 32)
                        && hex(&boundary["controls"]["a"], 32)
                        && ["k", "committed"]
                            .iter()
                            .all(|key| boundary["controls"][*key].is_null())
                        && boundary.get("q").is_some_and(Value::is_null)
                        && boundary["cuts"] == json!([2, 2])
                        && boundary["native_hash"] == original[1]["native_hash"],
                    "actual committed Accept boundary on original B"
                );
            }
            CutPolicy::SelectAfterCommit => {
                ensure!(
                    boundary["context"] == pair.c
                        && boundary["mode"] == "Selected"
                        && ["p", "a", "k"]
                            .iter()
                            .all(|key| hex(&boundary["controls"][*key], 32))
                        && boundary["controls"]
                            .get("committed")
                            .is_some_and(Value::is_null)
                        && hex(&boundary["q"], 32)
                        && boundary["cuts"] == json!([2, 2])
                        && hex(&boundary["native_hash"], 32)
                        && boundary["native_hash"] != original[0]["native_hash"],
                    "actual committed Select/conversion boundary on original A"
                );
            }
            CutPolicy::CommittedAfterCommit => {
                ensure!(
                    boundary["context"] == pair.c
                        && boundary["mode"] == "Active"
                        && ["p", "a", "k", "committed"]
                            .iter()
                            .all(|key| hex(&boundary["controls"][*key], 32))
                        && hex(&boundary["q"], 32)
                        && boundary["cuts"] == json!([2, 2])
                        && hex(&boundary["native_hash"], 32)
                        && boundary["native_hash"] != original[1]["native_hash"],
                    "actual committed final confirmation/conversion boundary on original B"
                );
            }
            CutPolicy::AcceptBeforeCommit => {
                ensure!(
                    boundary["context"] == pair.c
                        && boundary["mode"] == "L1"
                        && ["p", "a", "k", "committed"]
                            .iter()
                            .all(|key| boundary["controls"][*key].is_null())
                        && boundary.get("q").is_some_and(Value::is_null)
                        && boundary["cuts"] == json!([null, null])
                        && hex(&boundary["native_hash"], 32)
                        && boundary["native_hash"] == original[1]["native_hash"],
                    "actual B saved state before any typed Prepare transaction"
                );
            }
            CutPolicy::AcceptBeforePublication => {
                ensure!(
                    boundary.as_object().is_some_and(|fields| fields.len() == 3)
                        && boundary["control"] == "A"
                        && hex(&boundary["control_ref"], 32)
                        && hex(&boundary["signed_control_blake3"], 32),
                    "actual three-field Accept publication producer pair after durable Prepare"
                );
            }
            CutPolicy::SelectBeforeCommit => {
                ensure!(
                    boundary.as_object().is_some_and(|fields| fields.len() == 6)
                        && boundary["context"] == pair.c
                        && boundary["mode"] == "Offering"
                        && boundary["controls"]
                            .as_object()
                            .is_some_and(|fields| fields.len() == 4)
                        && hex(&boundary["controls"]["p"], 32)
                        && ["a", "k", "committed"]
                            .iter()
                            .all(|key| boundary["controls"][*key].is_null())
                        && boundary.get("q").is_some_and(Value::is_null)
                        && boundary["cuts"] == json!([2, null])
                        && hex(&boundary["native_hash"], 32)
                        && boundary["native_hash"] == original[0]["native_hash"],
                    "actual A Offering/P-only/original Native before typed Select storage"
                );
            }
            CutPolicy::SelectBeforePublication => {
                ensure!(
                    boundary.as_object().is_some_and(|fields| fields.len() == 3)
                        && boundary["control"] == "K"
                        && hex(&boundary["control_ref"], 32)
                        && hex(&boundary["signed_control_blake3"], 32),
                    "actual three-field K publication producer pair after durable Select"
                );
            }
            CutPolicy::CommittedBeforeCommit => {
                ensure!(
                    boundary.as_object().is_some_and(|fields| fields.len() == 6)
                        && boundary["context"] == pair.c
                        && boundary["mode"] == "Prepared"
                        && boundary["controls"]
                            .as_object()
                            .is_some_and(|fields| fields.len() == 4)
                        && hex(&boundary["controls"]["p"], 32)
                        && hex(&boundary["controls"]["a"], 32)
                        && ["k", "committed"]
                            .iter()
                            .all(|key| boundary["controls"][*key].is_null())
                        && boundary.get("q").is_some_and(Value::is_null)
                        && boundary["cuts"] == json!([2, 2])
                        && hex(&boundary["native_hash"], 32)
                        && boundary["native_hash"] == original[1]["native_hash"],
                    "actual B Prepared/P+Accept/original Native before typed CommitB storage"
                );
            }
            CutPolicy::CommittedBeforePublication => {
                ensure!(
                    boundary.as_object().is_some_and(|fields| fields.len() == 3)
                        && boundary["control"] == "Committed"
                        && hex(&boundary["control_ref"], 32)
                        && hex(&boundary["signed_control_blake3"], 32),
                    "actual three-field Committed producer pair after durable CommitB and signed outer reply"
                );
            }
        }
        check(marker_end, "accept exact P marker/facts")?;
        note(
            &mut diagnostics,
            if policy == CutPolicy::BeforePublication
                || accept_before_publication
                || select_before_publication
                || committed_before_publication
            {
                "exact held-publication facts"
            } else {
                "exact held-owner facts"
            },
            boundary.clone(),
        )?;
        // No further owner IPC before completed cut deaths.
        let deaths = if accept_after_commit
            || committed_after_commit
            || committed_before_commit
            || committed_before_publication
            || accept_before_commit
            || accept_before_publication
        {
            accept_cut_deaths(&mut pair)?
        } else {
            stop(&mut pair)?
        };
        note(
            &mut diagnostics,
            if accept_after_commit
                || committed_after_commit
                || committed_before_commit
                || committed_before_publication
                || accept_before_commit
                || accept_before_publication
            {
                "armed B then A deaths complete"
            } else {
                "armed A then B deaths complete"
            },
            deaths.clone(),
        )?;
        op(None, "clear external schedule after both deaths", || {
            schedule_file(held_schedule, &json!({}))
        })?;
        let recovery_schedule = if policy == CutPolicy::BeforeCommit {
            Some(b_schedule.as_path())
        } else {
            None
        };
        let mut isolated_b = Value::Null;
        let mut accept_isolated_b = Value::Null;
        let mut accept_isolated_a = Value::Null;
        let mut accept_isolated_b_death = Value::Null;
        let mut accept_saved_controls = [Value::Null, Value::Null];
        let mut select_isolated_b = Value::Null;
        let mut select_isolated_a = Value::Null;
        let mut select_isolated_b_death = Value::Null;
        let mut select_saved_controls = [Value::Null, Value::Null];
        let mut select_observed_q = [Value::Null, Value::Null];
        let mut select_converted_native = [Value::Null, Value::Null];
        let mut committed_isolated_b = Value::Null;
        let mut committed_isolated_a = Value::Null;
        let mut committed_isolated_b_death = Value::Null;
        let mut committed_saved_controls = [Value::Null, Value::Null];
        let mut committed_observed_q = [Value::Null, Value::Null];
        let mut committed_saved_native = [Value::Null, Value::Null];
        let mut a_before_isolated_b = Value::Null;
        let mut a_before_isolated_a = Value::Null;
        let mut a_before_isolated_b_death = Value::Null;
        let mut a_before_p0 = Value::Null;
        let mut a_before_observed = [Value::Null, Value::Null];
        let mut a_before_shared = Value::Null;
        let mut a_publication_seed_b = Value::Null;
        let mut a_publication_isolated_b = Value::Null;
        let mut a_publication_isolated_a = Value::Null;
        let mut a_publication_isolated_b_death = Value::Null;
        let mut a_publication_p0 = Value::Null;
        let a_publication_a0 = if accept_before_publication {
            boundary["control_ref"].clone()
        } else {
            Value::Null
        };
        let mut a_publication_observed = [Value::Null, Value::Null];
        let mut a_publication_shared = Value::Null;
        let mut k_before_seed_b = Value::Null;
        let mut k_before_isolated_b = Value::Null;
        let mut k_before_isolated_a = Value::Null;
        let mut k_before_isolated_b_death = Value::Null;
        let k_before_p0 = if select_before_commit {
            boundary["controls"]["p"].clone()
        } else {
            Value::Null
        };
        let mut k_before_a0 = Value::Null;
        let mut k_before_observed = [Value::Null, Value::Null];
        let mut k_before_shared = Value::Null;
        let mut k_publication_seed_b = Value::Null;
        let mut k_publication_seed_a = Value::Null;
        let mut k_publication_isolated_b = Value::Null;
        let mut k_publication_isolated_a = Value::Null;
        let mut k_publication_isolated_b_death = Value::Null;
        let k_publication_k0 = if select_before_publication {
            boundary["control_ref"].clone()
        } else {
            Value::Null
        };
        let k_publication_dk0 = if select_before_publication {
            boundary["signed_control_blake3"].clone()
        } else {
            Value::Null
        };
        let mut k_publication_p0 = Value::Null;
        let mut k_publication_a0 = Value::Null;
        let mut k_publication_q0 = Value::Null;
        let mut k_publication_observed = [Value::Null, Value::Null];
        let mut k_publication_shared = Value::Null;
        let mut committed_before_cold_b = Value::Null;
        let mut committed_before_seed_a = Value::Null;
        let mut committed_before_isolated_b = Value::Null;
        let mut committed_before_isolated_a = Value::Null;
        let mut committed_before_isolated_b_death = Value::Null;
        let committed_before_p0 = if committed_before_commit {
            boundary["controls"]["p"].clone()
        } else {
            Value::Null
        };
        let committed_before_a0 = if committed_before_commit {
            boundary["controls"]["a"].clone()
        } else {
            Value::Null
        };
        let mut committed_before_k0 = Value::Null;
        let mut committed_before_q0 = Value::Null;
        let mut committed_before_observed = [Value::Null, Value::Null];
        let mut committed_before_shared = Value::Null;
        let mut committed_publication_seed_b = Value::Null;
        let mut actual_cold_b_anchor = Value::Null;
        let mut committed_publication_cold_a = Value::Null;
        let mut committed_publication_isolated_b = Value::Null;
        let mut committed_publication_isolated_a = Value::Null;
        let mut committed_publication_isolated_b_death = Value::Null;
        let committed_publication_c0 = if committed_before_publication {
            boundary["control_ref"].clone()
        } else {
            Value::Null
        };
        let committed_publication_dc0 = if committed_before_publication {
            boundary["signed_control_blake3"].clone()
        } else {
            Value::Null
        };
        let mut committed_publication_p0 = Value::Null;
        let mut committed_publication_observed = [Value::Null, Value::Null];
        let mut committed_publication_shared = Value::Null;
        let mut committed_publication_observed_q = [Value::Null, Value::Null];
        let mut committed_publication_saved_native = [Value::Null, Value::Null];
        if policy == CutPolicy::BeforeCommit
            || accept_after_commit
            || select_after_commit
            || committed_after_commit
            || accept_before_commit
            || accept_before_publication
            || select_before_commit
            || select_before_publication
            || committed_before_commit
            || committed_before_publication
        {
            ensure!(pair.children[0].is_none(), "A dead before isolated B start");
            note(
                &mut diagnostics,
                if committed_before_publication {
                    "start isolated B Active after held Committed publication"
                } else if committed_before_commit {
                    "start isolated B Prepared before typed CommitB storage"
                } else if select_before_publication {
                    "start isolated B Prepared after outgoing Select publication hold"
                } else if select_before_commit {
                    "start isolated B Prepared before Select storage"
                } else if accept_before_publication {
                    "start isolated B Prepared after held Accept publication"
                } else if accept_before_commit {
                    "start isolated B L1 before Accept storage"
                } else if accept_after_commit {
                    "start isolated B Prepared witness"
                } else if select_after_commit {
                    "start isolated pre-Select B Prepared witness"
                } else if committed_after_commit {
                    "start isolated B Active Committed witness"
                } else {
                    "start isolated B intent witness"
                },
                Value::Null,
            )?;
            op(None, "reopen identical B profile alone", || {
                pair.start(1, recovery_schedule)
            })?;
            started(&pair, 1, "isolated B ready", &mut diagnostics)?;
            if committed_before_publication {
                let pid = pair.children[1]
                    .as_ref()
                    .context("actual cold Committed-publication B seed owner")?
                    .0
                    .id();
                let peer_absent = pair.children[0].is_none();
                diagnostics.data["committed_publication_seed_B"] = json!({
                    "role":1,"pid":pid,"slot":pair.nodes[1].slot,"context":pair.c,
                    "peer_absent_before":peer_absent
                });
                ensure!(
                    peer_absent,
                    "A absent before actual cold Committed-publication B seed"
                );
                let seed = op(None, "actual cold Committed-publication B seed row", || {
                    pair.row(1)
                })?;
                diagnostics.data["committed_publication_seed_B"]["row"] = seed.clone();
                origin(&seed, &original[1], 1, &pair.c)?;
                controls(&seed, true)?;
                ensure!(
                    seed["slot"] == pair.nodes[1].slot
                        && seed["enabled"] == true
                        && seed["mode"] == "Active"
                        && seed["wire_stage"] == 5
                        && seed["native_encoding"] == "Mixed7"
                        && seed["native_generation"].as_u64()
                            == original[1]["native_generation"]
                                .as_u64()
                                .and_then(|generation| generation.checked_add(1))
                        && seed["native_hash"] != original[1]["native_hash"]
                        && seed["controls"]
                            .as_object()
                            .is_some_and(|fields| fields.len() == 4)
                        && seed["controls"]["committed"] == committed_publication_c0
                        && hex(&committed_publication_c0, 32)
                        && hex(&seed["q"], 32)
                        && seed["cuts"] == json!([2, 2])
                        && seed["next"] == json!([2, 2])
                        && seed["native"]["own_next"] == 2
                        && seed["native"]["peer_next"] == 2
                        && empty(&seed, 1)
                        && seed["control_debt"] == true
                        && seed["freeze"] == false
                        && seed["lease_current"] == false
                        && seed["alpn1_allowed"] == false
                        && ["policy_error", "handoff_policy_error"]
                            .iter()
                            .all(|key| seed.get(*key).is_some_and(Value::is_null)),
                    "real cold B Active/original+1 Mixed7/P+A+K+marker C0/Q before anchor reuse"
                );
                coalesced_p(&seed["upgrade"], &seed["controls"]["p"])?;
                let seed_history = page(&pair, 1, None)?;
                diagnostics.data["committed_publication_seed_B"]["history"] = seed_history.clone();
                select_history_view(&seed_history, &original_history[1])?;
                ensure!(
                    seed_history["F"] == json!([2, 2])
                        && seed_history["items"] == original_history[1]["items"]
                        && pair.children[0].is_none(),
                    "cold B seed retains its exact own old history/view while A stays absent"
                );
                op(
                    None,
                    "actual cold Committed-publication B seed still alive",
                    || {
                        pair.children[1]
                            .as_mut()
                            .context("cold Committed-publication B seed")?
                            .alive()
                    },
                )?;
                diagnostics.data["committed_publication_seed_B"]["peer_absent_through"] =
                    true.into();
                diagnostics.data["committed_publication_seed_B"]["child_alive_after"] = true.into();
                committed_publication_seed_b =
                    diagnostics.data["committed_publication_seed_B"].clone();
                // A real checked cold row is the anchor; the producer marker stays three-field.
                actual_cold_b_anchor = seed.clone();
                committed_publication_p0 = seed["controls"]["p"].clone();
                diagnostics.data["committed_publication_actual_cold_B_anchor"] =
                    actual_cold_b_anchor.clone();
                a_before_progress(
                    &pair,
                    &seed,
                    &original[1],
                    1,
                    &committed_publication_p0,
                    &mut committed_publication_observed[1],
                    &mut committed_publication_shared,
                )?;
                committed_refs(
                    &seed,
                    1,
                    &actual_cold_b_anchor,
                    &mut committed_publication_observed_q[1],
                )?;
                committed_native(
                    &seed,
                    &original[1],
                    1,
                    &actual_cold_b_anchor,
                    &mut committed_publication_saved_native[1],
                )?;
                diagnostics.data["committed_publication_observed"] =
                    json!(&committed_publication_observed);
                diagnostics.data["committed_publication_shared"] =
                    committed_publication_shared.clone();
                diagnostics.data["committed_publication_observed_Q"] =
                    json!(&committed_publication_observed_q);
                diagnostics.data["committed_publication_saved_Native"] =
                    json!(&committed_publication_saved_native);
                committed_publication_isolated_b = committed_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &actual_cold_b_anchor,
                    &mut diagnostics,
                )?;
                diagnostics.data["committed_publication_isolated_B"] =
                    committed_publication_isolated_b.clone();
                for stage in ["before", "after"] {
                    let row = &committed_publication_isolated_b[stage];
                    a_before_progress(
                        &pair,
                        row,
                        &original[1],
                        1,
                        &committed_publication_p0,
                        &mut committed_publication_observed[1],
                        &mut committed_publication_shared,
                    )?;
                    committed_refs(
                        row,
                        1,
                        &actual_cold_b_anchor,
                        &mut committed_publication_observed_q[1],
                    )?;
                    committed_native(
                        row,
                        &original[1],
                        1,
                        &actual_cold_b_anchor,
                        &mut committed_publication_saved_native[1],
                    )?;
                    diagnostics.data["committed_publication_observed"] =
                        json!(&committed_publication_observed);
                    diagnostics.data["committed_publication_shared"] =
                        committed_publication_shared.clone();
                    diagnostics.data["committed_publication_observed_Q"] =
                        json!(&committed_publication_observed_q);
                    diagnostics.data["committed_publication_saved_Native"] =
                        json!(&committed_publication_saved_native);
                }
                note(
                    &mut diagnostics,
                    "isolated B Active anchors saved P0/Accept A0/K0/Q0/own Native and matches publication C0",
                    committed_publication_isolated_b.clone(),
                )?;
            } else if committed_before_commit {
                let pid = pair.children[1]
                    .as_ref()
                    .context("actual cold Committed-before B witness owner")?
                    .0
                    .id();
                let peer_absent = pair.children[0].is_none();
                diagnostics.data["committed_before_cold_B"] = json!({
                    "role":1,"pid":pid,"peer_absent_before":peer_absent
                });
                committed_before_cold_b = diagnostics.data["committed_before_cold_B"].clone();
                ensure!(
                    peer_absent,
                    "A absent before actual cold Committed-before B wrapper"
                );
                committed_before_isolated_b = select_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &boundary,
                    &mut diagnostics,
                )?;
                diagnostics.data["committed_before_isolated_B"] =
                    committed_before_isolated_b.clone();
                for stage in ["before", "after"] {
                    a_before_progress(
                        &pair,
                        &committed_before_isolated_b[stage],
                        &original[1],
                        1,
                        &committed_before_p0,
                        &mut committed_before_observed[1],
                        &mut committed_before_shared,
                    )?;
                    diagnostics.data["committed_before_observed"] =
                        json!(&committed_before_observed);
                    diagnostics.data["committed_before_shared"] = committed_before_shared.clone();
                }
                note(
                    &mut diagnostics,
                    "isolated B Prepared retains marker P0/Accept A0/original Native/history before CommitB",
                    committed_before_isolated_b.clone(),
                )?;
            } else if select_before_publication {
                let pid = pair.children[1]
                    .as_ref()
                    .context("actual cold K-publication B seed owner")?
                    .0
                    .id();
                let peer_absent = pair.children[0].is_none();
                diagnostics.data["k_publication_seed_B"] = json!({
                    "role":1,"pid":pid,"peer_absent_before":peer_absent
                });
                ensure!(
                    peer_absent,
                    "A absent before real cold K-publication B seed"
                );
                let seed = op(None, "actual cold K-publication B seed row", || pair.row(1))?;
                diagnostics.data["k_publication_seed_B"]["row"] = seed.clone();
                k_publication_seed_b = diagnostics.data["k_publication_seed_B"].clone();
                k_publication_p0 = seed["controls"]
                    .get("p")
                    .context("actual cold K-publication B seed P field")?
                    .clone();
                k_publication_a0 = seed["controls"]
                    .get("a")
                    .context("actual cold K-publication B seed Accept field")?
                    .clone();
                diagnostics.data["k_publication_P0"] = k_publication_p0.clone();
                diagnostics.data["k_publication_A0"] = k_publication_a0.clone();
                ensure!(
                    seed["mode"] == "Prepared"
                        && hex(&k_publication_p0, 32)
                        && hex(&k_publication_a0, 32),
                    "actual cold B Prepared first anchors genuine saved P0 and Accept A0"
                );
                select_prepared_state(&seed, &original[1], &pair.c, pair.nodes[1].slot, &seed)?;
                a_before_progress(
                    &pair,
                    &seed,
                    &original[1],
                    1,
                    &k_publication_p0,
                    &mut k_publication_observed[1],
                    &mut k_publication_shared,
                )?;
                diagnostics.data["k_publication_observed"] = json!(&k_publication_observed);
                diagnostics.data["k_publication_shared"] = k_publication_shared.clone();
                k_publication_isolated_b = select_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &seed,
                    &mut diagnostics,
                )?;
                diagnostics.data["k_publication_isolated_B"] = k_publication_isolated_b.clone();
                for stage in ["before", "after"] {
                    a_before_progress(
                        &pair,
                        &k_publication_isolated_b[stage],
                        &original[1],
                        1,
                        &k_publication_p0,
                        &mut k_publication_observed[1],
                        &mut k_publication_shared,
                    )?;
                    diagnostics.data["k_publication_observed"] = json!(&k_publication_observed);
                    diagnostics.data["k_publication_shared"] = k_publication_shared.clone();
                }
                note(
                    &mut diagnostics,
                    "isolated B Prepared anchors P0/Accept A0 and retains original Legacy7/history",
                    k_publication_isolated_b.clone(),
                )?;
            } else if select_before_commit {
                let pid = pair.children[1]
                    .as_ref()
                    .context("actual cold K-before B seed owner")?
                    .0
                    .id();
                let peer_absent = pair.children[0].is_none();
                diagnostics.data["k_before_seed_B"] = json!({
                    "role":1,"pid":pid,"peer_absent_before":peer_absent
                });
                ensure!(peer_absent, "A absent before real cold K-before B seed");
                let seed = op(None, "actual cold K-before B seed row", || pair.row(1))?;
                diagnostics.data["k_before_seed_B"]["row"] = seed.clone();
                k_before_seed_b = diagnostics.data["k_before_seed_B"].clone();
                k_before_a0 = seed["controls"]
                    .get("a")
                    .context("actual cold K-before B seed Accept field")?
                    .clone();
                diagnostics.data["k_before_P0"] = k_before_p0.clone();
                diagnostics.data["k_before_A0"] = k_before_a0.clone();
                ensure!(
                    seed["mode"] == "Prepared"
                        && hex(&k_before_p0, 32)
                        && seed["controls"]["p"] == k_before_p0
                        && hex(&k_before_a0, 32),
                    "actual cold B Prepared retains marker P0 and genuine first saved Accept A0"
                );
                select_prepared_state(&seed, &original[1], &pair.c, pair.nodes[1].slot, &seed)?;
                a_before_progress(
                    &pair,
                    &seed,
                    &original[1],
                    1,
                    &k_before_p0,
                    &mut k_before_observed[1],
                    &mut k_before_shared,
                )?;
                diagnostics.data["k_before_observed"] = json!(&k_before_observed);
                diagnostics.data["k_before_shared"] = k_before_shared.clone();
                k_before_isolated_b = select_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &seed,
                    &mut diagnostics,
                )?;
                for stage in ["before", "after"] {
                    a_before_progress(
                        &pair,
                        &k_before_isolated_b[stage],
                        &original[1],
                        1,
                        &k_before_p0,
                        &mut k_before_observed[1],
                        &mut k_before_shared,
                    )?;
                }
                diagnostics.data["k_before_observed"] = json!(&k_before_observed);
                diagnostics.data["k_before_shared"] = k_before_shared.clone();
                note(
                    &mut diagnostics,
                    "isolated B Prepared retains marker P0 and first durable Accept A0/Legacy7/history",
                    k_before_isolated_b.clone(),
                )?;
            } else if accept_before_publication {
                let pid = pair.children[1]
                    .as_ref()
                    .context("actual cold A-publication B seed owner")?
                    .0
                    .id();
                let peer_absent = pair.children[0].is_none();
                diagnostics.data["a_publication_seed_B"] = json!({
                    "role":1,"pid":pid,"peer_absent_before":peer_absent
                });
                ensure!(peer_absent, "A absent before real cold B publication seed");
                let seed = op(None, "actual cold B publication seed row", || pair.row(1))?;
                diagnostics.data["a_publication_seed_B"]["row"] = seed.clone();
                a_publication_seed_b = diagnostics.data["a_publication_seed_B"].clone();
                a_publication_p0 = seed["controls"]
                    .get("p")
                    .context("actual cold B publication seed P field")?
                    .clone();
                diagnostics.data["a_publication_P0"] = a_publication_p0.clone();
                diagnostics.data["a_publication_A0"] = a_publication_a0.clone();
                ensure!(
                    seed["mode"] == "Prepared"
                        && hex(&a_publication_p0, 32)
                        && hex(&a_publication_a0, 32)
                        && hex(&seed["controls"]["a"], 32)
                        && seed["controls"]["a"] == a_publication_a0,
                    "actual cold B Prepared P0 and independent publication A0"
                );
                select_prepared_state(&seed, &original[1], &pair.c, pair.nodes[1].slot, &seed)?;
                a_before_progress(
                    &pair,
                    &seed,
                    &original[1],
                    1,
                    &a_publication_p0,
                    &mut a_publication_observed[1],
                    &mut a_publication_shared,
                )?;
                diagnostics.data["a_publication_observed"] = json!(&a_publication_observed);
                diagnostics.data["a_publication_shared"] = a_publication_shared.clone();
                a_publication_isolated_b = select_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &seed,
                    &mut diagnostics,
                )?;
                for stage in ["before", "after"] {
                    a_before_progress(
                        &pair,
                        &a_publication_isolated_b[stage],
                        &original[1],
                        1,
                        &a_publication_p0,
                        &mut a_publication_observed[1],
                        &mut a_publication_shared,
                    )?;
                }
                diagnostics.data["a_publication_observed"] = json!(&a_publication_observed);
                diagnostics.data["a_publication_shared"] = a_publication_shared.clone();
                note(
                    &mut diagnostics,
                    "isolated B Prepared retains publication A0 and actual seed P0/Legacy7/history",
                    a_publication_isolated_b.clone(),
                )?;
            } else if accept_before_commit {
                a_before_isolated_b = a_before_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &mut diagnostics,
                )?;
                for stage in ["before", "after"] {
                    a_before_progress(
                        &pair,
                        &a_before_isolated_b[stage],
                        &original[1],
                        1,
                        &a_before_p0,
                        &mut a_before_observed[1],
                        &mut a_before_shared,
                    )?;
                }
                diagnostics.data["a_before_observed"] = json!(&a_before_observed);
                diagnostics.data["a_before_shared"] = a_before_shared.clone();
                note(
                    &mut diagnostics,
                    "isolated B L1 retains sent notification and original Native; no saved P/Accept",
                    a_before_isolated_b.clone(),
                )?;
            } else if committed_after_commit {
                committed_isolated_b = committed_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &boundary,
                    &mut diagnostics,
                )?;
                committed_saved_controls[1] = committed_isolated_b["before"]["controls"].clone();
                for stage in ["before", "after"] {
                    let row = &committed_isolated_b[stage];
                    remember_controls(&mut committed_saved_controls[1], row)?;
                    committed_refs(row, 1, &boundary, &mut committed_observed_q[1])?;
                    committed_native(
                        row,
                        &original[1],
                        1,
                        &boundary,
                        &mut committed_saved_native[1],
                    )?;
                }
                diagnostics.data["committed_saved_controls"] = json!(&committed_saved_controls);
                diagnostics.data["committed_observed_q"] = json!(&committed_observed_q);
                diagnostics.data["committed_saved_native"] = json!(&committed_saved_native);
                note(
                    &mut diagnostics,
                    "isolated B durable Active/Committed/held Native with control debt",
                    committed_isolated_b.clone(),
                )?;
            } else if select_after_commit {
                select_isolated_b = select_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &boundary,
                    &mut diagnostics,
                )?;
                select_saved_controls[1] = select_isolated_b["before"]["controls"].clone();
                for stage in ["before", "after"] {
                    let row = &select_isolated_b[stage];
                    remember_controls(&mut select_saved_controls[1], row)?;
                    select_refs(row, 1, &boundary, &mut select_observed_q[1])?;
                    select_native(
                        row,
                        &original[1],
                        1,
                        &boundary,
                        &mut select_converted_native[1],
                    )?;
                }
                diagnostics.data["select_saved_controls"] = json!(&select_saved_controls);
                diagnostics.data["select_observed_q"] = json!(&select_observed_q);
                note(
                    &mut diagnostics,
                    "isolated B pre-Select Prepared/Legacy7/history view",
                    select_isolated_b.clone(),
                )?;
            } else if accept_after_commit {
                accept_isolated_b = accept_cut_witness(
                    &mut pair,
                    1,
                    &original[1],
                    &original_history[1],
                    &boundary,
                )?;
                accept_saved_controls[1] = accept_isolated_b["after"]["controls"].clone();
                diagnostics.data["accept_saved_controls"] = json!(&accept_saved_controls);
                note(
                    &mut diagnostics,
                    "isolated B durable Prepared P/Accept",
                    accept_isolated_b.clone(),
                )?;
            } else {
                isolated_b = isolated_intent(&mut pair, 1, &original[1], &original_history[1])?;
                note(
                    &mut diagnostics,
                    "isolated B durable intent",
                    isolated_b.clone(),
                )?;
            }
            let mut child = pair.children[1].take().context("isolated B before death")?;
            let pid = child.0.id();
            op(None, "isolated B actual kill/wait before A start", || {
                child.kill_wait()
            })?;
            note(
                &mut diagnostics,
                "isolated B death complete",
                json!({"role":1,"pid":pid,"kill_wait_completed":true}),
            )?;
            if accept_after_commit {
                accept_isolated_b_death = json!({"role":1,"pid":pid,"kill_wait_completed":true});
            }
            if select_after_commit {
                select_isolated_b_death = json!({"role":1,"pid":pid,"kill_wait_completed":true});
            }
            if committed_after_commit {
                committed_isolated_b_death = json!({"role":1,"pid":pid,"kill_wait_completed":true});
            }
            if accept_before_commit {
                a_before_isolated_b_death = json!({
                    "role":1,"pid":pid,"kill_wait_completed":true
                });
            }
            if accept_before_publication {
                a_publication_isolated_b_death = json!({
                    "role":1,"pid":pid,"kill_wait_completed":true
                });
                diagnostics.data["a_publication_isolated_B_death"] =
                    a_publication_isolated_b_death.clone();
            }
            if select_before_commit {
                k_before_isolated_b_death = json!({
                    "role":1,"pid":pid,"kill_wait_completed":true
                });
                diagnostics.data["k_before_isolated_B_death"] = k_before_isolated_b_death.clone();
            }
            if select_before_publication {
                k_publication_isolated_b_death = json!({
                    "role":1,"pid":pid,"kill_wait_completed":true
                });
                diagnostics.data["k_publication_isolated_B_death"] =
                    k_publication_isolated_b_death.clone();
            }
            if committed_before_commit {
                committed_before_isolated_b_death = json!({
                    "role":1,"pid":pid,"kill_wait_completed":true
                });
                diagnostics.data["committed_before_isolated_B_death"] =
                    committed_before_isolated_b_death.clone();
            }
            if committed_before_publication {
                committed_publication_isolated_b_death = json!({
                    "role":1,"pid":pid,"kill_wait_completed":true
                });
                diagnostics.data["committed_publication_isolated_B_death"] =
                    committed_publication_isolated_b_death.clone();
                ensure!(
                    committed_publication_seed_b["pid"] == pid
                        && committed_publication_isolated_b["pid"] == pid,
                    "actual isolated B death belongs to the observed seed and complete witness"
                );
            }
            ensure!(
                pair.children.iter().all(Option::is_none),
                "both absent before isolated A"
            );
        }
        ensure!(
            pair.children[1].is_none(),
            "B dead before isolated A reopen"
        );
        note(&mut diagnostics, "start isolated A witness", Value::Null)?;
        op(None, "reopen identical A profile alone", || {
            pair.start(0, recovery_schedule)
        })?;
        started(&pair, 0, "isolated A ready", &mut diagnostics)?;
        ensure!(
            pair.children[1].is_none(),
            "B absent through first reopened witness"
        );
        let mut isolated_a_intent = Value::Null;
        let isolated = if policy == CutPolicy::BeforeCommit {
            isolated_a_intent = isolated_intent(&mut pair, 0, &original[0], &original_history[0])?;
            isolated_a_intent["after"].clone()
        } else if committed_before_publication {
            let pid = pair.children[0]
                .as_ref()
                .context("actual cold Committed-publication A witness owner")?
                .0
                .id();
            let peer_absent = pair.children[1].is_none();
            diagnostics.data["committed_publication_cold_A"] = json!({
                "role":0,"pid":pid,"slot":pair.nodes[0].slot,"context":pair.c,
                "peer_absent_before":peer_absent,
                "completed_B_death":committed_publication_isolated_b_death
            });
            committed_publication_cold_a = diagnostics.data["committed_publication_cold_A"].clone();
            ensure!(
                peer_absent
                    && committed_publication_isolated_b_death["kill_wait_completed"] == true,
                "B fully dead before actual cold Committed-publication A witness"
            );
            committed_publication_isolated_a = committed_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &actual_cold_b_anchor,
                &mut diagnostics,
            )?;
            diagnostics.data["committed_publication_isolated_A"] =
                committed_publication_isolated_a.clone();
            ensure!(
                committed_publication_isolated_a["pid"] == pid,
                "actual isolated A header and complete witness have the same owner"
            );
            for stage in ["before", "after"] {
                let row = &committed_publication_isolated_a[stage];
                a_before_progress(
                    &pair,
                    row,
                    &original[0],
                    0,
                    &committed_publication_p0,
                    &mut committed_publication_observed[0],
                    &mut committed_publication_shared,
                )?;
                committed_refs(
                    row,
                    0,
                    &actual_cold_b_anchor,
                    &mut committed_publication_observed_q[0],
                )?;
                committed_native(
                    row,
                    &original[0],
                    0,
                    &actual_cold_b_anchor,
                    &mut committed_publication_saved_native[0],
                )?;
                diagnostics.data["committed_publication_observed"] =
                    json!(&committed_publication_observed);
                diagnostics.data["committed_publication_shared"] =
                    committed_publication_shared.clone();
                diagnostics.data["committed_publication_observed_Q"] =
                    json!(&committed_publication_observed_q);
                diagnostics.data["committed_publication_saved_Native"] =
                    json!(&committed_publication_saved_native);
            }
            note(
                &mut diagnostics,
                "isolated A Selected retains cold B P0/Accept A0/K0/Q0 without Committed and first anchors own Native",
                committed_publication_isolated_a.clone(),
            )?;
            committed_publication_isolated_a["after"].clone()
        } else if committed_before_commit {
            let pid = pair.children[0]
                .as_ref()
                .context("actual cold Committed-before A seed owner")?
                .0
                .id();
            let peer_absent = pair.children[1].is_none();
            diagnostics.data["committed_before_seed_A"] = json!({
                "role":0,"pid":pid,"peer_absent_before":peer_absent
            });
            ensure!(
                peer_absent,
                "B absent before actual cold Committed-before A seed"
            );
            let seed = op(None, "actual cold Committed-before A seed row", || {
                pair.row(0)
            })?;
            diagnostics.data["committed_before_seed_A"]["row"] = seed.clone();
            committed_before_seed_a = diagnostics.data["committed_before_seed_A"].clone();
            committed_before_k0 = seed["controls"]
                .get("k")
                .context("actual cold Committed-before A seed K field")?
                .clone();
            committed_before_q0 = seed
                .get("q")
                .context("actual cold Committed-before A seed Q field")?
                .clone();
            diagnostics.data["committed_before_K0"] = committed_before_k0.clone();
            diagnostics.data["committed_before_Q0"] = committed_before_q0.clone();
            ensure!(
                seed["mode"] == "Selected"
                    && seed["controls"]["p"] == committed_before_p0
                    && seed["controls"]["a"] == committed_before_a0
                    && hex(&committed_before_p0, 32)
                    && hex(&committed_before_a0, 32)
                    && hex(&committed_before_k0, 32)
                    && hex(&committed_before_q0, 32)
                    && seed["controls"]
                        .get("committed")
                        .is_some_and(Value::is_null),
                "actual cold A Selected matches independent marker P0/Accept A0 before real boundary reuse"
            );
            select_selected_state(&seed, &original[0], &pair.c, pair.nodes[0].slot, &seed)?;
            a_before_progress(
                &pair,
                &seed,
                &original[0],
                0,
                &committed_before_p0,
                &mut committed_before_observed[0],
                &mut committed_before_shared,
            )?;
            diagnostics.data["committed_before_observed"] = json!(&committed_before_observed);
            diagnostics.data["committed_before_shared"] = committed_before_shared.clone();
            committed_before_isolated_a = select_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &seed,
                &mut diagnostics,
            )?;
            diagnostics.data["committed_before_isolated_A"] = committed_before_isolated_a.clone();
            for stage in ["before", "after"] {
                a_before_progress(
                    &pair,
                    &committed_before_isolated_a[stage],
                    &original[0],
                    0,
                    &committed_before_p0,
                    &mut committed_before_observed[0],
                    &mut committed_before_shared,
                )?;
                diagnostics.data["committed_before_observed"] = json!(&committed_before_observed);
                diagnostics.data["committed_before_shared"] = committed_before_shared.clone();
            }
            note(
                &mut diagnostics,
                "isolated A Selected matches marker P0/Accept A0 and first anchors saved K0/Q0/Mixed hash",
                committed_before_isolated_a.clone(),
            )?;
            committed_before_isolated_a["after"].clone()
        } else if select_before_publication {
            let pid = pair.children[0]
                .as_ref()
                .context("actual cold K-publication A seed owner")?
                .0
                .id();
            let peer_absent = pair.children[1].is_none();
            diagnostics.data["k_publication_seed_A"] = json!({
                "role":0,"pid":pid,"peer_absent_before":peer_absent
            });
            ensure!(
                peer_absent,
                "B absent before real cold K-publication A seed"
            );
            let seed = op(None, "actual cold K-publication A seed row", || pair.row(0))?;
            diagnostics.data["k_publication_seed_A"]["row"] = seed.clone();
            k_publication_seed_a = diagnostics.data["k_publication_seed_A"].clone();
            k_publication_q0 = seed
                .get("q")
                .context("actual cold K-publication A seed Q field")?
                .clone();
            diagnostics.data["k_publication_Q0"] = k_publication_q0.clone();
            ensure!(
                seed["mode"] == "Selected"
                    && seed["controls"]["p"] == k_publication_p0
                    && seed["controls"]["a"] == k_publication_a0
                    && seed["controls"]["k"] == k_publication_k0
                    && hex(&k_publication_p0, 32)
                    && hex(&k_publication_a0, 32)
                    && hex(&k_publication_k0, 32)
                    && hex(&k_publication_q0, 32)
                    && seed["controls"]
                        .get("committed")
                        .is_some_and(Value::is_null),
                "actual cold A Selected externally anchors P0/Accept A0/marker K0 before real boundary reuse"
            );
            select_selected_state(&seed, &original[0], &pair.c, pair.nodes[0].slot, &seed)?;
            a_before_progress(
                &pair,
                &seed,
                &original[0],
                0,
                &k_publication_p0,
                &mut k_publication_observed[0],
                &mut k_publication_shared,
            )?;
            diagnostics.data["k_publication_observed"] = json!(&k_publication_observed);
            diagnostics.data["k_publication_shared"] = k_publication_shared.clone();
            k_publication_isolated_a = select_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &seed,
                &mut diagnostics,
            )?;
            diagnostics.data["k_publication_isolated_A"] = k_publication_isolated_a.clone();
            for stage in ["before", "after"] {
                a_before_progress(
                    &pair,
                    &k_publication_isolated_a[stage],
                    &original[0],
                    0,
                    &k_publication_p0,
                    &mut k_publication_observed[0],
                    &mut k_publication_shared,
                )?;
                diagnostics.data["k_publication_observed"] = json!(&k_publication_observed);
                diagnostics.data["k_publication_shared"] = k_publication_shared.clone();
            }
            note(
                &mut diagnostics,
                "isolated A Selected retains marker K0/cold P0/Accept A0 and first Q0/Mixed7/history",
                k_publication_isolated_a.clone(),
            )?;
            k_publication_isolated_a["after"].clone()
        } else if select_before_commit {
            k_before_isolated_a = a_before_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &mut diagnostics,
            )?;
            for stage in ["before", "after"] {
                a_before_progress(
                    &pair,
                    &k_before_isolated_a[stage],
                    &original[0],
                    0,
                    &k_before_p0,
                    &mut k_before_observed[0],
                    &mut k_before_shared,
                )?;
            }
            diagnostics.data["k_before_observed"] = json!(&k_before_observed);
            diagnostics.data["k_before_shared"] = k_before_shared.clone();
            note(
                &mut diagnostics,
                "isolated A Offering retains marker P0/original Native; candidate Select was not stored",
                k_before_isolated_a.clone(),
            )?;
            k_before_isolated_a["after"].clone()
        } else if accept_before_publication {
            a_publication_isolated_a = a_before_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &mut diagnostics,
            )?;
            for stage in ["before", "after"] {
                a_before_progress(
                    &pair,
                    &a_publication_isolated_a[stage],
                    &original[0],
                    0,
                    &a_publication_p0,
                    &mut a_publication_observed[0],
                    &mut a_publication_shared,
                )?;
            }
            diagnostics.data["a_publication_observed"] = json!(&a_publication_observed);
            diagnostics.data["a_publication_shared"] = a_publication_shared.clone();
            note(
                &mut diagnostics,
                "isolated A Offering retains cold B seed P0 without receiving publication A0",
                a_publication_isolated_a.clone(),
            )?;
            a_publication_isolated_a["after"].clone()
        } else if accept_before_commit {
            a_before_isolated_a = a_before_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &mut diagnostics,
            )?;
            a_before_p0 = a_before_isolated_a["before"]["controls"]["p"].clone();
            diagnostics.data["a_before_p0"] = a_before_p0.clone();
            for stage in ["before", "after"] {
                a_before_progress(
                    &pair,
                    &a_before_isolated_a[stage],
                    &original[0],
                    0,
                    &a_before_p0,
                    &mut a_before_observed[0],
                    &mut a_before_shared,
                )?;
            }
            diagnostics.data["a_before_observed"] = json!(&a_before_observed);
            diagnostics.data["a_before_shared"] = a_before_shared.clone();
            note(
                &mut diagnostics,
                "isolated A Offering with first actual saved P0; no held-B P anchor",
                a_before_isolated_a.clone(),
            )?;
            a_before_isolated_a["after"].clone()
        } else if accept_after_commit {
            accept_isolated_a =
                accept_cut_witness(&mut pair, 0, &original[0], &original_history[0], &boundary)?;
            accept_saved_controls[0] = accept_isolated_a["after"]["controls"].clone();
            diagnostics.data["accept_saved_controls"] = json!(&accept_saved_controls);
            note(
                &mut diagnostics,
                "isolated A durable Offering P without Accept",
                accept_isolated_a.clone(),
            )?;
            accept_isolated_a["after"].clone()
        } else if select_after_commit {
            select_isolated_a = select_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &boundary,
                &mut diagnostics,
            )?;
            select_saved_controls[0] = select_isolated_a["before"]["controls"].clone();
            for stage in ["before", "after"] {
                let row = &select_isolated_a[stage];
                remember_controls(&mut select_saved_controls[0], row)?;
                select_refs(row, 0, &boundary, &mut select_observed_q[0])?;
                select_native(
                    row,
                    &original[0],
                    0,
                    &boundary,
                    &mut select_converted_native[0],
                )?;
            }
            diagnostics.data["select_saved_controls"] = json!(&select_saved_controls);
            diagnostics.data["select_observed_q"] = json!(&select_observed_q);
            diagnostics.data["select_converted_native"] = json!(&select_converted_native);
            note(
                &mut diagnostics,
                "isolated A durable Selected/Mixed7/Q/history view",
                select_isolated_a.clone(),
            )?;
            select_isolated_a["after"].clone()
        } else if committed_after_commit {
            committed_isolated_a = committed_cut_witness(
                &mut pair,
                0,
                &original[0],
                &original_history[0],
                &boundary,
                &mut diagnostics,
            )?;
            committed_saved_controls[0] = committed_isolated_a["before"]["controls"].clone();
            for stage in ["before", "after"] {
                let row = &committed_isolated_a[stage];
                remember_controls(&mut committed_saved_controls[0], row)?;
                committed_refs(row, 0, &boundary, &mut committed_observed_q[0])?;
                committed_native(
                    row,
                    &original[0],
                    0,
                    &boundary,
                    &mut committed_saved_native[0],
                )?;
            }
            diagnostics.data["committed_saved_controls"] = json!(&committed_saved_controls);
            diagnostics.data["committed_observed_q"] = json!(&committed_observed_q);
            diagnostics.data["committed_saved_native"] = json!(&committed_saved_native);
            note(
                &mut diagnostics,
                "isolated A durable Selected without Committed; first real cold Native anchor",
                committed_isolated_a.clone(),
            )?;
            committed_isolated_a["after"].clone()
        } else {
            op(None, "first isolated durable A row", || pair.row(0))?
        };
        origin(&isolated, &original[0], 0, &pair.c)?;
        controls(&isolated, false)?;
        let mut publication_history = Value::Null;
        let mut publication_isolated_after = Value::Null;
        match policy {
            CutPolicy::AfterCommit => {
                ensure!(
                    isolated["mode"] == "Offering"
                        && isolated["freeze"] == true
                        && isolated["native_encoding"] == "Legacy7"
                        && isolated["alpn1_allowed"] == false
                        && isolated["control_debt"] == true
                        && isolated["next"] == json!([2, 2])
                        && empty(&isolated, 1)
                        && isolated["controls"] == boundary["controls"]
                        && isolated["q"] == boundary["q"]
                        && isolated["cuts"] == boundary["cuts"]
                        && isolated["native_hash"] == boundary["native_hash"],
                    "isolated exact durable P witness before B start"
                );
            }
            CutPolicy::BeforeCommit => {
                ensure!(
                    isolated["controls"] == boundary["controls"]
                        && isolated["q"] == boundary["q"]
                        && isolated["cuts"] == boundary["cuts"]
                        && isolated["native_hash"] == boundary["native_hash"],
                    "isolated A retains absent P and original Native, with durable peer intent"
                );
            }
            CutPolicy::BeforePublication => {
                saved_offering_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &boundary["control_ref"],
                )?;
                publication_history = page(&pair, 0, None)?;
                ensure!(
                    publication_history["F"] == json!([2, 2])
                        && publication_history["items"] == original_history[0]["items"],
                    "complete exact original history at isolated saved publication P"
                );
                publication_isolated_after =
                    op(None, "isolated publication P row after history", || {
                        pair.row(0)
                    })?;
                saved_offering_state(
                    &publication_isolated_after,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &boundary["control_ref"],
                )?;
                ensure!(
                    pair.children[1].is_none(),
                    "B absent through publication history witness"
                );
                op(None, "isolated publication A still alive", || {
                    pair.children[0]
                        .as_mut()
                        .context("isolated publication A")?
                        .alive()
                })?;
                note(
                    &mut diagnostics,
                    "independent isolated publication P/history",
                    json!({"before":isolated,"history":publication_history,"after":publication_isolated_after,
                    "held_control_ref":boundary["control_ref"],"held_signed_control_blake3":boundary["signed_control_blake3"]}),
                )?;
            }
            CutPolicy::AcceptAfterCommit => {
                saved_offering_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &boundary["controls"]["p"],
                )?;
            }
            CutPolicy::SelectAfterCommit => {
                select_selected_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &boundary,
                )?;
            }
            CutPolicy::CommittedAfterCommit => {
                committed_selected_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &boundary,
                )?;
            }
            CutPolicy::AcceptBeforeCommit => {
                saved_offering_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &a_before_p0,
                )?;
            }
            CutPolicy::AcceptBeforePublication => {
                saved_offering_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &a_publication_p0,
                )?;
            }
            CutPolicy::SelectBeforeCommit => {
                saved_offering_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &k_before_p0,
                )?;
            }
            CutPolicy::SelectBeforePublication => {
                select_selected_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &k_publication_seed_a["row"],
                )?;
            }
            CutPolicy::CommittedBeforeCommit => {
                select_selected_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &committed_before_seed_a["row"],
                )?;
            }
            CutPolicy::CommittedBeforePublication => {
                committed_selected_state(
                    &isolated,
                    &original[0],
                    &pair.c,
                    pair.nodes[0].slot,
                    &actual_cold_b_anchor,
                )?;
            }
        }
        check(None, "accept isolated witness")?;
        note(
            &mut diagnostics,
            "isolated A durable witness",
            isolated.clone(),
        )?;
        let isolated_pid = pair.children[0].as_ref().context("reopened A")?.0.id();
        op(None, "start B only after A witness", || {
            pair.start(1, recovery_schedule)
        })?;
        started(&pair, 1, "final B ready after A witness", &mut diagnostics)?;
        let accept_final_b_pid = if accept_after_commit {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final Accept-cut B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let select_final_b_pid = if select_after_commit {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final Select-cut B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let committed_final_b_pid = if committed_after_commit {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final Committed-cut B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let a_before_final_b_pid = if accept_before_commit {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final A-before B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let a_publication_final_b_pid = if accept_before_publication {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final A-publication B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let k_before_final_b_pid = if select_before_commit {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final K-before B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let k_publication_final_b_pid = if select_before_publication {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final K-publication B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let committed_before_final_b_pid = if committed_before_commit {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final Committed-before B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let committed_publication_final_b_pid = if committed_before_publication {
            Some(
                pair.children[1]
                    .as_ref()
                    .context("final Committed-publication B")?
                    .0
                    .id(),
            )
        } else {
            None
        };
        let reopened = [
            isolated.clone(),
            op(None, "actual reopened B row", || pair.row(1))?,
        ];
        if committed_before_commit {
            diagnostics.data["committed_before_reopened"] = json!(&reopened);
        }
        if committed_before_publication {
            diagnostics.data["committed_publication_reopened"] = json!(&reopened);
        }
        origin(&reopened[1], &original[1], 1, &pair.c)?;
        controls(&reopened[1], false)?;
        if accept_after_commit {
            accept_saved_refs(&reopened[1], 1, &boundary)?;
        }
        if select_after_commit {
            diagnostics.data["select_reopened"] = json!(&reopened);
            for role in 0..2 {
                select_refs(
                    &reopened[role],
                    role,
                    &boundary,
                    &mut select_observed_q[role],
                )?;
                select_native(
                    &reopened[role],
                    &original[role],
                    role,
                    &boundary,
                    &mut select_converted_native[role],
                )?;
            }
            diagnostics.data["select_observed_q"] = json!(&select_observed_q);
            diagnostics.data["select_converted_native"] = json!(&select_converted_native);
        }
        if committed_after_commit {
            diagnostics.data["committed_reopened"] = json!(&reopened);
            for role in 0..2 {
                committed_refs(
                    &reopened[role],
                    role,
                    &boundary,
                    &mut committed_observed_q[role],
                )?;
                committed_native(
                    &reopened[role],
                    &original[role],
                    role,
                    &boundary,
                    &mut committed_saved_native[role],
                )?;
            }
            diagnostics.data["committed_observed_q"] = json!(&committed_observed_q);
            diagnostics.data["committed_saved_native"] = json!(&committed_saved_native);
        }
        if accept_before_commit {
            diagnostics.data["a_before_reopened"] = json!(&reopened);
            for role in 0..2 {
                a_before_progress(
                    &pair,
                    &reopened[role],
                    &original[role],
                    role,
                    &a_before_p0,
                    &mut a_before_observed[role],
                    &mut a_before_shared,
                )?;
            }
            diagnostics.data["a_before_observed"] = json!(&a_before_observed);
            diagnostics.data["a_before_shared"] = a_before_shared.clone();
        }
        if accept_before_publication {
            diagnostics.data["a_publication_reopened"] = json!(&reopened);
            for role in 0..2 {
                a_before_progress(
                    &pair,
                    &reopened[role],
                    &original[role],
                    role,
                    &a_publication_p0,
                    &mut a_publication_observed[role],
                    &mut a_publication_shared,
                )?;
            }
            diagnostics.data["a_publication_observed"] = json!(&a_publication_observed);
            diagnostics.data["a_publication_shared"] = a_publication_shared.clone();
        }
        if select_before_commit {
            diagnostics.data["k_before_reopened"] = json!(&reopened);
            for role in 0..2 {
                a_before_progress(
                    &pair,
                    &reopened[role],
                    &original[role],
                    role,
                    &k_before_p0,
                    &mut k_before_observed[role],
                    &mut k_before_shared,
                )?;
            }
            diagnostics.data["k_before_observed"] = json!(&k_before_observed);
            diagnostics.data["k_before_shared"] = k_before_shared.clone();
        }
        if select_before_publication {
            diagnostics.data["k_publication_reopened"] = json!(&reopened);
            for role in 0..2 {
                a_before_progress(
                    &pair,
                    &reopened[role],
                    &original[role],
                    role,
                    &k_publication_p0,
                    &mut k_publication_observed[role],
                    &mut k_publication_shared,
                )?;
                diagnostics.data["k_publication_observed"] = json!(&k_publication_observed);
                diagnostics.data["k_publication_shared"] = k_publication_shared.clone();
            }
        }
        if committed_before_commit {
            for role in 0..2 {
                a_before_progress(
                    &pair,
                    &reopened[role],
                    &original[role],
                    role,
                    &committed_before_p0,
                    &mut committed_before_observed[role],
                    &mut committed_before_shared,
                )?;
                diagnostics.data["committed_before_observed"] = json!(&committed_before_observed);
                diagnostics.data["committed_before_shared"] = committed_before_shared.clone();
            }
        }
        if committed_before_publication {
            for role in 0..2 {
                let row = &reopened[role];
                a_before_progress(
                    &pair,
                    row,
                    &original[role],
                    role,
                    &committed_publication_p0,
                    &mut committed_publication_observed[role],
                    &mut committed_publication_shared,
                )?;
                committed_refs(
                    row,
                    role,
                    &actual_cold_b_anchor,
                    &mut committed_publication_observed_q[role],
                )?;
                committed_native(
                    row,
                    &original[role],
                    role,
                    &actual_cold_b_anchor,
                    &mut committed_publication_saved_native[role],
                )?;
                diagnostics.data["committed_publication_observed"] =
                    json!(&committed_publication_observed);
                diagnostics.data["committed_publication_shared"] =
                    committed_publication_shared.clone();
                diagnostics.data["committed_publication_observed_Q"] =
                    json!(&committed_publication_observed_q);
                diagnostics.data["committed_publication_saved_Native"] =
                    json!(&committed_publication_saved_native);
            }
        }
        note(
            &mut diagnostics,
            "final current Locator2 imports start",
            json!({
            "A_imports":pair.nodes[1].locators.join(format!("{}.locator2", pair.c)),
            "B_imports":pair.nodes[0].locators.join(format!("{}.locator2", pair.c))}),
        )?;
        op(None, "new actual Locator2 exchange", || pair.exchange2())?;
        // One T_R after both public imports return; check/op additionally keep
        // the unchanged absolute s53_deadline, so the earlier deadline wins.
        let recovery_started = Instant::now();
        let recovery_end = Some(recovery_started + Duration::from_secs(policy.recovery_seconds()));
        let recovery_started_seconds = recovery_started
            .duration_since(diagnostics.started)
            .as_secs_f64();
        note(
            &mut diagnostics,
            "both final Locator2 imports returned; recovery T_R",
            json!({
            "started_seconds":recovery_started_seconds,"phase_seconds":policy.recovery_seconds(),
            "original_kernel_seconds":160,"both_imports_returned":true}),
        )?;
        let mut observed_controls = [
            reopened[0]["controls"].clone(),
            reopened[1]["controls"].clone(),
        ];
        if accept_after_commit {
            for role in 0..2 {
                observed_controls[role] = accept_saved_controls[role].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if select_after_commit {
            for role in 0..2 {
                observed_controls[role] = select_saved_controls[role].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if committed_after_commit {
            for role in 0..2 {
                observed_controls[role] = committed_saved_controls[role].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if accept_before_commit {
            for role in 0..2 {
                observed_controls[role] = a_before_observed[role]["controls"].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if accept_before_publication {
            for role in 0..2 {
                observed_controls[role] = a_publication_observed[role]["controls"].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if select_before_commit {
            for role in 0..2 {
                observed_controls[role] = k_before_observed[role]["controls"].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if select_before_publication {
            for role in 0..2 {
                observed_controls[role] = k_publication_observed[role]["controls"].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if committed_before_commit {
            for role in 0..2 {
                observed_controls[role] = committed_before_observed[role]["controls"].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        if committed_before_publication {
            for role in 0..2 {
                observed_controls[role] = committed_publication_observed[role]["controls"].clone();
                remember_controls(&mut observed_controls[role], &reopened[role])?;
            }
        }
        diagnostics.data["observed_committed_controls"] = json!(&observed_controls);
        let active = loop {
            let rows = [
                op(recovery_end, "recovery A", || pair.row(0))?,
                op(recovery_end, "recovery B", || pair.row(1))?,
            ];
            diagnostics.data["last_recovery_rows"] = json!(&rows);
            if committed_before_commit {
                diagnostics.data["committed_before_recovery_rows"] = json!(&rows);
            }
            if committed_before_publication {
                diagnostics.data["committed_publication_recovery_rows"] = json!(&rows);
            }
            for role in 0..2 {
                remember_controls(&mut observed_controls[role], &rows[role])?;
            }
            if accept_after_commit {
                for role in 0..2 {
                    origin(&rows[role], &original[role], role, &pair.c)?;
                    accept_saved_refs(&rows[role], role, &boundary)?;
                }
            }
            if select_after_commit {
                for role in 0..2 {
                    origin(&rows[role], &original[role], role, &pair.c)?;
                    select_refs(&rows[role], role, &boundary, &mut select_observed_q[role])?;
                    select_native(
                        &rows[role],
                        &original[role],
                        role,
                        &boundary,
                        &mut select_converted_native[role],
                    )?;
                }
                diagnostics.data["select_observed_q"] = json!(&select_observed_q);
                diagnostics.data["select_converted_native"] = json!(&select_converted_native);
            }
            if committed_after_commit {
                for role in 0..2 {
                    committed_refs(
                        &rows[role],
                        role,
                        &boundary,
                        &mut committed_observed_q[role],
                    )?;
                    committed_native(
                        &rows[role],
                        &original[role],
                        role,
                        &boundary,
                        &mut committed_saved_native[role],
                    )?;
                }
                diagnostics.data["committed_observed_q"] = json!(&committed_observed_q);
                diagnostics.data["committed_saved_native"] = json!(&committed_saved_native);
            }
            if accept_before_commit {
                for role in 0..2 {
                    a_before_progress(
                        &pair,
                        &rows[role],
                        &original[role],
                        role,
                        &a_before_p0,
                        &mut a_before_observed[role],
                        &mut a_before_shared,
                    )?;
                }
                diagnostics.data["a_before_observed"] = json!(&a_before_observed);
                diagnostics.data["a_before_shared"] = a_before_shared.clone();
            }
            if accept_before_publication {
                for role in 0..2 {
                    a_before_progress(
                        &pair,
                        &rows[role],
                        &original[role],
                        role,
                        &a_publication_p0,
                        &mut a_publication_observed[role],
                        &mut a_publication_shared,
                    )?;
                }
                diagnostics.data["a_publication_observed"] = json!(&a_publication_observed);
                diagnostics.data["a_publication_shared"] = a_publication_shared.clone();
            }
            if select_before_commit {
                for role in 0..2 {
                    a_before_progress(
                        &pair,
                        &rows[role],
                        &original[role],
                        role,
                        &k_before_p0,
                        &mut k_before_observed[role],
                        &mut k_before_shared,
                    )?;
                }
                diagnostics.data["k_before_observed"] = json!(&k_before_observed);
                diagnostics.data["k_before_shared"] = k_before_shared.clone();
            }
            if select_before_publication {
                diagnostics.data["k_publication_recovery_rows"] = json!(&rows);
                for role in 0..2 {
                    a_before_progress(
                        &pair,
                        &rows[role],
                        &original[role],
                        role,
                        &k_publication_p0,
                        &mut k_publication_observed[role],
                        &mut k_publication_shared,
                    )?;
                    diagnostics.data["k_publication_observed"] = json!(&k_publication_observed);
                    diagnostics.data["k_publication_shared"] = k_publication_shared.clone();
                }
            }
            if committed_before_commit {
                for role in 0..2 {
                    a_before_progress(
                        &pair,
                        &rows[role],
                        &original[role],
                        role,
                        &committed_before_p0,
                        &mut committed_before_observed[role],
                        &mut committed_before_shared,
                    )?;
                    diagnostics.data["committed_before_observed"] =
                        json!(&committed_before_observed);
                    diagnostics.data["committed_before_shared"] = committed_before_shared.clone();
                }
            }
            if committed_before_publication {
                for role in 0..2 {
                    let row = &rows[role];
                    a_before_progress(
                        &pair,
                        row,
                        &original[role],
                        role,
                        &committed_publication_p0,
                        &mut committed_publication_observed[role],
                        &mut committed_publication_shared,
                    )?;
                    committed_refs(
                        row,
                        role,
                        &actual_cold_b_anchor,
                        &mut committed_publication_observed_q[role],
                    )?;
                    committed_native(
                        row,
                        &original[role],
                        role,
                        &actual_cold_b_anchor,
                        &mut committed_publication_saved_native[role],
                    )?;
                    diagnostics.data["committed_publication_observed"] =
                        json!(&committed_publication_observed);
                    diagnostics.data["committed_publication_shared"] =
                        committed_publication_shared.clone();
                    diagnostics.data["committed_publication_observed_Q"] =
                        json!(&committed_publication_observed_q);
                    diagnostics.data["committed_publication_saved_Native"] =
                        json!(&committed_publication_saved_native);
                }
            }
            diagnostics.data["observed_committed_controls"] = json!(&observed_controls);
            if rows.iter().all(|row| {
                row["mode"] == "Active"
                    && empty(row, 1)
                    && row["control_debt"] == false
                    && live(row)
            }) {
                check(recovery_end, "accept actual recovery")?;
                break rows;
            }
            op(recovery_end, "bounded recovery poll", || {
                std::thread::sleep(Duration::from_millis(50));
                Ok(())
            })?;
        };
        let recovery_elapsed_seconds = recovery_started.elapsed().as_secs_f64();
        note(
            &mut diagnostics,
            "actual recovery accepted",
            json!({"elapsed_seconds":recovery_elapsed_seconds,"rows":active}),
        )?;
        if committed_before_commit {
            diagnostics.data["committed_before_final_pre_data"] = json!(&active);
        }
        if committed_before_publication {
            diagnostics.data["committed_publication_final_pre_data"] = json!(&active);
        }
        for role in 0..2 {
            origin(&active[role], &original[role], role, &pair.c)?;
            controls(&active[role], true)?;
            ensure!(
                active[role]["next"] == json!([2, 2])
                    && active[role]["cuts"] == json!([2, 2])
                    && hex(&active[role]["q"], 32)
                    && active[role]["alpn1_allowed"] == false,
                "original P/Q/cuts after recovery"
            );
            if policy == CutPolicy::AfterCommit {
                ensure!(
                    active[role]["controls"]["p"] == boundary["controls"]["p"],
                    "after-commit preserves its exact saved P"
                );
            }
            if policy == CutPolicy::BeforePublication {
                ensure!(
                    active[role]["controls"]["p"] == boundary["control_ref"],
                    "publication held ref equals isolated durable P and final P at both peers"
                );
            }
            if accept_after_commit {
                accept_saved_refs(&active[role], role, &boundary)?;
            }
            if select_after_commit {
                select_refs(&active[role], role, &boundary, &mut select_observed_q[role])?;
                select_native(
                    &active[role],
                    &original[role],
                    role,
                    &boundary,
                    &mut select_converted_native[role],
                )?;
            }
            if committed_after_commit {
                committed_refs(
                    &active[role],
                    role,
                    &boundary,
                    &mut committed_observed_q[role],
                )?;
                committed_native(
                    &active[role],
                    &original[role],
                    role,
                    &boundary,
                    &mut committed_saved_native[role],
                )?;
            }
            if accept_before_commit {
                a_before_progress(
                    &pair,
                    &active[role],
                    &original[role],
                    role,
                    &a_before_p0,
                    &mut a_before_observed[role],
                    &mut a_before_shared,
                )?;
            }
            if accept_before_publication {
                a_before_progress(
                    &pair,
                    &active[role],
                    &original[role],
                    role,
                    &a_publication_p0,
                    &mut a_publication_observed[role],
                    &mut a_publication_shared,
                )?;
                diagnostics.data["a_publication_observed"] = json!(&a_publication_observed);
                diagnostics.data["a_publication_shared"] = a_publication_shared.clone();
            }
            if select_before_commit {
                a_before_progress(
                    &pair,
                    &active[role],
                    &original[role],
                    role,
                    &k_before_p0,
                    &mut k_before_observed[role],
                    &mut k_before_shared,
                )?;
                diagnostics.data["k_before_observed"] = json!(&k_before_observed);
                diagnostics.data["k_before_shared"] = k_before_shared.clone();
            }
            if select_before_publication {
                diagnostics.data["k_publication_final_pre_data"] = json!(&active);
                a_before_progress(
                    &pair,
                    &active[role],
                    &original[role],
                    role,
                    &k_publication_p0,
                    &mut k_publication_observed[role],
                    &mut k_publication_shared,
                )?;
                diagnostics.data["k_publication_observed"] = json!(&k_publication_observed);
                diagnostics.data["k_publication_shared"] = k_publication_shared.clone();
            }
            if committed_before_commit {
                a_before_progress(
                    &pair,
                    &active[role],
                    &original[role],
                    role,
                    &committed_before_p0,
                    &mut committed_before_observed[role],
                    &mut committed_before_shared,
                )?;
                diagnostics.data["committed_before_observed"] = json!(&committed_before_observed);
                diagnostics.data["committed_before_shared"] = committed_before_shared.clone();
            }
            if committed_before_publication {
                let row = &active[role];
                a_before_progress(
                    &pair,
                    row,
                    &original[role],
                    role,
                    &committed_publication_p0,
                    &mut committed_publication_observed[role],
                    &mut committed_publication_shared,
                )?;
                committed_refs(
                    row,
                    role,
                    &actual_cold_b_anchor,
                    &mut committed_publication_observed_q[role],
                )?;
                committed_native(
                    row,
                    &original[role],
                    role,
                    &actual_cold_b_anchor,
                    &mut committed_publication_saved_native[role],
                )?;
                diagnostics.data["committed_publication_observed"] =
                    json!(&committed_publication_observed);
                diagnostics.data["committed_publication_shared"] =
                    committed_publication_shared.clone();
                diagnostics.data["committed_publication_observed_Q"] =
                    json!(&committed_publication_observed_q);
                diagnostics.data["committed_publication_saved_Native"] =
                    json!(&committed_publication_saved_native);
            }
            for saved in ["p", "a", "k", "committed"] {
                if !reopened[role]["controls"][saved].is_null() {
                    ensure!(
                        active[role]["controls"][saved] == reopened[role]["controls"][saved],
                        "saved controls never recut/resealed"
                    );
                }
            }
        }
        ensure!(
            active[0]["q"] == active[1]["q"] && active[0]["controls"] == active[1]["controls"],
            "one complete selected transcript"
        );
        note(
            &mut diagnostics,
            "original400/401 delivery starts",
            Value::Null,
        )?;
        let text = "first real mixed Text after lost control";
        let delivery_started = Instant::now();
        let delivery_end = Some(delivery_started + Duration::from_secs(30));
        let mut sends = Vec::new();
        for role in 0..2 {
            let response = op(delivery_end, "one original mixed Text", || {
                pair.nodes[role].send(400 + role as u64, text)
            })?;
            sends.push(serde_json::from_str::<Value>(&response)?);
        }
        let (final_rows, history, requests) = loop {
            let rows = [
                op(delivery_end, "delivery A", || pair.row(0))?,
                op(delivery_end, "delivery B", || pair.row(1))?,
            ];
            let requests = [
                request(&pair, 0, delivery_end)?,
                request(&pair, 1, delivery_end)?,
            ];
            let history = [page(&pair, 0, delivery_end)?, page(&pair, 1, delivery_end)?];
            diagnostics.data["last_delivery"] =
                json!({"rows":&rows,"requests":&requests,"history":&history});
            let mut delivered = rows.iter().all(|row| {
                row["mode"] == "Active"
                    && row["next"] == json!([3, 3])
                    && empty(row, 2)
                    && row["control_debt"] == false
                    && live(row)
            });
            for role in 0..2 {
                origin(&rows[role], &original[role], role, &pair.c)?;
                controls(&rows[role], true)?;
                ensure!(
                    rows[role]["controls"] == active[role]["controls"]
                        && rows[role]["q"] == active[role]["q"]
                        && rows[role]["cuts"] == active[role]["cuts"]
                        && rows[role]["alpn1_allowed"] == false
                        && rows[role]["native_encoding"] == "Mixed7",
                    "stable selected scope during delivery"
                );
                for (lane, request) in requests.iter().enumerate() {
                    let old = source(&original_history[role], lane, None)?
                        .context("captured original")?;
                    let now =
                        source(&history[role], lane, None)?.context("old Text still readable")?;
                    event(&now, 1, 1)?;
                    ensure!(
                        now["id"] == old["id"]
                            && now["request"] == old["request"]
                            && now["text"] == old["text"]
                            && now["delivered"] == true,
                        "original history after crash/recovery"
                    );
                    if let Some(now) = source(&history[role], lane, Some(400 + lane as u64))? {
                        event(&now, 2, 2)?;
                        ensure!(
                            now["text"] == text && now["proof_kind"] == "FullMixed",
                            "original new readable Text"
                        );
                        delivered &= now["delivered"] == true
                            && request.1
                            && request.0.as_ref().is_some_and(|id| now["id"] == *id);
                    } else {
                        delivered = false;
                    }
                }
                delivered &= history[role]["items"]
                    .as_array()
                    .is_some_and(|items| items.len() == 4);
            }
            check(
                delivery_end,
                "accept both original deliveries/history/receipts",
            )?;
            if delivered {
                break (rows, history, requests.map(|r| r.2));
            }
            op(delivery_end, "one shared delivery poll", || {
                std::thread::sleep(Duration::from_millis(50));
                Ok(())
            })?;
        };
        let delivery_elapsed_seconds = delivery_started.elapsed().as_secs_f64();
        if select_after_commit {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
            }
        }
        if committed_after_commit {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
            }
        }
        if accept_before_commit {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
            }
        }
        if accept_before_publication {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
            }
        }
        if select_before_commit {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
            }
        }
        if select_before_publication {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
            }
        }
        if committed_before_commit {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
            }
        }
        if committed_before_publication {
            for role in 0..2 {
                select_history_view(&history[role], &original_history[role])?;
                ensure!(
                    history[role]["F"] == json!([3, 3])
                        && final_rows[role]["wire_stage"] == 5
                        && final_rows[role]["freeze"] == false
                        && final_rows[role]["native"]["own_next"] == 3
                        && final_rows[role]["native"]["peer_next"] == 3
                        && final_rows[role]["native_generation"].as_u64().is_some_and(
                            |generation| {
                                active[role]["native_generation"]
                                    .as_u64()
                                    .is_some_and(|before| generation >= before)
                            }
                        ),
                    "final own view/frontiers/Active authority after legitimate Native data advancement"
                );
                for lane in 0..2 {
                    let original_item = source(&original_history[role], lane, None)?
                        .context("Committed-publication original source history")?;
                    let old = source(&history[role], lane, None)?
                        .context("Committed-publication final old source history")?;
                    let mixed = source(&history[role], lane, Some(400 + lane as u64))?
                        .context("Committed-publication final original mixed request")?;
                    ensure!(
                        old["sequence"] == original_item["sequence"]
                            && old["proof_kind"] == original_item["proof_kind"]
                            && mixed["sequence"] == 2,
                        "old source sequence/proof retained and original400/401 have actual sequence2"
                    );
                }
            }
        }
        let final_deaths = stop(&mut pair)?;
        note(
            &mut diagnostics,
            "final deaths complete",
            final_deaths.clone(),
        )?;
        drop(pair);
        let classification = match policy {
            CutPolicy::BeforeCommit => {
                "one real P-before-commit cut; durable intents, new committed controls and original mixed delivery"
            }
            CutPolicy::AfterCommit => {
                "one real P-after-commit durable cut; isolated reopen and original mixed delivery"
            }
            CutPolicy::BeforePublication => {
                "one real P-before-publication cut; paired producer fingerprint and independent durable P ref"
            }
            CutPolicy::AcceptAfterCommit => {
                "one real Accept-after-commit cut on original B; independent cold Prepared/Offering and saved P/Accept"
            }
            CutPolicy::SelectAfterCommit => {
                "one real Select-after-commit cut on original A; cold Prepared/Selected, saved K/Q and one conversion before data"
            }
            CutPolicy::CommittedAfterCommit => {
                "one real Committed-after-commit cut on original B; cold Active/Selected, saved confirmation and unchanged converted Native before data"
            }
            CutPolicy::AcceptBeforeCommit => {
                "one real Accept-before-commit cut on B; cold L1/Offering, saved P0 and regenerated unsaved Accept"
            }
            CutPolicy::AcceptBeforePublication => {
                "one real Accept-before-publication cut on B transport; cold Prepared/Offering, marker A0 and cold seed P0 retained"
            }
            CutPolicy::SelectBeforeCommit => {
                "one real Select-before-commit cut on A owner; cold Prepared/Offering, marker P0 and cold seed A0 retained"
            }
            CutPolicy::SelectBeforePublication => {
                "one real Select-before-publication cut on A transport; cold Prepared/Selected, marker K0 and actual seed P0/Accept A0/Q0 retained"
            }
            CutPolicy::CommittedBeforeCommit => {
                "one real Committed-before-commit cut on B owner; cold Prepared/Selected, marker P0/Accept A0 and actual cold A saved K0/Q0 retained"
            }
            CutPolicy::CommittedBeforePublication => {
                "one real Committed-before-publication cut on B incoming transport; cold Active/Selected, marker C0 and actual cold B P0/Accept A0/K0/Q0 retained"
            }
        };
        let publication_evidence = if policy == CutPolicy::BeforePublication {
            json!({"profiles":publication_profiles,"armed_plan":publication_plan,
                "complete_owned_trace":publication_trace_observation,"held_producer_pair":boundary,
                "isolated_history":publication_history,"isolated_row_after_history":publication_isolated_after,
                "witness_claim":"same-raw producer pair plus independent cold durable content ref and matching final transcript",
                "digest_readback_claim":"no independent plain-BLAKE3 readback; keyed retry lookup and saved-signature validation"})
        } else {
            Value::Null
        };
        let accept_evidence = if accept_after_commit {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual Accept-cut process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual Accept-cut runtime incarnations within the existing cap"
            );
            json!({"victim_role":1,"profiles":accept_profiles,"armed_plan":accept_plan,
                "held_marker_facts":boundary,"cut_death_order":[1,0],"cut_deaths":deaths,
                "isolated_B_Prepared":accept_isolated_b,"isolated_B_death":accept_isolated_b_death,
                "isolated_A_Offering":accept_isolated_a,"saved_controls_across_restarts":accept_saved_controls,
                "final_B_pid":accept_final_b_pid,
                "witness_claim":"held P/Accept refs equal independent cold B; same P at cold A; retained refs and matching final transcript",
                "selected_consent_claim":"unchanged Prepare and cold validation path; current public Consent2 is authority, no invented selected field",
                "raw_control_readback_claim":"no independent raw P/Accept export or disk digest readback"})
        } else {
            Value::Null
        };
        let select_evidence = if select_after_commit {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual Select-cut process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual Select-cut runtime incarnations within the existing cap"
            );
            json!({"victim_role":0,"profiles":select_profiles,"armed_plan":select_plan,
                "held_marker_facts":boundary,"cut_death_order":[0,1],"cut_deaths":deaths,
                "isolated_B_Prepared":select_isolated_b,"isolated_B_death":select_isolated_b_death,
                "isolated_A_Selected":select_isolated_a,"final_B_pid":select_final_b_pid,
                "saved_controls_across_restarts":select_saved_controls,
                "observed_Q_before_data":select_observed_q,"converted_Native_before_data":select_converted_native,
                "witness_claim":"real cold Prepared peer then cold Selected owner; exact held P/Accept/K/Q and per-peer history view/items",
                "native_claim":"A holds marker hash/original+1; B holds Legacy until Active then first Mixed hash/original+1; no Native freeze after Text",
                "held_RPC_claim":"reviewed synchronous K checkpoint after returned Accept, before outer Select/RPC; no global network-silence claim",
                "selected_consent_claim":"saved proof/Q and cold validation path; current public Consent2 is authority, no invented selected field",
                "raw_readback_claim":"no independent raw controls/Native/Seen export or disk digest readback"})
        } else {
            Value::Null
        };
        let committed_evidence = if committed_after_commit {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual Committed-cut process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual Committed-cut runtime incarnations within the existing cap"
            );
            json!({"victim_role":1,"profiles":committed_profiles,"armed_plan":committed_plan,
                "held_marker_facts":boundary,"cut_death_order":[1,0],"cut_deaths":deaths,
                "isolated_B_Active":committed_isolated_b,"isolated_B_death":committed_isolated_b_death,
                "isolated_A_Selected":committed_isolated_a,"final_B_pid":committed_final_b_pid,
                "saved_controls_across_restarts":committed_saved_controls,
                "observed_Q_before_data":committed_observed_q,
                "Native_and_debt_before_data":committed_saved_native,
                "witness_claim":"cold B Active/debt=true then cold A Selected/no Committed; held P/Accept/K/Q and per-peer complete history view/items",
                "native_claim":"B holds real marker Native; A holds first real cold-row Native; each original+1/lineage; cleared debt never regresses; no post-Text freeze",
                "A_anchor_limit":"no independent byte equality to an unobserved pre-cut A ciphertext",
                "held_RPC_claim":"reviewed synchronous CommitB checkpoint before raw return/outer reply/write; no global network-silence claim",
                "retry_claim":"actual saved K retry and saved Committed return, then Activate/Head debt clearance without another Native conversion",
                "selected_consent_claim":"saved proof/Q and cold validation; current public Consent2 is authority, no invented selected field",
                "raw_readback_claim":"no independent raw controls/Native/Seen export or disk digest readback"})
        } else {
            Value::Null
        };
        let a_before_evidence = if accept_before_commit {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual A-before process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual A-before runtime incarnations within the existing cap"
            );
            json!({
                "victim_role":1,"profiles":a_before_profiles,"armed_plan":a_before_plan,
                "held_marker_facts":boundary,"cut_death_order":[1,0],"cut_deaths":deaths,
                "isolated_B_L1":a_before_isolated_b,
                "isolated_B_death":a_before_isolated_b_death,
                "isolated_A_Offering":a_before_isolated_a,
                "final_B_pid":a_before_final_b_pid,
                "P0":a_before_p0,
                "per_peer_pre_data":a_before_observed,
                "first_observed_durable_refs_and_Q":a_before_shared,
                "witness_claim":"cold B L1/NotificationSentUnconfirmed then cold A Offering/CoalescedWithP; exact own original histories/views",
                "P0_anchor_limit":"first actual cold A before-row P ref; held B marker has no independent P anchor",
                "discarded_Accept_limit":"candidate not stored or publicly fingerprinted; no equality/inequality oracle for its unobserved identity",
                "retry_claim":"real fresh (2,1) admission; saved P/selected Consent retransmission and newly stored Accept, then normal Select/CommitB/Activate/Head",
                "native_claim":"original Legacy until conversion, then own original+1 and first actual Mixed hash retained on every observed pre-data row; no post-Text freeze",
                "debt_claim":"B L1false to Preparedtrue is legal; cleared-debt latch only within B Active; A Offering/Selectedtrue and Activefalse",
                "sequential_rows_claim":"monotonic progress per peer; no simultaneous admission-mode pair inferred from sequential row RPC",
                "authority_claim":"actual unchanged current Consent2 plus saved-control/proof validation; no invented selected_consent/local_intent public field",
                "held_RPC_claim":"synchronous before-storage Prepare checkpoint before raw return/outer Accept/write; no owner IPC through cut deaths",
                "raw_readback_claim":"no private control/Native/Seen export or independent ciphertext digest readback"
            })
        } else {
            Value::Null
        };
        let a_publication_evidence = if accept_before_publication {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual A-publication process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual A-publication runtime incarnations within the existing cap"
            );
            json!({
                "victim_role":1,"profiles":a_publication_profiles,"armed_plan":a_publication_plan,
                "held_publication_producer_pair":boundary,
                "cut_death_order":[1,0],"cut_deaths":deaths,
                "actual_cold_B_seed":a_publication_seed_b,
                "isolated_B_Prepared":a_publication_isolated_b,
                "isolated_B_death":a_publication_isolated_b_death,
                "isolated_A_Offering":a_publication_isolated_a,
                "final_B_pid":a_publication_final_b_pid,
                "P0":a_publication_p0,"A0":a_publication_a0,
                "per_peer_pre_data":a_publication_observed,
                "first_observed_durable_refs_and_Q":a_publication_shared,
                "witness_claim":"actual B publication A0 equals cold stored Accept; seed P0 matches both cold peers and final transcript; exact own old histories/views",
                "P0_anchor_limit":"first actual cold B seed P ref; three-field publication marker exports A0 and signed-control digest, no independent P field",
                "A0_anchor_claim":"independent actual transport publication ref compared to genuine cold B saved A, then retained by separate shared/per-peer trackers",
                "retry_claim":"real fresh (2,3) admission; saved P/selected Consent retransmission and saved Accept return before signing/new Prepare; fresh outer binding",
                "native_claim":"original Legacy until conversion, then own original+1 and first actual Mixed hash retained on every observed pre-data row; no post-Text freeze",
                "debt_claim":"A Offering/Selectedtrue and Activefalse; B starts Preparedtrue; nonregressing cleared-debt latch only within B Active",
                "sequential_rows_claim":"monotonic per-peer sampled progress, legal skipped samples; no simultaneous admission-mode pair inferred from sequential row RPC",
                "authority_claim":"actual unchanged shared current Consent2 and saved-control/proof validation; no invented selected_consent/local_intent public fields",
                "held_transport_claim":"transport poll after durable Prepare/raw owner reply and before outer Accept write; Owner7 not held; no owner IPC from sole B41 through both cut deaths",
                "raw_readback_claim":"producer signed_control_blake3 retained; no private stored-control/Native/Seen export or independent disk digest readback; no global network-silence claim"
            })
        } else {
            Value::Null
        };
        let k_before_evidence = if select_before_commit {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual K-before process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual K-before runtime incarnations within the existing cap"
            );
            json!({
                "victim_role":0,"profiles":k_before_profiles,"armed_plan":k_before_plan,
                "held_marker_facts":boundary,"cut_death_order":[0,1],"cut_deaths":deaths,
                "actual_cold_B_seed":k_before_seed_b,
                "isolated_B_Prepared":k_before_isolated_b,
                "isolated_B_death":k_before_isolated_b_death,
                "isolated_A_Offering":k_before_isolated_a,
                "final_B_pid":k_before_final_b_pid,
                "P0":k_before_p0,"A0":k_before_a0,
                "per_peer_pre_data":k_before_observed,
                "first_observed_durable_refs_and_Q":k_before_shared,
                "witness_claim":"real held A Offering/P-only/original Native equals cold A; same marker P0 at cold B Prepared with saved Accept; exact own old histories/views",
                "anchor_claim":"P0 independently anchored by held A repository marker; A0 first anchored by actual cold B durable seed, no independent pre-cut Accept marker/digest",
                "candidate_limit":"candidate K/proof/Q/converted bytes existed only in memory before typed Select call; no equality/inequality against their unobserved identities",
                "retry_claim":"fresh (2,3) admission; saved P/selected Consent retransmission and saved Accept return, then rebuilt Select and normal CommitB/Activate/Head; fresh outer binding",
                "native_claim":"one durable conversion per peer, original Legacy until conversion then own original+1/first Mixed hash retained before400/401; discarded candidate may be recomputed; no post-Text freeze",
                "debt_claim":"A Offering/Selectedtrue and Activefalse; B starts Preparedtrue; nonregressing cleared-debt latch only within B Active",
                "sequential_rows_claim":"monotonic per-peer sampled progress with legal skipped states; no simultaneous admission-mode pair inferred from sequential row RPC",
                "authority_claim":"actual unchanged shared current Consent2 plus current-authority/live gates and historical selected-proof validation; no invented selected_consent/local_intent fields",
                "held_owner_claim":"synchronous original A Select checkpoint before repository.commit_existing/outer Select construction; no parent owner IPC from sole B41 through both completed deaths",
                "raw_readback_claim":"no private raw controls/Native/Seen export or independent ciphertext digest readback; no global network-silence claim"
            })
        } else {
            Value::Null
        };
        let k_publication_evidence = if select_before_publication {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual K-publication process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual K-publication runtime incarnations within the existing cap"
            );
            json!({
                "victim_role":0,"profiles":k_publication_profiles,"armed_plan":k_publication_plan,
                "held_marker_facts":boundary,"cut_death_order":[0,1],"cut_deaths":deaths,
                "actual_cold_B_seed":k_publication_seed_b,
                "actual_cold_A_seed":k_publication_seed_a,
                "isolated_B_Prepared":k_publication_isolated_b,
                "isolated_B_death":k_publication_isolated_b_death,
                "isolated_A_Selected":k_publication_isolated_a,
                "cold_partials":diagnostics.data["select_cold_partial"],
                "final_B_pid":k_publication_final_b_pid,
                "K0":k_publication_k0,"DK0":k_publication_dk0,
                "P0":k_publication_p0,"A0":k_publication_a0,"Q0":k_publication_q0,
                "per_peer_pre_data":k_publication_observed,
                "first_observed_durable_refs_and_Q":k_publication_shared,
                "first_observed_Committed":k_publication_shared["controls"]["committed"],
                "witness_claim":"real cold B Prepared/Legacy first anchors P0/Accept A0; real cold A Selected matches P0/A0/independent marker K0 and first anchors Q0/own Mixed hash; exact own old histories/views",
                "anchor_claim":"only K0/DK0 are exported by the three-field producer marker; no pre-cut P/Accept/Q/Native observation or synthesized Selected boundary",
                "retry_claim":"fresh (4,3) current admission; saved K reused in a new outer binding, normal CommitB/Activate/Head; no rebuilt K or repeated A conversion",
                "native_claim":"A already durably converted at the hold, cold A original+1/first own Mixed hash retained before400/401; B original Legacy until its sole conversion, then original+1/first own Mixed retained; no post-Text freeze or unobserved held-hash comparison",
                "debt_claim":"A starts Selectedtrue then Activefalse; B starts Preparedtrue, nonregressing cleared-debt latch only within B Active",
                "sequential_rows_claim":"monotonic per-peer sampled progress with legal skipped states; no simultaneous admission-mode pair inferred from sequential row RPC",
                "authority_claim":"actual unchanged shared current Consent2 plus current-authority/live gates and historical selected-proof validation; no invented selected_consent/local_intent fields",
                "held_transport_claim":"outgoing A transport poll after typed Select/converted commit and returned signed outer dispatch, before Select trace/renewed lease gate/open_bidi/write; Owner7 not held; no parent owner IPC from sole B41 through both completed deaths",
                "hint_claim":"Prepared B may retain an opaque peer_q_hint while public Q remains null and Native Legacy; no hint-absence or all-background-write absence claim",
                "raw_readback_claim":"producer signed_control_blake3 retained; no private stored-control/Native/Seen export or independent disk digest readback/recalculation; no global network-silence claim"
            })
        } else {
            Value::Null
        };
        let committed_before_evidence = if committed_before_commit {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual Committed-before process records")?;
            ensure!(
                processes.len() == 7
                    && processes.iter().filter(|entry| entry["role"] == 0).count() == 3
                    && processes.iter().filter(|entry| entry["role"] == 1).count() == 4,
                "seven actual Committed-before runtime incarnations within the existing cap"
            );
            json!({
                "victim_role":1,"profiles":committed_before_profiles,"armed_plan":committed_before_plan,
                "held_marker_facts":boundary,"cut_death_order":[1,0],"cut_deaths":deaths,
                "actual_cold_B_header":committed_before_cold_b,
                "isolated_B_Prepared":committed_before_isolated_b,
                "isolated_B_death":committed_before_isolated_b_death,
                "actual_cold_A_seed":committed_before_seed_a,
                "isolated_A_Selected":committed_before_isolated_a,
                "cold_partials":diagnostics.data["select_cold_partial"],
                "final_B_pid":committed_before_final_b_pid,
                "P0":committed_before_p0,"A0":committed_before_a0,
                "K0":committed_before_k0,"Q0":committed_before_q0,
                "per_peer_pre_data":committed_before_observed,
                "first_observed_durable_refs_and_Q":committed_before_shared,
                "first_observed_Committed":committed_before_shared["controls"]["committed"],
                "witness_claim":"actual held B Prepared/P+Accept/original Native matches cold B; actual cold A Selected matches independent marker P0/A0 and first anchors saved K0/Q0/own Mixed hash; exact own old histories/views",
                "anchor_claim":"only saved P0/Accept A0/original B hash are exported by the six-field marker; first K0/Q0/A Mixed hash come from actual cold A, first Committed from later durable observation; no pre-cut K/Q or candidate fingerprint",
                "candidate_limit":"B proof/converted Native+Seen/signed Committed candidates existed only in memory before typed CommitB storage; recomputation is allowed, no equality/inequality against unobserved candidate bytes/refs",
                "retry_claim":"fresh (4,3) current admission; saved K reused in a new outer binding, B recomputes unsaved candidates under normal gates then commits once, normal Activate/Head; no rebuilt K or repeated A conversion",
                "native_claim":"A already durably converted before the B hold; first cold A original+1/own Mixed hash retained before400/401; B marker/cold original Legacy until one durable original+1 conversion then first own Mixed retained; candidate computations are not persisted conversions, no post-Text freeze",
                "debt_claim":"A starts Selectedtrue then Activefalse; B starts Preparedtrue, nonregressing cleared-debt latch only within B Active",
                "sequential_rows_claim":"monotonic per-peer sampled progress with legal skipped states; no simultaneous admission-mode pair inferred from sequential row RPC",
                "authority_claim":"actual unchanged shared current Consent2 plus current-authority/live gates and historical selected-proof validation; no invented selected_consent/local_intent fields",
                "held_owner_claim":"synchronous original B job after validated incoming K and candidate computation, before repository.commit_existing/outer Committed dispatch; late storage/final transaction gates not claimed executed at the marker; no parent owner IPC from sole B41 through both completed deaths",
                "hint_claim":"Prepared B may retain an earlier opaque peer_q_hint while public Q remains null and Native Legacy; no hint-absence or all-background-write absence claim",
                "raw_readback_claim":"no private stored-control/Native/Seen export, independent ciphertext digest readback or comparison to an unexported Committed candidate; no global network-silence claim"
            })
        } else {
            Value::Null
        };
        let traces = trace_files(&diagnostics.data);
        let committed_publication_evidence = if committed_before_publication {
            let processes = diagnostics.data["processes"]
                .as_array()
                .context("actual Committed-publication process records")?;
            let expected_roles = [0_u64, 1, 0, 1, 1, 0, 1];
            ensure!(
                processes.len() == expected_roles.len()
                    && processes.iter().zip(expected_roles).all(|(process, role)| {
                        process["role"] == role
                            && process["pid"].as_u64().is_some_and(|pid| pid > 0)
                    }),
                "seven actual ordered Committed-publication incarnations A3/B4"
            );
            let final_b_pid = committed_publication_final_b_pid
                .context("actual final Committed-publication B PID")?;
            ensure!(
                initial_deaths
                    .as_array()
                    .is_some_and(|rows| rows.len() == 2)
                    && deaths.as_array().is_some_and(|rows| rows.len() == 2)
                    && final_deaths.as_array().is_some_and(|rows| rows.len() == 2)
                    && initial_deaths[0]["role"] == 0
                    && initial_deaths[0]["pid"] == processes[0]["pid"]
                    && initial_deaths[0]["kill_wait_completed"] == true
                    && initial_deaths[1]["role"] == 1
                    && initial_deaths[1]["pid"] == processes[1]["pid"]
                    && initial_deaths[1]["kill_wait_completed"] == true
                    && deaths[0]["role"] == 1
                    && deaths[0]["pid"] == processes[3]["pid"]
                    && deaths[0]["kill_wait_completed"] == true
                    && deaths[1]["role"] == 0
                    && deaths[1]["pid"] == processes[2]["pid"]
                    && deaths[1]["kill_wait_completed"] == true
                    && committed_publication_seed_b["pid"] == processes[4]["pid"]
                    && committed_publication_isolated_b["pid"] == processes[4]["pid"]
                    && committed_publication_isolated_b_death["pid"] == processes[4]["pid"]
                    && committed_publication_isolated_b_death["role"] == 1
                    && committed_publication_isolated_b_death["kill_wait_completed"] == true
                    && committed_publication_cold_a["pid"] == processes[5]["pid"]
                    && committed_publication_isolated_a["pid"] == processes[5]["pid"]
                    && processes[5]["pid"] == isolated_pid
                    && processes[6]["pid"] == final_b_pid
                    && final_deaths[0]["role"] == 0
                    && final_deaths[0]["pid"] == processes[5]["pid"]
                    && final_deaths[0]["kill_wait_completed"] == true
                    && final_deaths[1]["role"] == 1
                    && final_deaths[1]["pid"] == processes[6]["pid"]
                    && final_deaths[1]["kill_wait_completed"] == true,
                "actual initial/cut B-then-A/isolated-B/final deaths match their recorded incarnations"
            );
            ensure!(
                committed_publication_seed_b["role"] == 1
                    && committed_publication_seed_b["peer_absent_before"] == true
                    && committed_publication_seed_b["peer_absent_through"] == true
                    && committed_publication_seed_b["child_alive_after"] == true
                    && actual_cold_b_anchor == committed_publication_seed_b["row"]
                    && actual_cold_b_anchor["controls"]["committed"] == committed_publication_c0
                    && committed_publication_cold_a["role"] == 0
                    && committed_publication_cold_a["peer_absent_before"] == true
                    && committed_publication_cold_a["completed_B_death"]
                        == committed_publication_isolated_b_death,
                "independent marker C0 and actual cold headers/peer absence anchor the durable row"
            );
            let cold_partials = &diagnostics.data["committed_cold_partial"];
            let cold_rows = [
                &committed_publication_seed_b["row"],
                &committed_publication_isolated_b["before"],
                &committed_publication_isolated_b["after"],
                &committed_publication_isolated_a["before"],
                &committed_publication_isolated_a["after"],
            ];
            ensure!(
                cold_rows.iter().all(|row| row.is_object())
                    && cold_partials
                        .as_array()
                        .is_some_and(|partials| partials.len() == 2),
                "five actual cold rows and role-keyed A0/B1 partials"
            );
            for (role, witness) in [
                &committed_publication_isolated_a,
                &committed_publication_isolated_b,
            ]
            .into_iter()
            .enumerate()
            {
                let partial = &cold_partials[role];
                ensure!(
                    witness["role"] == role as u64
                        && witness["peer_absent"] == true
                        && witness["child_alive_after"] == true
                        && partial["role"] == role as u64
                        && partial["pid"] == witness["pid"]
                        && partial["peer_absent_before"] == true
                        && partial["peer_absent_through"] == true
                        && partial["child_alive_after"] == true
                        && partial["before"] == witness["before"]
                        && partial["after"] == witness["after"]
                        && partial["history"] == witness["history"]
                        && partial["upgrade_status_raw"].is_string()
                        && partial["upgrade_status_raw"] == witness["upgrade_status_raw"]
                        && partial["upgrade_status"] == witness["upgrade_status"],
                    "complete retained actual cold witness/role-keyed raw partial role{role}"
                );
            }
            let expected_phases = [
                "initial A ready",
                "initial B ready",
                "initial deaths complete",
                "trace-only A ready",
                "armed B ready",
                "current family1 setup",
                "current family2 setup",
                "sole B request41 issuance",
                "actual B upgrade reply",
                "await exact held-publication marker",
                "exact held-publication facts",
                "armed B then A deaths complete",
                "start isolated B Active after held Committed publication",
                "isolated B ready",
                "isolated B Active anchors saved P0/Accept A0/K0/Q0/own Native and matches publication C0",
                "isolated B death complete",
                "start isolated A witness",
                "isolated A ready",
                "isolated A Selected retains cold B P0/Accept A0/K0/Q0 without Committed and first anchors own Native",
                "isolated A durable witness",
                "final B ready after A witness",
                "final current Locator2 imports start",
                "both final Locator2 imports returned; recovery T_R",
                "actual recovery accepted",
                "original400/401 delivery starts",
                "final deaths complete",
            ];
            let events = diagnostics.data["events"]
                .as_array()
                .context("actual Committed-publication ordered events")?;
            ensure!(
                events.len() == expected_phases.len()
                    && events.iter().zip(expected_phases).all(|(event, phase)| {
                        event["phase"] == phase
                            && event["elapsed_seconds"]
                                .as_f64()
                                .is_some_and(|elapsed| elapsed.is_finite() && elapsed >= 0.0)
                    })
                    && events.windows(2).all(|pair| {
                        pair[0]["elapsed_seconds"].as_f64() <= pair[1]["elapsed_seconds"].as_f64()
                    }),
                "26 actual ordered lifecycle events without synthetic seed stages"
            );
            let files = traces
                .as_array()
                .context("actual Committed-publication logs")?;
            ensure!(
                files.len() == 14,
                "14 complete owned stdout/stderr observations"
            );
            let mut log_paths = std::collections::BTreeSet::new();
            for (index, process) in processes.iter().enumerate() {
                for (stream_index, stream) in ["stdout", "stderr"].into_iter().enumerate() {
                    let path = process[stream].as_str().context("actual owned log path")?;
                    let file = &files[2 * index + stream_index];
                    ensure!(
                        !path.is_empty()
                            && log_paths.insert(path)
                            && file["path"] == path
                            && file.get("read_error").is_none()
                            && file["observation"]["bytes"]
                                .as_u64()
                                .is_some_and(|bytes| bytes <= 1024 * 1024)
                            && hex(&file["observation"]["blake3"], 32)
                            && file["observation"]["tail"].is_string(),
                        "complete whole-file fingerprint of actual dead-process {stream} log {index}"
                    );
                }
            }
            json!({
                "victim_role":1,"profiles":committed_publication_profiles,
                "armed_plan":committed_publication_plan,"held_publication_producer_pair":boundary,
                "cut_death_order":[1,0],"cut_deaths":deaths,
                "actual_cold_B_seed":committed_publication_seed_b,
                "actual_cold_B_anchor":actual_cold_b_anchor,
                "isolated_B_Active":committed_publication_isolated_b,
                "isolated_B_death":committed_publication_isolated_b_death,
                "actual_cold_A_header":committed_publication_cold_a,
                "isolated_A_Selected":committed_publication_isolated_a,
                "cold_partials":cold_partials,"cold_rows_count":cold_rows.len(),
                "process_count":processes.len(),"phase_event_count":events.len(),
                "complete_log_count":files.len(),"final_B_pid":final_b_pid,
                "C0":committed_publication_c0,"DC0":committed_publication_dc0,
                "P0":committed_publication_p0,"A0":actual_cold_b_anchor["controls"]["a"],
                "K0":actual_cold_b_anchor["controls"]["k"],"Q0":actual_cold_b_anchor["q"],
                "per_peer_pre_data":committed_publication_observed,
                "first_observed_durable_refs_and_Q":committed_publication_shared,
                "observed_Q_before_data":committed_publication_observed_q,
                "Native_and_debt_before_data":committed_publication_saved_native,
                "witness_claim":"actual cold B Active anchors P0/A0/K0/Q0/own converted Native and matches independent marker C0; cold A Selected retains P/A/K/Q without C and first anchors own Native; exact own old histories/views",
                "anchor_limit":"only C0/DC0 are exported by the three-field marker; P/A/K/Q/B Mixed hash first observed in real cold B seed; A Mixed hash first observed in its real cold before-row; no synthesized owner marker or pre-cut ciphertext comparison",
                "retry_claim":"fresh compatible (4,5) current admission; saved K gets a new outer binding, B returns saved C0 without another conversion/CommitB transaction; normal Activate/validated peer Head clears debt",
                "native_claim":"both already durably converted before hold; each own original+1/first cold Mixed hash retained on every observed pre-data row; no cross-peer Native equality or post-Text ciphertext freeze",
                "debt_claim":"A Selectedtrue then Activefalse; B starts Active/debttrue and cleared debt never regresses; saved tracker state after each accepted row",
                "sequential_rows_claim":"monotonic per-peer samples and actually-present shared refs/Q; legal skipped samples, no simultaneous online pair inferred from sequential row RPC",
                "authority_claim":"original shared current Consent2/current lease gates and historical saved proof/control validation; fresh PIDs/endpoints/normal family2 admission, no raw TLS exporter observation or numeric generation novelty claim",
                "held_transport_claim":"incoming B checkpoint after durable CommitB and signed outer Committed reply, Owner call complete; renewed lease/Dispatch.gate/write still ahead; no parent owner IPC after sole B41 through both deaths",
                "raw_readback_claim":"producer signed_control_blake3 retained in its own domain; no private raw signed controls/Native/Seen export, independent disk Blake3, opaque-hint oracle or global network-silence claim"
            })
        } else {
            Value::Null
        };
        let mut observations = json!({"classification":classification,
            "boundary":name,"original":original,"original_history":original_history,"initial_deaths":initial_deaths,
            "b_upgrade_transaction":notification,"b_upgrade_raw":upgrade_raw,"a_no_local_intent":no_local,"actual_marker":marker,
            "family1_setup":family1_setup,"family2_setup":family2_setup,"first_probe_before_trigger":first_probe_before,
            "first_probe_after_trigger":first_probe_after,"first_probe_at_hit":first_probe_at_hit,
            "owned_trace_paths":{"a":a_trace,"b":b_trace},
            "setup_bounds":{"shared_seconds":45,"quiet_seconds":15,"first_m":first_m,
                "trigger_elapsed_seconds":trigger_elapsed_seconds,"shared_bound_seconds_from_A0":shared_seconds_from_a0,
                "conservative_bound_seconds_from_A0":hit_seconds_from_a0},
            "actual_boundary_fingerprints":boundary,"deaths":deaths,"isolated_before_b_start":isolated,
            "isolated_a_pid":isolated_pid,"b_absent_at_isolated_witness":true,"reopened":reopened,"active":active,
            "before_only_isolated_b":isolated_b,"before_only_isolated_a_intent":isolated_a_intent,
            "publication_evidence":publication_evidence,"accept_evidence":accept_evidence,
            "select_evidence":select_evidence,
            "committed_evidence":committed_evidence,
            "processes":diagnostics.data["processes"],"stage_events":diagnostics.data["events"],"owned_trace_files":traces,
            "recovery":{"phase_seconds":policy.recovery_seconds(),"started_seconds":recovery_started_seconds,
                "elapsed_seconds":recovery_elapsed_seconds,"original_kernel_seconds":160,
                "start_claim":"one T_R after both final Locator2 imports returned, before Active polling",
                "observed_committed_controls":observed_controls},
            "sends":sends,"requests":requests,"history":history,"final":final_rows,"final_deaths":final_deaths,
            "delivery_window_seconds":30,"delivery_elapsed_seconds":delivery_elapsed_seconds,
            "control_identity_claim":"public signed-control content refs plus reviewed signature paths; no raw P/exporter/header observation"});
        observations["a_before_evidence"] = a_before_evidence;
        observations["a_publication_evidence"] = a_publication_evidence;
        observations["k_before_evidence"] = k_before_evidence;
        observations["k_publication_evidence"] = k_publication_evidence;
        observations["committed_before_evidence"] = committed_before_evidence;
        if committed_before_publication {
            observations["committed_publication_evidence"] = committed_publication_evidence;
        }
        let result = managed.finish(observations);
        diagnostics.complete = result.is_ok();
        return result;
    }
    // r17 P-after-commit target-only END
    let managed = Managed::new(&format!("F-{}-01", name.to_ascii_lowercase()), 160)?;
    let mut pair = Pair::fresh(&managed.run, "loss")?;
    let original = [pair.row(0)?, pair.row(1)?];
    pair.stop()?;
    let victim = usize::from(name.starts_with("s53-A-") || name.starts_with("s53-Committed-"));
    let schedule = pair.base.join("loss.schedule.json");
    schedule_file(&schedule, &json!({"pause_point":name}))?;
    pair.start(victim, Some(&schedule))?;
    pair.start(1 - victim, None)?;
    // Initial B-only intent is a public IPC operation; A has no local intent.
    pair.upgrade(1, 41)?;
    pair.exchange2()?;
    wait_file(
        pair.children[victim].as_mut().context("victim")?,
        &schedule.with_extension("hit"),
        45,
    )?;
    let boundary: Value = serde_json::from_slice(&std::fs::read(
        schedule.with_extension(format!("{name}.facts.json")),
    )?)?;
    pair.stop()?;
    schedule_file(&schedule, &json!({}))?;
    pair.start(0, None)?;
    pair.start(1, None)?;
    let reopened = [pair.row(0)?, pair.row(1)?];
    let control = name.split('-').nth(1).context("control")?;
    let before = name.ends_with("before-commit");
    let expected = match (control, before) {
        ("P", true) => "L1",
        ("P", false) => "Offering",
        ("A", true) => "L1",
        ("A", false) => "Prepared",
        ("K", true) => "Offering",
        ("K", false) => "Selected",
        ("Committed", true) => "Prepared",
        ("Committed", false) => "Active",
        _ => anyhow::bail!("loss tuple"),
    };
    ensure!(
        reopened[victim]["mode"] == expected,
        "durable stage at real kill/reopen"
    );
    let key = control.to_ascii_lowercase();
    if name.ends_with("before-publication") {
        ensure!(
            reopened[victim]["controls"][&key] == boundary["control_ref"],
            "same exact saved signed control across publication loss/reopen"
        );
    } else {
        ensure!(
            reopened[victim]["controls"] == boundary["controls"]
                && reopened[victim]["q"] == boundary["q"]
                && reopened[victim]["cuts"] == boundary["cuts"]
                && reopened[victim]["native_hash"] == boundary["native_hash"],
            "same durable control refs/Q/cuts/Native at committed kill boundary"
        );
    }
    for role in 0..2 {
        same_origin(&original[role], &reopened[role])?;
        if reopened[role]["mode"] != "L1" {
            ensure!(
                reopened[role]["alpn1_allowed"] == false,
                "no fallback after durable P/Prepared"
            );
        }
    }
    pair.exchange2()?;
    let active = pair.converge()?;
    ensure!(
        active[0]["q"] == active[1]["q"] && !active[0]["q"].is_null(),
        "single exact Q after lost real control"
    );
    for role in 0..2 {
        for saved in ["p", "a", "k", "committed"] {
            if !reopened[role]["controls"][saved].is_null() {
                ensure!(
                    active[role]["controls"][saved] == reopened[role]["controls"][saved],
                    "persisted controls never recut/resealed on new TLS exporter"
                );
            }
        }
    }
    for role in 0..2 {
        same_origin(&original[role], &active[role])?;
        pair.nodes[role].send(
            400 + role as u64,
            "first real mixed Text after lost control",
        )?;
    }
    let final_rows = pair.converge()?;
    for row in &final_rows {
        ensure!(
            row["native_encoding"] == "Mixed7" && row["alpn1_allowed"] == false,
            "selected data only"
        );
    }
    pair.stop()?;
    drop(pair);
    managed.finish(json!({"classification":"real two-process loss/reopen/public B notification and mixed data","boundary":name,"actual_boundary_fingerprints":boundary,"reopened":reopened,"active":active,"final":final_rows}))
}

#[test]
fn s53_discovery_preserves_ordinary_no_intent_public() -> Result<()> {
    let managed = Managed::new("F-s53-discovery-integrated-01", 230)?;
    let mut pair = Pair::fresh(&managed.run, "discovery")?;
    pair.stop()?;
    let schedule = pair.base.join("trace.schedule.json");
    schedule_file(&schedule, &json!({"trace_rpc":true}))?;
    pair.start(0, Some(&schedule))?;
    pair.start(1, Some(&schedule))?;
    pair.nodes[0].import(&pair.nodes[1])?;
    pair.nodes[1].import(&pair.nodes[0])?;
    pair.exchange2()?;
    let logs: [PathBuf; 2] = std::array::from_fn(|role| {
        pair.nodes[role]
            .branch
            .base
            .join(format!("live-{role}-{}.stderr.log", pair.starts[role]))
    });
    struct Trace {
        starts: Vec<u64>,
        resolved: Vec<(usize, u64)>,
        open: Option<(usize, u64)>,
        hash: String,
    }
    let trace = |path: &Path| -> Result<Trace> {
        let raw = std::fs::read_to_string(path)?;
        ensure!(raw.len() <= 1024 * 1024, "bounded scheduler trace");
        ensure!(
            !raw.contains("kind=OpenIntent") && !raw.contains("kind=Propose"),
            "neither peer polls OpenIntent or manufactures intent: {}",
            path.display()
        );
        let mut value = Trace {
            starts: Vec::new(),
            resolved: Vec::new(),
            open: None,
            hash: blake3::hash(raw.as_bytes()).to_hex().to_string(),
        };
        // A live read may end inside the last write. Only complete trace lines
        // can witness a start/resolution; the final post-shutdown read is exact.
        let complete = raw.rsplit_once('\n').map_or("", |(lines, _)| lines);
        for line in complete.lines() {
            let start = line.contains("phase=probe-start ");
            let resolved = line.contains("phase=probe-resolved ");
            if !start && !resolved {
                continue;
            }
            let stamp = line
                .split_once("monotonic_ms=")
                .context("actual probe timestamp")?
                .1
                .trim()
                .parse::<u64>()?;
            if start {
                ensure!(value.open.is_none(), "serialized probe starts");
                value.starts.push(stamp);
                value.open = Some((value.starts.len(), stamp));
            } else {
                let (ordinal, began) = value.open.take().context("matched probe resolution")?;
                ensure!(
                    line.contains("phase=probe-resolved mode=L1 ") && stamp >= began,
                    "no-intent probe resolves durable L1 in its own clock"
                );
                value.resolved.push((ordinal, stamp));
            }
        }
        Ok(value)
    };
    let context = pair.c.clone();
    let clean = |row: &Value, role: usize| -> Result<()> {
        ensure!(
            row["context"] == context && row["role"] == role as u64,
            "actual discovery context/role {role}"
        );
        ensure!(
            row["mode"] == "L1"
                && row["upgrade"] == "NoLocalIntent"
                && row["freeze"] == false
                && row["q"].is_null()
                && row["cuts"] == json!([null, null])
                && ["p", "a", "k", "committed"]
                    .iter()
                    .all(|key| { row["controls"].get(*key).is_some_and(Value::is_null) }),
            "Consent2/discovery alone never creates intent/P/Q/cuts or freezes role {role}: {row}"
        );
        ensure!(
            row["current_consent2"].as_str().is_some()
                && row.get("policy_error").is_some_and(Value::is_null)
                && row.get("handoff_policy_error").is_some_and(Value::is_null),
            "current discovery/ordinary permission setup role {role}: {row}"
        );
        Ok(())
    };
    let current = |row: &Value, family: u64| {
        row["lease_family"] == family
            && row["lease_current"] == true
            && row["lease_generation"].as_u64().is_some()
    };
    let limit = Instant::now() + Duration::from_secs(150);
    let mut sent = 0;
    let mut first_ordinary = None;
    let mut witnessed: Option<(usize, u64, Value)> = None;
    let mut ordinary_return = None;
    eprintln!("s53_discovery phase=first-ordinary event=waiting");
    let probes = loop {
        s53_deadline::check()?;
        for child in pair.children.iter_mut().flatten() {
            child.alive()?;
        }
        ensure!(
            Instant::now() < limit,
            "bounded real no-intent discovery observations sent={sent} first={} admitted={} returned={}",
            first_ordinary.is_some(),
            witnessed.is_some(),
            ordinary_return.is_some()
        );
        let before_trace = trace(&logs[0]).context("A discovery trace")?;
        trace(&logs[1]).context("B no-intent trace")?;
        let a_before = pair.row(0).context("A discovery observation")?;
        let b = pair.row(1).context("B discovery observation")?;
        clean(&a_before, 0)?;
        clean(&b, 1)?;
        ensure!(
            a_before["current_consent2"] == b["current_consent2"],
            "same current Consent2"
        );
        if sent == 0 && current(&a_before, 1) && current(&b, 1) {
            pair.nodes[0].send(
                700 + sent,
                "genuine ordinary Text1 during no-intent discovery",
            )?;
            sent += 1;
        }
        if sent == 1
            && first_ordinary.is_none()
            && a_before["counts"]["queued"] == 0
            && a_before["counts"]["debt"] == 0
            && a_before["next"][0] == 3
            && b["next"][0] == 3
            && b["counts"]["pending"] == 0
        {
            first_ordinary = Some(json!({"a":a_before,"b":b}));
            eprintln!("s53_discovery phase=first-ordinary event=delivered-acked");
        }
        if sent == 1
            && first_ordinary.is_some()
            && witnessed.is_none()
            && current(&a_before, 2)
            && current(&b, 2)
            && let Some((ordinal, began)) = before_trace.open
        {
            let a_after = pair.row(0).context("A family2 bracket after B")?;
            clean(&a_after, 0)?;
            let after_trace = trace(&logs[0])?;
            if current(&a_after, 2)
                && a_before["lease_generation"] == a_after["lease_generation"]
                && before_trace.open == after_trace.open
                && before_trace.starts == after_trace.starts
                && before_trace.resolved == after_trace.resolved
            {
                // Generations are local. Only A's before/after generation is
                // compared; B is bracketed inside that unchanged live A lease.
                witnessed = Some((
                    ordinal,
                    began,
                    json!({"probe_ordinal":ordinal,
                    "probe_start_ms":began,"a_before":a_before,"b":b,"a_after":a_after,
                    "a_trace_before":before_trace.hash,"a_trace_after":after_trace.hash}),
                ));
                eprintln!(
                    "s53_discovery phase=family2-same-opportunity event=witnessed ordinal={ordinal}"
                );
            }
        }
        let latest = trace(&logs[0])?;
        if sent == 1
            && let Some((ordinal, began, _)) = &witnessed
        {
            ensure!(
                latest.starts.len() <= *ordinal,
                "witnessed opportunity L1 return must be observed before successor probe"
            );
            if let Some((_, resolved)) = latest.resolved.iter().find(|(id, _)| id == ordinal)
                && latest.open.is_none()
                && current(&a_before, 1)
                && current(&b, 1)
            {
                ordinary_return = Some(json!({"probe_ordinal":ordinal,"probe_start_ms":began,
                    "probe_resolved_ms":resolved,"a":a_before,"b":b,"trace":latest.hash}));
                pair.nodes[0].send(
                    700 + sent,
                    "genuine ordinary Text1 during no-intent discovery",
                )?;
                sent += 1;
                eprintln!(
                    "s53_discovery phase=matched-L1-return event=second-send ordinal={ordinal}"
                );
            }
        }
        if latest.starts.len() >= 2 && ordinary_return.is_some() {
            break latest.starts;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    ensure!(
        probes[0] >= 5000 && probes[1] - probes[0] >= 60000,
        "actual coordinator-start lower bound and interval monotonic scheduler bounds"
    );
    let final_limit = Instant::now() + Duration::from_secs(30);
    let (final_a, final_b) = loop {
        s53_deadline::check()?;
        for child in pair.children.iter_mut().flatten() {
            child.alive()?;
        }
        let a = pair.row(0)?;
        let b = pair.row(1)?;
        clean(&a, 0)?;
        clean(&b, 1)?;
        trace(&logs[0])?;
        trace(&logs[1])?;
        if a["counts"]["queued"] == 0
            && a["counts"]["debt"] == 0
            && b["next"][0] == 4
            && b["counts"]["pending"] == 0
        {
            break (a, b);
        }
        ensure!(
            Instant::now() < final_limit,
            "bounded final ordinary delivery/ACK a={a} b={b}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    ensure!(
        sent == 2
            && final_a["mode"] == "L1"
            && final_a["next"][0] == 4
            && first_ordinary.is_some()
            && witnessed.is_some()
            && ordinary_return.is_some(),
        "actual ordinary progress survives no-P discovery"
    );
    trace(&logs[0])?;
    trace(&logs[1])?;
    pair.stop()?;
    let final_traces = [trace(&logs[0])?, trace(&logs[1])?];
    drop(pair);
    managed.finish(json!({"classification":"real ALPN1/ALPN2 scheduler without intent",
        "actual_probe_start_ms":probes,"ordinary_return_after_probe":ordinary_return.is_some(),
        "ordinary_texts":sent,"first_ordinary":first_ordinary,"same_opportunity":witnessed.map(|(_,_,value)|value),
        "matched_ordinary_return":ordinary_return,"final":final_a,"final_b":final_b,
        "final_trace_hashes":[final_traces[0].hash,final_traces[1].hash]}))
}

#[test]
fn s53_pre_q_gap_early_mixed_public() -> Result<()> {
    fn op<T>(until: Instant, phase: &str, action: impl FnOnce() -> Result<T>) -> Result<T> {
        s53_deadline::check()?;
        ensure!(Instant::now() < until, "{phase}: deadline before operation");
        let result = action();
        s53_deadline::check()?;
        ensure!(Instant::now() < until, "{phase}: deadline after operation");
        result.with_context(|| phase.to_owned())
    }
    fn queued(pair: &Pair, role: usize, id: u64, sequence: u64, until: Instant) -> Result<Value> {
        let raw = op(until, "queued request-status3", || {
            pair.command(
                role,
                "request-status3",
                &["--request-id".into(), format!("fa00000000000000{id:016x}")],
            )
        })?;
        let value: Value = serde_json::from_str(&raw)?;
        ensure!(
            value == json!([[[sequence], [], 0]]),
            "same real queued request {id}: {value}"
        );
        Ok(value)
    }
    fn accepted(raw: &str) -> Result<(u64, Value)> {
        let value: Value = serde_json::from_str(raw)?;
        ensure!(
            value.as_array().is_some_and(|v| v.len() == 2)
                && value[0].as_array().is_some_and(|v| v.len() == 1)
                && value[1] == json!([]),
            "IPC3 send tag2 queued option shape: {value}"
        );
        Ok((
            value[0][0].as_u64().context("accepted queue sequence")?,
            value,
        ))
    }
    fn source(page: &Value, role: usize, id: u64) -> Result<Option<Value>> {
        let request = format!("fa00000000000000{id:016x}");
        let found = page["items"]
            .as_array()
            .context("real history items")?
            .iter()
            .filter(|row| row["role"] == role as u64 && row["request"] == request)
            .collect::<Vec<_>>();
        ensure!(found.len() <= 1, "request {id} materialized at most once");
        Ok(found.first().map(|row| (*row).clone()))
    }
    fn event(row: &Value, format: u64, n: u64, text: &str) -> Result<()> {
        ensure!(
            row["format"] == format
                && row["ordinal"] == n
                && row["text"] == text
                && row["id"].as_str().is_some_and(|id| id.len() == 64),
            "actual source format/ordinal/text: {row}"
        );
        Ok(())
    }
    fn live(
        rows: &[Value; 2],
        original: &[Value; 2],
        family: u64,
        generations: Option<&[u64; 2]>,
    ) -> Result<()> {
        for role in 0..2 {
            same_origin(&original[role], &rows[role])?;
            ensure!(
                rows[role]["context"] == original[role]["context"]
                    && rows[role]["role"] == role as u64
                    && rows[role]["lease_family"] == family
                    && rows[role]["lease_current"] == true
                    && rows[role]["lease_generation"].as_u64().is_some()
                    && rows[role].get("policy_error").is_some_and(Value::is_null)
                    && rows[role]
                        .get("handoff_policy_error")
                        .is_some_and(Value::is_null),
                "actual live family{family} role{role}: {}",
                rows[role]
            );
            if let Some(generations) = generations {
                ensure!(
                    rows[role]["lease_generation"] == generations[role],
                    "same LOCAL lease generation role{role}"
                );
            }
        }
        Ok(())
    }
    fn marker(path: &Path, name: &str) -> Result<bool> {
        match std::fs::read_to_string(path) {
            Ok(value) => {
                ensure!(value == name, "exact publication marker");
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
    fn facts(path: &Path, name: &str, reference: &Value) -> Result<Value> {
        let raw = std::fs::read(
            path.with_extension(format!("s53-{name}-before-publication.facts.json")),
        )?;
        ensure!(raw.len() <= 4096, "bounded public control facts");
        let value: Value = serde_json::from_slice(&raw)?;
        ensure!(
            value["control"] == name
                && value["control_ref"] == *reference
                && reference.as_str().is_some_and(|s| s.len() == 64)
                && value["signed_control_blake3"]
                    .as_str()
                    .is_some_and(|s| s.len() == 64),
            "publication fingerprint matches persisted control: {value}"
        );
        Ok(value)
    }
    fn final_state(rows: &[Value; 2], original: &[Value; 2]) -> Result<()> {
        for role in 0..2 {
            same_origin(&original[role], &rows[role])?;
            ensure!(
                rows[role]["mode"] == "Active"
                    && rows[role]["next"] == json!([5, 4])
                    && rows[role]["counts"]["receipts"] == json!([4, 3])
                    && rows[role]["counts"]["queued"] == 0
                    && rows[role]["counts"]["pending"] == 0
                    && rows[role]["counts"]["debt"] == 0
                    && rows[role]["control_debt"] == false,
                "exact final original lineage/frontiers/receipts/empty ledgers role{role}: {}",
                rows[role]
            );
        }
        Ok(())
    }
    let managed = Managed::new("F-s53-preq-integrated-01", 190)?;
    let available = std::thread::available_parallelism()?.get();
    let inherited = std::env::var_os("TOKIO_WORKER_THREADS")
        .map(|s| {
            s.into_string()
                .map_err(|_| anyhow::anyhow!("non-UTF8 inherited Tokio worker setting"))
        })
        .transpose()?;
    let workers = inherited
        .as_deref()
        .map(str::parse::<usize>)
        .transpose()?
        .unwrap_or(available);
    ensure!(
        workers >= 2,
        "publication hooks require independently runnable Tokio workers"
    );
    let mut pair = Pair::fresh(&managed.run, "preq")?;
    let original = [pair.row(0)?, pair.row(1)?];
    let page = pair.nodes[1].history(1, None)?;
    let cursor = page["cursor"]
        .as_str()
        .context("pre-Q real HVI1 cursor")?
        .to_owned();
    let suffix = pair.nodes[1].history(32, Some(&cursor))?;
    pair.stop()?;
    let a_schedule = pair.base.join("a.schedule.json");
    let b_schedule = pair.base.join("b.schedule.json");
    let mut a_plan = json!({"context":pair.c,"hold_lane":0,"hold_ordinals":[2],"direct_batch":1,
        "disable_ack":true,"suspend_prefix_requests":true,"suspend_materialization":false,"trace_rpc":true,"pause_point":null});
    let mut b_plan = json!({"context":pair.c,"hold_lane":1,"hold_ordinals":[2],"direct_batch":null,
        "disable_ack":true,"suspend_prefix_requests":true,"suspend_materialization":false,"trace_rpc":true,"pause_point":null});
    let initial_plans = json!({"a":a_plan,"b":b_plan});
    schedule_file(&a_schedule, &a_plan)?;
    schedule_file(&b_schedule, &b_plan)?;
    pair.start(0, Some(&a_schedule))?;
    pair.start(1, Some(&b_schedule))?;
    pair.nodes[0].import(&pair.nodes[1])?;
    pair.nodes[1].import(&pair.nodes[0])?;
    let a_log = pair.nodes[0]
        .branch
        .base
        .join(format!("live-0-{}.stderr.log", pair.starts[0]));
    let until = Instant::now() + Duration::from_secs(25);
    loop {
        let rows = [
            op(until, "L1 admission A", || pair.row(0))?,
            op(until, "L1 admission B", || pair.row(1))?,
        ];
        if rows
            .iter()
            .all(|r| r["lease_family"] == 1 && r["lease_current"] == true)
        {
            live(&rows, &original, 1, None)?;
            ensure!(
                rows.iter().all(|r| r["mode"] == "L1"
                    && r["upgrade"] == "NoLocalIntent"
                    && r["q"].is_null()
                    && r["cuts"] == json!([null, null])),
                "real no-intent legacy setup"
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    op(until, "send real A gap", || {
        pair.nodes[0].send(100, "real held A gap")
    })?;
    op(until, "send real A suffix", || {
        pair.nodes[0].send(101, "real pending A suffix")
    })?;
    op(until, "send real B legacy", || {
        pair.nodes[1].send(102, "real last B Text1")
    })?;
    let (legacy, legacy_sources) = loop {
        let rows = [
            op(until, "legacy A", || pair.row(0))?,
            op(until, "legacy B", || pair.row(1))?,
        ];
        live(&rows, &original, 1, None)?;
        let ah = op(until, "legacy A sources", || {
            pair.nodes[0].history(32, None)
        })?;
        let bh = op(until, "legacy B sources", || {
            pair.nodes[1].history(32, None)
        })?;
        if rows[0]["next"] == json!([4, 2])
            && rows[0]["counts"]["debt"] == 2
            && rows[0]["counts"]["queued"] == 0
            && rows[1]["next"] == json!([2, 3])
            && rows[1]["counts"]["debt"] == 1
            && rows[1]["counts"]["queued"] == 0
            && rows[1]["counts"]["pending"] == 1
            && let (Some(a2), Some(a3), Some(b2)) = (
                source(&ah, 0, 100)?,
                source(&ah, 0, 101)?,
                source(&bh, 1, 102)?,
            )
        {
            event(&a2, 1, 2, "real held A gap")?;
            event(&a3, 1, 3, "real pending A suffix")?;
            event(&b2, 1, 2, "real last B Text1")?;
            ensure!(
                source(&bh, 0, 100)?.is_none()
                    && source(&bh, 0, 101)?.is_none()
                    && source(&ah, 1, 102)?.is_none(),
                "real missing A2/pending A3/unread B2"
            );
            let trace = op(until, "actual A3 possession trace", || {
                Ok(std::fs::read_to_string(&a_log)?)
            })?;
            ensure!(trace.len() <= 1024 * 1024, "bounded real source trace");
            let exact = format!(
                "exact_ids=[\"{}\"]",
                a3["id"].as_str().context("A3 source ID")?
            );
            if trace
                .lines()
                .any(|line| line.starts_with("l1_complete ") && line.contains(&exact))
            {
                break (
                    rows,
                    json!({"a2":a2,"a3":a3,"b2":b2,"a3_possession_trace":blake3::hash(trace.as_bytes()).to_hex().to_string()}),
                );
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    eprintln!("s53_preq phase=committed-legacy-gap event=reached");
    a_plan["suspend_materialization"] = true.into();
    a_plan["pause_point"] = "s53-K-before-publication".into();
    schedule_file(&a_schedule, &a_plan)?;
    let pre_p_plan = a_plan.clone();
    let (sequence103, send103) =
        accepted(&pair.nodes[0].send(103, "accepted before durable P freezes queue")?)?;
    let request103 = queued(
        &pair,
        0,
        103,
        sequence103,
        Instant::now() + Duration::from_secs(25),
    )?;
    let queued_before_p = pair.row(0)?;
    ensure!(
        queued_before_p["mode"] == "L1"
            && queued_before_p["counts"]["queued"] == 1
            && queued_before_p["next"][0] == 4,
        "genuine pre-P accepted queue without extra legacy packet"
    );
    pair.upgrade(0, 42)?;
    pair.exchange2()?;
    let until = Instant::now() + Duration::from_secs(45);
    let (selected, prepared, k_facts) = loop {
        if op(until, "K publication marker", || {
            marker(
                &a_schedule.with_extension("hit"),
                "s53-K-before-publication",
            )
        })? {
            let selected = op(until, "A Selected", || pair.row(0))?;
            let prepared = op(until, "B Prepared", || pair.row(1))?;
            live(&[selected.clone(), prepared.clone()], &original, 2, None)?;
            ensure!(
                selected["mode"] == "Selected"
                    && selected["q"].as_str().is_some()
                    && selected["cuts"] == json!([4, 3])
                    && selected["counts"]["queued"] == 1
                    && selected["next"] == json!([4, 2])
                    && prepared["mode"] == "Prepared"
                    && prepared["q"].is_null()
                    && prepared["next"] == json!([2, 3])
                    && prepared["counts"]["pending"] == 1
                    && prepared["counts"]["queued"] == 0,
                "real asymmetric Selected/Prepared with original gap/queue intact"
            );
            let ah = op(until, "K A sources", || pair.nodes[0].history(32, None))?;
            let bh = op(until, "K B sources", || pair.nodes[1].history(32, None))?;
            ensure!(
                source(&ah, 1, 102)?.is_none()
                    && source(&bh, 0, 100)?.is_none()
                    && source(&bh, 0, 101)?.is_none(),
                "held sources not consumed before Selected witness"
            );
            queued(&pair, 0, 103, sequence103, until)?;
            let facts = op(until, "saved K publication facts", || {
                facts(&a_schedule, "K", &selected["controls"]["k"])
            })?;
            break (selected, prepared, facts);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let generations = [
        selected["lease_generation"]
            .as_u64()
            .context("A local generation")?,
        prepared["lease_generation"]
            .as_u64()
            .context("B local generation")?,
    ];
    let (sequence104, send104) =
        accepted(&pair.nodes[1].send(104, "queued during Prepared freeze")?)?;
    queued(
        &pair,
        1,
        104,
        sequence104,
        Instant::now() + Duration::from_secs(25),
    )?;
    b_plan["hold_ordinals"] = json!([]);
    b_plan["disable_ack"] = false.into();
    b_plan["suspend_prefix_requests"] = false.into();
    schedule_file(&b_schedule, &b_plan)?;
    let repair_plan = b_plan.clone();
    let until = Instant::now() + Duration::from_secs(25);
    let repair = loop {
        let rows = [
            op(until, "late legacy A", || pair.row(0))?,
            op(until, "repair Prepared B", || pair.row(1))?,
        ];
        live(&rows, &original, 2, Some(&generations))?;
        ensure!(
            rows[0]["mode"] == "Selected"
                && rows[1]["mode"] == "Prepared"
                && rows[1]["q"].is_null(),
            "K remains withheld through genuine repair"
        );
        if rows[0]["next"][1] == 3
            && rows[0]["counts"]["queued"] == 1
            && rows[1]["next"] == json!([4, 3])
            && rows[1]["counts"]["pending"] == 0
            && rows[1]["counts"]["debt"] == 1
            && rows[1]["counts"]["queued"] == 1
        {
            let ah = op(until, "late real B2 history", || {
                pair.nodes[0].history(32, None)
            })?;
            let bh = op(until, "repaired real A2/A3 history", || {
                pair.nodes[1].history(32, None)
            })?;
            let a2 = source(&bh, 0, 100)?.context("repaired A2")?;
            let a3 = source(&bh, 0, 101)?.context("repaired A3")?;
            let b2 = source(&ah, 1, 102)?.context("late B2")?;
            ensure!(
                a2["id"] == legacy_sources["a2"]["id"]
                    && a3["id"] == legacy_sources["a3"]["id"]
                    && b2["id"] == legacy_sources["b2"]["id"]
                    && a2["delivered"] == true
                    && a3["delivered"] == true
                    && b2["delivered"] == true,
                "same real legacy sources/validated receipts after gap release"
            );
            event(&b2, 1, 2, "real last B Text1")?;
            queued(&pair, 0, 103, sequence103, until)?;
            queued(&pair, 1, 104, sequence104, until)?;
            break json!({"a":rows[0],"b":rows[1],"a2":a2,"a3":a3,"late_b2":b2,"receipt_claim":"Selected receipt path for format1; no raw-header observation"});
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    b_plan["pause_point"] = "s53-Committed-before-publication".into();
    schedule_file(&b_schedule, &b_plan)?;
    let committed_plan = b_plan.clone();
    a_plan["pause_point"] = Value::Null;
    a_plan["suspend_materialization"] = false.into();
    // One absolute bound starts BEFORE the blocking write which releases K.
    let early_started = Instant::now();
    let early_until = early_started + Duration::from_secs(10);
    op(
        early_until,
        "release K and materialization suspension",
        || schedule_file(&a_schedule, &a_plan),
    )?;
    let (early, committed_facts, early_b) = loop {
        if op(early_until, "Committed publication marker", || {
            marker(
                &b_schedule.with_extension("hit"),
                "s53-Committed-before-publication",
            )
        })? {
            let rows = [
                op(early_until, "early Selected A", || pair.row(0))?,
                op(early_until, "early Active B", || pair.row(1))?,
            ];
            live(&rows, &original, 2, Some(&generations))?;
            ensure!(
                rows[0]["mode"] == "Selected"
                    && rows[1]["mode"] == "Active"
                    && rows[0]["counts"]["queued"] == 1
                    && rows[0]["controls"]["k"] == selected["controls"]["k"]
                    && rows[1]["controls"]["k"] == selected["controls"]["k"],
                "Committed held while original A queue stays frozen"
            );
            let facts = op(early_until, "saved Committed publication facts", || {
                facts(&b_schedule, "Committed", &rows[1]["controls"]["committed"])
            })?;
            let ah = op(early_until, "actual early B104 at Selected A", || {
                pair.nodes[0].history(32, None)
            })?;
            let bh = op(early_until, "actual B104 source", || {
                pair.nodes[1].history(32, None)
            })?;
            queued(&pair, 0, 103, sequence103, early_until)?;
            if rows[0]["next"][1] == 4
                && rows[1]["next"][1] == 4
                && let (Some(received), Some(sent)) = (source(&ah, 1, 104)?, source(&bh, 1, 104)?)
            {
                event(&received, 2, 3, "queued during Prepared freeze")?;
                ensure!(
                    received["id"] == sent["id"] && received["delivered"] == true,
                    "actual early Event2 source/read/receipt"
                );
                let after_rows = [
                    op(early_until, "bracket early history with current A", || {
                        pair.row(0)
                    })?,
                    op(early_until, "bracket early history with current B", || {
                        pair.row(1)
                    })?,
                ];
                live(&after_rows, &original, 2, Some(&generations))?;
                ensure!(
                    after_rows[0]["mode"] == "Selected"
                        && after_rows[1]["mode"] == "Active"
                        && after_rows[0]["counts"]["queued"] == 1
                        && after_rows[0]["next"][1] == 4
                        && after_rows[1]["next"][1] == 4
                        && after_rows[0]["q"] == selected["q"]
                        && after_rows[1]["q"] == selected["q"]
                        && after_rows[0]["controls"]["k"] == selected["controls"]["k"]
                        && after_rows[1]["controls"]["k"] == selected["controls"]["k"]
                        && after_rows[1]["controls"]["committed"] == facts["control_ref"],
                    "same live Selected/Active/control scope after early history IO"
                );
                op(early_until, "accept early witness", || Ok(()))?;
                break (
                    json!({"a":after_rows[0],"a_before":rows[0],"b_before":rows[1],"b_after":after_rows[1],"source":sent,"received":received,"elapsed_seconds":early_started.elapsed().as_secs_f64()}),
                    facts,
                    after_rows[1].clone(),
                );
            }
        }
        op(early_until, "early poll sleep", || {
            std::thread::sleep(Duration::from_millis(20));
            Ok(())
        })?;
    };
    b_plan["pause_point"] = Value::Null;
    op(early_until, "prompt release of Committed", || {
        schedule_file(&b_schedule, &b_plan)
    })?;
    a_plan["hold_ordinals"] = json!([]);
    a_plan["direct_batch"] = Value::Null;
    a_plan["disable_ack"] = false.into();
    a_plan["suspend_prefix_requests"] = false.into();
    op(early_until, "clear completed A data holds", || {
        schedule_file(&a_schedule, &a_plan)
    })?;
    op(early_until, "finish bounded held-response phase", || Ok(()))?;
    let active = pair.converge()?;
    final_state(&active, &original)?;
    let current_history = pair.nodes[1].history(32, None)?;
    let accepted103 = source(&current_history, 0, 103)?.context("actual once-only request103")?;
    event(
        &accepted103,
        2,
        4,
        "accepted before durable P freezes queue",
    )?;
    ensure!(
        accepted103["delivered"] == true,
        "request103 delivered/ACKed"
    );
    let after = pair.nodes[1].history(32, Some(&cursor))?;
    ensure!(
        after == suffix,
        "exact pre-Q cursor suffix after handoff and appends"
    );
    pair.stop()?;
    pair.start(0, None)?;
    pair.start(1, None)?;
    ensure!(
        pair.nodes[1].history(32, Some(&cursor))? == suffix,
        "same frozen history after actual process restart"
    );
    pair.exchange2()?;
    let final_rows = pair.converge()?;
    final_state(&final_rows, &original)?;
    let reopened_history = pair.nodes[1].history(32, None)?;
    let reopened103 =
        source(&reopened_history, 0, 103)?.context("durable once-only103 after restart")?;
    ensure!(
        reopened103 == accepted103,
        "exact original Event2 request103 source survives restart"
    );
    pair.stop()?;
    drop(pair);
    managed.finish(json!({"classification":"real process pre-Q gap/pending/debt/queue/early legacy and Event2 receive with actual Committed withheld",
        "runtime_prerequisites":{"available_parallelism":available,"inherited_tokio_workers":inherited,"effective_worker_prerequisite":workers},
        "original":original,"initial_plans":initial_plans,"pre_p_plan":pre_p_plan,"repair_plan":repair_plan,"committed_plan":committed_plan,
        "legacy":legacy,"legacy_sources":legacy_sources,"send103":send103,"request103_before_p":request103,"queued_before_p":queued_before_p,
        "selected":selected,"prepared":prepared,"k_publication":k_facts,"local_generations":generations,"send104":send104,"repair":repair,
        "early_event2_selected":early["a"],"early_event2_witness":early,"early_active_b":early_b,"committed_publication":committed_facts,
        "active":active,"final":final_rows,"request103_source":accepted103,"request103_reopened":reopened103,"frozen_history_suffix":true}))
}

fn view_pair(base: &Path, nodes: [Node; 2], roots: [PathBuf; 2], c: &str) -> Pair {
    Pair {
        nodes,
        roots,
        children: [None, None],
        base: base.into(),
        c: c.into(),
        starts: [0, 0],
    }
}
fn route(node: &Node, policy: &str, relay: Option<&str>) -> Result<()> {
    let mut profile: Value = serde_json::from_slice(&std::fs::read(&node.profile)?)?;
    profile["route"] = json!({"policy":policy,"relay":relay,"bind":if policy=="relay-only"{None}else{Some("127.0.0.1:0")}});
    std::fs::write(&node.profile, serde_json::to_vec_pretty(&profile)?)?;
    Ok(())
}
fn wait_path(node: &Node, path: &str) -> Result<Value> {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        s53_deadline::check()?;
        let status = node.status()?;
        if status["routes"][node.slot as usize]["selected_path"] == path {
            return Ok(status);
        }
        ensure!(
            Instant::now() < limit,
            "actual selected route {path}: {status}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[test]
fn s53_two_c_pin_resources_routes_public() -> Result<()> {
    two_context::run()
}

#[cfg(windows)]
#[test]
fn s53_exhausted_exact_path_public() -> Result<()> {
    let managed = Managed::new("F-s53-exhausted-integrated-01", 540)?;
    let prepared = g1_prerequisite::Prepared::load(&managed)?;
    let manifest_path = prepared.manifest_path();
    let (nodes, roots, c) = prepared.nodes()?;
    let mut pair = Pair {
        nodes,
        roots,
        children: [None, None],
        base: managed.run.clone(),
        c,
        starts: [0, 0],
    };
    pair.start(0, None)?;
    pair.start(1, None)?;
    let before = [pair.row(0)?, pair.row(1)?];
    ensure!(
        before[0]["mode"] == "L1"
            && before[1]["mode"] == "L1"
            && before[0]["next"] == json!([512, 18])
            && before[1]["next"] == json!([512, 18]),
        "exact genuine exhausted source frontiers"
    );
    let original = before.clone();
    let original_history = [
        g1_witness::OriginalHistory::capture(&pair.nodes[0])?,
        g1_witness::OriginalHistory::capture(&pair.nodes[1])?,
    ];
    pair.stop()?;
    let original_records = [
        g1_witness::OriginalRecords::capture(&pair.nodes[0], 0)?,
        g1_witness::OriginalRecords::capture(&pair.nodes[1], 1)?,
    ];
    for role in 0..2 {
        original_records[role].bind(&original_history[role])?;
    }
    let trace_plan = pair.base.join("exhausted-admission.trace.schedule.json");
    schedule_file(&trace_plan, &json!({"context":pair.c,"trace_rpc":true}))?;
    let a0 = Instant::now();
    pair.start(0, Some(&trace_plan))?;
    pair.start(1, Some(&trace_plan))?;
    pair.ceremony("exhausted", "absent", 30 * 86400)?;
    let pre_active = g1_admission::before_upgrade(&mut pair, &original, a0)?;
    pre_active.upgrade(&mut pair)?;
    let active = pair.converge()?;
    let conversion_only = g1_admission::converted(&mut pair, &pre_active)?;
    // Current permission exposes the original Legacy7 keys; unchanged cold
    // hash/generation anchor those keys before the Native encoding conversion.
    let original = pre_active.original.clone();
    ensure!(
        active[0]["cuts"] == json!([512, 18]) && active[1]["cuts"] == json!([512, 18]),
        "no regeneration/rollback of real boundary cuts"
    );
    for role in 0..2 {
        same_origin(&original[role], &active[role])?;
    }
    pair.nodes[0].send(600, g1_admission::FIRST_TEXT)?;
    let a_delivered = g1_data::wait_delivery(&mut pair, &active, 1)?;
    pair.nodes[1].send(601, g1_admission::FIRST_TEXT)?;
    let b_delivered = g1_data::wait_delivery(&mut pair, &active, 2)?;
    g1_data::a_before_b(&a_delivered, &b_delivered)?;
    let after = b_delivered.rows();
    for (role, history) in original_history.iter().enumerate() {
        history.recheck(&pair.nodes[role])?;
    }
    let first_stop = g1_data::stop(&mut pair)?;
    for (role, expected) in original_records.iter().enumerate() {
        expected.recheck(&pair.nodes[role])?;
    }
    let new_records = g1_data::stopped_records(&pair, &b_delivered)?;
    pair.start(0, None)?;
    pair.start(1, None)?;
    for (role, history) in original_history.iter().enumerate() {
        history.recheck(&pair.nodes[role])?;
    }
    pair.exchange2()?;
    pair.converge()?;
    let reopened = g1_data::wait_delivery(&mut pair, &active, 2)?;
    g1_data::compare_delivery(&b_delivered, &reopened)?;
    let final_rows = reopened.rows();
    for role in 0..2 {
        ensure!(
            final_rows[role]["q"] == active[role]["q"]
                && final_rows[role]["repair_free"] == after[role]["repair_free"],
            "selected Q and actual spent pool survive restart"
        );
    }
    let final_stop = g1_data::stop(&mut pair)?;
    for (role, expected) in original_records.iter().enumerate() {
        expected.recheck(&pair.nodes[role])?;
    }
    let reopened_records = g1_data::stopped_records(&pair, &reopened)?;
    ensure!(
        new_records == reopened_records,
        "same new source/projection/proof/Receipt2/own request witnesses after reopen"
    );
    let a_delivery_report = a_delivered.report()?;
    let b_delivery_report = b_delivered.report()?;
    let restart_report = reopened.report()?;
    drop(pair);
    let history_reports = original_history
        .iter()
        .map(|h| h.report())
        .collect::<Result<Vec<_>>>()?;
    let record_reports = original_records
        .iter()
        .map(|r| r.report())
        .collect::<Vec<_>>();
    managed.finish(json!({"classification":"charged exhausted exact-path test replica, separate original Root prerequisite","manifest":manifest_path,"before":before,"original_history":history_reports,"pre_active_refusal":pre_active.report,"conversion_only":conversion_only,"original_records":record_reports,"active":active,"A_delivered_before_B_send":a_delivery_report,"B_delivered":b_delivery_report,"first_mixed":after,"new_records":new_records,"reopened_new_records":reopened_records,"restart":restart_report,"first_stop":first_stop,"final_stop":final_stop,"final":final_rows,"first_ordinals":[512,18]}))
}

#[cfg(windows)]
#[test]
fn s53_exhausted_continue_after_a_public() -> Result<()> {
    g1_continue::run()
}

#[test]
fn s53_b_before_alpn2_two_c_public() -> Result<()> {
    two_context::b_start_run()
}
