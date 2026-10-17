[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DUAL-READER-RUNTIME.json'
$matrixPath = Join-Path $workspace 'CANONICAL-SYNC-V2-MIXED-VERSION-DOWNGRADE-MATRIX.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0197-m1170-dormant-dual-reader-runtime-coordinator.md'

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
        [object]$Matrix,
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

    $status = 'dormant-dual-reader-runtime-coordinator-verified-not-active'
    $aggregateStatus = 'canonical-sync-v2-coverage-fuzz-harness-verified-campaign-not-run'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.70' -or
        [string]$Manifest.status -cne $status -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'dual-reader runtime header changed'
    }
    if ([string]$Manifest.implementation.coordinator -cne
            'CanonicalSyncV2DormantDualReaderCoordinator' -or
        [string]$Manifest.implementation.policy_plan -cne
            'CanonicalSyncV2DormantTransportPlan' -or
        [string]$Manifest.implementation.client_dispatch -cne
            'CanonicalSyncV2DormantClientDispatch' -or
        [string]$Manifest.implementation.responder_dispatch -cne
            'CanonicalSyncV2DormantResponderDispatch' -or
        [string]$Manifest.implementation.client_runtime -cne
            'CanonicalSyncV2StoredSessionRuntime' -or
        [string]$Manifest.implementation.responder_runtime -cne
            'CanonicalSyncV2StoredResponderRuntime') {
        throw 'dual-reader runtime implementation changed'
    }
    if ([string]$Manifest.dispatch_contract.legacy.result -cne 'LegacyPostcardV6' -or
        [bool]$Manifest.dispatch_contract.legacy.canonical_runtime_required -or
        [bool]$Manifest.dispatch_contract.legacy.canonical_runtime_action_exposed -or
        [bool]$Manifest.dispatch_contract.legacy.active_legacy_runtime_reimplemented -or
        [string]$Manifest.dispatch_contract.canonical.result -cne 'CanonicalCborV2' -or
        -not [bool]$Manifest.dispatch_contract.canonical.mutual_authenticated_selection_required -or
        -not [bool]$Manifest.dispatch_contract.canonical.durable_client_runtime_required -or
        -not [bool]$Manifest.dispatch_contract.canonical.durable_responder_runtime_required -or
        -not [bool]$Manifest.dispatch_contract.canonical.conversation_devices_session_and_protocol_exact_bound -or
        -not [bool]$Manifest.dispatch_contract.canonical.exact_persisted_restart_action_returned -or
        [bool]$Manifest.dispatch_contract.canonical.frame_rebuilt_from_current_state) {
        throw 'dual-reader dispatch contract changed'
    }
    foreach ($field in @(
        'canonical_runtime_missing', 'canonical_runtime_identity_mismatch',
        'checkpoint_frame_action_mismatch', 'canonical_without_mutual_selection',
        'legacy_after_authenticated_canonical_selection',
        'migration_epoch_rollback', 'fallback_after_selected_canonical_failure'
    )) {
        if (-not [bool]$Manifest.fail_closed.$field) {
            throw "dual-reader fail-closed field '$field' changed"
        }
    }
    if (-not [bool]$Manifest.activation.coordinator_integrated -or
        [bool]$Manifest.activation.active_transport_authorized -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled -or
        [bool]$Manifest.activation.transport_read_or_write_performed -or
        [bool]$Manifest.activation.active_postcard_path_changed -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'dual-reader activation boundary changed'
    }
    Assert-ExactStrings @($Manifest.remaining) @(
        'rollback-drill', 'sustained-fuzzing', 'fresh-hosted-and-field-evidence'
    ) 'dual-reader remaining work'

    if ([string]$Matrix.milestone -cne 'M1.1.69' -or
        [string]$Matrix.status -cne
            'dormant-mixed-version-and-downgrade-matrix-verified-not-active' -or
        [bool]$Matrix.security_boundary.active_transport_authorized) {
        throw 'mixed-version matrix prerequisite changed'
    }
    if ([string]$Migration.milestone -cne 'M1.1.73' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [string]$Migration.history_compatible_successor.dormant_dual_reader_runtime_coordinator -cne
            'verified' -or
        [string]$Migration.selection.dual_reader_runtime_coordinator -cne 'verified' -or
        [bool]$Migration.selection.selection_authorizes_registration_or_dial -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration dual-reader state changed'
    }
    $requirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'dual-reader-runtime-implementation'
    })
    if ($requirement.Count -ne 1 -or [string]$requirement[0].status -cne 'verified') {
        throw 'dual-reader runtime activation requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.73' -or
        [string]$TransportManifest.status -cne $aggregateStatus -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_dual_reader_runtime_coordinator -or
        -not [bool]$TransportManifest.selection.dual_reader_runtime_coordinator_verified -or
        [bool]$TransportManifest.selection.selection_authorizes_registration_or_dial -or
        'dual-reader-runtime-implementation' -cin @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport manifest dual-reader state changed'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.73' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.dormant_dual_reader_runtime_coordinator_integrated -or
        [string]$Compatibility.required_successor.status -cne $aggregateStatus) {
        throw 'history compatibility dual-reader state changed'
    }
    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.73' -or
        $surface.Count -ne 1 -or
        'CANONICAL-SYNC-V2-DUAL-READER-RUNTIME.json' -cnotin @($surface[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-dual-reader-runtime-boundary.ps1' -cnotin
            @($surface[0].artifacts) -or
        'docs/RFC-0197-m1170-dormant-dual-reader-runtime-coordinator.md' -cnotin @($surface[0].artifacts) -or
        'dormant-dual-reader-runtime-coordinator' -cnotin @($surface[0].covered) -or
        'dual-reader-runtime-implementation' -cin @($surface[0].remaining)) {
        throw 'coverage dual-reader runtime state changed'
    }

    foreach ($needle in @(
        'pub enum CanonicalSyncV2DormantClientDispatch',
        'pub enum CanonicalSyncV2DormantResponderDispatch',
        'pub struct CanonicalSyncV2DormantDualReaderCoordinator',
        'pub fn plan_client_dispatch(',
        'pub fn plan_responder_dispatch(',
        'CanonicalDurableRuntimeMissing',
        'CanonicalDurableRuntimeIdentityMismatch',
        'durable_client_action_from_runtime(',
        'durable_responder_action_from_runtime(',
        'dormant_dual_reader_coordinator_requires_canonical_runtime_only_for_canonical_plan'
    )) {
        if (-not $Adapter.Contains($needle)) {
            throw "dual-reader runtime source marker missing: $needle"
        }
    }
    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('CanonicalSyncV2DormantDualReaderCoordinator') -or
            $inactiveSource.Contains('CanonicalSyncV2DormantClientDispatch') -or
            $inactiveSource.Contains('plan_client_dispatch')) {
            throw 'dormant dual-reader coordinator leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'does not reimplement, wrap or modify the installed legacy runtime',
        'never rebuilt from current local state',
        'returns a dispatch description, not a transport permit',
        'active_transport_authorized()` result is always false',
        'absent from active wire, Iroh and CLI code'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "dual-reader runtime RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $matrixPath, $migrationPath, $transportManifestPath,
    $compatibilityPath, $coveragePath, $adapterPath, $wirePath, $transportPath,
    $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "dual-reader runtime file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$matrix = Get-Content -LiteralPath $matrixPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $matrix $migration $transportManifest $compatibility `
    $coverage $adapter $wire $transport $cli $rfc

if ($SelfTest) {
    $activated = Copy-JsonObject $manifest
    $activated.activation.active_transport_authorized = $true
    Assert-Rejected {
        Assert-Contract $activated $matrix $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'premature_dual_reader_activation'

    $missingRuntime = Copy-JsonObject $manifest
    $missingRuntime.dispatch_contract.canonical.durable_client_runtime_required = $false
    Assert-Rejected {
        Assert-Contract $missingRuntime $matrix $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'canonical_without_durable_client_runtime'

    $rebuilt = Copy-JsonObject $manifest
    $rebuilt.dispatch_contract.canonical.frame_rebuilt_from_current_state = $true
    Assert-Rejected {
        Assert-Contract $rebuilt $matrix $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'canonical_frame_rebuild'

    $identity = Copy-JsonObject $manifest
    $identity.dispatch_contract.canonical.conversation_devices_session_and_protocol_exact_bound = $false
    Assert-Rejected {
        Assert-Contract $identity $matrix $migration $transportManifest $compatibility `
            $coverage $adapter $wire $transport $cli $rfc
    } 'canonical_runtime_identity_not_bound'

    Assert-Rejected {
        Assert-Contract $manifest $matrix $migration $transportManifest $compatibility `
            $coverage $adapter $wire `
            ($transport + ' CanonicalSyncV2DormantDualReaderCoordinator') $cli $rfc
    } 'dual_reader_coordinator_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_dual_reader_runtime_self_test=passed'
}

Write-Output 'canonical_sync_v2_dual_reader_runtime_boundary=verified'
Write-Output 'legacy_dispatch_symbolic_only=true'
Write-Output 'canonical_durable_client_action_exact=true'
Write-Output 'canonical_durable_responder_action_exact=true'
Write-Output 'canonical_runtime_identity_bound=true'
Write-Output 'active_transport_authorized=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
