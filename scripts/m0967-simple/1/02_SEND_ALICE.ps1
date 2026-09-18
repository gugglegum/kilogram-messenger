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
$store = $null
$runtime = $null
try {
    $store = Start-M0967Process $script:StorePath @('--data-dir', $storeData) $storeLog
    $null = Wait-M0967LogPattern $storeLog '^status=listening$' $store 60
    $runtime = Start-M0967Process $script:CliPath @('runtime-from-profile', '--profile-file', $profile) $sendLog
    $null = Wait-M0967File $ipc 120 'Alice runtime IPC'
    $null = Invoke-M0967Cli @('runtime-ipc-ping', '--ipc-file', $ipc)
    Import-M0967Providers 'alice' $ipc

    $queuePath = Join-Path $script:EvidenceDirectory '03-alice-queue.log'
    $queueOutput = @(Invoke-M0967Cli @(
        'runtime-ipc-queue-message', '--ipc-file', $ipc,
        '--conversation', ([string]$run.conversation_label),
        '--peer-account', ([string]$role.bob_account_id),
        '--message', ([string]$run.message_marker)
    ))
    [IO.File]::WriteAllLines($queuePath, $queueOutput, [Text.UTF8Encoding]::new($false))
    $null = Wait-M0967LogPattern $sendLog '^runtime_outbound_status=mailbox-stored$' $runtime 240
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
[IO.File]::WriteAllLines((Join-Path $script:EvidenceDirectory '03-alice-offline.boundary'), @(
    'alice_replication_status=satisfied',
    "alice_replica_receipt_store_keys=$($receiptKeys -join ',')",
    'alice_runtime_ipc_reachable=false',
    "sender_stop_observed_utc=$([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))"
), [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'alice-sent.marker'), "sent`n")
Write-Host 'ALICE SEND COMPLETED AND ALICE RUNTIME IS OFFLINE.'

