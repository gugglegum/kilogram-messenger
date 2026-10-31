$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
$prefix='E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b2-01-attempt-04'
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 $inputs=@(git ls-files --cached --others --exclude-standard -- apps crates scripts Cargo.toml Cargo.lock | Where-Object{$_ -match '\.(rs|cs|ps1)$|(^|/)Cargo\.(toml|lock)$'} | Sort-Object -Unique)
 if($LASTEXITCODE -ne 0){throw 'Input enumeration'}
 function Pin([string]$p){$i=Get-Item -LiteralPath $p;[ordered]@{path=$p;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant()}}
 function Pins { @($inputs|ForEach-Object{Pin $_})|ConvertTo-Json -Compress }
 $before=Pins
 [IO.File]::WriteAllText($prefix+'.inputs.before.json',$before,[Text.UTF8Encoding]::new($false))
 & scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Profile verification-scale -CargoFeatures verification-s53-cold -CargoJobs 2 -SkipCacheStatus
 if($LASTEXITCODE -ne 0){throw "Cold stable staging exit $LASTEXITCODE"}
 $guard.CheckWork()
 & scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Profile verification-scale -CargoFeatures verification-state-lock-adoption -CargoJobs 2 -SkipCacheStatus
 if($LASTEXITCODE -ne 0){throw "State stable staging exit $LASTEXITCODE"}
 $guard.CheckWork()
 & cargo clippy --locked --offline --jobs 2 --profile verification-scale --package kilogram-cli --lib --test migration_s53_cold --features verification-s53-cold -- -D warnings
 if($LASTEXITCODE -ne 0){throw "Cold strict Clippy exit $LASTEXITCODE"}
 $guard.CheckWork()
 & cargo clippy --locked --offline --jobs 2 --profile verification-scale --package kilogram-state --lib --tests --features verification-state-lock-adoption -- -D warnings
 if($LASTEXITCODE -ne 0){throw "State strict Clippy exit $LASTEXITCODE"}
 $guard.CheckWork()
 $rust=@('apps/kilogram-cli/tests/migration_s53_cold.rs','apps/kilogram-cli/tests/support/s53_cold/readers.rs','apps/kilogram-cli/tests/support/s53_cold/history.rs','apps/kilogram-cli/tests/support/s53_cold/session.rs','apps/kilogram-cli/tests/support/s53_cold/checks.rs','apps/kilogram-cli/tests/support/s53_cold/fixed.rs','apps/kilogram-cli/tests/support/s53_cold/progress.rs','crates/kilogram-state/src/verification_worker_tests.rs','crates/kilogram-state/src/verification_held_tests.rs')
 & rustfmt --check --edition 2024 --config skip_children=true @rust
 if($LASTEXITCODE -ne 0){throw "Scoped rustfmt exit $LASTEXITCODE"}
 foreach($p in @('scripts/build-migration-s53-cold-b-worker.ps1','scripts/invoke-migration-s53-cold-b-worker.ps1','scripts/invoke-migration-s53-cold-b.ps1','scripts/invoke-migration-s53-held-preservation-synthetic.ps1')){
  $tokens=$null;$errors=$null;$null=[Management.Automation.Language.Parser]::ParseFile((Join-Path (Get-Location) $p),[ref]$tokens,[ref]$errors)
  if($errors.Count -ne 0){throw ('PowerShell syntax: '+$p)}
 }
 $after=Pins
 [IO.File]::WriteAllText($prefix+'.inputs.after.json',$after,[Text.UTF8Encoding]::new($false))
 if($before -cne $after){throw 'Source input drift'}
 $images=@('target/verification-scale/deps/kilogram-cli-test-migration_s53_cold-tests.exe','target/verification-scale/deps/kilogram-state-lib-kilogram_state-tests.exe','target/verification-scale/s53-cold-worker/worker.cs','target/verification-scale/s53-cold-worker/worker.dll','target/verification-scale/s53-cold-worker/artifact.json')
 [IO.File]::WriteAllText($prefix+'.images.json',(@($images|ForEach-Object{Pin $_})|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
 $old=Get-Content 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-cold-b2-01-attempt-01.preserved.json' -Raw | ConvertFrom-Json
 foreach($r in $old){foreach($p in @($r.original,$r.saved)){$now=Pin $p;if($now.bytes -ne $r.bytes -or $now.sha256 -cne $r.sha256){throw 'Historical artifact drift'}}}
 $core=Pin 'scripts/migration-s53-cold-preservation.ps1'
 if($core.bytes -ne 82999 -or $core.sha256 -cne 'eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f'){throw 'Frozen Core drift'}
 Write-Output ('input_count='+$inputs.Count)
 Write-Output 'strict_clippy_scoped_fmt_ps_syntax_prior_artifacts=PASS'
 $guard.CheckWork();$code=0
} catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 }
finally { try { $guard.CleanupChildren(1000); Write-Output ('owned_children_after='+$guard.Children().Length); $guard.Finish() } catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 } }
exit $code