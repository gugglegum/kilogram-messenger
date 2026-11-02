//! Actual admission parser/IO/policy path over explicitly public fixture files.
use super::*;
#[path = "migration_s54_admission_fixture.rs"]
mod fixture;
use fixture::Fixture;
#[test]
fn s54_admission_valid_public_binding_has_real_blake3_but_no_runtime_capability() -> Result<()> {
    let f = Fixture::new()?;
    let facts = f.inspect()?;
    let raw = std::fs::read(&f.input.inventory.path)?;
    ensure!(
        facts.facts()["classification"] == "public_schema_fixture_only"
            && facts.facts()["whole_attempt_verdict"] == "not_established",
        "public fixture scope only"
    );
    ensure!(
        facts.facts()["inventory_sha256"] == files::sha(&raw)
            && facts.facts()["inventory_blake3"] == blake3::hash(&raw).to_hex().to_string(),
        "real SHA256/BLAKE3 of same bytes"
    );
    ensure!(
        runtime::require_production(&f.policy).is_err(),
        "public fixture cannot create runtime capability"
    );
    if let Some(path) = std::env::var_os("KILOGRAM_S54_ADMISSION_FIXTURE_EXPORT") {
        let path = PathBuf::from(path);
        let owned = workspace().join(".tmp/s54-block4b-r1-01").canonicalize()?;
        ensure!(
            files::path(path.parent().context("export parent")?, false)?.starts_with(owned),
            "owned public export path"
        );
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&serde_json::to_vec(&json!({"classification":"public_schema_fixture_only","root":f.root,
            "inputs":{"inventory":f.input.inventory,"activation":f.input.activation,"launch":f.input.launch,"capture":f.input.capture}}))?)?;
        file.sync_all()?;
    }
    Ok(())
}
#[test]
fn s54_admission_strict_json_recurses_through_nested_maps_and_caps() -> Result<()> {
    for bytes in [
        b"{\"a\":1,\"a\":2}".as_slice(),
        b"{\"outer\":[{\"nested\":{\"x\":1,\"x\":2}}]}",
        b"\xef\xbb\xbf{}",
        b"\xff",
        b"{} trailing",
    ] {
        ensure!(
            files::json::<Value>(bytes, 1024).is_err(),
            "strict nested JSON/UTF8/BOM"
        );
    }
    ensure!(
        files::json::<Value>(&vec![b' '; 1025], 1024).is_err(),
        "sentinel cap"
    );
    let f = Fixture::new()?;
    let mut doc = f.inventory.clone();
    doc["extra"] = json!(true);
    ensure!(
        files::json::<Inventory>(&serde_json::to_vec(&doc)?, 2 * MIB).is_err(),
        "unknown required-schema field"
    );
    doc = f.inventory.clone();
    doc["runtime"]["cargo_jobs"] = json!(true);
    ensure!(
        files::json::<Inventory>(&serde_json::to_vec(&doc)?, 2 * MIB).is_err(),
        "boolean not integer"
    );
    doc = f.inventory.clone();
    doc["runtime"]["configs"][0]
        .as_object_mut()
        .context("config")?
        .remove("sha256");
    ensure!(
        files::json::<Inventory>(&serde_json::to_vec(&doc)?, 2 * MIB).is_err(),
        "nullable does not mean missing"
    );
    Ok(())
}
#[test]
fn s54_admission_rejects_missing_changed_oversized_and_duplicate_referenced_files() -> Result<()> {
    let mut f = Fixture::new()?;
    std::fs::remove_file(&f.input.inventory.path)?;
    ensure!(f.inspect().is_err(), "missing I");
    f.seal()?;
    std::fs::write(&f.input.inventory.path, vec![b' '; 2 * MIB + 1])?;
    ensure!(f.inspect().is_err(), "oversized I");
    f.seal()?;
    std::fs::write(&f.input.capture.path, b"{}")?;
    ensure!(f.inspect().is_err(), "changed C raw pin");
    f.seal()?;
    let mut doc: Value = files::json(&files::pin(&f.input.inventory, 2 * MIB)?, 2 * MIB)?;
    let path = PathBuf::from(
        doc["compiler"]["tests"]["path"]
            .as_str()
            .context("capture path")?,
    );
    let old = String::from_utf8(std::fs::read(&path)?)?;
    let changed = old.replacen(
        "\"opt_level\":\"3\"",
        "\"nested_unknown\":{\"d\":1,\"d\":2},\"opt_level\":\"3\"",
        1,
    );
    ensure!(changed != old, "mutated nested object");
    std::fs::write(&path, changed)?;
    doc["compiler"]["tests"] = json!(fixture::pin(&path)?);
    f.inventory = doc;
    f.seal()?;
    ensure!(
        f.inspect().is_err(),
        "recursive duplicate in referenced compiler JSON, with restamped outer pins"
    );
    let mut f = Fixture::new()?;
    f.edit_capture(false, |v| {
        let line = v["raw_compiler_artifact_json"][0].as_str().unwrap_or("");
        v["raw_compiler_artifact_json"][0] =
            json!(line.replacen("\"fresh\":true", "\"fresh\":false,\"fresh\":true", 1));
    })?;
    ensure!(
        f.inspect().is_err(),
        "duplicate inside embedded Cargo JSON whose last value would otherwise match"
    );
    let mut f = Fixture::new()?;
    let path = PathBuf::from(
        f.inventory["compiler"]["frozen_manifest"]["path"]
            .as_str()
            .context("frozen path")?,
    );
    let raw = String::from_utf8(std::fs::read(&path)?)?;
    let start = raw
        .find("\"units_identity\"")
        .context("arbitrary unit identity map")?;
    let changed = format!(
        "{}{}",
        &raw[..start],
        raw[start..].replacen(
            "\"package\":\"kilogram-cli\"",
            "\"package\":\"foreign\",\"package\":\"kilogram-cli\"",
            1
        )
    );
    ensure!(changed != raw, "mutated referenced Value map duplicate");
    std::fs::write(&path, changed)?;
    f.inventory["compiler"]["frozen_manifest"] = json!(fixture::pin(&path)?);
    f.seal()?;
    ensure!(
        f.inspect().is_err(),
        "duplicate in referenced arbitrary map, not just typed top-level fields"
    );
    Ok(())
}
#[test]
fn s54_admission_rejects_case_attempt_run_selection_and_dirty_reuse() -> Result<()> {
    for (field, value) in [
        ("case", json!("C-s53-controls-b-only-01")),
        ("attempt", json!("P-s54-whole-resumed-l1-01-attempt-02")),
        ("selector", json!("not-the-whole-selector")),
        ("run_id", json!("0".repeat(64))),
        ("run", json!("relative/path")),
        ("output", json!("E:/outside/assertions.json")),
    ] {
        let mut f = Fixture::new()?;
        f.inventory["identity"][field] = value;
        f.seal()?;
        ensure!(
            f.inspect().is_err(),
            "required identity substitution {field}"
        );
    }
    let f = Fixture::new()?;
    let run = f.input.inventory.path.parent().context("run")?;
    for name in ["p5-origin", "started.json", "assertions.json"] {
        std::fs::write(run.join(name), b"public reused marker")?;
        ensure!(f.inspect().is_err(), "reused/dirty attempt {name}");
        std::fs::remove_file(run.join(name))?;
    }
    Ok(())
}
#[test]
fn s54_admission_rejects_source_set_config_environment_and_tool_drift() -> Result<()> {
    let f = Fixture::new()?;
    std::fs::write(f.root.join("apps/new-source.rs"), b"new source")?;
    ensure!(
        f.inspect().is_err(),
        "added relevant source not in compiler closure"
    );
    let f = Fixture::new()?;
    std::fs::write(f.root.join("Cargo.lock"), b"changed source")?;
    ensure!(f.inspect().is_err(), "changed existing source");
    let f = Fixture::new()?;
    std::fs::write(&f.policy.configs[0], b"[profile.dev]\n")?;
    ensure!(f.inspect().is_err(), "new/unsupported Cargo config");
    let mut f = Fixture::new()?;
    f.inventory["runtime"]["environment"][0]["present"] = json!(true);
    f.seal()?;
    ensure!(f.inspect().is_err(), "actual environment mismatch");
    let f = Fixture::new()?;
    std::fs::write(f.root.join("tools/cargo.exe"), b"changed tool")?;
    ensure!(f.inspect().is_err(), "tool/shim bytes drift");
    let f = Fixture::new()?;
    std::fs::create_dir_all(f.root.join("ai-docs/history"))?;
    std::fs::write(
        f.root.join("ai-docs/history/archived.rs"),
        b"not runtime input",
    )?;
    f.inspect()?;
    Ok(())
}
#[test]
fn s54_admission_rejects_image_roles_path_aliases_escape_and_shell_shadow() -> Result<()> {
    let mut f = Fixture::new()?;
    let duplicate = f.inventory["images"][0].clone();
    f.inventory["images"][1] = duplicate;
    f.seal()?;
    ensure!(f.inspect().is_err(), "duplicate/missing image role");
    let mut f = Fixture::new()?;
    f.inventory["images"][1]["pin"]["path"] = f.inventory["images"][0]["pin"]["path"].clone();
    f.seal()?;
    ensure!(f.inspect().is_err(), "foreign role alias path");
    let mut f = Fixture::new()?;
    f.inventory["source"]["files"][0]["path"] = json!(f.root.join("../escaped.rs"));
    f.seal()?;
    ensure!(f.inspect().is_err(), "parent escape");
    let f = Fixture::new()?;
    std::fs::write(f.root.join(IMAGES[5].1), b"changed image")?;
    ensure!(f.inspect().is_err(), "harness byte drift");
    let f = Fixture::new()?;
    std::fs::write(f.root.join("powershell.exe"), b"shadow")?;
    ensure!(f.inspect().is_err(), "runtime shell shadow");
    let mut f = Fixture::new()?;
    f.inventory["runtime"]["resolutions"][0]["path"] = json!(f.root.join("tools/git.exe"));
    f.seal()?;
    ensure!(f.inspect().is_err(), "resolver identity");
    Ok(())
}
#[test]
fn s54_admission_rejects_compiler_profile_graph_role_and_raw_unit_substitution() -> Result<()> {
    for mutation in 0..5 {
        let mut f = Fixture::new()?;
        f.edit_capture(false, |v| match mutation {
            0 => v["units"][0]["profile"]["overflow_checks"] = json!(false),
            1 => {
                v["build_inputs_before"]["resolved_graph"] = json!([]);
                v["build_inputs_after"]["resolved_graph"] = json!([]);
            }
            2 => v["stable_artifacts"] = json!([]),
            3 => {
                v["units"].as_array_mut().map(|a| a.pop());
                v["raw_compiler_artifact_json"]
                    .as_array_mut()
                    .map(|a| a.pop());
            }
            _ => v["build_inputs_before"]["tools"][0]["sha256"] = json!("0".repeat(64)),
        })?;
        ensure!(
            f.inspect().is_err(),
            "production compiler evidence rejection {mutation}"
        );
    }
    let mut f = Fixture::new()?;
    f.inventory["compiler"]["normal_argv"] = json!(["test", "--no-run"]);
    f.seal()?;
    ensure!(
        f.inspect().is_err(),
        "normal non-test CLI identity cannot be test image"
    );
    Ok(())
}
#[test]
fn s54_admission_rejects_activation_launch_capture_stale_and_clock_reset() -> Result<()> {
    for mutation in 0..8 {
        let mut f = Fixture::new()?;
        match mutation {
            0 => f.activation["selector"] = json!("wrong"),
            1 => f.activation["not_after_utc_ms"] = json!(1),
            2 => f.launch["controller_tick_ms"] = json!(2355001),
            3 => f.capture["whole_utc_ms"] = json!(999),
            4 => f.capture["attempt"] = json!("foreign"),
            5 => f.launch["owner_pid"] = json!(101),
            6 => f.activation["retry"] = json!("automatic"),
            _ => f.launch["start_tick_ms"] = json!(u64::MAX),
        }
        f.seal()?;
        ensure!(f.inspect().is_err(), "exact I/A/L/C relation {mutation}");
    }
    Ok(())
}
#[test]
fn s54_admission_rejects_actual_public_junction_ancestor() -> Result<()> {
    let path = workspace().join(".tmp/s54-block4a-01/reparse-public-fixture/alias");
    ensure!(
        path.exists(),
        "owned public junction fixture is required, not silently skipped"
    );
    ensure!(
        files::path(&path, false).is_err(),
        "actual reparse ancestry refuses public pinning"
    );
    Ok(())
}
#[test]
fn s54_admission_compiler_boundary_parity_mutations_reach_the_required_refusal() -> Result<()> {
    // Matching production-path mutation names are exercised in PowerShell.
    for (name, expected) in [
        ("duplicate_artifacts", "duplicate compiler image path"),
        ("invented_guard", "no invented frozen guard"),
        ("wrong_expected_hash", "frozen expected-manifest identity"),
        ("missing_guard", "frozen expected-manifest identity"),
        ("normal_guard", "frozen expected-manifest identity"),
        ("runtime_debuginfo", "runtime opt3/debug/overflow profile"),
        (
            "runtime_debuginfo_array",
            "runtime opt3/debug/overflow profile",
        ),
        (
            "oversized_compiler_string",
            "bounded unique compiler string",
        ),
        ("duplicate_package_id", "unique package ID/name map"),
        ("invalid_dep_kind", "bounded dependency kinds"),
        ("duplicate_dep_kind", "bounded dependency kinds"),
        (
            "escaped_executable",
            "compiler executable target/profile path",
        ),
        ("duplicate_features", "bounded unique compiler string"),
        ("empty_target_kind", "bounded compiler target/output facts"),
        ("duplicate_output", "unique compiler output paths"),
        (
            "unadmitted_workspace_source",
            "workspace compiler target belongs to admitted live sources",
        ),
        (
            "tool_basename",
            "compiler executable role, not arbitrary data",
        ),
        ("duplicate_graph_features", "bounded unique compiler string"),
    ] {
        let mut f = Fixture::new()?;
        f.inspect()?; // Positive baseline; no expired A/stale reference shortcut.
        let hash = f.inventory["compiler"]["frozen_manifest"]["sha256"].clone();
        let root = f.root.clone();
        f.try_edit_capture(name == "normal_guard", |v| {
            match name {
                "duplicate_artifacts" => {
                    v["stable_artifacts"] = json!(vec![v["stable_artifacts"][0].clone(); 6])
                }
                "invented_guard" => v["frozen_guard_verified"] = json!(true),
                "wrong_expected_hash" => {
                    v["expected_manifest_sha256"] = json!("0".repeat(64));
                    v["frozen_guard_verified"] = json!(true);
                }
                "missing_guard" | "normal_guard" => v["expected_manifest_sha256"] = hash,
                "duplicate_package_id" => {
                    v["build_inputs_before"]["package_names"][1] =
                        v["build_inputs_before"]["package_names"][0].clone()
                }
                "invalid_dep_kind" | "duplicate_dep_kind" => {
                    let kinds = if name == "invalid_dep_kind" {
                        json!([{"kind":"foreign","target":null}])
                    } else {
                        json!([{"kind":null,"target":null},{"kind":null,"target":null}])
                    };
                    v["build_inputs_before"]["resolved_graph"][0]["deps"] =
                        json!([{"name":"self","pkg":"fixture:kilogram-cli","dep_kinds":kinds}]);
                }
                "duplicate_graph_features" => {
                    v["build_inputs_before"]["resolved_graph"][0]["features"] =
                        json!(["same", "same"])
                }
                "oversized_compiler_string" => {
                    v["build_inputs_before"]["cargo"] = json!(["\u{e9}".repeat(513)])
                }
                "tool_basename" => {
                    v["build_inputs_before"]["tools"][0]["path"] =
                        json!(root.join("tools/pwsh.exe"));
                    v["build_inputs_before"]["tools"][0]["sha256"] = json!(
                        fixture::pin(&root.join("tools/pwsh.exe"))
                            .context("public tool pin")?
                            .sha256
                    );
                }
                _ => {
                    let mut raw: Value = serde_json::from_str(
                        v["raw_compiler_artifact_json"][0]
                            .as_str()
                            .context("public line")?,
                    )
                    .context("public JSON")?;
                    match name {
                        "runtime_debuginfo" => {
                            v["units"][0]["profile"]["debuginfo"] = json!(0);
                            raw["profile"]["debuginfo"] = json!(0);
                        }
                        "runtime_debuginfo_array" => {
                            v["units"][0]["profile"]["debuginfo"] = json!(["line-tables-only"]);
                            raw["profile"]["debuginfo"] = json!(["line-tables-only"]);
                        }
                        "escaped_executable" => {
                            let exe = json!(root.join(IMAGES[3].1));
                            v["units"][0]["executable"] = exe.clone();
                            raw["executable"] = exe;
                        }
                        "duplicate_features" => {
                            v["units"][0]["features"] = json!(["same", "same"]);
                            raw["features"] = json!(["same", "same"]);
                        }
                        "empty_target_kind" => {
                            v["units"][0]["kind"] = json!([]);
                            raw["target"]["kind"] = json!([]);
                        }
                        "duplicate_output" => {
                            let a = json!([raw["filenames"][0], raw["filenames"][0]]);
                            v["units"][0]["filenames"] = a.clone();
                            raw["filenames"] = a;
                        }
                        "unadmitted_workspace_source" => {
                            raw["target"]["src_path"] = json!(root.join("tools/cargo.exe"))
                        }
                        _ => anyhow::bail!("unknown compiler parity mutation"),
                    }
                    v["raw_compiler_artifact_json"][0] = json!(serde_json::to_string(&raw)?);
                }
            }
            v["build_inputs_after"] = v["build_inputs_before"].clone();
            Ok(())
        })?;
        let error = f
            .inspect()
            .err()
            .context("required compiler refusal")?
            .to_string();
        ensure!(
            error.contains(expected),
            "{name}: intended compiler refusal {expected}, actual {error}"
        );
        println!("compiler_parity_refusal={name};reason={error}");
    }
    Ok(())
}
#[test]
fn s54_admission_embedded_inputs_and_compiler_dependencies_are_closed() -> Result<()> {
    for name in files::EMBEDDED {
        let f = Fixture::new()?;
        f.inspect()?;
        std::fs::write(f.root.join(name), b"changed public embedded input")?;
        ensure!(f.inspect().is_err(), "changed embedded source {name}");
        let f = Fixture::new()?;
        std::fs::remove_file(f.root.join(name))?;
        ensure!(f.inspect().is_err(), "missing embedded source {name}");
    }
    for (name, expected) in [
        ("missing_before", "exact embedded before/after input set"),
        (
            "changed_after",
            "embedded before/after/current source identity",
        ),
        ("duplicate_dep_info", "exact compiler dep-info role"),
        ("unknown_dependency", "unadmitted compiler dependency input"),
        ("missing_dependency", "embedded dep-info completeness"),
    ] {
        let mut f = Fixture::new()?;
        f.inspect()?;
        match name {
            "missing_before" => f.inventory["compiler"]["embedded"]["before"] = json!([]),
            "changed_after" => {
                f.inventory["compiler"]["embedded"]["after"][0]["sha256"] = json!("0".repeat(64))
            }
            "duplicate_dep_info" => {
                f.inventory["compiler"]["embedded"]["dep_info"][1] =
                    f.inventory["compiler"]["embedded"]["dep_info"][0].clone()
            }
            _ => {
                let pins = f.inventory["compiler"]["embedded"]["dep_info"]
                    .as_array_mut()
                    .context("dep pins")?;
                for pin in pins {
                    let path = PathBuf::from(pin["path"].as_str().context("dep path")?);
                    let source = if name == "unknown_dependency" {
                        let path = f.root.join("apps/public-unadmitted.json");
                        std::fs::write(&path, b"unadmitted public compile input")?;
                        path
                    } else {
                        f.root
                            .join("apps/kilogram-cli/tests/support/migration_s54_p5.rs")
                    };
                    std::fs::write(&path, format!("{}: {}\n", path.display(), source.display()))?;
                    *pin = json!(fixture::pin(&path)?);
                }
            }
        }
        f.seal()?;
        let error = f
            .inspect()
            .err()
            .context("required embedded refusal")?
            .to_string();
        ensure!(
            error.contains(expected),
            "{name}: intended embedded refusal {expected}, actual {error}"
        );
        println!("embedded_refusal={name};reason={error}");
    }
    Ok(())
}
#[test]
fn s54_owned_states_claim_once_and_revalidate_without_fresh_reuse() -> Result<()> {
    let f = Fixture::new()?;
    let i: Inventory = files::json(&serde_json::to_vec(&f.inventory)?, 2 * MIB)?;
    let run = &i.identity.run;
    let started = Started {
        version: 1,
        classification: "s54_exclusive_attempt".into(),
        case: CASE.into(),
        attempt: ATTEMPT.into(),
        run_id: i.identity.run_id.clone(),
        selector: SELECTOR.into(),
        inventory: f.input.inventory.clone(),
        activation: f.input.activation.clone(),
        owner_pid: 100,
        nonce: "c".repeat(64),
    };
    std::fs::write(run.join("started.json"), serde_json::to_vec(&started)?)?;
    ensure!(f.inspect().is_err(), "started is never fresh/reusable");
    inspect_state(&f.policy, &f.input, Directory::BeforeNative(&started))?;
    let claim = Claim {
        version: 1,
        classification: "s54_native_started".into(),
        case: CASE.into(),
        attempt: ATTEMPT.into(),
        run_id: i.identity.run_id.clone(),
        owner_pid: 100,
        launcher_pid: 101,
        harness_pid: 102,
        nonce: started.nonce.clone(),
        inventory_sha256: f.input.inventory.sha256.clone(),
        launch_sha256: f.input.launch.sha256.clone(),
    };
    let path = run.join("native-started.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    std::io::Write::write_all(&mut file, &serde_json::to_vec(&claim)?)?;
    drop(file);
    ensure!(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .is_err(),
        "exclusive native claim cannot be reused"
    );
    ensure!(
        inspect_state(&f.policy, &f.input, Directory::BeforeNative(&started)).is_err(),
        "launch state cannot restart after native claim"
    );
    std::fs::create_dir(run.join("p5-origin"))?;
    std::fs::write(
        run.join("command-0000.stdout.log"),
        b"public sanitized fixture output",
    )?;
    std::fs::write(run.join("memory-observer-final.json"), b"{}")?;
    inspect_state(&f.policy, &f.input, Directory::Owned(&started, &claim))?;
    for name in ["native-stdout.protocol.log", "native-stderr.protocol.log"] {
        let transcript = run.join(name);
        std::fs::write(&transcript, b"public protocol-only fixture\n")?;
        inspect_state(&f.policy, &f.input, Directory::Owned(&started, &claim))?;
        ensure!(
            f.inspect().is_err(),
            "transcript never makes an attempt fresh"
        );
        std::fs::OpenOptions::new()
            .write(true)
            .open(&transcript)?
            .set_len((MIB + 1) as u64)?;
        ensure!(
            inspect_state(&f.policy, &f.input, Directory::Owned(&started, &claim)).is_err(),
            "exact transcript byte cap"
        );
        std::fs::remove_file(&transcript)?;
        std::fs::create_dir(&transcript)?;
        ensure!(
            inspect_state(&f.policy, &f.input, Directory::Owned(&started, &claim)).is_err(),
            "transcript role cannot be a directory"
        );
        std::fs::remove_dir(&transcript)?;
        std::fs::write(&transcript, b"public protocol-only fixture\n")?;
    }
    let mut wrong = started.clone();
    wrong.owner_pid = 999;
    ensure!(
        inspect_state(&f.policy, &f.input, Directory::Owned(&wrong, &claim)).is_err(),
        "owner binding survives post-start output"
    );
    std::fs::write(run.join("unexpected.json"), b"{}")?;
    ensure!(
        inspect_state(&f.policy, &f.input, Directory::Owned(&started, &claim)).is_err(),
        "owned is still a closed directory state"
    );
    Ok(())
}
