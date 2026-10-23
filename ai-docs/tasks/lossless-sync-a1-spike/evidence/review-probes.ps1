# Reproduce three boundaries found by review without editing the submitted spike.
# Run from any directory. Builds an offline scratch copy, no sockets or cargo test.
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$taskSpike = Join-Path $taskRoot 'ai-docs/tasks/lossless-sync-a1-spike'
$taskScratch = Join-Path $taskRoot '.tmp/a1-review-probe'
New-Item -ItemType Directory -Path (Join-Path $taskScratch 'src') -Force | Out-Null
$env:CARGO_NET_OFFLINE = 'true'
$env:TEMP = Join-Path $taskRoot '.tmp'
$env:TMP = $env:TEMP

$taskManifest = [IO.File]::ReadAllText((Join-Path $taskSpike 'Cargo.toml'))
$taskManifest = $taskManifest.Replace('../../../crates/kilogram-identity', '../../crates/kilogram-identity')
$taskManifest += "`n[[bin]]`nname = `"kilogram-a1-review-probe`"`npath = `"src/main.rs`"`n"
[IO.File]::WriteAllText((Join-Path $taskScratch 'Cargo.toml'), $taskManifest)
Copy-Item -LiteralPath (Join-Path $taskSpike 'Cargo.lock') -Destination (Join-Path $taskScratch 'Cargo.lock')

$taskF1 = @'

pub fn review_pending_invalid(directory: &Path) -> Result<()> {
    let mut pair = Pair::new("review-invalid-pending")?;
    let predecessor = pair.alice.send(&pair.alice_signer, "data", "valid predecessor")?;
    let mut invalid = pair.alice.send(&pair.alice_signer, "data", "invalid successor")?;
    *invalid.ciphertext.last_mut().ok_or_else(|| anyhow::anyhow!("ciphertext"))? ^= 1;
    invalid.resign(&pair.alice_signer)?;
    ensure!(pair.bob.accept(invalid.clone())? == Outcome::Pending);
    pair.bob.reload(&directory.join("review-invalid.json"))?;
    let before = pair.bob.canonical()?;
    let error = pair.bob.accept(predecessor.clone()).err()
        .ok_or_else(|| anyhow::anyhow!("expected fixture drain error"))?;
    ensure!(error.to_string() == "invalid pending fixture");
    ensure!(pair.bob.canonical()? != before);
    ensure!(pair.bob.state.receive.next == 2 && pair.bob.state.readable.len() == 1);
    ensure!(pair.bob.state.pending.contains_key(&invalid.meta.n));
    ensure!(pair.bob.accept(predecessor)? == Outcome::Replay);
    ensure!(pair.bob.state.pending.contains_key(&invalid.meta.n));
    println!("review.f1=invalid_future_pending_survives_reload; valid_predecessor_committed_before_error; invalid_remains_pending; predecessor_retry=Replay_no_drain");
    Ok(())
}
'@
$taskF2 = @'

pub fn review_model_boundaries() -> Result<()> {
    let mut model = Model::new();
    let original = Fake::new(3);
    ensure!(model.stage(&original)? == Transition::AcceptedPending);
    let mut different = original.clone();
    different.hash = 99;
    let result = batch(&mut model, &[different.clone()], None)?;
    ensure!(different != original && model.pending.get(&3) == Some(&original));
    ensure!(result.complete_ids == Some(vec![3]));
    println!("review.f2_identity=distinct_fake_event_same_ordinal_confirmed; original_retained; exact_EventId_semantics_not_proven");

    let mut tight = Model::new();
    tight.vault_cap = 150;
    for n in [3, 4, 5] {
        ensure!(tight.stage(&Fake::new(n))? == Transition::AcceptedPending);
    }
    ensure!(tight.admit(&Fake::new(0))? == Transition::AcceptedReadable);
    ensure!(tight.admit(&Fake::new(1))? == Transition::AcceptedReadable);
    ensure!(tight.history_cost() + tight.pending_leases() == 138);
    ensure!(tight.head_reserve() == 30);
    let before = tight.snapshot()?;
    ensure!(tight.admit(&Fake::new(2))? == Transition::StorageBlocked("FullCommitBudget"));
    ensure!(before == tight.snapshot()? && tight.pending.len() == 3);
    ensure!(6 * Fake::new(0).durable() == 144 && 144 <= tight.vault_cap);
    println!("review.f2_reserve=cap150; committed_plus_leases138; next_head_lease30; StorageBlocked; final_history144_fits; moving_head_reserve_not_invariant");
    Ok(())
}
'@
[IO.File]::WriteAllText((Join-Path $taskScratch 'src/f1.rs'), ([IO.File]::ReadAllText((Join-Path $taskSpike 'src/f1.rs')) + $taskF1))
[IO.File]::WriteAllText((Join-Path $taskScratch 'src/f2.rs'), ([IO.File]::ReadAllText((Join-Path $taskSpike 'src/f2.rs')) + $taskF2))
[IO.File]::WriteAllText((Join-Path $taskScratch 'src/main.rs'), @'
#![allow(dead_code)]
mod f1;
mod f2;
fn main() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    f1::review_pending_invalid(directory.path())?;
    f2::review_model_boundaries()?;
    println!("review_counterexamples=3_confirmed; submitted_sources_unchanged=true");
    Ok(())
}
'@)

# The existing wrapper exits with Cargo's code; invoke it in a child shell.
$taskWrapper = Join-Path $taskRoot 'scripts/invoke-cargo-friendly.ps1'
try {
    $ErrorActionPreference = 'Continue'
    $taskBuild = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $taskWrapper build -CargoJobs 2 --locked --offline --manifest-path (Join-Path $taskScratch 'Cargo.toml') --target-dir (Join-Path $taskRoot 'target') 2>&1
    $taskBuildExit = $LASTEXITCODE
} finally {
    $ErrorActionPreference = 'Stop'
}
$taskBuild | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'review-probes-build.log') -Encoding utf8
$taskBuild
if ($taskBuildExit -ne 0) { throw "Review probe build failed: $taskBuildExit" }
$taskOutput = & (Join-Path $taskRoot 'target/debug/kilogram-a1-review-probe.exe') 2>&1
$taskExit = $LASTEXITCODE
$taskOutput | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'review-probes.log') -Encoding utf8
"probe_exit=$taskExit" | Add-Content -LiteralPath (Join-Path $PSScriptRoot 'review-probes.log') -Encoding utf8
$taskOutput
if ($taskExit -ne 0) { throw "Review counterexample not reproduced: $taskExit" }
