[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$mainPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$runtimeHelperPath = Join-Path $workspace 'scripts\invoke-kilogram-mailbox-field-runtime.ps1'
$captureHelperPath = Join-Path $workspace 'scripts\capture-kilogram-mailbox-field-status.ps1'
$evidenceVerifierPath = Join-Path $workspace 'scripts\verify-kilogram-mailbox-field-evidence.ps1'
$main = Get-Content -LiteralPath $mainPath -Raw
$manifest = Get-Content -LiteralPath $manifestPath -Raw
$runtimeHelper = Get-Content -LiteralPath $runtimeHelperPath -Raw
$captureHelper = Get-Content -LiteralPath $captureHelperPath -Raw
$evidenceVerifier = Get-Content -LiteralPath $evidenceVerifierPath -Raw

foreach ($required in @(
    'KILOGRAM_TEST_DROP_MAILBOX_CAPABILITY_ACK_ONCE',
    '#[cfg(not(debug_assertions))]',
    'is available only in debug builds',
    '#[cfg(debug_assertions)]',
    'RuntimeMailboxCapabilityAckDropped',
    'runtime_test_fault_armed=mailbox-capability-ack-drop-after-durable-apply-once',
    'runtime_test_fault_ack_written=false',
    'runtime_test_fault_status=triggered-after-durable-apply-and-vault-mirror',
    'debug-mailbox-capability-ack-drop'
)) {
    if (-not $main.Contains($required)) {
        throw "debug field-test fault contract is missing '$required'"
    }
}

$apply = $main.IndexOf('apply_runtime_mailbox_capability_update(')
$drop = $main.IndexOf('take_mailbox_capability_ack_drop', $apply)
$sign = $main.IndexOf('SignedMailboxCapabilityAcknowledgement::sign(', $drop)
if ($apply -lt 0 -or $drop -le $apply -or $sign -le $drop) {
    throw 'controlled ACK drop is not ordered strictly after durable apply and before ACK signing'
}
$handlerStart = $main.IndexOf('async fn handle_runtime_application_connection')
$handlerEnd = $main.IndexOf('async fn listen_inner', $handlerStart)
$handler = $main.Substring($handlerStart, $handlerEnd - $handlerStart)
$runtimeTrigger = $main.IndexOf('triggered-after-durable-apply-and-vault-mirror')
if (-not $handler.Contains('guard.finish()') -or
    -not $handler.Contains('Ok(operation_result)') -or
    $runtimeTrigger -lt 0) {
    throw 'runtime does not stop at an evidence point after the vault mirror completes'
}

foreach ($entry in @(
    @{ Name = 'runtime helper'; Text = $runtimeHelper },
    @{ Name = 'status helper'; Text = $captureHelper },
    @{ Name = 'evidence verifier'; Text = $evidenceVerifier }
)) {
    foreach ($forbidden in @('cargo build', '--release', 'Compress-Archive', '.zip')) {
        if ($entry.Text.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "$($entry.Name) unexpectedly builds or packages artifacts: $forbidden"
        }
    }
}

foreach ($required in @(
    'runtime-from-profile',
    'DropMailboxCapabilityAckAfterApplyOnce',
    'field evidence already exists and will not be overwritten'
)) {
    if (-not $runtimeHelper.Contains($required)) {
        throw "runtime field helper is missing '$required'"
    }
}
foreach ($required in @(
    'runtime-ipc-mailbox-status',
    '01-alice-pending.status',
    '07-bob-revoked.status',
    'field evidence already exists and will not be overwritten'
)) {
    if (-not $captureHelper.Contains($required)) {
        throw "status field helper is missing '$required'"
    }
}
foreach ($required in @(
    'mailbox_capability_update_store=Inserted',
    'mailbox_capability_update_store=AlreadyPresent',
    'activation-pending',
    'rotation-pending',
    'revocation-pending',
    'runtime_outbound_status=mailbox-stored',
    'runtime_mailbox_inbound_status=deleted-after-commit',
    'storage_format=opaque-redb-v1',
    "'direct' -notin `$routes -or 'relay' -notin `$routes",
    'metadata_leak_evidence=rejected'
)) {
    if (-not $evidenceVerifier.Contains($required)) {
        throw "fail-closed field evidence verifier is missing '$required'"
    }
}

if ($manifest -match '(?m)^\s*\[\[bin\]\]\s*$') {
    throw 'mailbox field test unexpectedly added another executable target'
}

Write-Output 'mailbox_field_test_boundary=verified'
Write-Output 'fault_scope=debug-only-one-shot'
Write-Output 'fault_order=durable-apply-then-drop-before-ack'
Write-Output 'restart_boundary=automatic-runtime-stop-after-vault-mirror'
Write-Output 'evidence=fail-closed-direct-and-relay-lifecycle'
Write-Output 'mailbox_round_trip=store-signed-opaque-commit-before-delete'
Write-Output 'portable_package=false'
Write-Output 'new_executable=false'
