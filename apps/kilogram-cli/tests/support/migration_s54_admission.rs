//! Public schema inspection and opaque production admission. Facts are not a capability.
use super::*;
#[path = "migration_s54_admission_schema.rs"]
mod schema;
pub(super) use schema::Pin;
use schema::*;
#[path = "migration_s54_admission_diagnostic.rs"]
mod diagnostic;
use diagnostic::Code as D;
pub(super) use diagnostic::{Code as ReleaseCode, report_code as report_release};
#[path = "migration_s54_admission_compiler.rs"]
mod compiler;
#[path = "migration_s54_admission_files.rs"]
mod files;
#[cfg(all(test, windows))]
#[path = "migration_s54_admission_tests.rs"]
mod tests;
const CASE: &str = "P-s54-whole-resumed-l1-01";
const ATTEMPT: &str = "P-s54-whole-resumed-l1-01-attempt-01";
const SELECTOR: &str = "live_l1::handoff::final_p5::s54_continuous_whole_resumed_l1_p5_public";
const TARGET: &str = "x86_64-pc-windows-msvc";
const PROFILE: &str = "verification-scale";
const MIB: usize = 1024 * 1024;
const LEGACY: &str = "d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1";
const RELAY: &str = "cf9d029cebf1d04745c90a9712401d70e823ecf4af5a082fe7269649df358e5e";
const S53: &str = "1f404b54ebce47ae3b52eaeb6855e30f3b8f3309";
const CLOSURE: &str = "20418e41e4c486158fdc30b0c9f857750432e25e";
// role, exact fixed relative path, package, target kind, target name.
const IMAGES: [(&str, &str, &str, &str, &str); 9] = [
    (
        "legacy-cli",
        "target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe",
        "",
        "",
        "",
    ),
    (
        "current-cli",
        "target/compatibility/long-lived-dialog-migration/kilogram-cli.exe",
        "kilogram-cli",
        "bin",
        "kilogram-cli",
    ),
    (
        "relay",
        "target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe",
        "",
        "",
        "",
    ),
    (
        "stable-cli-bin",
        "target/verification-scale/deps/kilogram-cli-bin-kilogram-cli-tests.exe",
        "kilogram-cli",
        "bin",
        "kilogram-cli",
    ),
    (
        "stable-cli-lib",
        "target/verification-scale/deps/kilogram-cli-lib-kilogram_cli-tests.exe",
        "kilogram-cli",
        "lib",
        "kilogram_cli",
    ),
    (
        "stable-migration",
        "target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe",
        "kilogram-cli",
        "test",
        "migration_s5_process",
    ),
    (
        "stable-runtime2",
        "target/verification-scale/deps/kilogram-cli-test-runtime2_process-tests.exe",
        "kilogram-cli",
        "test",
        "runtime2_process",
    ),
    (
        "stable-iroh-lib",
        "target/verification-scale/deps/kilogram-transport-iroh-lib-kilogram_transport_iroh-tests.exe",
        "kilogram-transport-iroh",
        "lib",
        "kilogram_transport_iroh",
    ),
    (
        "stable-iroh-relay",
        "target/verification-scale/deps/kilogram-transport-iroh-test-local_relay_transport-tests.exe",
        "kilogram-transport-iroh",
        "test",
        "local_relay_transport",
    ),
];
struct Policy {
    root: PathBuf,
    configs: Vec<PathBuf>,
    resolutions: Vec<Resolution>,
    legacy: (u64, String),
    relay: (u64, String),
    fixture: bool,
}
impl Policy {
    fn production() -> Result<Self> {
        ensure!(
            cfg!(all(windows, target_pointer_width = "64")),
            "S54 production admission requires x64 Windows"
        );
        let root = workspace().canonicalize()?;
        Ok(Self {
            configs: config_paths(&root)?,
            resolutions: resolutions()?,
            root,
            legacy: (61531648, LEGACY.into()),
            relay: (10460160, RELAY.into()),
            fixture: false,
        })
    }
}
#[derive(Clone)]
pub(super) struct Inputs {
    pub inventory: Pin,
    pub activation: Pin,
    pub launch: Pin,
    pub capture: Pin,
}
pub(super) struct Inspection {
    facts: Value,
    inventory: Inventory,
    launch: Launch,
    capture: Capture,
    blake3: String,
}
#[path = "migration_s54_admission_runtime.rs"]
mod runtime;
pub(super) use runtime::AdmittedRun;
impl Inspection {
    pub fn facts(&self) -> &Value {
        &self.facts
    }
}
pub(super) fn inspect(input: &Inputs) -> Result<Inspection> {
    inspect_with(&Policy::production()?, input)
}
fn same_pin(a: &Pin, b: &Pin) -> Result<()> {
    files::same(&a.path, &b.path)?;
    ensure!(
        a.bytes == b.bytes && a.sha256 == b.sha256,
        "exact bound public pin bytes/hash"
    );
    Ok(())
}
fn deadline(d: &Deadline, purpose: Purpose) -> Result<()> {
    ensure!(
        [d.controller, d.work, d.outer] == purpose.limits()
            && d.origin == "owned_launch"
            && d.clock == "windows_tick_count64",
        "exact original S54 deadline policy"
    );
    Ok(())
}
fn head(root: &Path) -> Result<String> {
    let raw = files::read(&root.join(".git/HEAD"), 4096)?;
    let s = std::str::from_utf8(&raw)?.trim();
    let value = if let Some(reference) = s.strip_prefix("ref: ") {
        ensure!(
            reference.starts_with("refs/heads/") && !reference.contains(".."),
            "local source HEAD ref"
        );
        let loose = root.join(".git").join(reference);
        if loose.is_file() {
            String::from_utf8(files::read(&loose, 4096)?)?
                .trim()
                .to_owned()
        } else {
            let packed = String::from_utf8(files::read(&root.join(".git/packed-refs"), MIB)?)?;
            let found = packed
                .lines()
                .filter_map(|s| s.split_once(' '))
                .filter(|(_, r)| *r == reference)
                .collect::<Vec<_>>();
            ensure!(found.len() == 1, "exact packed source HEAD");
            found[0].0.to_owned()
        }
    } else {
        s.into()
    };
    files::digest(&value, 40)?;
    Ok(value)
}
fn unique_pins(rows: &[Pin], cap: usize) -> Result<()> {
    ensure!(
        !rows.is_empty() && rows.len() <= cap,
        "bounded nonempty pin set"
    );
    let mut seen = BTreeSet::new();
    for p in rows {
        ensure!(seen.insert(files::key(&p.path)?), "duplicate path pin");
    }
    Ok(())
}
fn config_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for dir in root.ancestors() {
        for n in ["config", "config.toml"] {
            out.push(dir.join(".cargo").join(n));
        }
    }
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|s| PathBuf::from(s).join(".cargo")))
        .context("explicit Cargo home")?;
    for n in ["config", "config.toml"] {
        out.push(home.join(n));
    }
    out.sort();
    out.dedup();
    Ok(out)
}
fn config_check(expected: &[PathBuf], rows: &[Config]) -> Result<()> {
    ensure!(
        rows.len() == expected.len() && rows.len() <= 64,
        "exact Cargo config search set"
    );
    let mut seen = BTreeSet::new();
    for r in rows {
        ensure!(
            seen.insert(files::key(&r.path)?),
            "duplicate Cargo config path"
        );
        ensure!(
            expected.iter().any(|p| files::same(p, &r.path).is_ok()),
            "unapproved config path role"
        );
        ensure!(
            r.exists == r.path.is_file() && r.exists == r.sha256.is_some(),
            "actual config presence"
        );
        if let Some(hash) = &r.sha256 {
            files::digest(hash, 64)?;
            let raw = files::read(&r.path, 64 * 1024)?;
            ensure!(files::sha(&raw) == *hash, "actual config bytes");
            let text = std::str::from_utf8(&raw)?.to_lowercase();
            for word in [
                "profile",
                "rustflags",
                "rustc",
                "wrapper",
                "include",
                "[env]",
                "linker",
                "build.target",
            ] {
                ensure!(
                    !text.contains(word),
                    "unsupported profile-sensitive Cargo config"
                );
            }
        } else {
            files::path(&r.path, true)?;
        }
    }
    Ok(())
}
fn environment() -> Result<Vec<Environment>> {
    let mut map = BTreeMap::new();
    for (k, v) in std::env::vars_os() {
        let k = k.to_string_lossy().to_uppercase();
        let v = v.to_string_lossy().into_owned();
        if let Some(old) = map.insert(k, v.clone()) {
            ensure!(old == v, "conflicting environment aliases");
        }
    }
    let mut names = BTreeSet::from_iter(
        [
            "PATH",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
            "CARGO_NET_OFFLINE",
            "CARGO_INCREMENTAL",
            "CC",
            "CXX",
            "AR",
            "CFLAGS",
            "CXXFLAGS",
            "ARFLAGS",
            "INCLUDE",
            "LIB",
            "LIBPATH",
            "WINDOWSSDKDIR",
            "VCTOOLSINSTALLDIR",
            "VSINSTALLDIR",
        ]
        .map(str::to_owned),
    );
    for k in map.keys() {
        if k.starts_with("RUST")
            || [
                "CARGO_PROFILE_",
                "CARGO_BUILD_",
                "CARGO_TARGET_",
                "CARGO_ENCODED_",
                "CARGO_INCREMENTAL",
                "CARGO_FEATURE_",
                "CARGO_FEATURES",
                "CARGO_FLAGS",
            ]
            .iter()
            .any(|p| k.starts_with(p))
        {
            names.insert(k.clone());
        }
    }
    ensure!(names.len() <= 128, "bounded relevant environment");
    let mut out = Vec::new();
    for n in names {
        let value = map.get(&n);
        if let Some(v) = value {
            ensure!(
                !(n == "RUSTFLAGS"
                    || n == "CARGO_ENCODED_RUSTFLAGS"
                    || n == "RUSTC"
                    || n.starts_with("RUSTC_")
                    || n.starts_with("CARGO_PROFILE_")
                    || n.starts_with("CARGO_BUILD_")
                    || n == "CARGO_TARGET_DIR"
                    || (n.starts_with("CARGO_TARGET_")
                        && ["_RUSTFLAGS", "_LINKER", "_RUNNER"]
                            .iter()
                            .any(|p| n.ends_with(p)))
                    || (n == "CARGO_INCREMENTAL" && v != "0")),
                "unproved compiler environment override"
            );
        }
        out.push(Environment {
            name: n,
            present: value.is_some(),
            value_sha256: value.map(|v| files::sha(v.as_bytes())),
        });
    }
    Ok(out)
}
fn env_check(rows: &[Environment]) -> Result<()> {
    diagnostic::at(D::Environment, || {
        ensure!(
            env_identity(rows)? == environment()? && std::env::var("CARGO_NET_OFFLINE")? == "true",
            "exact actual frozen environment closure"
        );
        Ok(())
    })?;
    Ok(())
}
fn env_identity(rows: &[Environment]) -> Result<Vec<Environment>> {
    ensure!(rows.len() <= 128, "environment row bound");
    let mut sorted = rows.to_vec();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for pair in sorted.windows(2) {
        ensure!(pair[0].name != pair[1].name, "duplicate environment role");
    }
    Ok(sorted)
}
fn resolutions() -> Result<Vec<Resolution>> {
    let dirs = std::env::var_os("PATH").context("runtime PATH")?;
    let dirs = std::env::split_paths(&dirs).collect::<Vec<_>>();
    let mut out = Vec::new();
    for name in [
        "pwsh.exe",
        "powershell.exe",
        "git.exe",
        "rustup.exe",
        "cargo.exe",
        "rustc.exe",
    ] {
        let p = dirs
            .iter()
            .map(|p| p.join(name))
            .find(|p| p.is_file())
            .context("required scalar executable resolution")?;
        out.push(Resolution {
            name: name.into(),
            path: files::path(&p, false)?,
        });
    }
    Ok(out)
}
fn source_check(p: &Policy, s: &Source) -> Result<()> {
    ensure!(
        s.s53_product == S53 && s.closure_publication == CLOSURE,
        "distinct accepted S53 product/closure identities"
    );
    files::digest(&s.implementation, 40)?;
    files::digest(&s.observed_head, 40)?;
    diagnostic::at(D::SourceHead, || {
        ensure!(
            s.implementation == s.observed_head
                && s.observed_head == diagnostic::at(D::SourceHead, || head(&p.root))?,
            "actual reviewed implementation HEAD"
        );
        Ok(())
    })?;
    unique_pins(&s.files, 4096)?;
    let actual = files::sources(&p.root)?;
    let expected = s
        .files
        .iter()
        .map(|v| files::key(&v.path))
        .collect::<Result<BTreeSet<_>>>()?;
    diagnostic::at(D::SourceSet, || {
        ensure!(
            expected
                == actual
                    .iter()
                    .map(|v| files::key(v))
                    .collect::<Result<BTreeSet<_>>>()?,
            "exact relevant filesystem source set, no historical recursion"
        );
        Ok(())
    })?;
    for r in &s.files {
        diagnostic::at(D::SourcePin, || files::pin(r, 8 * MIB))?;
    }
    Ok(())
}
fn image<'a>(i: &'a Inventory, role: &str) -> Result<&'a Pin> {
    Ok(&i
        .images
        .iter()
        .find(|r| r.role == role)
        .context("required executable role")?
        .pin)
}
fn images(p: &Policy, i: &Inventory) -> Result<()> {
    diagnostic::at(D::ImageRoles, || {
        ensure!(i.images.len() == 9, "exact nine executable roles");
        Ok(())
    })?;
    let mut roles = BTreeSet::new();
    for r in &i.images {
        diagnostic::at(D::ImageRoles, || {
            ensure!(roles.insert(r.role.as_str()), "duplicate executable role");
            Ok(())
        })?;
        let spec = IMAGES
            .iter()
            .find(|s| s.0 == r.role)
            .context("unknown executable role")?;
        files::same(&r.pin.path, &p.root.join(spec.1))?;
        ensure!(r.pin.bytes > 0, "nonempty image");
        diagnostic::at(D::ImagePin, || files::pin(&r.pin, 128 * MIB))?;
    }
    unique_pins(
        &i.images.iter().map(|r| r.pin.clone()).collect::<Vec<_>>(),
        9,
    )?;
    for (role, expected) in [("legacy-cli", &p.legacy), ("relay", &p.relay)] {
        let pin = image(i, role)?;
        ensure!(
            pin.bytes == expected.0 && pin.sha256 == expected.1,
            "fixed accepted origin/relay policy bytes"
        );
    }
    Ok(())
}
fn runtime(p: &Policy, r: &Runtime) -> Result<()> {
    diagnostic::at(D::RuntimePolicy, || {
        ensure!(
            r.cargo_jobs == 2
                && r.priority == "BelowNormal"
                && r.offline
                && r.locked
                && r.skip_cache
                && r.architecture == "x64",
            "required runtime resource/tool policy"
        );
        Ok(())
    })?;
    ensure!(
        r.resolutions.len() == p.resolutions.len(),
        "exact runtime resolvers"
    );
    for (a, b) in r.resolutions.iter().zip(&p.resolutions) {
        ensure!(a.name == b.name, "resolver name/order");
        files::same(&a.path, &b.path)?;
    }
    files::same(&r.metadata_shell.path, &p.resolutions[0].path)?;
    files::same(&r.memory_shell.path, &p.resolutions[1].path)?;
    if !p.fixture {
        files::same(
            &r.memory_shell.path,
            Path::new("C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"),
        )?;
    }
    for pin in [&r.metadata_shell, &r.memory_shell] {
        ensure!(pin.bytes > 0, "nonempty shell");
        files::pin(pin, 128 * MIB)?;
    }
    for dir in [&p.root, &p.root.join("target/verification-scale/deps")] {
        ensure!(!dir.join("powershell.exe").exists(), "memory shell shadow");
    }
    diagnostic::at(D::Environment, || env_check(&r.environment))?;
    diagnostic::at(D::RuntimeConfig, || config_check(&p.configs, &r.configs))?;
    let actual = std::env::var("TOKIO_WORKER_THREADS").ok();
    let available = std::thread::available_parallelism()?.get();
    let effective = actual
        .as_ref()
        .map(|v| v.parse::<usize>())
        .transpose()?
        .unwrap_or(available);
    ensure!(
        r.workers.present == actual.is_some()
            && r.workers.value == actual
            && r.workers.available == available
            && r.workers.effective == effective
            && effective >= 2,
        "actual runtime worker identity, not Cargo jobs"
    );
    Ok(())
}
fn fresh(p: &Policy, i: &Identity) -> Result<()> {
    directory(p, i, Directory::Fresh)
}
#[derive(Clone, Copy)]
enum Directory<'a> {
    Fresh,
    BeforeNative(&'a Started),
    Owned(&'a Started, &'a Claim),
}
fn directory(p: &Policy, i: &Identity, state: Directory<'_>) -> Result<()> {
    ensure!(
        Purpose::identity(&i.case, &i.attempt, &i.selector).is_ok(),
        "exact S54 case/attempt/selector"
    );
    files::digest(&i.run_id, 64)?;
    ensure!(i.run_id != "0".repeat(64), "nonzero run identity");
    files::same(&i.workspace, &p.root)?;
    files::same(&i.run, &Purpose::from_case(&i.case)?.run(&p.root))?;
    ensure!(i.run.is_dir(), "reserved fresh owned run directory");
    for (path, name) in [
        (&i.output, "assertions.json"),
        (&i.activation, "activation.json"),
        (&i.launch, "launch.json"),
        (&i.capture, "capture.json"),
    ] {
        files::same(path, &i.run.join(name))?;
    }
    for e in std::fs::read_dir(&i.run)? {
        let e = e?;
        files::regular(&e.path())?;
        let name = e.file_name();
        let name = name.to_str().context("owned run filename")?;
        let basic = [
            "inputs.json",
            "activation.json",
            "launch.json",
            "capture.json",
        ]
        .contains(&name);
        let owned = match state {
            Directory::Fresh => false,
            Directory::BeforeNative(_) => name == "started.json",
            Directory::Owned(_, claim) => {
                [
                    "started.json",
                    "native-started.json",
                    "clock-request.json",
                    "clock-reply.json",
                    "assertions.json",
                    "memory-observer-final.json",
                    "memory-observer-abort.json",
                ]
                .contains(&name)
                    || name == format!("join-{}.json", claim.harness_pid)
                    || name == format!("permit-{}.json", claim.harness_pid)
                    || (["native-stdout.protocol.log", "native-stderr.protocol.log"]
                        .contains(&name)
                        && e.path().is_file()
                        && e.metadata()?.len() <= MIB as u64)
                    || (name == Purpose::from_case(&i.case)?.origin_name() && e.path().is_dir())
                    || runtime::command_log_name(name)
            }
        };
        ensure!(basic || owned, "dirty/reused attempt directory");
    }
    if let Directory::BeforeNative(started) | Directory::Owned(started, _) = state {
        let actual: Started = files::json(
            &files::read(&i.run.join("started.json"), 16 * 1024)?,
            16 * 1024,
        )?;
        ensure!(
            &actual == started
                && actual.case == i.case
                && actual.attempt == i.attempt
                && actual.run_id == i.run_id
                && actual.selector == i.selector
                && actual.version == 1
                && actual.classification == "s54_exclusive_attempt",
            "owner-bound started claim"
        );
        files::digest(&actual.nonce, 64)?;
        ensure!(
            actual.nonce != "0".repeat(64) && actual.owner_pid > 0,
            "started nonce/owner"
        );
    }
    if let Directory::Owned(started, claim) = state {
        let actual: Claim = files::json(
            &files::read(&i.run.join("native-started.json"), 16 * 1024)?,
            16 * 1024,
        )?;
        ensure!(
            &actual == claim
                && claim.version == 1
                && claim.classification == "s54_native_started"
                && claim.case == i.case
                && claim.attempt == i.attempt
                && claim.run_id == i.run_id
                && claim.owner_pid == started.owner_pid
                && claim.nonce == started.nonce
                && claim.inventory_sha256 == started.inventory.sha256
                && claim.launcher_pid > 0
                && claim.harness_pid > 0
                && claim.harness_pid != claim.owner_pid
                && claim.harness_pid != claim.launcher_pid,
            "same exclusive native claim at revalidation"
        );
    }
    Ok(())
}
fn selection(i: &Inventory) -> Result<()> {
    let s = &i.selection;
    ensure!(
        s.harness_ref == "stable-migration"
            && s.named == 1
            && s.native_markers == 1
            && s.wrapper_markers == 1
            && s.aggregates == 0,
        "one exact whole selection, no synthetic success flags"
    );
    let doc: Isolated = files::document(&s.isolated, 64 * 1024)?;
    ensure!(
        doc.format_version == 1 && doc.tests.len() == 1,
        "one isolated test"
    );
    let t = &doc.tests[0];
    ensure!(
        t.package == "kilogram-cli"
            && t.target == "migration_s5_process"
            && t.kind == "test"
            && t.name == Purpose::from_case(&i.identity.case)?.selector(),
        "exact isolated full name/target"
    );
    let expected_selector = Purpose::from_case(&i.identity.case)?.selector();
    let list = files::pin(&s.list, MIB)?;
    let list = std::str::from_utf8(&list)?;
    ensure!(
        list.lines()
            .filter(|v| *v == format!("{}: test", expected_selector))
            .count()
            == 1,
        "prepared exact selector inventory once"
    );
    Ok(())
}
fn references(p: &Policy, i: &Inventory) -> Result<()> {
    let dir = p
        .root
        .join(".tmp")
        .join(Purpose::from_case(&i.identity.case)?.public_namespace())
        .join(Purpose::from_case(&i.identity.case)?.attempt());
    for (pin, name) in [
        (&i.compiler.normal, "normal.json"),
        (&i.compiler.tests, "tests.json"),
        (&i.compiler.frozen_manifest, "frozen.json"),
        (&i.selection.isolated, "isolated.json"),
        (&i.selection.list, "list.txt"),
    ] {
        files::same(&pin.path, &dir.join(name))?;
    }
    Ok(())
}
fn inspect_with(p: &Policy, input: &Inputs) -> Result<Inspection> {
    inspect_state(p, input, Directory::Fresh)
}
fn inspect_state(p: &Policy, input: &Inputs, state: Directory<'_>) -> Result<Inspection> {
    let run = input
        .inventory
        .path
        .parent()
        .context("fixed purpose input directory")?;
    diagnostic::at(D::Namespace, || {
        ensure!(
            [Purpose::WholeP, Purpose::FeasibilityF]
                .into_iter()
                .any(|purpose| purpose.run(&p.root) == run),
            "fixed S54 input namespace"
        );
        Ok(())
    })?;
    for (pin, name) in [
        (&input.inventory, "inputs.json"),
        (&input.activation, "activation.json"),
        (&input.launch, "launch.json"),
        (&input.capture, "capture.json"),
    ] {
        diagnostic::at(D::Namespace, || files::same(&pin.path, &run.join(name)))?;
    }
    let raw = diagnostic::at(D::InputPin, || files::pin(&input.inventory, 2 * MIB))?;
    let i: Inventory = diagnostic::at(D::InputDocument, || files::json(&raw, 2 * MIB))?;
    let a: Activation = diagnostic::at(D::InputDocument, || {
        files::document(&input.activation, 16 * 1024)
    })?;
    let l: Launch = diagnostic::at(D::InputDocument, || {
        files::document(&input.launch, 16 * 1024)
    })?;
    let c: Capture = diagnostic::at(D::InputDocument, || files::document(&input.capture, 4096))?;
    diagnostic::at(D::InputDocument, || {
        ensure!(
            i.schema == 1
                && i.classification == Purpose::from_case(&i.identity.case)?.inventory_class(),
            "S54 inventory schema"
        );
        Ok(())
    })?;
    let purpose = diagnostic::at(D::Namespace, || {
        Purpose::identity(&i.identity.case, &i.identity.attempt, &i.identity.selector)
    })?;
    diagnostic::at(D::Namespace, || deadline(&i.deadline, purpose))?;
    match state {
        Directory::Fresh => diagnostic::at(D::Directory, || fresh(p, &i.identity))?,
        _ => diagnostic::at(D::Directory, || directory(p, &i.identity, state))?,
    }
    diagnostic::at(D::Namespace, || references(p, &i))?;
    diagnostic::at(D::SourceUnknown, || source_check(p, &i.source))?;
    diagnostic::at(D::ImagesUnknown, || images(p, &i))?;
    diagnostic::at(D::RuntimeUnknown, || runtime(p, &i.runtime))?;
    diagnostic::at(D::Selection, || selection(&i))?;
    files::same(&input.inventory.path, &i.identity.run.join("inputs.json"))?;
    for (x, y) in [
        (&input.activation.path, &i.identity.activation),
        (&input.launch.path, &i.identity.launch),
        (&input.capture.path, &i.identity.capture),
    ] {
        files::same(x, y)?;
    }
    diagnostic::at(D::Activation, || {
        ensure!(
            a.schema == 1
                && a.classification == purpose.activation_class()
                && a.case == i.identity.case
                && a.attempt == i.identity.attempt
                && a.run_id == i.identity.run_id
                && a.selector == i.identity.selector
                && a.implementation == i.source.implementation
                && a.execution_mode == "NativeToken"
                && a.deadline == i.deadline
                && a.retry == "root_reallocation_only",
            "external activation binds exact I, not itself"
        );
        Ok(())
    })?;
    same_pin(&a.inventory, &input.inventory)?;
    files::same(&a.run, &i.identity.run)?;
    if let Directory::BeforeNative(started) | Directory::Owned(started, _) = state {
        same_pin(&started.inventory, &input.inventory)?;
        same_pin(&started.activation, &input.activation)?;
        diagnostic::at(D::NativeClaim, || {
            ensure!(
                started.owner_pid == l.owner_pid && started.owner_pid == c.owner_pid,
                "actual launch owner matches claim"
            );
            Ok(())
        })?;
    }
    if let Directory::Owned(_, claim) = state {
        diagnostic::at(D::NativeClaim, || {
            ensure!(
                claim.launch_sha256 == input.launch.sha256
                    && claim.launcher_pid == c.launcher_pid
                    && (p.fixture || claim.harness_pid == std::process::id()),
                "native original launch/harness binding"
            );
            Ok(())
        })?;
    }
    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis(),
    )?;
    diagnostic::at(D::ActivationValidity, || {
        ensure!(
            a.not_before_utc_ms < a.not_after_utc_ms
                && now >= a.not_before_utc_ms
                && now < a.not_after_utc_ms,
            "activation validity window"
        );
        Ok(())
    })?;
    diagnostic::at(D::Launch, || {
        ensure!(
            l.schema == 1
                && l.classification == "s54_original_launch_binding"
                && l.case == a.case
                && l.attempt == a.attempt
                && l.run_id == a.run_id
                && l.start_tick_ms > 0
                && l.start_utc_ms >= a.not_before_utc_ms
                && l.start_utc_ms < a.not_after_utc_ms
                && l.start_utc_ms <= now,
            "exact I/A/L/C launch identities/order"
        );
        Ok(())
    })?;
    same_pin(&l.inventory, &input.inventory)?;
    same_pin(&l.activation, &input.activation)?;
    same_pin(&l.capture, &input.capture)?;
    files::same(&l.run, &a.run)?;
    for (actual, seconds) in [
        (l.controller_tick_ms, purpose.limits()[0]),
        (l.work_tick_ms, purpose.limits()[1]),
        (l.outer_tick_ms, purpose.limits()[2]),
    ] {
        diagnostic::at(D::Launch, || {
            ensure!(
                actual
                    == l.start_tick_ms
                        .checked_add(seconds * 1000)
                        .context("launch tick overflow")?,
                "original numeric monotonic cutoffs"
            );
            Ok(())
        })?;
    }
    let utc_end = l
        .start_utc_ms
        .checked_add(purpose.limits()[2] * 1000)
        .context("launch UTC overflow")?;
    diagnostic::at(D::Capture, || {
        ensure!(
            now < utc_end
                && c.version == 1
                && c.attempt == a.attempt
                && c.owner_pid == l.owner_pid
                && c.launcher_pid == l.launcher_pid
                && c.owner_pid > 0
                && c.launcher_pid > 0
                && c.owner_pid != c.launcher_pid
                && c.whole_utc_ms == utc_end,
            "exact current capture binding, no reset"
        );
        Ok(())
    })?;
    files::same(&c.harness_path, &image(&i, "stable-migration")?.path)?;
    if !p.fixture {
        files::same(&std::env::current_exe()?, &c.harness_path)?;
    }
    diagnostic::at(D::CompilerUnknown, || compiler::validate(p, &i))?;
    // All admitted public files are checked again after traversing references.
    for pin in [
        &input.inventory,
        &input.activation,
        &input.launch,
        &input.capture,
    ] {
        diagnostic::at(D::InputPin, || files::pin(pin, 2 * MIB))?;
    }
    let inventory_blake3 = blake3::hash(&raw).to_hex().to_string();
    Ok(Inspection {
        facts: json!({"classification":if p.fixture{"public_schema_fixture_only"}else{"slice_a_schema_inspection_only"},
        "inventory_sha256":files::sha(&raw),"inventory_blake3":blake3::hash(&raw).to_hex().to_string(),
        "case":i.identity.case,"attempt":i.identity.attempt,"run_id":i.identity.run_id,
        "clock":"inspection only; production runtime requires original owner rendezvous",
        "whole_attempt_verdict":"not_established"}),
        inventory: i,
        launch: l,
        capture: c,
        blake3: inventory_blake3,
    })
}
