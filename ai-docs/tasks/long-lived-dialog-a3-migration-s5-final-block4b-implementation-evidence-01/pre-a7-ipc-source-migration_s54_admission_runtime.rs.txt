//! The only production capability: fixed policy, exclusive claim and an actual
//! outer-owner rendezvous. Public inspection can never construct this value.
use super::*;
use std::collections::BTreeMap;
use std::io::Write;

pub(in super::super) struct AdmittedRun {
    input: Inputs,
    verified: Inspection,
    started: Started,
    claim: Claim,
    control: [Pin; 4],
    origin: Instant,
    until: Instant,
}
pub(super) fn command_log_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("command-") else {
        return false;
    };
    let Some(n) = rest
        .strip_suffix(".stdout.log")
        .or_else(|| rest.strip_suffix(".stderr.log"))
    else {
        return false;
    };
    (4..=5).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit())
}
pub(super) fn require_production(p: &Policy) -> Result<()> {
    ensure!(
        !p.fixture && files::path(&p.root, false)? == files::path(&workspace(), false)?,
        "public fixture inspection cannot activate runtime"
    );
    Ok(())
}
fn pin(path: &Path, cap: usize) -> Result<Pin> {
    let raw = files::read(path, cap)?;
    Ok(Pin {
        path: path.into(),
        bytes: raw.len() as u64,
        sha256: files::sha(&raw),
    })
}
fn create<T: serde::Serialize>(path: &Path, value: &T) -> Result<Pin> {
    let raw = serde_json::to_vec(value)?;
    ensure!(raw.len() <= 16 * 1024, "bounded transient owner IPC");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let mut f = options.open(path)?;
    f.write_all(&raw)?;
    f.flush()?;
    f.sync_all()?;
    drop(f);
    pin(path, 16 * 1024)
}
fn input_from(root: &Path, get: impl Fn(&str) -> Result<String>) -> Result<Inputs> {
    let run = root.join(".tmp/migration-s54-integrated").join(ATTEMPT);
    let take = |role: &str, file: &str, cap| -> Result<Pin> {
        let expected = get(role)?;
        files::digest(&expected, 64)?;
        let actual = pin(&run.join(file), cap)?;
        ensure!(actual.sha256 == expected, "required external runtime pin");
        Ok(actual)
    };
    Ok(Inputs {
        inventory: take("KILOGRAM_S54_INVENTORY_SHA256", "inputs.json", 2 * MIB)?,
        activation: take(
            "KILOGRAM_S54_ACTIVATION_SHA256",
            "activation.json",
            16 * 1024,
        )?,
        launch: take("KILOGRAM_S54_LAUNCH_SHA256", "launch.json", 16 * 1024)?,
        capture: take("KILOGRAM_S54_CAPTURE_SHA256", "capture.json", 4096)?,
    })
}
fn decoded(value: &str) -> Result<[u8; 32]> {
    files::digest(value, 64)?;
    let mut out = [0; 32];
    for (byte, pair) in out.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(out)
}
/// The earlier Instant, not reply receipt, anchors the sampled remaining bound.
fn original(
    before: Instant,
    now: Instant,
    launch: &Launch,
    reply: &ClockReply,
) -> Result<(Instant, Instant)> {
    ensure!(
        before <= now
            && reply.origin_tick_ms == launch.start_tick_ms
            && reply.sampled_tick_ms >= reply.origin_tick_ms
            && reply.sampled_tick_ms < launch.controller_tick_ms
            && reply.remaining_controller_ms == launch.controller_tick_ms - reply.sampled_tick_ms
            && reply.remaining_controller_ms <= 2_350_000,
        "original monotonic sample relation"
    );
    let until = before
        .checked_add(Duration::from_millis(reply.remaining_controller_ms))
        .context("sample Instant overflow")?;
    ensure!(now < until, "late/expired original clock reply");
    let origin = until
        .checked_sub(Duration::from_secs(2350))
        .context("sample original underflow")?;
    ensure!(origin <= before, "no new clock origin");
    Ok((origin, until))
}
fn reply_binding(reply: &ClockReply, claim: &Claim, request: &Pin) -> Result<()> {
    ensure!(
        reply.version == 1
            && reply.classification == "s54_original_clock_reply"
            && reply.case == claim.case
            && reply.attempt == claim.attempt
            && reply.run_id == claim.run_id
            && reply.owner_pid == claim.owner_pid
            && reply.launcher_pid == claim.launcher_pid
            && reply.harness_pid == claim.harness_pid
            && reply.nonce == claim.nonce
            && reply.inventory_sha256 == claim.inventory_sha256
            && reply.launch_sha256 == claim.launch_sha256
            && reply.request_sha256 == request.sha256,
        "owner clock nonce/actual harness/input binding"
    );
    Ok(())
}
impl AdmittedRun {
    pub(in super::super) fn load() -> Result<Self> {
        // Refuse an unactivated selector before resolving tools or inspecting files.
        let required = [
            "KILOGRAM_S54_INVENTORY_SHA256",
            "KILOGRAM_S54_ACTIVATION_SHA256",
            "KILOGRAM_S54_LAUNCH_SHA256",
            "KILOGRAM_S54_CAPTURE_SHA256",
        ]
        .into_iter()
        .map(|key| {
            let value = std::env::var(key).context("required S54 runtime input absent")?;
            files::digest(&value, 64)?;
            Ok((key, value))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
        let p = Policy::production()?;
        require_production(&p)?;
        let input = input_from(&p.root, |key| {
            required
                .get(key)
                .cloned()
                .context("required S54 runtime pin")
        })?;
        let run = input
            .inventory
            .path
            .parent()
            .context("owned runtime directory")?;
        let started_pin = pin(&run.join("started.json"), 16 * 1024)?;
        let started: Started = files::document(&started_pin, 16 * 1024)?;
        let verified = inspect_state(&p, &input, Directory::BeforeNative(&started))?;
        let capture_env =
            std::env::var_os("KILOGRAM_S54_CAPTURE_ADMISSION").context("required capture path")?;
        files::same(Path::new(&capture_env), &input.capture.path)?;
        let l = &verified.launch;
        let claim = Claim {
            version: 1,
            classification: "s54_native_started".into(),
            case: CASE.into(),
            attempt: ATTEMPT.into(),
            run_id: verified.inventory.identity.run_id.clone(),
            owner_pid: l.owner_pid,
            launcher_pid: l.launcher_pid,
            harness_pid: std::process::id(),
            nonce: started.nonce.clone(),
            inventory_sha256: input.inventory.sha256.clone(),
            launch_sha256: input.launch.sha256.clone(),
        };
        ensure!(
            claim.harness_pid != claim.owner_pid && claim.harness_pid != claim.launcher_pid,
            "separate actual runtime process"
        );
        let claim_pin = create(&run.join("native-started.json"), &claim)?;
        let mut request = claim.clone();
        request.classification = "s54_clock_request".into();
        let before = Instant::now();
        let request_pin = create(&run.join("clock-request.json"), &request)?;
        let reply_path = run.join("clock-reply.json");
        let wait_until = before
            .checked_add(Duration::from_secs(5))
            .context("rendezvous wait bound")?;
        while !reply_path.exists() {
            ensure!(
                Instant::now() < wait_until,
                "bounded original owner clock rendezvous"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let reply_pin = pin(&reply_path, 16 * 1024)?;
        let reply: ClockReply = files::document(&reply_pin, 16 * 1024)?;
        reply_binding(&reply, &claim, &request_pin)?;
        let (origin, until) = original(before, Instant::now(), l, &reply)?;
        Ok(Self {
            input,
            verified,
            started,
            claim,
            control: [started_pin, claim_pin, request_pin, reply_pin],
            origin,
            until,
        })
    }
    pub(in super::super) fn run(&self) -> &Path {
        &self.verified.inventory.identity.run
    }
    pub(in super::super) fn origin(&self) -> Instant {
        self.origin
    }
    pub(in super::super) fn until(&self) -> Instant {
        self.until
    }
    pub(in super::super) fn memory_input(&self) -> Result<memory_scope::RequiredInput> {
        let i = &self.verified.inventory;
        let convert = |p: &Pin| memory_scope::Pin {
            path: p.path.clone(),
            bytes: p.bytes,
            sha256: p.sha256.clone(),
        };
        Ok(memory_scope::RequiredInput {
            run: decoded(&i.identity.run_id)?,
            attempt: i.identity.attempt.clone(),
            parent: self.run().into(),
            inventory: convert(&self.input.inventory),
            images: [
                convert(image(i, "legacy-cli")?),
                convert(image(i, "current-cli")?),
            ],
            shell: s54_memory::query::Shell::validated(
                &i.runtime.memory_shell.path,
                i.runtime.memory_shell.bytes,
                &i.runtime.memory_shell.sha256,
            )?,
        })
    }
    fn revalidate(&self) -> Result<()> {
        s53_deadline::check()?;
        ensure!(
            Instant::now() < self.until,
            "original controller tail deadline"
        );
        for p in &self.control {
            files::pin(p, 16 * 1024)?;
        }
        let p = Policy::production()?;
        require_production(&p)?;
        let next = inspect_state(
            &p,
            &self.input,
            Directory::Owned(&self.started, &self.claim),
        )?;
        ensure!(
            next.inventory == self.verified.inventory
                && next.launch == self.verified.launch
                && next.capture == self.verified.capture
                && next.blake3 == self.verified.blake3,
            "same original admitted runtime closure"
        );
        s53_deadline::check()?;
        Ok(())
    }
    pub(in super::super) fn persist(&self, completed: &final_phase::Completed) -> Result<()> {
        self.revalidate()?;
        let i = &self.verified.inventory;
        let provenance = attempt_result::Provenance::BoundPreRunInventory {
            path: self.input.inventory.path.clone(),
            blake3: self.verified.blake3.clone(),
            identity: i.identity.run_id.clone(),
        };
        attempt_result::persist_bounded_before_tail(
            &i.identity.output,
            &attempt_result::Envelope {
                case: CASE,
                run: self.run(),
                output: &i.identity.output,
                provenance: &provenance,
                observations: completed.facts(),
            },
            512 * 1024,
            || self.revalidate(),
        )?;
        let assertion = pin(&i.identity.output, 512 * 1024)?;
        s53_deadline::check()?;
        let marker = FunctionalMarker {
            version: 1,
            classification: "functional_assertions_completed".into(),
            whole_attempt_verdict: "not_established".into(),
            case: CASE.into(),
            attempt: ATTEMPT.into(),
            run_id: i.identity.run_id.clone(),
            selector: SELECTOR.into(),
            inventory_sha256: self.input.inventory.sha256.clone(),
            inventory_blake3: self.verified.blake3.clone(),
            assertions_sha256: assertion.sha256,
            assertions_bytes: assertion.bytes,
        };
        println!("\nS54_FUNCTIONAL {}", serde_json::to_string(&marker)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_sample_charges_wait_and_never_uses_reply_receipt_as_origin() -> Result<()> {
        let now = Instant::now();
        let before = now - Duration::from_secs(9);
        let dummy_pin = Pin {
            path: PathBuf::from("E:/unused"),
            bytes: 1,
            sha256: "0".repeat(64),
        };
        let l = Launch {
            schema: 1,
            classification: String::new(),
            inventory: dummy_pin.clone(),
            activation: dummy_pin.clone(),
            capture: dummy_pin,
            case: String::new(),
            attempt: String::new(),
            run_id: String::new(),
            run: PathBuf::new(),
            owner_pid: 1,
            launcher_pid: 2,
            start_tick_ms: 1000,
            start_utc_ms: 99999,
            controller_tick_ms: 2351000,
            work_tick_ms: 2376000,
            outer_tick_ms: 2401000,
        };
        let mut r = ClockReply {
            version: 1,
            classification: String::new(),
            case: String::new(),
            attempt: String::new(),
            run_id: String::new(),
            owner_pid: 1,
            launcher_pid: 2,
            harness_pid: 3,
            nonce: String::new(),
            inventory_sha256: String::new(),
            launch_sha256: String::new(),
            request_sha256: String::new(),
            origin_tick_ms: 1000,
            sampled_tick_ms: 11000,
            remaining_controller_ms: 2340000,
        };
        let (origin, until) = original(before, now, &l, &r)?;
        ensure!(
            until == before + Duration::from_millis(2340000)
                && origin == before - Duration::from_secs(10),
            "conservative original origin includes startup/wait"
        );
        let mut changed_wall_clock = l.clone();
        changed_wall_clock.start_utc_ms = u64::MAX;
        ensure!(
            original(before, now, &changed_wall_clock, &r)? == (origin, until),
            "wall-clock metadata cannot extend the monotonic mapping"
        );
        r.sampled_tick_ms = 0;
        ensure!(
            original(before, now, &l, &r).is_err(),
            "stale/reset owner sample"
        );
        r.sampled_tick_ms = 2350999;
        r.remaining_controller_ms = 1;
        ensure!(
            original(before, now, &l, &r).is_err(),
            "delayed reply cannot extend expired bound"
        );
        Ok(())
    }
    #[test]
    fn missing_runtime_inputs_and_command_namespace_refuse_without_setup() -> Result<()> {
        let root = workspace();
        ensure!(
            input_from(&root, |_| anyhow::bail!("missing public input")).is_err(),
            "no default inventory"
        );
        ensure!(
            input_from(&root, |_| Ok("true".into())).is_err(),
            "no boolean input bypass"
        );
        for name in [
            "command-x.stdout.log",
            "command-0000.raw.json",
            "command-0000.stdout.log/../x",
        ] {
            ensure!(!command_log_name(name), "closed command namespace");
        }
        Ok(())
    }
    #[test]
    fn original_reply_requires_the_same_nonce_owner_harness_and_request_bytes() -> Result<()> {
        let claim = Claim {
            version: 1,
            classification: "s54_native_started".into(),
            case: "public-fixture".into(),
            attempt: "public-fixture".into(),
            run_id: "a".repeat(64),
            owner_pid: 1,
            launcher_pid: 2,
            harness_pid: 3,
            nonce: "b".repeat(64),
            inventory_sha256: "c".repeat(64),
            launch_sha256: "d".repeat(64),
        };
        let request = Pin {
            path: PathBuf::from("E:/public-request"),
            bytes: 1,
            sha256: "e".repeat(64),
        };
        let reply = ClockReply {
            version: 1,
            classification: "s54_original_clock_reply".into(),
            case: claim.case.clone(),
            attempt: claim.attempt.clone(),
            run_id: claim.run_id.clone(),
            owner_pid: 1,
            launcher_pid: 2,
            harness_pid: 3,
            nonce: claim.nonce.clone(),
            inventory_sha256: claim.inventory_sha256.clone(),
            launch_sha256: claim.launch_sha256.clone(),
            request_sha256: request.sha256.clone(),
            origin_tick_ms: 1,
            sampled_tick_ms: 2,
            remaining_controller_ms: 1,
        };
        reply_binding(&reply, &claim, &request)?;
        for n in 0..5 {
            let mut bad = reply.clone();
            match n {
                0 => bad.nonce = "f".repeat(64),
                1 => bad.owner_pid = 9,
                2 => bad.harness_pid = 9,
                3 => bad.request_sha256 = "0".repeat(64),
                _ => bad.launch_sha256 = "0".repeat(64),
            }
            ensure!(
                reply_binding(&bad, &claim, &request).is_err(),
                "same actual owner IPC binding {n}"
            );
        }
        Ok(())
    }
}
