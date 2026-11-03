//! Public compiler-only replay. Never loads runtime admission or a capability.
use super::*;

const UNIT_CHECK: &str = "normalized actual compiler units, no duplicate or invented profile";

fn fixture_inventory(f: &Fixture) -> Result<Inventory> {
    files::json(&serde_json::to_vec(&f.inventory)?, 2 * MIB)
}
fn public_f04() -> Result<(Policy, Inventory)> {
    let pin = Pin {
        path: workspace().join(".tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-04/inputs.json"),
        bytes: 116874,
        sha256: "b548cf47be85fdae660bf7ed183438b5810f376bc2c3275b78f38e7ac824b85d".into(),
    };
    let i: Inventory = files::document(&pin, 2 * MIB)?;
    ensure!(
        i.identity.attempt == "F-s54-mixed-hooks-feasibility-01-attempt-04"
            && i.source.implementation == "9013f9cfb2e1803d279eaa0377b2865c835618ec"
            && i.identity.run_id
                == "27f67783ab7bbcfd5b306178bc010a11317b213a8aba317d7230915ccccb95d2",
        "exact public compiler replay binding"
    );
    Ok((Policy::production()?, i))
}

#[test]
fn s54_compiler_public_f04_replay_without_runtime_admission() -> Result<()> {
    // The ordinary test always exercises compiler validation on a fresh tiny
    // fixture. Explicit phase opt-in additionally reads the pinned public F04
    // artifacts; its actual result is printed separately from that fixture.
    let f = Fixture::new()?;
    compiler::validate(&f.policy, &fixture_inventory(&f)?)?;
    let Some(phase) = std::env::var_os("KILOGRAM_S54_F04_COMPILER_PROBE_PHASE") else {
        return Ok(());
    };
    ensure!(
        phase == "before" || phase == "after",
        "closed public probe phase"
    );
    let (p, i) = public_f04()?;
    let counts = [&i.compiler.normal, &i.compiler.tests]
        .into_iter()
        .map(|pin| -> Result<usize> {
            let c: CompilerCapture = files::document(pin, 8 * MIB)?;
            Ok(c.units
                .iter()
                .filter(|u| {
                    let mut names = u.filenames.clone();
                    names.sort();
                    names != u.filenames
                })
                .count())
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        counts == [44, 45],
        "unchanged actual public filename representation counts"
    );
    println!(
        "S54_PUBLIC_COMPILER_ORDER normal_nonordinal={};tests_nonordinal={}",
        counts[0], counts[1]
    );
    let result = compiler::validate(&p, &i);
    if phase == "before" {
        let error = result
            .err()
            .context("expected original public compiler refusal")?;
        ensure!(
            error.chain().any(|cause| cause.to_string() == UNIT_CHECK),
            "original compiler replay failed outside the expected fixed check"
        );
        println!("S54_PUBLIC_COMPILER_CHECK_LABEL {UNIT_CHECK}");
        println!(
            "S54_PUBLIC_COMPILER_REPLAY phase=before;result=refused;check=normalized_actual_compiler_units;runtime_capability=false"
        );
    } else {
        // Render only a finite known label if a later check is exposed; never
        // print arbitrary error strings or payloads from the actual artifacts.
        if let Err(error) = result {
            let check = [
                (UNIT_CHECK, "normalized_actual_compiler_units"),
                (
                    "existing normalized unit identities without freshness labels",
                    "frozen_unit_identities",
                ),
                ("existing frozen artifact manifest", "frozen_manifest"),
                ("compiler capture boundary", "capture_boundary"),
                (
                    "same shared source/compiler closure for normal CLI and tests",
                    "shared_build_inputs",
                ),
            ]
            .into_iter()
            .find(|(label, _)| error.chain().any(|cause| cause.to_string() == *label))
            .map_or("unknown", |(_, code)| code);
            println!(
                "S54_PUBLIC_COMPILER_REPLAY phase=after;result=refused;check={check};runtime_capability=false"
            );
            anyhow::bail!("repaired public compiler replay refused; static check code printed");
        }
        println!("S54_PUBLIC_COMPILER_REPLAY phase=after;result=passed;runtime_capability=false");
    }
    Ok(())
}

fn real_shaped_normal() -> Result<Fixture> {
    let mut f = Fixture::new()?;
    let dir = f.root.join("target").join(TARGET).join(PROFILE);
    let exe = dir.join("kilogram-cli.exe");
    let pdb = dir.join("kilogram_cli.pdb");
    std::fs::write(&exe, std::fs::read(f.root.join(IMAGES[1].1))?)?;
    std::fs::write(&pdb, b"public synthetic PDB output")?;
    f.try_edit_capture(true, |capture| {
        let mut raw: Value = files::json(
            capture["raw_compiler_artifact_json"][0]
                .as_str()
                .context("public raw unit")?
                .as_bytes(),
            128 * 1024,
        )?;
        // The actual PowerShell-produced representation has PDB before EXE.
        capture["units"][0]["filenames"] = json!([pdb, exe]);
        capture["units"][0]["executable"] = json!(exe);
        raw["filenames"] = capture["units"][0]["filenames"].clone();
        raw["executable"] = capture["units"][0]["executable"].clone();
        capture["raw_compiler_artifact_json"][0] = json!(serde_json::to_string(&raw)?);
        Ok(())
    })?;
    Ok(f)
}

#[test]
fn s54_compiler_exe_pdb_permutations_preserve_exact_collection() -> Result<()> {
    let mut f = real_shaped_normal()?;
    compiler::validate(&f.policy, &fixture_inventory(&f)?)?;
    if let Some(path) = std::env::var_os("KILOGRAM_S54_COMPILER_PARITY_FIXTURE_EXPORT") {
        use std::io::Write;
        let export = real_shaped_normal()?;
        compiler::validate(&export.policy, &fixture_inventory(&export)?)?;
        let path = PathBuf::from(path);
        let owned = workspace()
            .join(".tmp/s54-block4n-compiler-parity-01")
            .canonicalize()?;
        ensure!(
            files::path(path.parent().context("public parity export parent")?, false)?
                .starts_with(owned),
            "owned fresh public parity export only"
        );
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&serde_json::to_vec(&json!({"classification":"public_schema_fixture_only","root":export.root,
            "inputs":{"inventory":export.input.inventory,"activation":export.input.activation,"launch":export.input.launch,"capture":export.input.capture}}))?)?;
        file.sync_all()?;
    }
    f.try_edit_capture(true, |c| {
        c["units"][0]["filenames"]
            .as_array_mut()
            .context("supplied filenames")?
            .reverse();
        Ok(())
    })?;
    compiler::validate(&f.policy, &fixture_inventory(&f)?)?;
    f.try_edit_capture(true, |c| {
        let mut raw: Value = files::json(
            c["raw_compiler_artifact_json"][0]
                .as_str()
                .context("raw")?
                .as_bytes(),
            128 * 1024,
        )?;
        raw["filenames"]
            .as_array_mut()
            .context("raw names")?
            .reverse();
        c["raw_compiler_artifact_json"][0] = json!(serde_json::to_string(&raw)?);
        Ok(())
    })?;
    compiler::validate(&f.policy, &fixture_inventory(&f)?)?;
    println!("S54_COMPILER_FILENAME_PERMUTATIONS accepted=3;membership_unchanged=true");
    Ok(())
}

#[test]
fn s54_compiler_filename_membership_profile_and_provenance_refusals() -> Result<()> {
    let mut checked = 0;
    for mutation in 0..11 {
        let mut f = real_shaped_normal()?;
        f.try_edit_capture(true, |c| {
            match mutation {
                0 => {
                    c["units"][0]["filenames"]
                        .as_array_mut()
                        .context("names")?
                        .pop();
                }
                1 => {
                    let extra = f_placeholder_path(c)?;
                    c["units"][0]["filenames"]
                        .as_array_mut()
                        .context("names")?
                        .push(json!(extra));
                }
                2 => {
                    let duplicate = c["units"][0]["filenames"][0].clone();
                    c["units"][0]["filenames"]
                        .as_array_mut()
                        .context("names")?
                        .push(duplicate);
                }
                3 => {
                    c["units"][0]["filenames"][0] = json!(f_placeholder_path(c)?);
                }
                4 => {
                    c["units"][0]["profile"]["opt_level"] = json!("2");
                }
                5 => {
                    c["frozen_guard_verified"] = json!(true);
                }
                6 => {
                    c["units"][0]["fresh"] = json!(false);
                }
                7 => {
                    c["units"][0]["features"] = json!(["unproved-feature"]);
                }
                8 => {
                    c["units"][0]["executable"] = json!(f_placeholder_path(c)?);
                }
                9 => {
                    let mut raw: Value = files::json(
                        c["raw_compiler_artifact_json"][0]
                            .as_str()
                            .context("raw")?
                            .as_bytes(),
                        128 * 1024,
                    )?;
                    let duplicate = raw["filenames"][0].clone();
                    raw["filenames"]
                        .as_array_mut()
                        .context("raw names")?
                        .push(duplicate);
                    c["raw_compiler_artifact_json"][0] = json!(serde_json::to_string(&raw)?);
                }
                _ => {
                    let duplicate = c["units"][0].clone();
                    c["units"].as_array_mut().context("units")?.push(duplicate);
                }
            }
            Ok(())
        })?;
        ensure!(
            compiler::validate(&f.policy, &fixture_inventory(&f)?).is_err(),
            "exact collection/profile/provenance refusal {mutation}"
        );
        checked += 1;
    }
    ensure!(
        checked == 11,
        "all finite filename/identity refusals executed"
    );
    println!("S54_COMPILER_FILENAME_REFUSALS executed={checked};no_deduplication=true");
    Ok(())
}
fn f_placeholder_path(c: &Value) -> Result<PathBuf> {
    let exe = c["units"][0]["executable"].as_str().context("public exe")?;
    Ok(Path::new(exe).with_file_name("changed-output.pdb"))
}
