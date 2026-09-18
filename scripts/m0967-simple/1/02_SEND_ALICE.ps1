. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$run = Get-M0967Run
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'providers-ready.marker') 900 'providers ready marker'
$private = Get-M0967PrivateRoot 'alice' ([string]$run.run_id)
$role = Get-Content -LiteralPath (Join-Path $private 'role.json') -Raw | ConvertFrom-Json
$profile = [string]$role.profile
$ipc = [string]$role.ipc
$storeData = [string]$role.store_data

$storeLog = Join-Path $private 'compatibility-store-send.log'
$sendLog = Join-Path $script:EvidenceDirectory '03-send-alice.log'
$providerLog = Join-Path $script:EvidenceDirectory '02-alice-providers.log'
$queuePath = Join-Path $script:EvidenceDirectory '03-alice-queue.log'
$offlinePath = Join-Path $script:EvidenceDirectory '03-alice-offline.boundary'
$sentMarker = Join-Path $script:SharedDirectory 'alice-sent.marker'
$resumeQueuedMessage = Test-Path -LiteralPath $queuePath -PathType Leaf
foreach ($path in @($offlinePath, $sentMarker)) {
    if (Test-Path -LiteralPath $path) {
        throw "Alice send has already completed: $path"
    }
}
if ($resumeQueuedMessage) {
    $queueLines = @(Get-Content -LiteralPath $queuePath)
    $queueRequestId = Get-M0967ExactValue $queueLines 'runtime_ipc_request_id' '[0-9a-f]{64}'
    $queueId = Get-M0967ExactValue $queueLines 'runtime_queue_id' '[0-9a-f]{64}'
    $queueStore = Get-M0967ExactValue $queueLines 'runtime_queue_store' 'Inserted|AlreadyPresent'
    $queueBody = Get-M0967ExactValue $queueLines 'runtime_queue_body' 'encrypted-at-rest'
    $queueStatus = Get-M0967ExactValue $queueLines 'status' 'runtime-message-queued'
    if ($queueRequestId -cne $queueId -or $queueStore -notin @('Inserted', 'AlreadyPresent') -or
        $queueBody -cne 'encrypted-at-rest' -or $queueStatus -cne 'runtime-message-queued') {
        throw 'Alice queue evidence is internally inconsistent; refusing an ambiguous resume.'
    }
    if (-not (Test-Path -LiteralPath $sendLog -PathType Leaf)) {
        throw 'Alice queued message has no prior send log; refusing an ambiguous resume.'
    }
    $previousSend = @(Get-Content -LiteralPath $sendLog)
    foreach ($line in @(
        'runtime_outbound_status=mailbox-stored',
        'runtime_mailbox_replication_status=incomplete'
    )) {
        if ($line -cnotin $previousSend) {
            throw "Alice queued message is not at the expected resumable boundary: $line"
        }
    }
    Move-M0967FailedAttemptAside @($storeLog, $sendLog, $providerLog) 'post-queue'
    Write-Host 'RESUMING THE EXISTING DURABLE ALICE QUEUE ITEM; NO NEW MESSAGE WILL BE CREATED.'
} else {
    if ((Test-Path -LiteralPath $sendLog) -and
        [regex]::IsMatch((Get-Content -LiteralPath $sendLog -Raw), '(?m)^runtime_outbound_status=')) {
        throw 'Alice send log already contains an outbound result without queue evidence.'
    }
    Move-M0967FailedAttemptAside @($storeLog, $sendLog, $providerLog) 'pre-queue'
}
$store = $null
$runtime = $null
try {
    $store = Start-M0967Process $script:StorePath @('--data-dir', $storeData) $storeLog
    $null = Wait-M0967LogPattern $storeLog '^status=listening$' $store 60
    if (Test-Path -LiteralPath $ipc) { Remove-Item -LiteralPath $ipc -Force }
    $runtime = Start-M0967Process $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $sendLog
    Wait-M0967IpcReady $ipc $runtime 120
    Import-M0967Providers 'alice' $ipc

    if (-not $resumeQueuedMessage) {
        $queueOutput = @(Invoke-M0967Cli @(
            'runtime-ipc-queue-message', '--ipc-file', $ipc,
            '--conversation', ([string]$run.conversation_label),
            '--peer-account', ([string]$role.bob_account_id),
            '--message', ([string]$run.message_marker)
        ))
        [IO.File]::WriteAllLines($queuePath, $queueOutput, [Text.UTF8Encoding]::new($false))
        $null = Wait-M0967LogPattern $sendLog '^runtime_outbound_status=mailbox-stored$' $runtime 240
    }
    $sendText = Wait-M0967LogPattern $sendLog '^runtime_mailbox_replication_status=satisfied$' $runtime 240
    if (-not [regex]::IsMatch($sendText, '(?m)^runtime_mailbox_replication_receipts=2/2$')) {
        throw 'Alice did not retain the required 2/2 volunteer receipts.'
    }
}
finally {
    Stop-M0967Process $runtime
    Stop-M0967Process $store
}

$probe = @(& $script:CliPath runtime-ipc-ping --ipc-file $ipc 2>&1)
if ($LASTEXITCODE -eq 0) { throw 'Alice runtime is still reachable after stop.' }
$sendText = Get-Content -LiteralPath $sendLog -Raw
$receiptKeys = @([regex]::Matches($sendText, '(?m)^runtime_mailbox_replication_receipt_store_key=([0-9a-f]{64})$') |
    ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
if ($receiptKeys.Count -lt 2) { throw 'Alice send log has fewer than two distinct receipt keys.' }
[IO.File]::WriteAllLines($offlinePath, @(
    'alice_replication_status=satisfied',
    "alice_replica_receipt_store_keys=$($receiptKeys -join ',')",
    'alice_runtime_ipc_reachable=false',
    "sender_stop_observed_utc=$([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))"
), [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText($sentMarker, "sent`n")
Write-Host 'ALICE SEND COMPLETED AND ALICE RUNTIME IS OFFLINE.'
