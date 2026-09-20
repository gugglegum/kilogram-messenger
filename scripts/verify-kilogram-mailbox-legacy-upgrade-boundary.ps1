[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0093-automatic-legacy-mailbox-upgrade.md'
$v2RfcPath = Join-Path $workspace 'docs\RFC-0098-service-free-exact-mailbox-capability-v2.md'

foreach ($path in @($runtimePath, $ipcPath, $manifestPath, $rfcPath, $v2RfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "automatic legacy mailbox upgrade boundary file is missing: $path"
    }
}

$runtime = Get-Content -LiteralPath $runtimePath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$v2Rfc = Get-Content -LiteralPath $v2RfcPath -Raw

foreach ($required in @(
    'RUNTIME_MAILBOX_LEGACY_UPGRADE_INTERVAL',
    'prepare_runtime_mailbox_legacy_upgrade',
    'attempt_runtime_mailbox_legacy_upgrade',
    'head.is_exact_volunteer()',
    'mailbox_update_acknowledgements',
    'RuntimeMailboxProvisioningMode::ExactVolunteer',
    'exact volunteer mailbox capability requires at least two admission-qualified transport-distinct providers',
    'MailboxCapabilityBindingState::RotationOverlap',
    'runtime_mailbox_legacy_upgrade_status=rotated',
    'acknowledged_legacy_mailbox_upgrades_once_to_service_free_v2',
    'assert!(upgraded.queued.is_empty())'
)) {
    if (-not $runtime.Contains($required)) {
        throw "automatic legacy mailbox upgrade implementation is missing '$required'"
    }
}

$tick = $runtime.IndexOf('attempt_runtime_mailbox_legacy_upgrade(&context.state_directory).await')
if ($tick -lt 0) {
    throw 'automatic legacy upgrade scheduler call is missing'
}
$push = $runtime.IndexOf('attempt_next_runtime_mailbox_capability_update(', $tick)
if ($push -le $tick) {
    throw 'automatic legacy upgrade is not ordered before authenticated capability delivery'
}

if (-not $ipc.Contains('const IPC_VERSION: u8 = 26;')) {
    throw 'service-free automatic legacy mailbox upgrade requires authenticated IPC version 26'
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
    'automatic check runs only while',
    'normal Kilogram runtime'
)) {
    if (-not $rfc.Contains($required)) {
        throw "automatic legacy mailbox upgrade RFC is missing '$required'"
    }
}

foreach ($required in @(
    'v1 -> v2',
    'v2 -> v1',
    'does not contain an HTTPS URL',
    'does not attempt HTTPS fallback'
)) {
    if (-not $v2Rfc.Contains($required)) {
        throw "service-free legacy migration RFC is missing '$required'"
    }
}

if (Select-String -LiteralPath $manifestPath -Pattern '^\s*\[\[bin\]\]\s*$') {
    throw 'automatic legacy mailbox upgrade unexpectedly added another executable target'
}

Write-Output 'mailbox_legacy_upgrade_boundary=verified'
Write-Output 'eligibility=acknowledged-legacy-head-and-two-admission-qualified-transport-distinct-providers'
Write-Output 'transition=ordered-rotation-with-predecessor-overlap'
Write-Output 'message_requeue=false'
Write-Output 'https_compatibility=legacy-v1-only'
Write-Output 'new_executable=false'
