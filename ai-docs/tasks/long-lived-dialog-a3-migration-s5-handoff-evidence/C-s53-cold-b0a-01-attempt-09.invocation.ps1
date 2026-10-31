$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 & cargo test --locked --offline --no-run --jobs 2 --profile verification --package kilogram-state --features verification-state-lock-adoption
 if($LASTEXITCODE -ne 0){throw "Compile-only exit $LASTEXITCODE"}
 foreach($filter in @('verification_lock::tests::default_stream_provenance_with_existing_ads','verification_lock::tests::concurrent_clones_have_one_adoption_attempt')) {
  & 'E:\AI-Projects\kilogram-p2p-messenger\scripts\run-cargo-tests-stable.ps1' -Package kilogram-state -Profile verification -CargoFeatures verification-state-lock-adoption -CargoJobs 2 -SkipCacheStatus -Filter $filter -Run -NoCapture
  if($LASTEXITCODE -ne 0){throw "Focused runner exit $LASTEXITCODE"}
 }
 & cargo clippy --locked --offline --jobs 2 --profile verification --package kilogram-state --lib --tests --features verification-state-lock-adoption -- -D warnings
 if($LASTEXITCODE -ne 0){throw "Scoped strict Clippy exit $LASTEXITCODE"}
 & rustfmt --check --edition 2024 --config skip_children=true crates/kilogram-state/src/verification_lock.rs crates/kilogram-state/src/verification_lock_tests.rs crates/kilogram-state/src/lib.rs
 if($LASTEXITCODE -ne 0){throw "Scoped rustfmt exit $LASTEXITCODE"}
 $code=0
 $guard.CheckWork()
} catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 }
finally { try { $guard.CleanupChildren(1000); Write-Output ('owned_children_after='+$guard.Children().Length); $guard.Finish() } catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 } }
exit $code