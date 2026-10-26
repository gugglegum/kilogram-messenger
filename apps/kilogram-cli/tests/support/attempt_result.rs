//! Test-only assertion lifetime. A retained snapshot never establishes a whole
//! attempt verdict, and the explicit caller owns all provenance decisions.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Provenance {
    BoundPreRunInventory {
        path: PathBuf,
        blake3: String,
        identity: String,
    },
    UnavailableLegacyRunner,
    #[serde(rename = "opaque_unbound_standalone_fixture")]
    OpaqueStandaloneFixture,
}

pub(super) struct Envelope<'a> {
    pub case: &'a str,
    pub run: &'a Path,
    pub output: &'a Path,
    pub provenance: &'a Provenance,
    pub observations: &'a Value,
}

pub(super) fn persist_before_tail<T>(
    path: &Path,
    envelope: &Envelope<'_>,
    tail: impl FnOnce() -> Result<T>,
) -> Result<T> {
    ensure!(
        !envelope.case.is_empty() && envelope.run.is_absolute() && envelope.output == path,
        "assertion snapshot case/run/output identity"
    );
    let snapshot = json!({
        "classification":"functional_assertions_completed",
        "whole_attempt_verdict":"not_established",
        "case":envelope.case,"run":envelope.run,"output":envelope.output,
        "provenance":envelope.provenance,"observations":envelope.observations,
    });
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context("create assertion snapshot without overwriting")?;
    serde_json::to_writer_pretty(&mut file, &snapshot)?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    tail()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, io::Read};

    const CASE: &str = "C-result-lifetime-01";
    const CASE_ENV: &str = "KILOGRAM_RB_CASE";
    const INVENTORY_ENV: &str = "KILOGRAM_RB_INVENTORY";
    const ID_ENV: &str = "KILOGRAM_RB_INVENTORY_ID";

    // Only this managed test adapter reads environment. The persistence helper
    // and the legacy controller receive explicit envelopes and never discover it.
    fn test_provenance() -> Result<Provenance> {
        let supplied = [CASE_ENV, INVENTORY_ENV, ID_ENV].map(std::env::var_os);
        if supplied.iter().all(Option::is_none) {
            return Ok(Provenance::OpaqueStandaloneFixture);
        }
        ensure!(
            supplied.iter().all(Option::is_some),
            "partial managed binding"
        );
        let case = supplied[0].as_ref().context("managed case")?;
        let path = PathBuf::from(supplied[1].as_ref().context("managed inventory")?);
        let identity = supplied[2]
            .as_ref()
            .and_then(|v| v.to_str())
            .filter(|v| !v.is_empty())
            .context("managed inventory identity")?;
        ensure!(
            case == CASE && path.is_absolute(),
            "managed case/inventory path"
        );
        ensure!(
            std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "inventory cap"
        );
        let raw = std::fs::read(&path)?;
        let inventory: Value = serde_json::from_slice(&raw)?;
        ensure!(
            inventory["classification"] == "bound_pre_run_inventory"
                && inventory["case"] == CASE
                && inventory["inventory_identity"] == identity
                && inventory["baseline"] == "9016b578bed8b8b5e43dd0e2e94edc376804e4f0"
                && inventory["binding_sha256"]
                    .as_str()
                    .is_some_and(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
                && inventory["sources"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty())
                && inventory["images"]
                    .as_array()
                    .is_some_and(|v| !v.is_empty()),
            "invalid supplied pre-run inventory"
        );
        Ok(Provenance::BoundPreRunInventory {
            path,
            blake3: blake3::hash(&raw).to_hex().to_string(),
            identity: identity.to_owned(),
        })
    }

    fn envelope<'a>(
        run: &'a Path,
        output: &'a Path,
        provenance: &'a Provenance,
        observations: &'a Value,
    ) -> Envelope<'a> {
        Envelope {
            case: match provenance {
                Provenance::BoundPreRunInventory { .. } => CASE,
                _ => "opaque-unbound-standalone-lifecycle",
            },
            run,
            output,
            provenance,
            observations,
        }
    }

    fn read_closed(path: &Path) -> Result<Vec<u8>> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // This open fails if the snapshot writer still owns its handle.
            options.share_mode(0);
        }
        let mut file = options.open(path)?;
        let mut raw = Vec::new();
        file.read_to_end(&mut raw)?;
        Ok(raw)
    }

    fn check_snapshot(raw: &[u8], envelope: &Envelope<'_>) -> Result<()> {
        let value: Value = serde_json::from_slice(raw)?;
        ensure!(
            value["classification"] == "functional_assertions_completed"
                && value["whole_attempt_verdict"] == "not_established"
                && value["case"] == envelope.case
                && value["run"] == json!(envelope.run)
                && value["output"] == json!(envelope.output)
                && value["provenance"] == serde_json::to_value(envelope.provenance)?
                && value["observations"] == *envelope.observations
                && value.get("whole_exit_code").is_none(),
            "snapshot identity/classification/provenance/observations changed"
        );
        Ok(())
    }

    #[derive(Debug)]
    struct ExpectedTailFailure;
    impl std::fmt::Display for ExpectedTailFailure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("deliberate opaque tail failure")
        }
    }
    impl std::error::Error for ExpectedTailFailure {}

    #[test]
    fn pre_s53_attempt_result_persists_before_failing_tail() -> Result<()> {
        let run = tempfile::tempdir()?;
        let output = run.path().join("assertions.json");
        let provenance = test_provenance()?;
        let observations = json!({"opaque":[0,1,127,255],"fixture":"lifecycle only"});
        let envelope = envelope(run.path(), &output, &provenance, &observations);
        let seen = std::cell::RefCell::new(Vec::new());
        let result: Result<()> = persist_before_tail(&output, &envelope, || {
            let raw = read_closed(&output)?;
            check_snapshot(&raw, &envelope)?;
            *seen.borrow_mut() = raw;
            Err(ExpectedTailFailure.into())
        });
        let error = result.err().context("failing tail was accepted")?;
        ensure!(
            error.is::<ExpectedTailFailure>(),
            "original tail error replaced"
        );
        ensure!(
            read_closed(&output)? == *seen.borrow(),
            "snapshot changed after tail failure"
        );
        Ok(())
    }

    #[test]
    fn pre_s53_attempt_result_refuses_overwrite_and_skips_tail_on_write_error() -> Result<()> {
        let run = tempfile::tempdir()?;
        let output = run.path().join("existing.json");
        let old = b"existing opaque bytes";
        std::fs::write(&output, old)?;
        let provenance = test_provenance()?;
        let observations = json!({"opaque":"no product state"});
        let called = Cell::new(0);
        let existing = envelope(run.path(), &output, &provenance, &observations);
        let collision = persist_before_tail(&output, &existing, || {
            called.set(called.get() + 1);
            Ok(())
        });
        ensure!(
            collision.is_err() && called.get() == 0,
            "collision ran the tail"
        );
        ensure!(std::fs::read(&output)? == old, "existing bytes overwritten");
        let parent = run.path().join("file-not-directory");
        std::fs::write(&parent, b"opaque parent")?;
        let invalid = parent.join("assertions.json");
        let invalid_envelope = envelope(run.path(), &invalid, &provenance, &observations);
        let failed = persist_before_tail(&invalid, &invalid_envelope, || {
            called.set(called.get() + 1);
            Ok(())
        });
        ensure!(
            failed.is_err() && called.get() == 0,
            "invalid output ran the tail"
        );
        ensure!(
            std::fs::read(&parent)? == b"opaque parent",
            "invalid parent changed"
        );
        ensure!(
            std::fs::read(&output)? == old,
            "collision bytes later changed"
        );
        Ok(())
    }

    #[test]
    fn pre_s53_attempt_result_success_keeps_separate_whole_outcome() -> Result<()> {
        let run = tempfile::tempdir()?;
        let output = run.path().join("assertions.json");
        let provenance = test_provenance()?;
        let observations = json!({"opaque":"successful tail is independently required"});
        let envelope = envelope(run.path(), &output, &provenance, &observations);
        let expected = json!({"actual_tail_result":[13,37]});
        let result = persist_before_tail(&output, &envelope, || {
            check_snapshot(&read_closed(&output)?, &envelope)?;
            Ok(expected.clone())
        })?;
        ensure!(result == expected, "actual successful tail result lost");
        check_snapshot(&read_closed(&output)?, &envelope)?;
        Ok(())
    }
}
