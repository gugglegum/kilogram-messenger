//! A real read-only runner gate, charged only when explicitly activated.
use super::*;
#[test]
fn s53_metadata_shared_budget_and_classification_public() -> Result<()> {
    let managed = Managed::new("C-s53-metadata-r2-01", 20)?;
    let inventory =
        PathBuf::from(std::env::var_os("KILOGRAM_S53_INVENTORY").context("actual inventory")?);
    let input: Value = serde_json::from_slice(&std::fs::read(inventory)?)?;
    let dependencies = input["runtime_dependencies"]
        .as_array()
        .context("bound runtime dependencies")?;
    ensure!(
        dependencies.len() == 1 && dependencies[0]["role"] == "metadata-shell",
        "one reviewed actual shell dependency"
    );
    let dependency = dependencies[0].clone();
    let shell = PathBuf::from(
        dependency["path"]
            .as_str()
            .context("absolute reviewed shell path")?,
    );
    let inherited =
        PathBuf::from(std::env::var_os("KILOGRAM_S53_SHELL").context("explicit adapter shell")?);
    let digest = dependency["sha256"]
        .as_str()
        .context("reviewed shell SHA256")?;
    let bytes = dependency["bytes"]
        .as_u64()
        .context("reviewed shell bytes")?;
    ensure!(
        shell.is_absolute()
            && shell == inherited
            && shell.is_file()
            && std::fs::metadata(&shell)?.len() == bytes
            && digest.len() == 64
            && digest.bytes().all(|b| b.is_ascii_hexdigit()),
        "same absolute inventory/env/file identity; child verifies actual SHA256 and host before probes"
    );
    let output = s53_deadline::output(
        command(&shell)
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
            .arg(workspace().join("scripts/test-migration-s53-metadata.ps1"))
            .arg("-RunDirectory")
            .arg(&managed.run)
            .arg("-ShellExecutable")
            .arg(&shell)
            .arg("-ShellSha256")
            .arg(digest)
            .arg("-ShellBytes")
            .arg(bytes.to_string()),
    )?;
    ensure!(
        output.status.success(),
        "metadata gate: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let observations: Value = serde_json::from_slice(&std::fs::read(
        managed.run.join("metadata-observations.json"),
    )?)?;
    ensure!(
        observations["runtime_dependency"] == dependency,
        "actual script host/file identity equals original inventory"
    );
    managed.finish(observations)
}
