[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw

function Get-RustStructBlock {
    param(
        [Parameter(Mandatory)] [string] $Source,
        [Parameter(Mandatory)] [string] $Name
    )
    $match = [regex]::Match(
        $Source,
        "(?s)pub struct $([regex]::Escape($Name))\s*\{.*?\n\}"
    )
    if (-not $match.Success) {
        throw "runtime IPC type '$Name' is missing"
    }
    return $match.Value
}

if (-not $ipc.Contains('const IPC_VERSION: u8 = 24;')) {
    throw 'mailbox desktop-control contract must use authenticated IPC version 24'
}

foreach ($required in @(
    'CreateMailboxCapability',
    'RotateMailboxCapability',
    'RevokeMailboxCapability',
    'MailboxCapabilityChanged',
    'RuntimeIpcMailboxCapabilityStatus',
    'RuntimeIpcMailboxCapabilityTransition',
    'pub capabilities: Vec<RuntimeIpcMailboxCapabilityStatus>'
)) {
    if (-not $ipc.Contains($required)) {
        throw "mailbox desktop IPC contract is missing '$required'"
    }
}

foreach ($type in @(
    'RuntimeIpcMailboxCapabilityStatus',
    'RuntimeIpcMailboxCapabilityTransition'
)) {
    $block = Get-RustStructBlock -Source $ipc -Name $type
    foreach ($forbidden in @(
        'read_capability',
        'write_capability',
        'device_secret',
        'root_secret',
        'offer_file',
        'encrypted_offer'
    )) {
        if ($block.Contains($forbidden)) {
            throw "secret-bearing field '$forbidden' escaped through $type"
        }
    }
}

foreach ($required in @(
    'RuntimeIpcCommand::CreateMailboxCapability',
    'RuntimeIpcCommand::RotateMailboxCapability',
    'RuntimeIpcCommand::RevokeMailboxCapability',
    'with_locked_state(state_directory',
    'MailboxCapabilityChanged(Box::new(report.transition))',
    'collect_runtime_mailbox_status(state_directory)',
    'capabilities.push(RuntimeIpcMailboxCapabilityStatus'
)) {
    if (-not $runtime.Contains($required)) {
        throw "single runtime owner is missing mailbox lifecycle element '$required'"
    }
}

$createHandler = [regex]::Match(
    $runtime,
    '(?s)RuntimeIpcCommand::CreateMailboxCapability\s*\{.*?(?=RuntimeIpcCommand::RotateMailboxCapability)'
).Value
$rotateHandler = [regex]::Match(
    $runtime,
    '(?s)RuntimeIpcCommand::RotateMailboxCapability\s*\{.*?(?=RuntimeIpcCommand::RevokeMailboxCapability)'
).Value
foreach ($entry in @(
    @{ Name = 'activation'; Body = $createHandler },
    @{ Name = 'rotation'; Body = $rotateHandler }
)) {
    if (-not $entry.Body.Contains('provision_runtime_mailbox(') -or
        -not $entry.Body.Contains('None,') -or
        -not $entry.Body.Contains('state_changed = true')) {
        throw "runtime mailbox $($entry.Name) is not a locked mutation without a manual offer export"
    }
    if ($entry.Body.Contains('print_runtime_mailbox_provisioning_report')) {
        throw "runtime mailbox $($entry.Name) leaks provisioning details through IPC handling"
    }
}

foreach ($required in @(
    'Blind mailbox fallback',
    'egui::RichText::new("M0.9.57")',
    'RuntimeUiAction::CreateMailbox',
    'RuntimeUiAction::RotateMailbox',
    'RuntimeUiAction::RevokeMailbox',
    'RuntimeUiAction::RefreshMailboxStatus',
    'WorkerRequest::MailboxCreate',
    'WorkerRequest::MailboxRotate',
    'WorkerRequest::MailboxRevoke',
    'WorkerRequest::MailboxStatus',
    'RuntimeIpcResponse::MailboxCapabilityChanged',
    'RuntimeIpcResponse::MailboxStatus',
    'mailbox_peer_device_id',
    'Exact peer device',
    'Public store key',
    'never cross desktop IPC'
)) {
    if (-not $desktop.Contains($required)) {
        throw "Windows mailbox control is missing '$required'"
    }
}

$desktopMailboxAction = [regex]::Match(
    $desktop,
    '(?s)fn start_mailbox_change\(.*?(?=\n\s*fn start_mailbox_status)'
).Value
if (-not $desktopMailboxAction.Contains('self.model.descriptor()') -or
    -not $desktopMailboxAction.Contains('DeviceId::from_str') -or
    $desktopMailboxAction.Contains('run_recovery_command(') -or
    $desktopMailboxAction.Contains('Command::new(')) {
    throw 'desktop mailbox lifecycle must use the exact recipient Device ID and authenticated IPC only'
}

foreach ($manifestPath in @(
    (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'),
    (Join-Path $workspace 'apps\kilogram-windows\Cargo.toml')
)) {
    $manifest = Get-Content -LiteralPath $manifestPath -Raw
    if ($manifest -match '(?m)^\s*\[\[bin\]\]\s*$') {
        throw "mailbox desktop control unexpectedly added another executable target in $manifestPath"
    }
}

Write-Output 'mailbox_desktop_control=verified'
Write-Output 'ipc_version=24'
Write-Output 'runtime_owner=single-locked-actor'
Write-Output 'mutations=activate-rotate-revoke'
Write-Output 'status=lifecycle-heads-and-convergence'
Write-Output 'recipient_selection=exact-device-id'
Write-Output 'secret_material_in_ipc=false'
Write-Output 'manual_offer_export=false'
Write-Output 'new_executable=false'
