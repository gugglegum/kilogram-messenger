$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 $out='E:\AI-Projects\kilogram-p2p-messenger\target\verification\s53-cold-worker'
 $archive='E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-b1-artifacts\attempt-01'
 if(Test-Path -LiteralPath $archive){throw 'Artifact archive collision'}
 $null=New-Item -ItemType Directory -Path $archive
 foreach($name in @('worker.cs','worker.dll','artifact.json')){[IO.File]::Copy((Join-Path $out $name),(Join-Path $archive $name),$false);if((Get-FileHash -LiteralPath (Join-Path $out $name)).Hash -cne (Get-FileHash -LiteralPath (Join-Path $archive $name)).Hash){throw 'Artifact archive mismatch'}}
 if(@(Get-ChildItem -LiteralPath $out -Force).Count -ne 3){throw 'Unexpected artifact directory contents'}
 foreach($name in @('worker.cs','worker.dll','artifact.json')){Remove-Item -LiteralPath (Join-Path $out $name)}
 Remove-Item -LiteralPath $out
 & scripts/build-migration-s53-cold-b-worker.ps1
 $guard.CheckWork()
 & scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Profile verification -CargoFeatures verification-s53-cold -CargoJobs 2 -SkipCacheStatus -Filter cold_b1_ -Run -NoCapture
 if($LASTEXITCODE -ne 0){throw "Focused stable runner exit $LASTEXITCODE"}
 $code=0
} catch {Write-Error -ErrorRecord $_ -ErrorAction Continue;$code=1}
finally {try{$guard.CleanupChildren(1000);Write-Output ('owned_children_after='+$guard.Children().Length);$guard.Finish()}catch{Write-Error -ErrorRecord $_ -ErrorAction Continue;$code=1}}
exit $code