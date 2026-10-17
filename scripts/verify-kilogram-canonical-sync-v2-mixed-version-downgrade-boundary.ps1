[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-MIXED-VERSION-DOWNGRADE-MATRIX.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$selectionPath = Join-Path $workspace 'CANONICAL-SYNC-V2-MUTUAL-CANDIDATE-SELECTION.json'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0196-m1169-mixed-version-downgrade-matrix.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactStrings {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $values = @($Actual | ForEach-Object { [string]$_ })
    if (($values -join "`n") -cne ($Expected -join "`n")) {
        throw "$Label changed"
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Compatibility,
        [object]$Coverage,
        [object]$Selection,
        [string]$Adapter,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    $status = 'dormant-mixed-version-and-downgrade-matrix-verified-not-active'
    $aggregateStatus = 'canonical-sync-v2-bounded-mutation-corpus-verified-not-active'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.69' -or
        [string]$Manifest.status -cne $status -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'mixed-version matrix header changed'
    }
    if ([string]$Manifest.implementation.migration_state -cne
            'CanonicalSyncV2MigrationState' -or
        [string]$Manifest.implementation.policy_evaluator -cne
            'evaluate_dormant_canonical_sync_v2_transport_policy' -or
        [string]$Manifest.implementation.dormant_plan -cne
            'CanonicalSyncV2DormantTransportPlan' -or
        [string]$Manifest.implementation.authenticated_selection -cne
            'CanonicalSyncV2MutualCandidateSelection') {
        throw 'mixed-version matrix implementation changed'
    }

    Assert-ExactStrings @($Manifest.matrix | ForEach-Object {
        "$($_.state)|$($_.selection)|$($_.proposed_protocol)|$($_.result)"
    }) @(
        'legacy-only|absent|legacy-postcard-v6|planned-dormant',
        'legacy-only|present|legacy-postcard-v6|planned-dormant-selection-not-active',
        'legacy-only|any|canonical-cbor-v2|rejected-premature-canonical',
        'advertise-cbor-v2|absent|legacy-postcard-v6|planned-dormant',
        'advertise-cbor-v2|present|legacy-postcard-v6|planned-dormant-selection-not-active',
        'advertise-cbor-v2|any|canonical-cbor-v2|rejected-premature-canonical',
        'dual-reader-prefer-legacy|absent|legacy-postcard-v6|planned-mixed-version-legacy-peer',
        'dual-reader-prefer-legacy|present|canonical-cbor-v2|planned-authenticated-canonical',
        'dual-reader-prefer-legacy|present|legacy-postcard-v6|rejected-authenticated-downgrade',
        'dual-reader-prefer-legacy|absent|canonical-cbor-v2|rejected-selection-required',
        'dual-reader-prefer-cbor|absent|legacy-postcard-v6|planned-mixed-version-legacy-peer',
        'dual-reader-prefer-cbor|present|canonical-cbor-v2|planned-authenticated-canonical',
        'dual-reader-prefer-cbor|present|legacy-postcard-v6|rejected-authenticated-downgrade',
        'dual-reader-prefer-cbor|absent|canonical-cbor-v2|rejected-selection-required',
        'cbor-required|present|canonical-cbor-v2|planned-authenticated-canonical',
        'cbor-required|absent|canonical-cbor-v2|rejected-selection-required',
        'cbor-required|any|legacy-postcard-v6|rejected-canonical-required'
    ) 'mixed-version and downgrade matrix'

    foreach ($field in @(
        'canonical_without_mutual_selection',
        'legacy_after_authenticated_canonical_selection',
        'migration_epoch_rollback',
        'fallback_after_selected_canonical_failure',
        'format_sniffing',
        'parse_error_fallback'
    )) {
        if (-not [bool]$Manifest.fail_closed.$field) {
            throw "fail-closed matrix field '$field' changed"
        }
    }
    if (-not [bool]$Manifest.security_boundary.dual_reader_selection_absent_legacy_is_only_mixed_version_outcome -or
        -not [bool]$Manifest.security_boundary.signed_selection_must_be_carried_inside_authenticated_transport_context -or
        -not [bool]$Manifest.security_boundary.pre_offer_stripping_not_claimed_solved_by_local_matrix_alone -or
        [bool]$Manifest.security_boundary.active_transport_authorized -or
        [bool]$Manifest.security_boundary.endpoint_registered -or
        [bool]$Manifest.security_boundary.canonical_dial_enabled -or
        [bool]$Manifest.security_boundary.active_postcard_path_changed) {
        throw 'mixed-version security or activation boundary changed'
    }
    Assert-ExactStrings @($Manifest.remaining) @(
        'dual-reader-runtime-implementation',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    ) 'mixed-version remaining work'
    if ([bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'mixed-version scope changed'
    }

    if ([string]$Selection.milestone -cne 'M1.1.68' -or
        [string]$Selection.status -cne
            'dormant-mutual-authenticated-candidate-selection-verified-not-active' -or
        [bool]$Selection.activation.selection_authorizes_transport_activation) {
        throw 'authenticated selection prerequisite changed'
    }
    if ([string]$Migration.milestone -cne 'M1.1.72' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [string]$Migration.history_compatible_successor.dormant_mixed_version_and_downgrade_matrix -cne
            'verified' -or
        [string]$Migration.selection.mixed_version_and_downgrade_matrix -cne 'verified' -or
        [bool]$Migration.selection.selection_authorizes_registration_or_dial -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration matrix state changed'
    }
    $requirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'mixed-version-and-downgrade-tests'
    })
    if ($requirement.Count -ne 1 -or [string]$requirement[0].status -cne 'verified') {
        throw 'mixed-version activation requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.72' -or
        [string]$TransportManifest.status -cne $aggregateStatus -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_mixed_version_and_downgrade_matrix -or
        -not [bool]$TransportManifest.selection.mixed_version_and_downgrade_matrix_verified -or
        [bool]$TransportManifest.selection.selection_authorizes_registration_or_dial -or
        'mixed-version-and-downgrade-tests' -cin @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport matrix state changed'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.72' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.dormant_mixed_version_and_downgrade_matrix_integrated -or
        [string]$Compatibility.required_successor.status -cne $aggregateStatus) {
        throw 'history compatibility matrix state changed'
    }
    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.72' -or
        $surface.Count -ne 1 -or
        'CANONICAL-SYNC-V2-MIXED-VERSION-DOWNGRADE-MATRIX.json' -cnotin @($surface[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-mixed-version-downgrade-boundary.ps1' -cnotin
            @($surface[0].artifacts) -or
        'docs/RFC-0196-m1169-mixed-version-downgrade-matrix.md' -cnotin @($surface[0].artifacts) -or
        'dormant-mixed-version-and-downgrade-matrix' -cnotin @($surface[0].covered) -or
        'mixed-version-and-downgrade-tests' -cin @($surface[0].remaining)) {
        throw 'coverage matrix state changed'
    }

    foreach ($needle in @(
        'pub enum CanonicalSyncV2MigrationState',
        'LegacyOnly',
        'AdvertiseCanonicalV2',
        'DualReaderPreferLegacy',
        'DualReaderPreferCanonical',
        'CanonicalRequired',
        'pub struct CanonicalSyncV2DormantTransportPlan',
        'pub fn evaluate_dormant_canonical_sync_v2_transport_policy(',
        'CanonicalFallbackForbidden',
        'MigrationEpochRollback',
        'AuthenticatedDowngradeDetected',
        'CanonicalSelectionRequired',
        'mixed_version_and_downgrade_matrix_fails_closed'
    )) {
        if (-not $Adapter.Contains($needle)) {
            throw "mixed-version source marker missing: $needle"
        }
    }
    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('evaluate_dormant_canonical_sync_v2_transport_policy') -or
            $inactiveSource.Contains('CanonicalSyncV2DormantTransportPlan') -or
            $inactiveSource.Contains('CanonicalSyncV2MigrationState')) {
            throw 'dormant matrix leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'genuinely old peer',
        'authenticated downgrade and fails closed',
        'never authorizes a legacy retry',
        'does not claim to prevent an attacker from stripping the offer',
        'active_transport_authorized=false'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "mixed-version RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $selectionPath, $adapterPath, $wirePath, $transportPath,
    $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "mixed-version matrix file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$selection = Get-Content -LiteralPath $selectionPath -Raw | ConvertFrom-Json
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $selection $adapter $wire $transport $cli $rfc

if ($SelfTest) {
    $activated = Copy-JsonObject $manifest
    $activated.security_boundary.active_transport_authorized = $true
    Assert-Rejected {
        Assert-Contract $activated $migration $transportManifest $compatibility $coverage `
            $selection $adapter $wire $transport $cli $rfc
    } 'premature_matrix_activation'

    $fallback = Copy-JsonObject $manifest
    $fallback.fail_closed.fallback_after_selected_canonical_failure = $false
    Assert-Rejected {
        Assert-Contract $fallback $migration $transportManifest $compatibility $coverage `
            $selection $adapter $wire $transport $cli $rfc
    } 'selected_canonical_fallback'

    $downgrade = Copy-JsonObject $manifest
    $downgrade.matrix = @($downgrade.matrix | Where-Object {
        [string]$_.result -cne 'rejected-authenticated-downgrade'
    })
    Assert-Rejected {
        Assert-Contract $downgrade $migration $transportManifest $compatibility $coverage `
            $selection $adapter $wire $transport $cli $rfc
    } 'missing_authenticated_downgrade_rows'

    $epoch = Copy-JsonObject $manifest
    $epoch.fail_closed.migration_epoch_rollback = $false
    Assert-Rejected {
        Assert-Contract $epoch $migration $transportManifest $compatibility $coverage `
            $selection $adapter $wire $transport $cli $rfc
    } 'migration_epoch_rollback'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $selection $adapter $wire `
            ($transport + ' evaluate_dormant_canonical_sync_v2_transport_policy') $cli $rfc
    } 'matrix_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_mixed_version_downgrade_self_test=passed'
}

Write-Output 'canonical_sync_v2_mixed_version_downgrade_boundary=verified'
Write-Output 'mixed_version_legacy_peer=true'
Write-Output 'canonical_requires_mutual_selection=true'
Write-Output 'authenticated_downgrade_rejected=true'
Write-Output 'migration_epoch_rollback_rejected=true'
Write-Output 'selected_canonical_fallback_rejected=true'
Write-Output 'active_transport_authorized=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
