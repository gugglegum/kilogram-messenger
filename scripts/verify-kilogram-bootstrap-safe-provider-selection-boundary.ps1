[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$clientLibPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\lib.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0107-bootstrap-safe-provider-selection.md'

foreach ($path in @($providerPath, $clientLibPath, $runtimePath, $ipcPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "bootstrap-safe provider selection boundary file is missing: $path"
    }
}

$provider = Get-Content -LiteralPath $providerPath -Raw
$clientLib = Get-Content -LiteralPath $clientLibPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

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

foreach ($required in @(
    'MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE: u8 = 1',
    'select_bootstrap_safe',
    'select_bootstrap_safe_from_active_offers',
    'select_bootstrap_safe_active_offers',
    'bootstrap_safe_selection_prefers_binary_corroboration_and_fills_fallback',
    'selected_identities.insert(*offer.transport_identity())'
)) {
    if ($provider.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "bootstrap-safe provider selection is missing '$required'"
    }
}
if ($provider.Contains('select_admission_qualified')) {
    throw 'obsolete admission-only new-provider selection remains callable'
}
if (-not $clientLib.Contains('MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE')) {
    throw 'mailbox client does not export the local corroboration preference threshold'
}

$selection = Get-SourceBlock -Source $provider `
    -Start 'fn select_bootstrap_safe_active_offers(' -End 'fn provider_selection_rank('
foreach ($required in @(
    'offer.admission_work_bits() >= u16::from(minimum_work_bits)',
    '< MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE',
    'bootstrap_fallback',
    'provider_selection_rank(&offer, selection_salt)',
    'selected_identities.insert(*offer.transport_identity())'
)) {
    if ($selection.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "bootstrap-safe ranking block is missing '$required'"
    }
}
if ($selection -match 'authenticated_observation_count\(\)\s*\.cmp|cmp\([^\r\n]*authenticated_observation_count') {
    throw 'numeric authenticated observation count unexpectedly affects provider rank'
}
if ($selection -match '(?s)\.filter\(\|offer\|\s*\{?\s*offer\.authenticated_observation_count') {
    throw 'authenticated observation preference unexpectedly became a hard selection gate'
}

foreach ($required in @(
    'select_bootstrap_safe_from_active_offers',
    '.select_bootstrap_safe(selection_salt, requested, now)',
    'provider_selection_policy=bootstrap-safe-admission-plus-local-corroboration',
    'provider_minimum_authenticated_observations_for_preference=',
    'provider_authenticated_observation_count_rank_weight=false',
    'provider_unobserved_bootstrap_fallback=true',
    'runtime_volunteer_storage_selection=bootstrap-safe-admission-plus-local-corroboration',
    'runtime_volunteer_storage_authenticated_observation_count_rank_weight=false',
    'runtime_volunteer_storage_unobserved_bootstrap_fallback=true'
)) {
    if ($runtime.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime bootstrap-safe provider policy is missing '$required'"
    }
}

$provisioning = Get-SourceBlock -Source $runtime `
    -Start 'fn provision_runtime_mailbox(' -End 'fn revoke_runtime_mailbox('
$diagnostic = Get-SourceBlock -Source $runtime `
    -Start 'fn select_runtime_volunteer_storage_providers(' -End 'struct RuntimeIpcDispatchOutcome'
$legacyUpgrade = Get-SourceBlock -Source $runtime `
    -Start 'fn prepare_runtime_mailbox_legacy_upgrade(' `
    -End 'async fn attempt_runtime_volunteer_mailbox_replication('
if (-not $provisioning.Contains('select_bootstrap_safe_from_active_offers')) {
    throw 'new exact mailbox provisioning bypasses bootstrap-safe provider selection'
}
if (-not $diagnostic.Contains('.select_bootstrap_safe(selection_salt, requested, now)')) {
    throw 'diagnostic provider selection bypasses bootstrap-safe policy'
}
if (-not $legacyUpgrade.Contains('select_bootstrap_safe_from_active_offers')) {
    throw 'automatic legacy mailbox upgrade bypasses bootstrap-safe provider selection'
}

$ipcProvider = Get-SourceBlock -Source $ipc `
    -Start 'pub struct RuntimeIpcVolunteerStorageProvider {' `
    -End 'pub struct RuntimeIpcVolunteerStorageOfferImport {'
foreach ($forbidden in @(
    'observer_tag',
    'authenticated_observation',
    'observation_count',
    'AccountId',
    'DeviceId'
)) {
    if ($ipcProvider.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
        throw "provider IPC exposes forbidden local provenance '$forbidden'"
    }
}
if (-not $ipc.Contains('const IPC_VERSION: u8 = 34;')) {
    throw 'bootstrap-safe provider policy unexpectedly changed authenticated IPC v30'
}

foreach ($required in @(
    'preference, not another admission gate',
    'Observation count is deliberately binary for ranking',
    'fill every remaining',
    'unobserved admission-qualified offers',
    'Replacement or',
    'expiry removes it',
    'not proof of operator',
    'network or physical independence',
    'network-free and debug-only'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0107 is missing '$required'"
    }
}

function Assert-ExecutableBoundary {
    param(
        [Parameter(Mandatory)] [string] $MailboxManifest,
        [Parameter(Mandatory)] [string] $CliManifest
    )
    if ($MailboxManifest -match '(?m)^\s*\[\[bin\]\]\s*$') {
        throw 'bootstrap-safe mailbox client unexpectedly added an executable'
    }
    if ([regex]::Matches($CliManifest, '(?m)^\s*\[\[bin\]\]\s*$').Count -ne 1) {
        throw 'CLI must expose exactly one explicit evidence-only executable'
    }
    $featureBlock = Get-SourceBlock -Source $CliManifest -Start '[features]' -End '[[bin]]'
    $binaryBlock = Get-SourceBlock -Source $CliManifest -Start '[[bin]]' -End '[dependencies]'
    $expectedFeatures = @(
        '[features]',
        'default = []',
        'sync-v2-evidence = ["dep:ed25519-dalek"]'
    )
    $expectedBinary = @(
        '[[bin]]',
        'name = "kilogram-sync-v2-evidence"',
        'path = "src/bin/kilogram-sync-v2-evidence.rs"',
        'required-features = ["sync-v2-evidence"]'
    )
    $actualFeatures = @($featureBlock.Replace("`r`n", "`n").Split("`n") | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    $actualBinary = @($binaryBlock.Replace("`r`n", "`n").Split("`n") | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    if (($actualFeatures -join "`n") -cne ($expectedFeatures -join "`n")) {
        throw 'CLI evidence feature/default boundary changed'
    }
    if (($actualBinary -join "`n") -cne ($expectedBinary -join "`n")) {
        throw 'CLI evidence executable lost its exact feature guard'
    }
}

$mailboxManifest = Get-Content -LiteralPath (Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml') -Raw
$cliManifest = Get-Content -LiteralPath (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml') -Raw
Assert-ExecutableBoundary $mailboxManifest $cliManifest

$unguardedFailed = $false
try {
    Assert-ExecutableBoundary $mailboxManifest ($cliManifest.Replace(
        'required-features = ["sync-v2-evidence"]',
        'required-features = []'
    ))
}
catch { $unguardedFailed = $true }
if (-not $unguardedFailed) { throw 'unguarded evidence executable mutation did not fail closed' }

$extraBinaryFailed = $false
try {
    Assert-ExecutableBoundary $mailboxManifest ($cliManifest + "`n[[bin]]`nname = `"unexpected`"`n")
}
catch { $extraBinaryFailed = $true }
if (-not $extraBinaryFailed) { throw 'extra CLI executable mutation did not fail closed' }

Write-Output 'bootstrap_safe_provider_selection_boundary=verified'
Write-Output 'admission_work_floor_bits=18'
Write-Output 'authenticated_observation_preference_threshold=1'
Write-Output 'observation_count_rank_weight=false'
Write-Output 'unobserved_bootstrap_fallback=true'
Write-Output 'transport_identity_distinct=true'
Write-Output 'exact_commitment_resolution_changed=false'
Write-Output 'operator_independence=false'
Write-Output 'ipc_version=30'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'production_new_executable=false'
Write-Output 'feature_gated_evidence_executable=true'
Write-Output 'executable_negative_mutations=2'
