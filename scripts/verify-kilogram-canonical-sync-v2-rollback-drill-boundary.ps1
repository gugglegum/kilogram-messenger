[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLBACK-DRILL.json'
$dualReaderPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DUAL-READER-RUNTIME.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0198-m1171-deterministic-rollback-drill.md'

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
        [object]$DualReader,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$Adapter,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    $rollbackStatus = 'dormant-deterministic-rollback-drill-verified-not-active'
    $aggregateStatus = 'canonical-sync-v2-opt-in-dark-launch-verified'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.71' -or
        [string]$Manifest.status -cne $rollbackStatus -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'rollback drill header changed'
    }
    if ([string]$Manifest.implementation.snapshot -cne
            'CanonicalSyncV2RollbackSnapshot' -or
        [string]$Manifest.implementation.record -cne
            'CanonicalSyncV2RollbackDrillRecord' -or
        [string]$Manifest.implementation.capture -cne
            'capture_dormant_canonical_sync_v2_rollback_snapshot' -or
        [string]$Manifest.implementation.planner -cne
            'plan_dormant_canonical_sync_v2_rollback' -or
        [UInt64]$Manifest.implementation.snapshot_version -ne 1 -or
        [UInt64]$Manifest.implementation.snapshot_bytes -ne 107 -or
        [UInt64]$Manifest.implementation.max_reason_bytes -ne 256) {
        throw 'rollback drill implementation contract changed'
    }
    foreach ($field in @(
        'snapshot_fixed_width', 'snapshot_strict_decode', 'record_strict_decode',
        'restart_from_persisted_record_verified'
    )) {
        if (-not [bool]$Manifest.persistence_contract.$field) {
            throw "rollback persistence field '$field' changed"
        }
    }
    if ([string]$Manifest.persistence_contract.snapshot_id -cne
            'blake3-derived-key-over-exact-snapshot-bytes' -or
        [string]$Manifest.persistence_contract.record_id -cne
            'blake3-derived-key-over-exact-record-payload' -or
        [bool]$Manifest.persistence_contract.active_runtime_store_integrated) {
        throw 'rollback persistence activation boundary changed'
    }
    Assert-ExactStrings @($Manifest.rollback_policy.allowed_triggers) @(
        'explicit-local', 'release-policy'
    ) 'rollback trigger allowlist'
    foreach ($field in @(
        'automatic_parse_or_network_trigger_forbidden', 'reason_required',
        'reason_trimmed_and_control_free', 'migration_epoch_advances_exactly_one',
        'one_previous_state_only', 'same_input_same_record',
        'selected_session_blocks_rollback',
        'durable_client_progress_blocks_rollback',
        'durable_responder_progress_blocks_rollback', 'legacy_dispatch_symbolic_only'
    )) {
        if (-not [bool]$Manifest.rollback_policy.$field) {
            throw "rollback policy field '$field' changed"
        }
    }
    $transitions = @($Manifest.rollback_transitions | ForEach-Object {
        "$([string]$_.from)->$([string]$_.to)"
    })
    Assert-ExactStrings $transitions @(
        'canonical-required->dual-reader-prefer-canonical',
        'dual-reader-prefer-canonical->dual-reader-prefer-legacy',
        'dual-reader-prefer-legacy->advertise-cbor-v2',
        'advertise-cbor-v2->legacy-only'
    ) 'rollback transition matrix'
    foreach ($field in @(
        'unknown_snapshot_version_or_flags', 'non_canonical_record_length',
        'record_id_mismatch', 'snapshot_mismatch', 'future_selection_epoch',
        'stale_or_skipped_epoch', 'skipped_migration_state',
        'automatic_error_fallback', 'selected_or_progressed_canonical_session'
    )) {
        if (-not [bool]$Manifest.fail_closed.$field) {
            throw "rollback fail-closed field '$field' changed"
        }
    }
    if (-not [bool]$Manifest.activation.drill_verified -or
        [bool]$Manifest.activation.active_transport_authorized -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled -or
        [bool]$Manifest.activation.transport_read_or_write_performed -or
        [bool]$Manifest.activation.active_postcard_path_changed -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'rollback drill activation boundary changed'
    }
    Assert-ExactStrings @($Manifest.remaining) @(
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    ) 'rollback drill remaining work'

    if ([string]$DualReader.milestone -cne 'M1.1.70' -or
        [string]$DualReader.status -cne
            'dormant-dual-reader-runtime-coordinator-verified-not-active' -or
        [bool]$DualReader.activation.active_transport_authorized) {
        throw 'dual-reader prerequisite changed'
    }
    if ([string]$Migration.milestone -cne 'M1.1.85' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [string]$Migration.history_compatible_successor.dormant_deterministic_rollback_drill -cne
            'verified' -or
        [string]$Migration.selection.rollback_drill -cne 'verified' -or
        [string]$Migration.rollback.deterministic_drill -cne 'verified' -or
        [UInt64]$Migration.rollback.snapshot_bytes -ne 107 -or
        -not [bool]$Migration.rollback.explicit_one_step_only -or
        -not [bool]$Migration.rollback.selected_or_durable_canonical_evidence_blocks -or
        [bool]$Migration.selection.selection_authorizes_registration_or_dial) {
        throw 'canonical migration rollback state changed'
    }
    $rollbackRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'rollback-drill'
    })
    if ($rollbackRequirement.Count -ne 1 -or
        [string]$rollbackRequirement[0].status -cne 'verified') {
        throw 'rollback drill activation requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.85' -or
        [string]$TransportManifest.status -cne $aggregateStatus -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_deterministic_rollback_drill -or
        -not [bool]$TransportManifest.selection.rollback_drill_verified -or
        'rollback-drill' -cin @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport manifest rollback state changed'
    }
    Assert-ExactStrings @($TransportManifest.remaining) @() 'transport remaining work'
    if ([string]$Compatibility.milestone -cne 'M1.1.85' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.dormant_deterministic_rollback_drill_integrated -or
        [string]$Compatibility.required_successor.status -cne $aggregateStatus) {
        throw 'history compatibility rollback state changed'
    }
    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.85' -or
        $surface.Count -ne 1 -or
        'CANONICAL-SYNC-V2-ROLLBACK-DRILL.json' -cnotin @($surface[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-rollback-drill-boundary.ps1' -cnotin
            @($surface[0].artifacts) -or
        'docs/RFC-0198-m1171-deterministic-rollback-drill.md' -cnotin @($surface[0].artifacts) -or
        'dormant-deterministic-rollback-drill' -cnotin @($surface[0].covered) -or
        'rollback-drill' -cin @($surface[0].remaining)) {
        throw 'coverage rollback drill state changed'
    }
    Assert-ExactStrings @($surface[0].remaining) @() 'coverage remaining work'

    foreach ($needle in @(
        'pub struct CanonicalSyncV2RollbackSnapshot',
        'pub struct CanonicalSyncV2RollbackDrillRecord',
        'pub fn capture_dormant_canonical_sync_v2_rollback_snapshot(',
        'pub fn plan_dormant_canonical_sync_v2_rollback(',
        'RollbackAutomaticTriggerForbidden',
        'RollbackCanonicalEvidencePresent',
        'RollbackRecordIdMismatch',
        'dormant_rollback_drill_is_restart_stable_and_fails_closed_on_evidence'
    )) {
        if (-not $Adapter.Contains($needle)) {
            throw "rollback drill source marker missing: $needle"
        }
    }
    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('CanonicalSyncV2RollbackSnapshot') -or
            $inactiveSource.Contains('CanonicalSyncV2RollbackDrillRecord') -or
            $inactiveSource.Contains('plan_dormant_canonical_sync_v2_rollback')) {
            throw 'dormant rollback drill leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'strict fixed-width 107-byte record',
        'parse error or network error can never trigger automatic fallback',
        'migration epoch advances exactly once',
        'Rollback is blocked once an authenticated canonical session has been selected',
        'active_transport_authorized()` result is always false',
        'absent from active wire, Iroh and CLI code'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "rollback drill RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $dualReaderPath, $migrationPath, $transportManifestPath,
    $compatibilityPath, $coveragePath, $adapterPath, $wirePath, $transportPath,
    $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "rollback drill file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$dualReader = Get-Content -LiteralPath $dualReaderPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $dualReader $migration $transportManifest $compatibility `
    $coverage $adapter $wire $transport $cli $rfc

if ($SelfTest) {
    $activated = Copy-JsonObject $manifest
    $activated.activation.active_transport_authorized = $true
    Assert-Rejected {
        Assert-Contract $activated $dualReader $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'premature_rollback_activation'

    $automatic = Copy-JsonObject $manifest
    $automatic.rollback_policy.automatic_parse_or_network_trigger_forbidden = $false
    Assert-Rejected {
        Assert-Contract $automatic $dualReader $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'automatic_error_rollback'

    $epoch = Copy-JsonObject $manifest
    $epoch.rollback_policy.migration_epoch_advances_exactly_one = $false
    Assert-Rejected {
        Assert-Contract $epoch $dualReader $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'non_monotonic_rollback_epoch'

    $selected = Copy-JsonObject $manifest
    $selected.rollback_policy.selected_session_blocks_rollback = $false
    Assert-Rejected {
        Assert-Contract $selected $dualReader $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'selected_session_rollback'

    $persistence = Copy-JsonObject $manifest
    $persistence.persistence_contract.record_strict_decode = $false
    Assert-Rejected {
        Assert-Contract $persistence $dualReader $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'missing_persisted_record_codec'

    Assert-Rejected {
        Assert-Contract $manifest $dualReader $migration $transportManifest $compatibility `
            $coverage $adapter $wire `
            ($transport + ' CanonicalSyncV2RollbackDrillRecord') $cli $rfc
    } 'rollback_drill_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_rollback_drill_self_test=passed'
}

Write-Output 'canonical_sync_v2_rollback_drill_boundary=verified'
Write-Output 'snapshot_bytes=107'
Write-Output 'restart_from_persisted_record=verified'
Write-Output 'automatic_error_fallback=false'
Write-Output 'selected_or_progressed_canonical_session_blocks_rollback=true'
Write-Output 'active_transport_authorized=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
