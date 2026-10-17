[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DURABLE-CLIENT-ADAPTER.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0192-m1166-durable-client-transition-adapter.md'

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
        [string]$Adapter,
        [string]$Store,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.66' -or
        [string]$Manifest.status -cne
            'dormant-durable-client-transition-adapter-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'canonical sync v2 durable client adapter header changed'
    }
    if ([string]$Manifest.implementation.crate -cne 'kilogram-session' -or
        [string]$Manifest.implementation.adapter -cne
            'CanonicalSyncV2Client<CanonicalSyncV2CarrierStore>' -or
        [string]$Manifest.implementation.durable_state -cne
            'CanonicalSyncV2StoredSessionRuntime' -or
        [string]$Manifest.implementation.atomic_transition -cne
            'CanonicalSyncV2CarrierStore::commit_session_runtime_transition') {
        throw 'canonical sync v2 durable client adapter implementation changed'
    }
    foreach ($field in @(
        'bootstrap_restores_exact_session_identity',
        'inventory_persisted_before_transport_write',
        'diff_verification_precedes_storage',
        'received_carriers_checkpoint_head_and_batch_frame_commit_atomically',
        'event_batch_persisted_before_transport_write',
        'receive_complete_preserves_exact_event_batch_for_retry',
        'complete_verifies_exact_current_round_sent_keys',
        'continuation_advances_exactly_one_round',
        'terminal_completion_clears_pending_frame',
        'restart_dispatch_uses_exact_persisted_frame',
        'unexpected_action_or_frame_fails_closed',
        'exact_lost_response_retry_idempotent'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "durable client adapter contract '$field' is not verified"
        }
    }
    foreach ($field in @('format_sniffing', 'parse_error_fallback')) {
        if ([bool]$Manifest.contracts.$field) {
            throw "durable client adapter unsafe contract '$field' is enabled"
        }
    }
    foreach ($field in @(
        'two_round_client_session_reaches_terminal_checkpoint',
        'received_carrier_commits_with_send_batch_checkpoint',
        'inventory_and_event_batch_resume_exactly',
        'database_reopen_restores_next_round',
        'empty_second_round_is_supported',
        'diff_transition_exact_retry_is_idempotent',
        'batch_sent_transition_exact_retry_is_idempotent',
        'complete_transition_exact_retry_is_idempotent',
        'cumulative_received_evidence_is_round_scoped_for_stats'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "durable client adapter regression '$field' is not verified"
        }
    }
    if (-not [bool]$Manifest.activation.atomic_store_transition_integrated -or
        -not [bool]$Manifest.activation.durable_client_adapter_integrated -or
        -not [bool]$Manifest.activation.exact_client_restart_dispatch_integrated -or
        [bool]$Manifest.activation.durable_server_adapter_integrated -or
        [bool]$Manifest.activation.mutual_capability_selection_integrated -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled -or
        'dormant-server-runtime-transition-integration' -cnotin @($Manifest.remaining) -or
        'signed-checkpoint-runtime-transition-integration' -cin @($Manifest.remaining) -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'canonical sync v2 durable client adapter activation boundary changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.75' -or
        [string]$Migration.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending' -or
        [string]$Migration.history_compatible_successor.dormant_durable_client_transition_adapter -cne
            'verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration lost durable client adapter state'
    }
    $requirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-durable-client-transition-adapter' }
    )
    if ($requirement.Count -ne 1 -or [string]$requirement[0].status -cne 'verified') {
        throw 'canonical migration durable client adapter requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.75' -or
        [string]$TransportManifest.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_durable_client_transition_adapter -or
        'dormant-server-runtime-transition-integration' -cin
            @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport sync manifest lost durable client adapter state'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.75' -or
        [string]$Compatibility.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending' -or
        -not [bool]$Compatibility.required_successor.dormant_durable_client_transition_adapter_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'canonical-sync-v2-instrumented-smoke-verified-sustained-pending') {
        throw 'history compatibility manifest lost durable client adapter state'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) | Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.75' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-DURABLE-CLIENT-ADAPTER.json' -cnotin
            @($transportCoverage[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-durable-client-adapter-boundary.ps1' -cnotin
            @($transportCoverage[0].artifacts) -or
        'docs/RFC-0192-m1166-durable-client-transition-adapter.md' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-durable-client-transition-adapter' -cnotin
            @($transportCoverage[0].covered) -or
        'dormant-server-runtime-transition-integration' -cin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include durable client adapter artifacts'
    }

    foreach ($needle in @(
        'pub fn restore_durable_runtime(',
        'pub fn durable_action(',
        'pub fn begin_durable_round(',
        'pub fn accept_durable_diff(',
        'pub fn mark_durable_batch_sent(',
        'pub fn accept_durable_complete(',
        'CanonicalSyncV2DurableClientAction',
        'commit_session_runtime_transition(',
        'durable_client_transitions_resume_exact_frames_and_complete_two_rounds'
    )) {
        if (-not $Adapter.Contains($needle)) {
            throw "kilogram-session lost durable client adapter source marker: $needle"
        }
    }
    if (-not $Store.Contains('pub fn commit_session_runtime_transition(')) {
        throw 'kilogram-store lost atomic runtime transition prerequisite'
    }
    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('begin_durable_round(') -or
            $inactiveSource.Contains('accept_durable_diff(') -or
            $inactiveSource.Contains('restore_durable_runtime(')) {
            throw 'durable canonical client adapter leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'exact persisted Inventory',
        'commit through the single M1.1.65 transaction',
        'drops and reopens redb',
        'remains unregistered and undialed'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "durable client adapter RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $adapterPath, $storePath, $wirePath, $transportPath, $cliPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 durable client adapter file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$store = Get-Content -LiteralPath $storePath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $adapter $store $wire $transport $cli $rfc

if ($SelfTest) {
    $premature = Copy-JsonObject $manifest
    $premature.activation.durable_server_adapter_integrated = $true
    Assert-Rejected {
        Assert-Contract $premature $migration $transportManifest $compatibility $coverage `
            $adapter $store $wire $transport $cli $rfc
    } 'premature_server_activation'

    $unsafeFallback = Copy-JsonObject $manifest
    $unsafeFallback.contracts.parse_error_fallback = $true
    Assert-Rejected {
        Assert-Contract $unsafeFallback $migration $transportManifest $compatibility $coverage `
            $adapter $store $wire $transport $cli $rfc
    } 'parse_error_fallback'

    $missingExactResume = Copy-JsonObject $manifest
    $missingExactResume.contracts.restart_dispatch_uses_exact_persisted_frame = $false
    Assert-Rejected {
        Assert-Contract $missingExactResume $migration $transportManifest $compatibility $coverage `
            $adapter $store $wire $transport $cli $rfc
    } 'rebuilt_restart_frame'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $adapter $store $wire ($transport + ' begin_durable_round(') $cli $rfc
    } 'adapter_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_durable_client_adapter_self_test=passed'
}

Write-Output 'canonical_sync_v2_durable_client_adapter_boundary=verified'
Write-Output 'client_transitions=inventory+diff+event-batch+complete'
Write-Output 'restart_dispatch=exact-persisted-frame'
Write-Output 'atomic_received_carrier_transition=true'
Write-Output 'durable_server_adapter=false'
Write-Output 'mutual_capability_selection=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
