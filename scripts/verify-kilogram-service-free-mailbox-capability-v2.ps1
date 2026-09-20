[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$provisioningPath = Join-Path $workspace 'crates\kilogram-mailbox-provisioning\src\lib.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0098-service-free-exact-mailbox-capability-v2.md'

foreach ($path in @($provisioningPath, $ipcPath, $runtimePath, $desktopPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "service-free mailbox capability v2 source is missing: $path"
    }
}

$provisioning = Get-Content -LiteralPath $provisioningPath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    'const EXACT_VOLUNTEER_VERSION: u8 = 2;',
    'struct ExactLocalBindingContent {',
    'struct ExactMailboxOfferContent {',
    'pub fn create_exact_volunteer(',
    'ActivateExactVolunteer {',
    'pub fn activate_exact_volunteer(',
    'mailbox capability chain cannot downgrade from v2 to v1',
    'exact_volunteer_v2_omits_service_and_migrates_without_downgrade'
)) {
    if ($provisioning.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "mailbox provisioning v2 boundary is missing '$required'"
    }
}

function Get-SourceBlock {
    param(
        [Parameter(Mandatory)] [string] $Source,
        [Parameter(Mandatory)] [string] $Start,
        [Parameter(Mandatory)] [string] $End
    )
    $startIndex = $Source.IndexOf($Start, [StringComparison]::Ordinal)
    $endIndex = if ($startIndex -ge 0) {
        $Source.IndexOf($End, $startIndex + $Start.Length, [StringComparison]::Ordinal)
    }
    else {
        -1
    }
    if ($startIndex -lt 0 -or $endIndex -le $startIndex) {
        throw "could not isolate source block '$Start'"
    }
    $Source.Substring($startIndex, $endIndex - $startIndex)
}

$exactLocal = Get-SourceBlock -Source $provisioning `
    -Start 'struct ExactLocalBindingContent {' -End 'impl Drop for ExactLocalBindingContent'
$exactOffer = Get-SourceBlock -Source $provisioning `
    -Start 'struct ExactMailboxOfferContent {' -End 'impl Drop for ExactMailboxOfferContent'
foreach ($block in @($exactLocal, $exactOffer)) {
    foreach ($forbidden in @('MailboxServiceDescriptor', 'service:', 'base_url', 'expected_store_key')) {
        if ($block.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "v2 exact mailbox wire content contains forbidden central service field '$forbidden'"
        }
    }
}

foreach ($required in @(
    'CreateExactMailboxCapability {',
    'RotateExactMailboxCapability {',
    'pub capability_format: String',
    'const IPC_VERSION: u8 = 26;'
)) {
    if ($ipc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC v2 mailbox boundary is missing '$required'"
    }
}

foreach ($required in @(
    'RuntimeMailboxProvisioningMode::ExactVolunteer',
    'SealedLocalMailboxBinding::create_exact_volunteer(',
    'SignedMailboxCapabilityUpdate::activate_exact_volunteer(',
    'exact volunteer mailbox capability requires at least two admission-qualified transport-distinct providers',
    'runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer',
    'exact volunteer replication is incomplete and the v2 capability has no HTTPS fallback',
    'runtime_mailbox_http_poll=not-attempted-v2-exact-volunteer',
    'v2-exact-volunteer',
    'acknowledged_legacy_mailbox_upgrades_once_to_service_free_v2'
)) {
    if ($runtime.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime service-free mailbox boundary is missing '$required'"
    }
}

$exactCreate = Get-SourceBlock -Source $runtime `
    -Start 'RuntimeMailboxExactOfferCreate {' -End 'RuntimeMailboxRotate {'
$exactRotate = Get-SourceBlock -Source $runtime `
    -Start 'RuntimeMailboxExactRotate {' -End 'RuntimeMailboxRevoke {'
foreach ($block in @($exactCreate, $exactRotate)) {
    foreach ($forbidden in @('service_base_url', 'store_key', 'MailboxStoreKey')) {
        if ($block.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "new exact CLI command contains forbidden legacy service argument '$forbidden'"
        }
    }
}

foreach ($required in @(
    'RuntimeIpcCommand::CreateExactMailboxCapability {',
    'RuntimeIpcCommand::RotateExactMailboxCapability {',
    'Storage: exact signed volunteer replica set (no central HTTPS mailbox)',
    'capability.capability_format'
)) {
    if ($desktop.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "desktop service-free mailbox boundary is missing '$required'"
    }
}
foreach ($forbidden in @('mailbox_service_base_url', 'mailbox_store_key')) {
    if ($desktop.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
        throw "desktop default mailbox flow still retains legacy service field '$forbidden'"
    }
}

foreach ($required in @(
    'v1 -> v2',
    'v2 -> v1',
    'does not contain an HTTPS URL',
    'does not contain a central store key',
    'at least two active transport-distinct volunteer providers',
    'does not attempt HTTPS fallback',
    'no new executable'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "RFC-0098 is missing '$required'"
    }
}

Write-Output 'service_free_mailbox_capability_v2=verified'
Write-Output 'new_wire_service_descriptor=absent'
Write-Output 'new_wire_central_store_key=absent'
Write-Output 'legacy_v1_read_migration=retained'
Write-Output 'v2_to_v1_downgrade=forbidden'
Write-Output 'v2_incomplete_replication=https-fallback-forbidden'
Write-Output 'desktop_default=v2-exact-volunteer'
Write-Output 'new_executable=false'
