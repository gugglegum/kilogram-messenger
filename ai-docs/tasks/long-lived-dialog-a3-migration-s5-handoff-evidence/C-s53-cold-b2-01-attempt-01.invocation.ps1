$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(570,590)
$code=1
$prefix='E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-b2-01-attempt-01'
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 $saved='.tmp/C-s53-cold-b2-artifacts-before'
 if(Test-Path -LiteralPath $saved){throw 'Preservation collision'}
 $null=New-Item -ItemType Directory -Path $saved
 $old=@(
 @('target/verification/deps/kilogram-cli-test-migration_s53_cold-tests.exe',17871360,'e31573a26d60a745e503ba28cb9ff22c24d46bd79c796450445dfec277051dd6'),
 @('target/verification/s53-cold-worker/worker.cs',129239,'aac4840a7b948b3954faa247b0c6beac01a479385d62c40ff8023cf70c10b2a4'),
 @('target/verification/s53-cold-worker/worker.dll',142848,'8b0fcaa118472474597d6150436f7a76bc4847b333a94c440da104fcde305369'),
 @('target/verification/s53-cold-worker/artifact.json',2214,'c37340fb15ae2a78150df512ebf9c397bf399777942239487db919ae40aa5d93'))
 $preserved=@(foreach($row in $old){
  $item=Get-Item -LiteralPath $row[0];$hash=(Get-FileHash -LiteralPath $row[0] -Algorithm SHA256).Hash.ToLowerInvariant()
  if($item.Length -ne $row[1] -or $hash -cne $row[2]){throw 'Prior artifact changed'}
  $dst=Join-Path $saved $item.Name
  Copy-Item -LiteralPath $item.FullName -Destination $dst
  if((Get-FileHash -LiteralPath $dst -Algorithm SHA256).Hash.ToLowerInvariant() -cne $hash){throw 'Preservation hash'}
  [ordered]@{original=$row[0];saved=$dst;bytes=$item.Length;sha256=$hash}
 })
 [IO.File]::WriteAllText($prefix+'.preserved.json',($preserved|ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
 $inputs=@(git ls-files --cached --others --exclude-standard -- apps crates scripts Cargo.toml Cargo.lock | Where-Object{$_ -match '\.(rs|cs|ps1)$|(^|/)Cargo\.(toml|lock)$'} | Sort-Object -Unique)
 if($LASTEXITCODE -ne 0){throw 'Input enumeration'}
 function Pins { @($inputs|ForEach-Object{$i=Get-Item -LiteralPath $_;[ordered]@{path=$_;bytes=$i.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}})|ConvertTo-Json -Compress }
 $before=Pins
 [IO.File]::WriteAllText($prefix+'.inputs.before.json',$before,[Text.UTF8Encoding]::new($false))
 & scripts/build-migration-s53-cold-b-worker.ps1 | Out-Null
 $guard.CheckWork()
 & cargo test --locked --offline --no-run --jobs 2 --profile verification-scale --package kilogram-cli --test migration_s53_cold --features verification-s53-cold
 if($LASTEXITCODE -ne 0){throw "Cold harness compile-only exit $LASTEXITCODE"}
 $guard.CheckWork()
 & cargo test --locked --offline --no-run --jobs 2 --profile verification-scale --package kilogram-state --lib --features verification-state-lock-adoption
 if($LASTEXITCODE -ne 0){throw "State lib compile-only exit $LASTEXITCODE"}
 $guard.CheckWork()
 $after=Pins
 [IO.File]::WriteAllText($prefix+'.inputs.after.json',$after,[Text.UTF8Encoding]::new($false))
 if($before -cne $after){throw 'Source input drift'}
 Write-Output ('input_count='+$inputs.Count)
 $code=0
} catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 }
finally { try { $guard.CleanupChildren(1000); Write-Output ('owned_children_after='+$guard.Children().Length); $guard.Finish() } catch { Write-Error -ErrorRecord $_ -ErrorAction Continue; $code=1 } }
exit $code