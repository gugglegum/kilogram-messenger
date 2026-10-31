$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 $inputs=@('Cargo.toml','Cargo.lock','crates/kilogram-state/Cargo.toml','crates/kilogram-state/src/lib.rs','crates/kilogram-state/src/verification_lock.rs','crates/kilogram-state/src/verification_lock_tests.rs','scripts/run-cargo-tests-stable.ps1','scripts/cargo-resource-policy.ps1','scripts/migration-s5-l1-deadline.ps1')
 function Pins { @($inputs|ForEach-Object{$i=Get-Item -LiteralPath $_;[pscustomobject]@{path=$_;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})|ConvertTo-Json -Compress }
 $before=Pins
 [IO.File]::WriteAllText('E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b0a-01-attempt-07.inputs.before.json',$before,[Text.UTF8Encoding]::new($false))
 & cargo test --locked --offline --no-run --jobs 2 --profile verification --package kilogram-state --no-default-features
 if($LASTEXITCODE -ne 0){throw "Default-feature compile-only exit $LASTEXITCODE"}
 $guard.CheckWork()
 & cargo test --locked --offline --no-run --jobs 2 --profile verification --package kilogram-state --features verification-state-lock-adoption
 if($LASTEXITCODE -ne 0){throw "Adoption compile-only exit $LASTEXITCODE"}
 $guard.CheckWork()
 & 'E:\AI-Projects\kilogram-p2p-messenger\scripts\run-cargo-tests-stable.ps1' -Package kilogram-state -Profile verification -CargoFeatures verification-state-lock-adoption -CargoJobs 2 -SkipCacheStatus -Filter 'verification_lock::tests::' -Run -NoCapture
 if($LASTEXITCODE -ne 0){throw "Focused final runner exit $LASTEXITCODE"}
 $after=Pins
 [IO.File]::WriteAllText('E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b0a-01-attempt-07.inputs.after.json',$after,[Text.UTF8Encoding]::new($false))
 if($before -cne $after){throw 'Source input drift during final verification'}
 $images=@('target/verification/deps/kilogram-state-lib-kilogram_state-tests.exe','target/verification/deps/kilogram-state-test-a3_s1_root_review-tests.exe','target/verification/deps/kilogram-state-test-a3_s2_root_review-tests.exe')
 $pins=@($images|ForEach-Object{$i=Get-Item -LiteralPath $_;[pscustomobject]@{path=$_;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})|ConvertTo-Json -Compress
 [IO.File]::WriteAllText('E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b0a-01-attempt-07.images.json',$pins,[Text.UTF8Encoding]::new($false))
 $code=0
 $guard.CheckWork()
} catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 }
finally { try { $guard.CleanupChildren(1000); Write-Output ('owned_children_after='+$guard.Children().Length); $guard.Finish() } catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 } }
exit $code