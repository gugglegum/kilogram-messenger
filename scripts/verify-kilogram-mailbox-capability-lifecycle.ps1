[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$provisioningPath = Join-Path $workspace 'crates\kilogram-mailbox-provisioning\src\lib.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$mainPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'

$provisioning = Get-Content -LiteralPath $provisioningPath -Raw
$protocol = Get-Content -LiteralPath $protocolPath -Raw
$main = Get-Content -LiteralPath $mainPath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$manifest = Get-Content -LiteralPath $manifestPath -Raw

foreach ($required in @(
    'SignedMailboxCapabilityUpdate',
    'MailboxCapabilityUpdateId',
    'verify_chain_link',
    'previous_update_id',
    'pub fn revoke(',
    'mailbox capability rotation must install a different binding'
)) {
    if (-not $provisioning.Contains($required)) {
        throw "mailbox capability artifact is missing '$required'"
    }
}

foreach ($required in @(
    'MailboxCapabilityUpdatePush',
    'MailboxCapabilityUpdateAcknowledged',
    'MailboxCapabilityUpdateRejected',
    'MAX_MAILBOX_CAPABILITY_UPDATE_WIRE_BYTES'
)) {
    if (-not $protocol.Contains($required)) {
        throw "mailbox capability wire contract is missing '$required'"
    }
}

foreach ($required in @(
    'RuntimeMailboxRotate',
    'RuntimeMailboxRevoke',
    'RUNTIME_LOCAL_MAILBOX_UPDATES_DIRECTORY',
    'RUNTIME_PEER_MAILBOX_UPDATES_DIRECTORY',
    'RUNTIME_MAILBOX_UPDATE_ACKNOWLEDGEMENTS_DIRECTORY',
    'attempt_next_runtime_mailbox_capability_update',
    'apply_runtime_mailbox_capability_update',
    'runtime_mailbox_update_acknowledgement_relative_path',
    'mailbox capability update is outside the authenticated Device session',
    'mailbox capability update disappeared before ACK commit',
    'mailbox capability update capacity exceeded'
)) {
    if (-not $main.Contains($required)) {
        throw "runtime mailbox capability lifecycle is missing '$required'"
    }
}

$automaticPush = $main.IndexOf('attempt_next_runtime_mailbox_capability_update(')
$ordinaryDelivery = $main.IndexOf('attempt_next_runtime_delivery(', $automaticPush)
if ($automaticPush -lt 0 -or $ordinaryDelivery -le $automaticPush) {
    throw 'runtime does not schedule mailbox capability convergence before ordinary delivery work'
}

$incomingApply = $main.IndexOf('apply_runtime_mailbox_capability_update(')
$incomingAck = $main.IndexOf('SignedMailboxCapabilityAcknowledgement::sign(', $incomingApply)
if ($incomingApply -lt 0 -or $incomingAck -le $incomingApply) {
    throw 'recipient ACK is not ordered after durable mailbox capability application'
}

foreach ($required in @(
    'SignedMailboxCapabilityAcknowledgement',
    'CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN',
    'session_binding',
    'recipient_identity.device_id() == update.recipient_device_id()'
)) {
    if (-not $provisioning.Contains($required)) {
        throw "shared mailbox capability acknowledgement is missing '$required'"
    }
}

foreach ($required in @(
    'local_unacknowledged_update_count',
    'local_revoked_head_count',
    'peer_revoked_head_count'
)) {
    if (-not $ipc.Contains($required)) {
        throw "runtime mailbox lifecycle IPC status is missing '$required'"
    }
}

$headGuards = [regex]::Matches(
    $main,
    'is not the active capability-chain head'
).Count
if ($headGuards -lt 2) {
    throw 'runtime mailbox readers do not fail closed against superseded or revoked chain heads'
}

foreach ($publicPath in @(
    (Join-Path $workspace 'apps\kilogram-cli\src\runtime_publication.rs'),
    (Join-Path $workspace 'apps\kilogram-cli\src\runtime_endpoint_announcement.rs')
)) {
    $publicSource = Get-Content -LiteralPath $publicPath -Raw
    if ($publicSource -match 'MailboxCapabilityUpdate|EncryptedMailboxOffer|MailboxWriteCapability|MailboxReadCapability') {
        throw "mailbox capability material escaped into public discovery source $publicPath"
    }
}

if ($manifest -match '(?m)^\s*\[\[bin\]\]') {
    throw 'mailbox capability lifecycle unexpectedly added another executable target'
}

Write-Output 'mailbox_capability_lifecycle=verified'
Write-Output 'ordering=device-signed-contiguous-generation-chain'
Write-Output 'transport=existing-authenticated-device-session'
Write-Output 'acknowledgement=recipient-device-signed-session-bound'
Write-Output 'rotation_and_revocation=explicit'
Write-Output 'current_binding_selection=fail-closed-head-with-acked-rotation-overlap'
Write-Output 'public_ticket_capability_material=false'
Write-Output 'new_executable=false'
