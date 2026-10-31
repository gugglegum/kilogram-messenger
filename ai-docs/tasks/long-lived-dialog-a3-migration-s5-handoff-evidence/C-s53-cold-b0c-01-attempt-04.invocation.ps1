$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 $inputs=@('Cargo.toml','Cargo.lock','crates/kilogram-state/Cargo.toml','crates/kilogram-state/src/lib.rs','crates/kilogram-state/src/verification_lock.rs','crates/kilogram-state/src/verification_lock_tests.rs','crates/kilogram-state/src/verification_worker_tests.rs','crates/kilogram-state/src/verification_held_tests.rs','scripts/migration-s53-cold-preservation.ps1','scripts/migration-s53-typed-lock-worker.ps1','scripts/migration-s53-held-preservation.cs','scripts/invoke-migration-s53-held-preservation-synthetic.ps1','scripts/run-cargo-tests-stable.ps1','scripts/cargo-resource-policy.ps1','scripts/migration-s5-l1-deadline.ps1')
 function Pins { @($inputs|ForEach-Object{$i=Get-Item -LiteralPath $_;[pscustomobject]@{path=$_;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})|ConvertTo-Json -Compress }
 $before=Pins
 [IO.File]::WriteAllText('E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b0c-01-attempt-04.inputs.before.json',$before,[Text.UTF8Encoding]::new($false))
 & cargo test --locked --offline --no-run --jobs 2 --profile verification --package kilogram-state --features verification-state-lock-adoption
 if($LASTEXITCODE -ne 0){throw "Feature compile-only exit $LASTEXITCODE"}
 foreach($filter in @('verification_lock::held_tests::','verification_lock::worker_tests::')) {
  $guard.CheckWork()
  & 'E:\AI-Projects\kilogram-p2p-messenger\scripts\run-cargo-tests-stable.ps1' -Package kilogram-state -Profile verification -CargoFeatures verification-state-lock-adoption -CargoJobs 2 -SkipCacheStatus -Filter $filter -Run -NoCapture
  if($LASTEXITCODE -ne 0){throw "Focused runner exit $LASTEXITCODE"}
 }
 & cargo clippy --locked --offline --jobs 2 --profile verification --package kilogram-state --lib --tests --features verification-state-lock-adoption -- -D warnings
 if($LASTEXITCODE -ne 0){throw "Strict Clippy exit $LASTEXITCODE"}
 & rustfmt --check --edition 2024 --config skip_children=true crates/kilogram-state/src/verification_lock.rs crates/kilogram-state/src/verification_worker_tests.rs crates/kilogram-state/src/verification_held_tests.rs
 if($LASTEXITCODE -ne 0){throw "Scoped rustfmt exit $LASTEXITCODE"}
 $after=Pins
 [IO.File]::WriteAllText('E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b0c-01-attempt-04.inputs.after.json',$after,[Text.UTF8Encoding]::new($false))
 if($before -cne $after){throw 'Source input drift'}
 $images=@('target/verification/deps/kilogram-state-lib-kilogram_state-tests.exe','target/verification/deps/kilogram-state-test-a3_s1_root_review-tests.exe','target/verification/deps/kilogram-state-test-a3_s2_root_review-tests.exe')
 $pins=@($images|ForEach-Object{$i=Get-Item -LiteralPath $_;[pscustomobject]@{path=$_;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})|ConvertTo-Json -Compress
 [IO.File]::WriteAllText('E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b0c-01-attempt-04.images.json',$pins,[Text.UTF8Encoding]::new($false))
 $code=0
 $guard.CheckWork()
} catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 }
finally { try { $guard.CleanupChildren(1000); Write-Output ('owned_children_after='+$guard.Children().Length); $guard.Finish() } catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 } }
exit $code