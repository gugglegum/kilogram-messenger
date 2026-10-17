[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ATOMIC-RUNTIME-TRANSITION.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0191-m1165-atomic-runtime-transition-transaction.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$Store,
        [string]$Adapter,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.65' -or
        [string]$Manifest.status -cne
            'dormant-atomic-runtime-transition-transaction-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'canonical sync v2 atomic runtime transition header changed'
    }
    if ([string]$Manifest.implementation.crate -cne 'kilogram-store' -or
        [string]$Manifest.implementation.store -cne 'CanonicalSyncV2CarrierStore' -or
        [string]$Manifest.implementation.transition_api -cne
            'commit_session_runtime_transition' -or
        [string]$Manifest.implementation.durability -cne
            'redb-immediate-single-write-transaction' -or
        [string]$Manifest.implementation.optimistic_concurrency_token -cne
            'CanonicalSyncV2StoredSessionRuntime') {
        throw 'canonical sync v2 atomic runtime transition implementation contract changed'
    }
    foreach ($field in @(
        'expected_checkpoint_and_runtime_match_exact_durable_state',
        'successor_signature_and_transition_verified_before_write',
        'received_carriers_closed_codec_verified_before_write',
        'received_carriers_exactly_match_signed_successor_evidence_delta',
        'received_carrier_conflicts_preflighted_before_write',
        'successor_orphan_conflict_preflighted_before_write',
        'received_carriers_successor_head_and_runtime_committed_atomically',
        'replacement_pending_frame_bound_to_successor_checkpoint',
        'exact_lost_response_retry_idempotent',
        'different_retry_successor_or_replacement_frame_rejected',
        'malformed_existing_carrier_rejects_whole_transition'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 atomic transition contract '$field' is not verified"
        }
    }
    foreach ($field in @('format_sniffing', 'parse_error_fallback')) {
        if ([bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 atomic transition unsafe contract '$field' is enabled"
        }
    }
    foreach ($field in @(
        'atomic_transition_survives_reopen',
        'received_carrier_visible_with_successor_only_after_commit',
        'replacement_pending_frame_survives_reopen',
        'exact_transition_retry_idempotent',
        'carrier_key_conflict_rolls_back_new_carriers',
        'carrier_key_conflict_preserves_previous_head',
        'carrier_key_conflict_preserves_previous_runtime'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 atomic transition regression '$field' is not verified"
        }
    }
    if (-not [bool]$Manifest.activation.atomic_store_transition_integrated -or
        [bool]$Manifest.activation.adapter_runtime_transition_integrated -or
        [bool]$Manifest.activation.runtime_resume_from_checkpoint_integrated -or
        [bool]$Manifest.activation.mutual_capability_selection_integrated -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled) {
        throw 'canonical sync v2 atomic transition activation boundary changed'
    }
    if ('signed-checkpoint-runtime-transition-integration' -cnotin @($Manifest.remaining) -or
        'atomic-received-carrier-and-checkpoint-transition' -cin @($Manifest.remaining) -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'canonical sync v2 atomic transition scope changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.75' -or
        [string]$Migration.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_runtime_transition_transaction -cne
            'verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration lost atomic runtime transition state'
    }
    $requirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-atomic-runtime-transition-transaction' }
    )
    if ($requirement.Count -ne 1 -or [string]$requirement[0].status -cne 'verified') {
        throw 'canonical migration atomic runtime transition requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.75' -or
        [string]$TransportManifest.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_runtime_transition_transaction -or
        'atomic-received-carrier-and-checkpoint-transition' -cin
            @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport sync manifest lost atomic runtime transition state'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.75' -or
        [string]$Compatibility.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending' -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_runtime_transition_transaction_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending') {
        throw 'history compatibility manifest lost atomic runtime transition state'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) | Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.75' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-ATOMIC-RUNTIME-TRANSITION.json' -cnotin
            @($transportCoverage[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-atomic-runtime-transition-boundary.ps1' -cnotin
            @($transportCoverage[0].artifacts) -or
        'docs/RFC-0191-m1165-atomic-runtime-transition-transaction.md' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-atomic-runtime-transition-transaction' -cnotin
            @($transportCoverage[0].covered) -or
        'atomic-received-carrier-and-checkpoint-transition' -cin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include atomic runtime transition artifacts'
    }

    foreach ($needle in @(
        'pub fn commit_session_runtime_transition(',
        'prepare_canonical_sync_v2_carriers(',
        'CanonicalSyncV2SessionRuntimeExpectationMismatch',
        'CanonicalSyncV2SessionRuntimeTransitionConflict',
        'CanonicalSyncV2SessionRuntimeReceivedCarrierMismatch',
        'canonical_sync_v2_session_runtime_transition_commits_carriers_checkpoint_and_frame_atomically',
        'canonical_sync_v2_session_runtime_transition_conflict_rolls_back_everything'
    )) {
        if (-not $Store.Contains($needle)) {
            throw "kilogram-store lost atomic runtime transition source marker: $needle"
        }
    }
    if (-not $Adapter.Contains('commit_session_runtime_transition')) {
        throw 'atomic runtime transition is no longer integrated by the dormant adapter'
    }
    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('commit_session_runtime_transition')) {
            throw 'atomic runtime transition leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'optimistic concurrency token',
        'commits all missing carriers',
        'complete rollback on a carrier-key conflict',
        'remains unregistered and undialed'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "atomic runtime transition RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $storePath, $adapterPath, $wirePath, $transportPath, $cliPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 atomic runtime transition file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$store = Get-Content -LiteralPath $storePath -Raw
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $store $adapter $wire $transport $cli $rfc

if ($SelfTest) {
    $premature = Copy-JsonObject $manifest
    $premature.activation.adapter_runtime_transition_integrated = $true
    Assert-Rejected {
        Assert-Contract $premature $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'premature_adapter_activation'

    $fallback = Copy-JsonObject $manifest
    $fallback.contracts.parse_error_fallback = $true
    Assert-Rejected {
        Assert-Contract $fallback $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'parse_error_fallback'

    $nonAtomic = Copy-JsonObject $manifest
    $nonAtomic.contracts.received_carriers_successor_head_and_runtime_committed_atomically = $false
    Assert-Rejected {
        Assert-Contract $nonAtomic $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'non_atomic_transition'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire ($transport + ' commit_session_runtime_transition') $cli $rfc
    } 'transition_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_atomic_runtime_transition_self_test=passed'
}

Write-Output 'canonical_sync_v2_atomic_runtime_transition_boundary=verified'
Write-Output 'transition_commit=received-carriers+signed-successor+head+runtime-frame'
Write-Output 'durability=redb-immediate-single-write-transaction'
Write-Output 'exact_retry=idempotent'
Write-Output 'adapter_runtime_transition=false'
Write-Output 'runtime_resume_from_checkpoint=false'
Write-Output 'mutual_capability_selection=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
