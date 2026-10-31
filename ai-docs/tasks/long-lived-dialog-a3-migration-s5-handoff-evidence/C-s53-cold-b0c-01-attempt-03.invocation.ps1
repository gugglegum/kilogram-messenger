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
 $guard.CheckWork()
 & 'E:\AI-Projects\kilogram-p2p-messenger\scripts\run-cargo-tests-stable.ps1' -Package kilogram-state -Profile verification -CargoFeatures verification-state-lock-adoption -CargoJobs 2 -SkipCacheStatus -Filter 'verification_lock::held_tests::unchanged_and_allowed_slot_delta' -Run -NoCapture
 $code=$LASTEXITCODE
 $guard.CheckWork()
} catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 }
finally { try { $guard.CleanupChildren(1000); Write-Output ('owned_children_after='+$guard.Children().Length); $guard.Finish() } catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 } }
exit $code