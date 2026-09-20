[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0109-positive-provider-path-colocation-avoidance.md'

foreach ($path in @($providerPath, $runtimePath, $ipcPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "provider co-location boundary file is missing: $path"
    }
}

$provider = Get-Content -LiteralPath $providerPath -Raw
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

$selection = Get-SourceBlock -Source $provider `
    -Start 'fn select_bootstrap_safe_active_offers(' -End 'fn provider_selection_rank('
foreach ($required in @(
    'offer.admission_work_bits() >= u16::from(minimum_work_bits)',
    '< MIN_AUTHENTICATED_PROVIDER_OBSERVATIONS_FOR_PREFERENCE',
    'provider_selection_rank(&offer, selection_salt)',
    'selected_identities.contains(offer.transport_identity())',
    'shares_verified_path_domain_with(chosen)',
    'known_colocated.push(offer)',
    'for offer in known_colocated',
    'selected_identities.insert(*offer.transport_identity())'
)) {
    if ($selection.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "provider co-location selection is missing '$required'"
    }
}
if ($selection -match '(?s)\.filter\([^\)]*(has_verified_path_domain|verified_path_domain_kind)') {
    throw 'missing provider path-domain evidence unexpectedly became an admission gate'
}
if ($selection -match 'authenticated_observation_count\(\)\s*\.cmp|cmp\([^\r\n]*authenticated_observation_count') {
    throw 'numeric authenticated observation count unexpectedly affects provider rank'
}

foreach ($required in @(
    'bootstrap_safe_selection_avoids_known_colocation_without_unknown_deadlock',
    'assert!(!selected[1].has_verified_path_domain())',
    'registry.select_bootstrap_safe(salt, 4, 1_003)',
    'all.last().map(MailboxProviderOffer::offer_id)'
)) {
    if ($provider.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "provider co-location regression is missing '$required'"
    }
}

foreach ($required in @(
    'provider_known_path_domain_colocation_avoidance=true',
    'provider_unknown_path_domain_fallback=true',
    'provider_path_domain_inequality_proves_independence=false',
    'runtime_volunteer_storage_known_path_domain_colocation_avoidance=true',
    'runtime_volunteer_storage_unknown_path_domain_fallback=true',
    'runtime_volunteer_storage_path_domain_inequality_proves_independence=false'
)) {
    if ($runtime.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime provider co-location policy is missing '$required'"
    }
}

$provisioning = Get-SourceBlock -Source $runtime `
    -Start 'fn provision_runtime_mailbox(' -End 'fn revoke_runtime_mailbox('
$diagnostic = Get-SourceBlock -Source $runtime `
    -Start 'fn select_runtime_volunteer_storage_providers(' -End 'struct RuntimeIpcDispatchOutcome'
$legacyUpgrade = Get-SourceBlock -Source $runtime `
    -Start 'fn prepare_runtime_mailbox_legacy_upgrade(' `
    -End 'async fn attempt_runtime_volunteer_mailbox_replication('
foreach ($block in @($provisioning, $legacyUpgrade)) {
    if (-not $block.Contains('select_bootstrap_safe_from_active_offers')) {
        throw 'new exact-provider selection bypasses co-location-aware bootstrap policy'
    }
}
if (-not $diagnostic.Contains('.select_bootstrap_safe(selection_salt, requested, now)')) {
    throw 'diagnostic provider selection bypasses co-location-aware bootstrap policy'
}
if (-not $provider.Contains('active_offers_for_store_keys')) {
    throw 'exact committed provider lookup unexpectedly disappeared'
}

$ipcProvider = Get-SourceBlock -Source $ipc `
    -Start 'pub struct RuntimeIpcVolunteerStorageProvider {' `
    -End 'pub struct RuntimeIpcVolunteerStorageOfferImport {'
foreach ($forbidden in @(
    'path_domain',
    'domain_tag',
    'derivation_epoch',
    'remote_ip',
    'relay_origin'
)) {
    if ($ipcProvider.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "provider IPC exposes local path provenance '$forbidden'"
    }
}
if (-not $ipc.Contains('const IPC_VERSION: u8 = 26;')) {
    throw 'provider co-location preference unexpectedly changed authenticated IPC v26'
}

foreach ($required in @(
    'positive meaning of that evidence',
    'preference, not an admission rule',
    'no path-domain evidence remains eligible',
    'different known tag receives no independence bonus',
    'reconsider deferred co-located offers',
    'existing exact',
    'replica set continues to resolve',
    'Unequal tags are not',
    'proof of independence',
    'network-free and debug-only'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0109 is missing '$required'"
    }
}

foreach ($manifest in @(
    (Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml'),
    (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml')
)) {
    if (Select-String -LiteralPath $manifest -Pattern '^\s*\[\[bin\]\]\s*$') {
        throw "provider co-location preference unexpectedly added an executable: $manifest"
    }
}

Write-Output 'provider_colocation_avoidance_boundary=verified'
Write-Output 'known_equal_path_domain_deferred=true'
Write-Output 'unknown_path_domain_first_pass_eligible=true'
Write-Output 'deferred_colocated_availability_fallback=true'
Write-Output 'different_tags_prove_independence=false'
Write-Output 'missing_tags_rejected=false'
Write-Output 'exact_commitment_resolution_changed=false'
Write-Output 'path_tags_transmitted=false'
Write-Output 'path_tags_in_ipc=false'
Write-Output 'ipc_version=26'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'new_executable=false'
