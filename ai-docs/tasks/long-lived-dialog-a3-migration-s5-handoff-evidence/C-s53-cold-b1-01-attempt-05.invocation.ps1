$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 $prefix='E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b1-01-attempt-05'
 $sourcePaths=@(& git ls-files --cached --others --exclude-standard -- '*.rs' '*.c' '*.h' '*.cpp' 'Cargo.toml' '**/Cargo.toml' 'Cargo.lock' 'rust-toolchain*')
 $sourcePaths+=@('scripts/build-migration-s53-cold-b-worker.ps1','scripts/invoke-migration-s53-cold-b-worker.ps1','scripts/invoke-migration-s53-cold-b.ps1','scripts/migration-s53-cold-b-private.cs','scripts/migration-s53-held-preservation.cs','scripts/migration-s53-cold-preservation.ps1','scripts/migration-s53-typed-lock-worker.ps1','scripts/migration-s53-root-copy.ps1','scripts/migration-s5-l1-deadline.ps1','scripts/run-cargo-tests-stable.ps1','scripts/cargo-resource-policy.ps1','.cargo/config.toml')
 $sourcePaths=@($sourcePaths|Where-Object {$_ -cne '.cargo/config.toml' -or (Test-Path -LiteralPath $_)}|Sort-Object -Unique)
 function Pins { @($sourcePaths|ForEach-Object{$i=Get-Item -LiteralPath $_;[ordered]@{path=$_;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})|ConvertTo-Json -Compress }
 $before=Pins
 [IO.File]::WriteAllText($prefix+'.inputs.before.json',$before,[Text.UTF8Encoding]::new($false))
 & cargo clippy --locked --offline --jobs 2 --profile verification --package kilogram-cli --lib --test migration_s53_cold --features verification-s53-cold -- -D warnings
 if($LASTEXITCODE -ne 0){throw "Strict affected CLI Clippy exit $LASTEXITCODE"}
 $guard.CheckWork()
 & cargo clippy --locked --offline --jobs 2 --profile verification --package kilogram-state --lib --features verification-state-lock-adoption -- -D warnings
 if($LASTEXITCODE -ne 0){throw "Strict affected state Clippy exit $LASTEXITCODE"}
 & cargo check --locked --offline --jobs 2 --profile verification --package kilogram-cli --lib --no-default-features
 if($LASTEXITCODE -ne 0){throw "Default-feature compile check exit $LASTEXITCODE"}
 $guard.CheckWork()
 & cargo test --locked --offline --no-run --jobs 2 --profile verification --package kilogram-cli --test migration_s53_cold --features verification-s53-cold
 if($LASTEXITCODE -ne 0){throw "Final focused compile-only exit $LASTEXITCODE"}
 & scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Profile verification -CargoFeatures verification-s53-cold -CargoJobs 2 -SkipCacheStatus -Filter cold_b1_ 
 if($LASTEXITCODE -ne 0){throw "Final stable staging exit $LASTEXITCODE"}
 & rustfmt --check --edition 2024 --config skip_children=true apps/kilogram-cli/tests/migration_s53_cold.rs apps/kilogram-cli/tests/support/s53_cold/readers.rs apps/kilogram-cli/tests/support/s53_cold/history.rs apps/kilogram-cli/tests/support/s53_cold/fixed.rs apps/kilogram-cli/tests/support/s53_cold/session.rs apps/kilogram-cli/tests/support/s53_cold/checks.rs apps/kilogram-cli/tests/support/s53_cold/fixture.rs crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/verification_close.rs
 if($LASTEXITCODE -ne 0){throw "Scoped rustfmt exit $LASTEXITCODE"}
 $after=Pins
 [IO.File]::WriteAllText($prefix+'.inputs.after.json',$after,[Text.UTF8Encoding]::new($false))
 if($before -cne $after){throw 'Source input drift'}
 $images=@('target/verification/deps/kilogram-cli-test-migration_s53_cold-tests.exe','target/verification/s53-cold-worker/worker.cs','target/verification/s53-cold-worker/worker.dll','target/verification/s53-cold-worker/artifact.json')
 $pins=@($images|ForEach-Object{$i=Get-Item -LiteralPath $_;[ordered]@{path=$_;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})|ConvertTo-Json -Compress
 [IO.File]::WriteAllText($prefix+'.images.json',$pins,[Text.UTF8Encoding]::new($false))
 $code=0
 $guard.CheckWork()
} catch {Write-Error -ErrorRecord $_ -ErrorAction Continue;$code=1}
finally {try{$guard.CleanupChildren(1000);Write-Output ('owned_children_after='+$guard.Children().Length);$guard.Finish()}catch{Write-Error -ErrorRecord $_ -ErrorAction Continue;$code=1}}
exit $code