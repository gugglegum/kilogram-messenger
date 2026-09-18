. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$run = Get-M0967Run
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'alice-sent.marker') 1800 'Alice sent marker'
$private = Get-M0967PrivateRoot 'bob' ([string]$run.run_id)
$role = Get-Content -LiteralPath (Join-Path $private 'role.json') -Raw | ConvertFrom-Json
$profile = [string]$role.profile
$ipc = [string]$role.ipc
$state = [string]$role.state

$receiveLog = Join-Path $script:EvidenceDirectory '04-receive-bob.log'
$runtime = $null
try {
    $runtime = Start-M0967Process $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $receiveLog
    $null = Wait-M0967File $ipc 120 'Bob runtime IPC'
    $null = Invoke-M0967Cli @('runtime-ipc-ping', '--ipc-file', $ipc)
    Import-M0967Providers 'bob' $ipc
    $null = Wait-M0967LogCount $receiveLog '^runtime_mailbox_inbound_source=volunteer-iroh$' 2 $runtime 300
    $null = Wait-M0967LogCount $receiveLog '^runtime_mailbox_replica_delete_status=deleted-after-commit$' 2 $runtime 120
}
finally { Stop-M0967Process $runtime }

$restartLog = Join-Path $script:EvidenceDirectory '05-restart-bob.log'
$restart = $null
try {
    if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
    $restart = Start-M0967Process $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $restartLog
    $null = Wait-M0967LogPattern $restartLog '^status=runtime-listening$' $restart 180
    Start-Sleep -Seconds 20
}
finally { Stop-M0967Process $restart }
if ([regex]::IsMatch((Get-Content -LiteralPath $restartLog -Raw), '(?m)^runtime_mailbox_inbound_source=volunteer-iroh$')) {
    throw 'Deleted volunteer replica was delivered again after restart.'
}
$historyPath = Join-Path $script:EvidenceDirectory '05-bob-history.log'
$history = @(Invoke-M0967Cli @('history', '--state-dir', $state, '--conversation', ([string]$run.conversation_label)))
[IO.File]::WriteAllLines($historyPath, $history, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'bob-complete.marker'), "complete`n")
Write-Host 'BOB RECEIVE AND RESTART COMPLETED SUCCESSFULLY.'

