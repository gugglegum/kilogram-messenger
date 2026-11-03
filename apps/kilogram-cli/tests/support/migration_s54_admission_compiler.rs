//! Validate the existing frozen capture shape; do not build another Cargo graph.
use super::*;
pub(super) const REQUIRED: [&str; 12] = [
    "kilogram-cli",
    "kilogram-state",
    "kilogram-identity",
    "kilogram-protocol",
    "kilogram-transport-iroh",
    "kilogram-ratchet",
    "redb",
    "ed25519-dalek",
    "curve25519-dalek",
    "chacha20poly1305",
    "iroh",
    "vodozemac",
];
fn strings(rows: &[String], max: usize) -> Result<()> {
    ensure!(rows.len() <= max, "bounded compiler strings");
    let mut seen = BTreeSet::new();
    for s in rows {
        ensure!(
            !s.is_empty() && s.len() <= 1024 && seen.insert(s),
            "bounded unique compiler string"
        );
    }
    Ok(())
}
fn inputs(p: &Policy, i: &Inventory, b: &BuildInputs) -> Result<()> {
    ensure!(
        b.format_version == 1
            && b.profile == PROFILE
            && b.target == TARGET
            && b.packages == ["kilogram-cli", "kilogram-transport-iroh"]
            && b.cargo_features.is_empty(),
        "shared frozen build selection"
    );
    ensure!(
        env_identity(&b.environment)? == env_identity(&i.runtime.environment)?
            && b.configs == i.runtime.configs,
        "compiler/runtime config/environment identity"
    );
    ensure!(
        b.rustc.iter().any(|v| v == "release: 1.98.0")
            && b.rustc.iter().any(|v| v == &format!("host: {TARGET}"))
            && !b.cargo.is_empty(),
        "required compiler/host version evidence"
    );
    strings(&b.rustc, 32)?;
    strings(&b.cargo, 32)?;
    ensure!(b.tools.len() == 2, "exact compiler tool roles");
    for (r, name) in b.tools.iter().zip(["cargo", "rustc"]) {
        ensure!(r.name == name, "compiler tool order");
        files::digest(&r.sha256, 64)?;
        files::digest(&r.shim_sha256, 64)?;
        ensure!(
            r.path
                .file_name()
                .is_some_and(|v| v == format!("{name}.exe").as_str()),
            "compiler executable role, not arbitrary data"
        );
        let resolution = i
            .runtime
            .resolutions
            .iter()
            .find(|r| r.name == format!("{name}.exe"))
            .context("actual tool resolver")?;
        files::same(&r.shim_path, &resolution.path)?;
        for (path, hash) in [(&r.path, &r.sha256), (&r.shim_path, &r.shim_sha256)] {
            let bytes = files::read(path, 128 * MIB)?;
            ensure!(
                !bytes.is_empty() && files::sha(&bytes) == *hash,
                "actual compiler/tool shim bytes"
            );
        }
    }
    let mut source = BTreeMap::new();
    ensure!(b.source_inputs.len() <= 4096, "compiler source rows bound");
    for row in &b.source_inputs {
        files::digest(&row.sha256, 64)?;
        ensure!(
            !row.path.contains('\\')
                && !Path::new(&row.path).is_absolute()
                && !Path::new(&row.path)
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                && source
                    .insert(row.path.to_lowercase(), row.sha256.as_str())
                    .is_none(),
            "unique relative compiler source row"
        );
    }
    // Existing captures include archived ai-docs/fuzz rows. Project only the
    // explicitly admitted live code scope; never traverse those archives.
    let expected = i
        .source
        .files
        .iter()
        .filter_map(|r| {
            let root = files::path(&p.root, false).ok()?;
            let canonical = files::path(&r.path, false).ok()?;
            let rel = canonical
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            if files::EMBEDDED.contains(&rel.as_str())
                || (files::HELPERS.contains(&rel.as_str())
                    && !files::HELPERS[..3].contains(&rel.as_str()))
            {
                None
            } else {
                Some((rel.to_lowercase(), r.sha256.as_str()))
            }
        })
        .collect::<BTreeMap<_, _>>();
    let actual = b
        .source_inputs
        .iter()
        .filter(|r| files::relevant(&r.path) && !files::EMBEDDED.contains(&r.path.as_str()))
        .map(|r| (r.path.to_lowercase(), r.sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        actual == expected,
        "frozen machinery relevant source projection, explicit extra S54 helpers separately pinned"
    );
    ensure!(
        !b.package_names.is_empty()
            && b.package_names.len() <= 4096
            && b.resolved_graph.len() == b.package_names.len(),
        "complete bounded dependency graph"
    );
    let mut packages = BTreeMap::new();
    for r in &b.package_names {
        ensure!(
            !r.id.is_empty()
                && r.id.len() <= 1024
                && !r.name.is_empty()
                && r.name.len() <= 256
                && packages.insert(&r.id, &r.name).is_none(),
            "unique package ID/name map"
        );
    }
    let mut graph = BTreeSet::new();
    for r in &b.resolved_graph {
        ensure!(
            packages.contains_key(&r.id) && graph.insert(&r.id),
            "exact graph package set"
        );
        strings(&r.features, 256)?;
        ensure!(r.deps.len() <= 4096, "dependency edge bound");
        let mut deps = BTreeSet::new();
        for d in &r.deps {
            ensure!(
                packages.contains_key(&d.pkg)
                    && !d.name.is_empty()
                    && d.name.len() <= 256
                    && deps.insert((&d.name, &d.pkg))
                    && !d.dep_kinds.is_empty()
                    && d.dep_kinds.len() <= 32,
                "known unique graph edge"
            );
            let mut kinds = BTreeSet::new();
            for k in &d.dep_kinds {
                ensure!(
                    k.kind
                        .as_deref()
                        .is_none_or(|v| ["normal", "build", "dev"].contains(&v))
                        && k.target.as_ref().is_none_or(|v| v.len() <= 1024)
                        && kinds.insert((&k.kind, &k.target)),
                    "bounded dependency kinds"
                );
            }
        }
    }
    Ok(())
}
fn units(p: &Policy, i: &Inventory, c: &CompilerCapture) -> Result<()> {
    ensure!(
        !c.units.is_empty()
            && c.units.len() <= 4096
            && c.raw_compiler_artifact_json.len() == c.units.len(),
        "complete compiler artifact/units evidence"
    );
    let packages = c
        .build_inputs_before
        .package_names
        .iter()
        .map(|r| (&r.id, &r.name))
        .collect::<BTreeMap<_, _>>();
    let prefix = files::path(&p.root.join("target").join(TARGET).join(PROFILE), true)?;
    let mut normalized = Vec::new();
    for line in &c.raw_compiler_artifact_json {
        let r: CargoArtifact = files::json(line.as_bytes(), 128 * 1024)?;
        ensure!(
            r.reason == "compiler-artifact",
            "actual compiler-artifact record"
        );
        let package = packages
            .get(&r.package_id)
            .context("known compiler package ID")?;
        strings(&r.target.kind, 8)?;
        strings(&r.target.crate_types, 8)?;
        strings(&r.features, 256)?;
        ensure!(
            !r.target.kind.is_empty()
                && !r.target.name.is_empty()
                && r.target.name.len() <= 256
                && ["2015", "2018", "2021", "2024"].contains(&r.target.edition.as_str())
                && !r.filenames.is_empty()
                && r.filenames.len() <= 32,
            "bounded compiler target/output facts"
        );
        let mut names = BTreeSet::new();
        for path in &r.filenames {
            ensure!(
                names.insert(files::key(path)?),
                "unique compiler output paths"
            );
        }
        let manifest = files::path(&r.manifest_path, false)?;
        files::path(&r.target.src_path, false)?;
        if manifest.starts_with(files::path(&p.root, false)?) {
            for path in [&r.manifest_path, &r.target.src_path] {
                ensure!(
                    i.source
                        .files
                        .iter()
                        .any(|f| files::same(&f.path, path).is_ok()),
                    "workspace compiler target belongs to admitted live sources"
                );
            }
        }
        let runtime = r
            .filenames
            .iter()
            .any(|f| files::path(f, true).is_ok_and(|v| v.starts_with(&prefix)))
            && !r
                .target
                .kind
                .iter()
                .any(|s| ["custom-build", "proc-macro"].contains(&s.as_str()));
        if runtime {
            ensure!(
                r.profile.opt_level == "3"
                    && r.profile.debug_assertions
                    && r.profile.overflow_checks
                    && r.profile.debuginfo == "line-tables-only",
                "runtime opt3/debug/overflow profile"
            );
        }
        let mut filenames = r.filenames;
        filenames.sort();
        normalized.push(Unit {
            package_id: r.package_id,
            package: (*package).clone(),
            kind: r.target.kind,
            target: r.target.name,
            features: r.features,
            profile: r.profile,
            filenames,
            executable: r.executable,
            class: if runtime {
                "runtime-target"
            } else {
                "host-build-or-proc-macro"
            }
            .into(),
            fresh: r.fresh,
        });
    }
    let encoded = |u: &Unit| {
        let mut u = u.clone();
        u.features.sort();
        u.filenames.sort();
        serde_json::to_string(&u)
    };
    let supplied = c
        .units
        .iter()
        .map(encoded)
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    let actual = normalized
        .iter()
        .map(encoded)
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    ensure!(
        supplied.len() == c.units.len() && actual.len() == normalized.len() && supplied == actual,
        "normalized actual compiler units, no duplicate or invented profile"
    );
    for name in REQUIRED {
        ensure!(
            c.units
                .iter()
                .any(|u| u.class == "runtime-target" && u.package == name),
            "required runtime dependency profile evidence"
        );
    }
    Ok(())
}
fn artifacts(p: &Policy, i: &Inventory, c: &CompilerCapture, normal: bool) -> Result<()> {
    let roles = if normal {
        vec![IMAGES[1]]
    } else {
        IMAGES[3..].to_vec()
    };
    ensure!(
        c.stable_artifacts.len() == roles.len(),
        "exact compiler image set"
    );
    let mut seen = BTreeSet::new();
    for a in &c.stable_artifacts {
        ensure!(
            seen.insert(files::key(&a.path)?),
            "duplicate compiler image path"
        );
        let role = roles
            .iter()
            .find(|r| a.package == r.2 && a.kind == r.3 && a.target == r.4)
            .context("compiler image role association")?;
        let pin = image(i, role.0)?;
        ensure!(
            a.path == pin.path
                && a.bytes == pin.bytes
                && a.sha256 == pin.sha256
                && a.profile == PROFILE,
            "same fixed image pin"
        );
        files::same(&a.path, &p.root.join(role.1))?;
        let found = c
            .units
            .iter()
            .filter(|u| {
                u.package == a.package
                    && u.kind == [a.kind.clone()]
                    && u.target == a.target
                    && u.profile.test != normal
                    && u.class == "runtime-target"
                    && u.executable.is_some()
            })
            .collect::<Vec<_>>();
        ensure!(
            found.len() == 1,
            "one actual compiler executable unit for fixed image"
        );
        let exe = found[0]
            .executable
            .as_ref()
            .context("actual compiler executable")?;
        ensure!(
            files::path(exe, false)?.starts_with(files::path(
                &p.root.join("target").join(TARGET).join(PROFILE),
                false
            )?),
            "compiler executable target/profile path"
        );
        let raw = files::read(exe, 128 * MIB)?;
        ensure!(
            raw.len() as u64 == pin.bytes && files::sha(&raw) == pin.sha256,
            "compiler output bytes match fixed runtime image"
        );
    }
    Ok(())
}
pub(super) fn validate(p: &Policy, i: &Inventory) -> Result<()> {
    let normal: CompilerCapture = files::document(&i.compiler.normal, 8 * MIB)?;
    let tests: CompilerCapture = files::document(&i.compiler.tests, 8 * MIB)?;
    let expected_build = vec![
        "build",
        "--package",
        "kilogram-cli",
        "--bin",
        "kilogram-cli",
        "--profile",
        PROFILE,
        "--target",
        TARGET,
        "--jobs",
        "2",
        "--offline",
        "--locked",
        "--message-format=json-render-diagnostics",
    ];
    let expected_tests = vec![
        "test",
        "--package",
        "kilogram-cli",
        "--package",
        "kilogram-transport-iroh",
        "--profile",
        PROFILE,
        "--target",
        TARGET,
        "--jobs",
        "2",
        "--offline",
        "--locked",
        "--no-run",
        "--message-format=json-render-diagnostics",
    ];
    ensure!(
        i.compiler.normal_argv == expected_build && i.compiler.tests_argv == expected_tests,
        "exact literal compiler argv, no runtime preparation"
    );
    for (c, is_normal) in [(&normal, true), (&tests, false)] {
        ensure!(
            c.classification
                == if is_normal {
                    "nonexecuting-normal-cli-compiler-provenance"
                } else {
                    "nonexecuting-compiler-artifact-provenance"
                }
                && c.profile == PROFILE
                && c.target == TARGET
                && c.build_inputs_before == c.build_inputs_after,
            "compiler capture boundary"
        );
        if let Some(hash) = &c.expected_manifest_sha256 {
            files::digest(hash, 64)?;
            ensure!(
                !is_normal && *hash == i.compiler.frozen_manifest.sha256 && c.frozen_guard_verified,
                "frozen expected-manifest identity"
            );
        } else {
            ensure!(!c.frozen_guard_verified, "no invented frozen guard");
        }
        inputs(p, i, &c.build_inputs_before)?;
        units(p, i, c)?;
        artifacts(p, i, c, is_normal)?;
    }
    embedded(p, i, &tests)?;
    ensure!(
        normal.build_inputs_before == tests.build_inputs_before,
        "same shared source/compiler closure for normal CLI and tests"
    );
    let frozen: FrozenManifest = files::document(&i.compiler.frozen_manifest, 4 * MIB)?;
    ensure!(
        frozen.format_version == 1
            && frozen.classification == "frozen-scale-test-artifacts"
            && frozen.profile == PROFILE
            && frozen.target == TARGET
            && frozen.build_inputs == tests.build_inputs_before
            && frozen.artifacts == tests.stable_artifacts,
        "existing frozen artifact manifest"
    );
    files::same(
        &frozen.stable_directory,
        &p.root.join("target/verification-scale/deps"),
    )?;
    let identities = tests
        .units
        .iter()
        .map(|u| -> Result<Value> {
            let mut v = serde_json::to_value(u)?;
            v.as_object_mut().context("unit object")?.remove("fresh");
            Ok(v)
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        frozen.units_identity == identities,
        "existing normalized unit identities without freshness labels"
    );
    Ok(())
}
fn embedded(p: &Policy, i: &Inventory, tests: &CompilerCapture) -> Result<()> {
    let e = &i.compiler.embedded;
    ensure!(
        e.before.len() == files::EMBEDDED.len() && e.after.len() == e.before.len(),
        "exact embedded before/after input set"
    );
    for rows in [&e.before, &e.after] {
        let mut seen = BTreeSet::new();
        for pin in rows {
            let expected = files::EMBEDDED
                .iter()
                .find(|s| files::same(&p.root.join(s), &pin.path).is_ok())
                .context("embedded input role")?;
            ensure!(seen.insert(*expected), "duplicate embedded input");
            let source = i
                .source
                .files
                .iter()
                .find(|r| files::same(&r.path, &pin.path).is_ok())
                .context("embedded admitted source")?;
            ensure!(
                source.bytes == pin.bytes && source.sha256 == pin.sha256,
                "embedded before/after/current source identity"
            );
            files::pin(pin, 8 * MIB)?;
        }
    }
    let mut expected = BTreeSet::new();
    // The affected compiler selection is the six-test-image build. The normal
    // CLI is not built here; its original provenance stays a separate role.
    for role in &IMAGES[3..] {
        let u = tests
            .units
            .iter()
            .find(|u| {
                u.package == role.2
                    && u.kind == [role.3]
                    && u.target == role.4
                    && u.profile.test
                    && u.executable.is_some()
            })
            .context("selected compiler dep-info role")?;
        expected.insert(files::key(
            &u.executable
                .as_ref()
                .context("dep-info exe")?
                .with_extension("d"),
        )?);
    }
    ensure!(
        e.dep_info.len() == expected.len(),
        "selected compiler dep-info set"
    );
    let mut seen = BTreeSet::new();
    let mut embedded = BTreeSet::new();
    for pin in &e.dep_info {
        let key = files::key(&pin.path)?;
        ensure!(
            expected.contains(&key) && seen.insert(key),
            "exact compiler dep-info role"
        );
        for dep in files::dep_sources(&p.root, pin)? {
            ensure!(
                i.source
                    .files
                    .iter()
                    .any(|r| files::same(&r.path, &dep).is_ok()),
                "unadmitted compiler dependency input"
            );
            for name in files::EMBEDDED {
                if files::same(&p.root.join(name), &dep).is_ok() {
                    embedded.insert(name);
                }
            }
        }
    }
    ensure!(
        embedded == BTreeSet::from(files::EMBEDDED),
        "embedded dep-info completeness"
    );
    Ok(())
}
