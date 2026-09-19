[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0093-automatic-legacy-mailbox-upgrade.md'

foreach ($path in @($runtimePath, $ipcPath, $manifestPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "automatic legacy mailbox upgrade boundary file is missing: $path"
    }
}

$runtime = Get-Content -LiteralPath $runtimePath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    'RUNTIME_MAILBOX_LEGACY_UPGRADE_INTERVAL',
    'prepare_runtime_mailbox_legacy_upgrade',
    'attempt_runtime_mailbox_legacy_upgrade',
    'head.replica_set().is_some()',
    'mailbox_update_acknowledgements',
    'require_exact_replica_set',
    'automatic legacy mailbox upgrade requires at least two active transport-distinct volunteer providers',
    'MailboxCapabilityBindingState::RotationOverlap',
    'runtime_mailbox_legacy_upgrade_status=rotated',
    'acknowledged_legacy_mailbox_upgrades_once_to_exact_replica_set',
    'assert!(upgraded.queued.is_empty())'
)) {
    if (-not $runtime.Contains($required)) {
        throw "automatic legacy mailbox upgrade implementation is missing '$required'"
    }
}

$tick = $runtime.IndexOf('attempt_runtime_mailbox_legacy_upgrade(&state_dir).await')
$push = $runtime.IndexOf('attempt_next_runtime_mailbox_capability_update(', $tick)
if ($tick -lt 0 -or $push -le $tick) {
    throw 'automatic legacy upgrade is not ordered before authenticated capability delivery'
}

if (-not $ipc.Contains('const IPC_VERSION: u8 = 25;')) {
    throw 'automatic legacy mailbox upgrade requires authenticated IPC version 25'
}

foreach ($required in @(
    'replica_set_discovery',
    'replica_set_commitment_id',
    'replica_set_store_count'
)) {
    if (-not $ipc.Contains($required) -or -not $runtime.Contains($required)) {
        throw "mailbox IPC exact-locator status is missing '$required'"
    }
}

foreach ($required in @(
    'durable recipient-signed acknowledgement',
    'RotationOverlap',
    'does not retransmit conversation history',
    'keeps the HTTPS mailbox descriptor and upload copy',
    'automatic check runs only while',
    'normal Kilogram runtime'
)) {
    if (-not $rfc.Contains($required)) {
        throw "automatic legacy mailbox upgrade RFC is missing '$required'"
    }
}

if (Select-String -LiteralPath $manifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'automatic legacy mailbox upgrade unexpectedly added another executable target'
}

Write-Output 'mailbox_legacy_upgrade_boundary=verified'
Write-Output 'eligibility=acknowledged-legacy-head-and-two-transport-distinct-providers'
Write-Output 'transition=ordered-rotation-with-predecessor-overlap'
Write-Output 'message_requeue=false'
Write-Output 'https_compatibility=retained'
Write-Output 'new_executable=false'
