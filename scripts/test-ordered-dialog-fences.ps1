[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$RunDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
$workspace=Split-Path -Parent $PSScriptRoot
$exe=(Resolve-Path -LiteralPath (Join-Path $workspace 'target/field-tests/ordered-dialog/kilogram-cli.exe')).Path
$old=(Resolve-Path -LiteralPath (Join-Path $workspace 'target/compatibility/ordered-dialog-baseline/kilogram-cli.exe')).Path
if ((Get-FileHash -LiteralPath $old -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'df4ae0d4760c85eb41ab833655061875472e4054295844d452137859f7d4c58c') {throw 'Old executable provenance mismatch.'}
$run=(Resolve-Path -LiteralPath $RunDirectory).Path
$result=Get-Content -LiteralPath (Join-Path $run 'result.json') -Raw | ConvertFrom-Json
$evidence=Join-Path $run 'fences'
if (Test-Path -LiteralPath $evidence) {throw 'Use a completed run without previous fence evidence.'}
[void](New-Item -ItemType Directory -Path $evidence)
$trace=Join-Path $evidence 'trace.log'
$processes=@()
function Exec([string]$Binary,[string[]]$Arguments,[bool]$Denied=$false) {
    $output=& $Binary @Arguments 2>&1
    $code=$LASTEXITCODE
    $rendered=($output | ForEach-Object {$_.ToString()}) -join "`n"
    Add-Content -LiteralPath $trace -Value "exit=$code binary=$Binary $($Arguments -join ' ')`n$rendered"
    if (($Denied -and $code -eq 0) -or (-not $Denied -and $code -ne 0)) {throw "Unexpected fence command result: $code $rendered"}
    return $rendered
}
function Protected-Hashes([string]$Root) {
    $hashes=@{}
    foreach ($file in Get-ChildItem -LiteralPath $Root -Recurse -File) {
        $relative=[IO.Path]::GetRelativePath($Root,$file.FullName)
        # Opening redb can change physical bookkeeping. Ciphertext, projections,
        # native blob, public Head, claim and all authoritative shadow records
        # must remain byte exact; the vault is authenticated again on reopen.
        if ($relative -ne 'state-vault.redb' -and $relative -notmatch 'lock') {
            $hashes[$relative]=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        }
    }
    return $hashes
}
function Assert-Hashes([string]$Root,[hashtable]$Before) {
    $after=Protected-Hashes $Root
    if ($after.Count -ne $Before.Count) {throw 'Unsupported writer changed protected file count.'}
    foreach ($key in $Before.Keys) {if ($after[$key] -cne $Before[$key]) {throw "Unsupported writer changed $key"}}
}
try {
    foreach ($side in @('one','two')) {
        $profile=Join-Path $run "$side.profile"
        $processes+=Start-Process -FilePath $exe -ArgumentList @('runtime-from-profile','--profile-file',('"{0}"' -f $profile)) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence "$side.stdout.log") -RedirectStandardError (Join-Path $evidence "$side.stderr.log")
    }
    Start-Sleep -Seconds 1
    foreach ($side in @('one','two')) {
        $root=Join-Path $run "$side.state"
        [void](Exec $old @('state-vault-migrate','--state-dir',$root) $true)
        [void](Exec $exe @('identity','--state-dir',$root) $true)
    }
    foreach ($p in $processes) {if (-not $p.HasExited) {Stop-Process -Id $p.Id;$p.WaitForExit()}}
    $processes=@()
    foreach ($side in @('one','two')) {
        $root=Join-Path $run "$side.state"
        $before=Protected-Hashes $root
        [void](Exec $old @('state-vault-migrate','--state-dir',$root) $true)
        [void](Exec $old @('identity','--state-dir',$root) $true)
        [void](Exec $old @('runtime-from-profile','--profile-file',(Join-Path $run "$side.profile")) $true)
        [void](Exec $exe @('state-vault-migrate','--state-dir',$root) $true)
        [void](Exec $exe @('device-enroll','--account-dir',(Join-Path $run "$side.account"),'--state-dir',$root) $true)
        Assert-Hashes $root $before
        [void](Exec $exe @('ordered','inspect','--state-dir',$root))
    }
    # A real old runtime owns a real ordinary profile before OD1 activation.
    $account=Join-Path $evidence 'legacy.account';$root=Join-Path $evidence 'legacy.state'
    $accountOutput=Exec $old @('account-create','--account-dir',$account)
    $accountId=[regex]::Match($accountOutput,'(?m)^account_id=(\S+)').Groups[1].Value
    $cert=Join-Path $evidence 'legacy.cert';$devices=Join-Path $evidence 'legacy.devices'
    [void](Exec $old @('device-enroll','--account-dir',$account,'--state-dir',$root,'--certificate-file',$cert))
    [void](Exec $old @('account-device-list','--account-dir',$account,'--device-certificate-file',$cert,'--device-list-file',$devices))
    $profile=Join-Path $evidence 'legacy.profile';$ipc=Join-Path $evidence 'legacy.ipc'
    [void](Exec $old @('runtime-profile-create','--profile-file',$profile,'--state-dir',$root,'--allow-account',$accountId,'--device-list-file',$devices,'--ipc-file',$ipc,'--relay-wait-seconds','0','--route-policy','direct-only','--disable-volunteer-storage'))
    $p=Start-Process -FilePath $old -ArgumentList @('runtime-from-profile','--profile-file',('"{0}"' -f $profile)) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence 'old-live.stdout.log') -RedirectStandardError (Join-Path $evidence 'old-live.stderr.log')
    $processes+= $p
    $deadline=[DateTime]::UtcNow.AddSeconds(15)
    while (-not (Test-Path -LiteralPath $ipc)) {if($p.HasExited -or [DateTime]::UtcNow -ge $deadline){throw 'Old runtime did not start.'};Start-Sleep -Milliseconds 100}
    $before=Protected-Hashes $root
    [void](Exec $exe @('device-enroll','--profile-mode','ordered-dialog-v1','--account-dir',$account,'--state-dir',$root) $true)
    Assert-Hashes $root $before
    $summary=[pscustomobject]@{status='PASS';old_executable=$old;old_sha256=(Get-FileHash -LiteralPath $old -Algorithm SHA256).Hash.ToLowerInvariant();new_sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant();old_live_pid=$p.Id;od1_offline_roots=2;protected_records_unchanged=$true;scope='old CLI offline/live owner, unsupported new writers, fresh-only refusal; no migration'}
    $summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'result.json')
    $summary | ConvertTo-Json
} finally {foreach($p in $processes){if(-not $p.HasExited){Stop-Process -Id $p.Id;$p.WaitForExit()}};Write-Host "fence_evidence_directory=$evidence"}
