//! Empty directory fixtures only. Never opens a profile, DB, key or runtime.
use super::*;

fn fixture() -> Result<tempfile::TempDir> {
    let base = workspace().join(".tmp/s54-block4z-f07-queue-witness-01/scope-fixtures");
    std::fs::create_dir_all(&base)?;
    Ok(tempfile::Builder::new().prefix("empty-").tempdir_in(base)?)
}
fn state_at(workspace: &Path, purpose: Purpose) -> Result<PathBuf> {
    let state = purpose
        .run(workspace)
        .join(purpose.origin_name())
        .join("state0");
    std::fs::create_dir_all(&state)?;
    Ok(state)
}

#[test]
fn s54_source_owned_scope_accepts_exact_p_and_f_origins() -> Result<()> {
    let fixture = fixture()?;
    for purpose in [Purpose::WholeP, Purpose::FeasibilityF] {
        let state = state_at(fixture.path(), purpose)?;
        ensure!(
            owned_state(fixture.path(), &state, &purpose.attempt())? == purpose,
            "actual owner guard returns the exact closed admitted purpose"
        );
        println!("s54_source_scope_accepted={}", purpose.attempt());
    }
    Ok(())
}

#[test]
fn s54_source_owned_scope_refuses_cross_attempt_namespace_origin_and_escape() -> Result<()> {
    let fixture = fixture()?;
    let root = fixture.path();
    let p = state_at(root, Purpose::WholeP)?;
    let f = state_at(root, Purpose::FeasibilityF)?;
    for (state, attempt) in [
        (&p, Purpose::FeasibilityF.attempt()),
        (&f, Purpose::WholeP.attempt()),
    ] {
        ensure!(
            owned_state(root, state, &attempt).is_err(),
            "cross-purpose state rejected"
        );
    }
    let mut checked = 0;
    for suffix in ["01", "02", "03", "04", "05", "06"] {
        let old = format!("F-s54-mixed-hooks-feasibility-01-attempt-{suffix}");
        ensure!(
            owned_state(root, &f, &old).is_err(),
            "consumed attempt cannot select a source root"
        );
        println!("s54_source_scope_refused=F{suffix}");
        checked += 1;
    }
    ensure!(checked == 6, "all six consumed source attempts checked");
    println!("s54_source_scope_consumed_executed={checked}");
    let purpose = Purpose::FeasibilityF;
    for wrong in [
        purpose.run(root),
        purpose.run(root).join(purpose.origin_name()),
        purpose.run(root).join("foreign-origin").join("state0"),
        root.join(".tmp/migration-s54-feasibility/other-attempt/f-origin/state0"),
        root.join(".tmp/migration-s54-feasibility-public-evidence")
            .join(purpose.attempt())
            .join("f-origin/state0"),
        root.join("outside/state0"),
    ] {
        std::fs::create_dir_all(&wrong)?;
        ensure!(
            owned_state(root, &wrong, &purpose.attempt()).is_err(),
            "only exact admitted origin descendants"
        );
    }
    let escape = purpose
        .run(root)
        .join(purpose.origin_name())
        .join("state0/../../foreign-origin/state0");
    ensure!(
        owned_state(root, &escape, &purpose.attempt()).is_err(),
        "canonical parent escape refused"
    );
    ensure!(
        owned_state(root, &f, "invented-attempt").is_err(),
        "closed admitted attempt only"
    );
    ensure!(
        owned_state(root, &f.join("missing"), &purpose.attempt()).is_err(),
        "missing source state refused"
    );
    Ok(())
}

#[test]
fn s54_source_owned_requires_active_capture_before_path_access() -> Result<()> {
    let missing = workspace().join(".tmp/s54-block4z-f07-queue-witness-01/no-source-created");
    ensure!(!missing.exists(), "uncreated source fixture");
    let node = Node {
        branch: Branch {
            base: missing.clone(),
            state: missing.clone(),
            account: String::new(),
            device: String::new(),
            config: missing.clone(),
            context: String::new(),
            claim: String::new(),
        },
        profile: missing.clone(),
        ipc: missing.clone(),
        locators: missing,
        indexed: false,
        slot: 0,
    };
    let refusal = owned(&node)
        .err()
        .context("unadmitted source must be refused")?;
    ensure!(
        refusal.to_string() == "capture scope failed or absent",
        "admission checked before missing path access"
    );
    ensure!(
        Legacy::capture(&node, &[String::new(), String::new()])
            .err()
            .context("capture must refuse before opening missing storage")?
            .to_string()
            == "capture scope failed or absent",
        "actual Legacy capture checks healthy admission before source/key access"
    );
    Ok(())
}
#[test]
fn s54_source_owned_stale_or_unknown_attempt_refuses_before_missing_path_access() -> Result<()> {
    let missing = workspace().join(".tmp/s54-block4z-f07-queue-witness-01/no-stale-source");
    ensure!(!missing.exists(), "uncreated stale source fixture");
    for attempt in (1..=6)
        .map(|n| format!("F-s54-mixed-hooks-feasibility-01-attempt-{n:02}"))
        .chain(["invented-attempt".into()])
    {
        let refusal = owned_state(&missing, &missing, &attempt)
            .err()
            .context("stale or unknown attempt must refuse")?;
        ensure!(
            refusal.to_string() == "source requires current admitted S54 purpose",
            "closed admission identity checked before even workspace canonicalization"
        );
    }
    println!("s54_source_queue_stale_admissions_refused=7;path_access=false");
    Ok(())
}
