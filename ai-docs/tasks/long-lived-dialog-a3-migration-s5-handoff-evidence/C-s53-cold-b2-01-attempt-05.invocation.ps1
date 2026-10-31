$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
$prefix='E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b2-01-attempt-05'
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 $inputs=@(git ls-files --cached --others --exclude-standard -- apps crates scripts Cargo.toml Cargo.lock | Where-Object{$_ -match '\.(rs|cs|ps1)$|(^|/)Cargo\.(toml|lock)$'} | Sort-Object -Unique)
 if($LASTEXITCODE -ne 0){throw 'Input enumeration'}
 function Pin([string]$p){$i=Get-Item -LiteralPath $p;[ordered]@{path=$p;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant()}}
 function Pins { @($inputs|ForEach-Object{Pin $_})|ConvertTo-Json -Compress }
 $before=Pins
 [IO.File]::WriteAllText($prefix+'.inputs.before.json',$before,[Text.UTF8Encoding]::new($false))
 $expected=Get-Content 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-cold-b2-01-attempt-04.images.json' -Raw | ConvertFrom-Json
 $imageBefore=@(foreach($r in $expected){$actual=Pin $r.path;if($actual.bytes -ne $r.bytes -or $actual.sha256 -cne $r.sha256){throw 'Reviewed image before focused run changed'};$actual})
 [IO.File]::WriteAllText($prefix+'.images.before.json',($imageBefore|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
 & scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Profile verification-scale -CargoFeatures verification-s53-cold -CargoJobs 2 -SkipCacheStatus -Filter cold_b1_ -Run -NoCapture
 if($LASTEXITCODE -ne 0){throw "Small L1 reader/progress focused exit $LASTEXITCODE"}
 $guard.CheckWork()
 & scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Profile verification-scale -CargoFeatures verification-state-lock-adoption -CargoJobs 2 -SkipCacheStatus -Filter verification_lock::held_tests::continuation_ -Run -NoCapture
 if($LASTEXITCODE -ne 0){throw "Genuine continuation focused exit $LASTEXITCODE"}
 $guard.CheckWork()
 foreach($r in $expected){$actual=Pin $r.path;if($actual.bytes -ne $r.bytes -or $actual.sha256 -cne $r.sha256){throw 'Reviewed image changed during focused run'}}
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
 Write-Output 'focused_and_image_identity=PASS'
 $guard.CheckWork();$code=0
} catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 }
finally { try { $guard.CleanupChildren(1000); Write-Output ('owned_children_after='+$guard.Children().Length); $guard.Finish() } catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 } }
exit $code