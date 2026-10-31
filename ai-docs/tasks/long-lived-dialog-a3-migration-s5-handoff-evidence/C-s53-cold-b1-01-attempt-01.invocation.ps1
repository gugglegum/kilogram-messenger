$ErrorActionPreference='Stop'
$env:CARGO_NET_OFFLINE='true'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. 'E:\AI-Projects\kilogram-p2p-messenger\scripts\migration-s5-l1-deadline.ps1'
$guard=[KilogramS5Deadline]::new(165,175)
$code=1
try {
 Set-Location 'E:\AI-Projects\kilogram-p2p-messenger'
 & scripts/build-migration-s53-cold-b-worker.ps1
 if($LASTEXITCODE -and $LASTEXITCODE -ne 0){throw "Materializer exit $LASTEXITCODE"}
 $guard.CheckWork()
 & cargo test --locked --offline --no-run --jobs 2 --profile verification --package kilogram-cli --test migration_s53_cold --features verification-s53-cold
 if($LASTEXITCODE -ne 0){throw "Focused compile-only exit $LASTEXITCODE"}
 $code=0
} catch {Write-Error -ErrorRecord $_ -ErrorAction Continue;$code=1}
finally {try{$guard.CleanupChildren(1000);Write-Output ('owned_children_after='+$guard.Children().Length);$guard.Finish()}catch{Write-Error -ErrorRecord $_ -ErrorAction Continue;$code=1}}
exit $code