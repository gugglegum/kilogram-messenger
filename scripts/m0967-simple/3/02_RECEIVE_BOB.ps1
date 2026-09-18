. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$run = Get-M0967Run
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'alice-sent.marker') 1800 'Alice sent marker'
$private = Get-M0967PrivateRoot 'bob' ([string]$run.run_id)
$role = Get-Content -LiteralPath (Join-Path $private 'role.json') -Raw | ConvertFrom-Json
$profile = [string]$role.profile
$ipc = [string]$role.ipc
$state = [string]$role.state
$attemptStamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
$attemptDirectory = Join-Path $private "attempt-logs\$attemptStamp"
New-M0967Directory $attemptDirectory
$script:BobReceiveStage = 'bootstrap'
$attemptTrace = Join-Path $attemptDirectory 'receive-attempt.log'
[IO.File]::WriteAllText(
    $attemptTrace,
    "attempt_utc=$attemptStamp`nstage=$($script:BobReceiveStage)`n",
    [Text.UTF8Encoding]::new($false)
)

function Set-M0967BobReceiveStage {
    param([Parameter(Mandatory)] [string] $Stage)
    $script:BobReceiveStage = $Stage
    [IO.File]::AppendAllText(
        $attemptTrace,
        "stage=$Stage`n",
        [Text.UTF8Encoding]::new($false)
    )
}

function Publish-M0967BobAttemptFile {
    param(
        [Parameter(Mandatory)] [string] $Source,
        [Parameter(Mandatory)] [string] $Destination
    )
    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) { return }
    $temporary = "$Destination.publish-$PID-$([Guid]::NewGuid().ToString('N'))"
    [IO.File]::WriteAllBytes($temporary, [IO.File]::ReadAllBytes($Source))
    Move-Item -LiteralPath $temporary -Destination $Destination -Force
}

function Publish-M0967BobAttemptLogs {
    param(
        [Parameter(Mandatory)] [string] $LocalLog,
        [Parameter(Mandatory)] [string] $SharedLog
    )
    Publish-M0967BobAttemptFile $LocalLog $SharedLog
    Publish-M0967BobAttemptFile "$LocalLog.stderr" "$SharedLog.stderr"
}

$receiveLog = Join-Path $script:EvidenceDirectory '04-receive-bob.log'
$providerLog = Join-Path $script:EvidenceDirectory '02-bob-providers.log'
$localReceiveLog = Join-Path $attemptDirectory '04-receive-bob.log'
$localRestartLog = Join-Path $attemptDirectory '05-restart-bob.log'

try {
    Set-M0967BobReceiveStage 'guard-previous-attempt'
    if (Test-Path -LiteralPath $receiveLog -PathType Leaf) {
        $previousReceive = @(Get-Content -LiteralPath $receiveLog)
        $hasReplicaLedgerEvidence = @($previousReceive | Where-Object {
            $_ -cmatch '^runtime_mailbox_inbound_source=' -or
            $_ -cmatch '^runtime_mailbox_replica_delete_status=' -or
            $_ -cmatch '^runtime_mailbox_replica_source_store_key='
        }).Count -gt 0
        if ($hasReplicaLedgerEvidence) {
            throw 'Bob has partial replica-ledger evidence; refusing an ambiguous receive retry.'
        }
        $receivedEventIds = @($previousReceive | ForEach-Object {
            if ($_ -cmatch '^runtime_mailbox_received_event_id=([0-9a-f]{64})$') { $Matches[1] }
        } | Sort-Object -Unique)
        if ($receivedEventIds.Count -gt 1) {
            throw 'Bob previous attempt contains more than one received event; refusing an ambiguous retry.'
        }
        if ($receivedEventIds.Count -eq 1) {
            $receiveOutcomes = @($previousReceive | Where-Object {
                $_ -cmatch '^runtime_mailbox_receive_store=(Inserted|AlreadyPresent)$'
            })
            if ($receiveOutcomes.Count -lt 1) {
                throw 'Bob received-event evidence has no durable store outcome.'
            }
            Move-M0967FailedAttemptAside @($receiveLog, $providerLog) 'post-application-commit'
            Write-Host 'RESUMING AFTER THE EXISTING DURABLE BOB COMMIT; NO MESSAGE WILL BE INSERTED TWICE.'
        }
        else {
            Move-M0967FailedAttemptAside @($receiveLog, $providerLog) 'pre-inbound'
            Write-Host 'RETRYING BOB BEFORE THE FIRST INBOUND COMMIT; NO MESSAGE WAS CONSUMED.'
        }
    }

    $runtime = $null
    try {
        Set-M0967BobReceiveStage 'remove-stale-ipc'
        if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
        Set-M0967BobReceiveStage 'start-runtime-local-log'
        $runtime = Start-M0967Process `
            $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $localReceiveLog
        Set-M0967BobReceiveStage 'wait-runtime-ipc'
        Wait-M0967IpcReady $ipc $runtime 120
        Set-M0967BobReceiveStage 'import-provider-snapshots'
        Import-M0967Providers 'bob' $ipc
        Set-M0967BobReceiveStage 'wait-two-inbound-commits'
        $null = Wait-M0967LogCount `
            $localReceiveLog '^runtime_mailbox_inbound_source=volunteer-iroh$' 2 $runtime 300
        Set-M0967BobReceiveStage 'wait-two-replica-deletes'
        $null = Wait-M0967LogCount `
            $localReceiveLog '^runtime_mailbox_replica_delete_status=deleted-after-commit$' 2 $runtime 120
    }
    finally {
        try { Stop-M0967Process $runtime }
        finally { Publish-M0967BobAttemptLogs $localReceiveLog $receiveLog }
    }

    $restartLog = Join-Path $script:EvidenceDirectory '05-restart-bob.log'
    $restart = $null
    try {
        Set-M0967BobReceiveStage 'restart-runtime-local-log'
        if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
        $restart = Start-M0967Process `
            $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $localRestartLog
        Wait-M0967IpcReady $ipc $restart 180
        $null = Wait-M0967LogPattern $localRestartLog '^status=runtime-listening$' $restart 30
        Start-Sleep -Seconds 20
    }
    finally {
        try { Stop-M0967Process $restart }
        finally { Publish-M0967BobAttemptLogs $localRestartLog $restartLog }
    }
    if ([regex]::IsMatch((Get-Content -LiteralPath $localRestartLog -Raw), '(?m)^runtime_mailbox_inbound_source=volunteer-iroh$')) {
        throw 'Deleted volunteer replica was delivered again after restart.'
    }
    Set-M0967BobReceiveStage 'write-history'
    $historyPath = Join-Path $script:EvidenceDirectory '05-bob-history.log'
    $history = @(Invoke-M0967Cli @('history', '--state-dir', $state, '--conversation', ([string]$run.conversation_label)))
    [IO.File]::WriteAllLines($historyPath, $history, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'bob-complete.marker'), "complete`n")
    Set-M0967BobReceiveStage 'complete'
    Write-Host 'BOB RECEIVE AND RESTART COMPLETED SUCCESSFULLY.'
}
catch {
    $failure = $_
    $failureLocal = Join-Path $attemptDirectory '04-receive-bob.failure.log'
    $failureShared = Join-Path $script:EvidenceDirectory "04-receive-bob.failure-$attemptStamp.log"
    $failureLines = @(
        "attempt_utc=$attemptStamp",
        "stage=$($script:BobReceiveStage)",
        "exception=$([string]$failure.Exception.Message -replace "`r?`n", ' | ')",
        "script_stack=$([string]$failure.ScriptStackTrace -replace "`r?`n", ' | ')"
    )
    [IO.File]::WriteAllLines($failureLocal, $failureLines, [Text.UTF8Encoding]::new($false))
    Publish-M0967BobAttemptFile $failureLocal $failureShared
    throw
}
