[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$mainPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$mailboxPath = Join-Path $workspace 'apps\kilogram-cli\src\runtime_mailbox.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$main = Get-Content -LiteralPath $mainPath -Raw
$mailbox = Get-Content -LiteralPath $mailboxPath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$manifest = Get-Content -LiteralPath $manifestPath -Raw

foreach ($required in @(
    'kilogram-mailbox-client',
    'attempt_runtime_mailbox_fallback',
    'prepare_orphan_runtime_mailbox_dispatch',
    'attempt_pending_runtime_mailbox_upload',
    'attempt_runtime_mailbox_poll',
    'commit_runtime_mailbox_text',
    'commit_runtime_mailbox_acknowledgement',
    'prepare_runtime_reverse_mailbox_acknowledgement',
    'runtime_outbound_status=mailbox-stored',
    'runtime_mailbox_inbound_status=deleted-after-commit'
)) {
    if (-not ($manifest.Contains($required) -or $main.Contains($required))) {
        throw "runtime mailbox flow is missing required element '$required'"
    }
}

$direct = $main.IndexOf('send_runtime_delivery_with_failover(endpoint, state_directory, &prepared).await')
$fallback = $main.IndexOf('attempt_runtime_mailbox_fallback(endpoint, state_directory, &prepared).await', $direct)
if ($direct -lt 0 -or $fallback -lt 0 -or $fallback -le $direct) {
    throw 'mailbox fallback is not ordered after bounded direct/relay delivery'
}

$applicationCommit = $main.IndexOf('ledger.record_inbound_commit(&inbound, application_commit_id')
$delete = $main.IndexOf('client.delete(&delete_request).await?', $applicationCommit)
if ($applicationCommit -lt 0 -or $delete -lt 0 -or $delete -le $applicationCommit) {
    throw 'mailbox deletion is not ordered after durable application commit evidence'
}

$repair = $main.IndexOf('async fn prepare_orphan_runtime_mailbox_dispatch')
$repairEnqueue = $main.IndexOf('ledger.enqueue_outbound(request', $repair)
$repairUpload = $main.IndexOf('upload_runtime_mailbox_request(endpoint, state_directory, upload).await?', $repairEnqueue)
if ($repair -lt 0 -or $repairEnqueue -le $repair -or $repairUpload -le $repairEnqueue) {
    throw 'orphan mailbox dispatch is not durably re-enqueued before network upload'
}

foreach ($required in @(
    'SignedRuntimeMailboxDispatch',
    'RuntimeMailboxPayload',
    'runtime_mailbox_item_id',
    'runtime_mailbox_event_item_id'
)) {
    if (-not $mailbox.Contains($required)) {
        throw "runtime mailbox authenticated state is missing '$required'"
    }
}

foreach ($required in @(
    'MailboxPending',
    'MailboxStored',
    'MailboxExpired',
    'MailboxFailed',
    'RuntimeIpcMailboxStatus',
    'MailboxStatus'
)) {
    if (-not $ipc.Contains($required)) {
        throw "runtime mailbox IPC contract is missing '$required'"
    }
}

$newBinaryDefinitions = Select-String -Path $manifestPath -Pattern '^\s*\[\[bin\]\]\s*$'
if ($newBinaryDefinitions) {
    throw 'runtime mailbox flow unexpectedly added another executable target'
}

Write-Output 'runtime_mailbox_flow=verified'
Write-Output 'delivery_order=direct-relay-then-mailbox'
Write-Output 'delete_order=application-commit-then-delete'
Write-Output 'reverse_ack=durable-mailbox-fallback'
Write-Output 'orphan_dispatch=repair-before-network'
Write-Output 'ipc_states=pending-stored-received-deleted-expired-failed'
Write-Output 'new_executable=false'
