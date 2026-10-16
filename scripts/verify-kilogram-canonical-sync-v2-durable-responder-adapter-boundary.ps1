[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DURABLE-RESPONDER-ADAPTER.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_sync_responder_session_v2.rs'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\canonical_sync_v2_responder_store.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0194-m1167-durable-responder-transition-adapter.md'

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
        [string]$Protocol,
        [string]$Adapter,
        [string]$Store,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.67' -or
        [string]$Manifest.status -cne
            'dormant-durable-responder-transition-adapter-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'canonical sync v2 durable responder adapter header changed'
    }
    if ([string]$Manifest.implementation.checkpoint -cne
            'CanonicalSignedSyncResponderCheckpointV2' -or
        [string]$Manifest.implementation.adapter -cne
            'CanonicalSyncV2Server<CanonicalSyncV2CarrierStore>' -or
        [string]$Manifest.implementation.durable_state -cne
            'CanonicalSyncV2StoredResponderRuntime' -or
        [string]$Manifest.implementation.atomic_transition -cne
            'CanonicalSyncV2CarrierStore::commit_responder_runtime_transition') {
        throw 'canonical sync v2 durable responder implementation changed'
    }
    foreach ($field in @(
        'responder_checkpoint_signature_domain_is_separate',
        'responder_tables_are_separate_from_initiator_tables',
        'bootstrap_requires_completed_exact_legacy_import',
        'diff_persisted_before_transport_write',
        'receive_event_batch_preserves_exact_diff_for_retry',
        'batch_verification_precedes_storage',
        'received_carriers_checkpoint_head_and_complete_frame_commit_atomically',
        'complete_persisted_before_transport_write',
        'continuation_advances_exactly_one_round',
        'terminal_completion_clears_pending_frame',
        'restart_dispatch_uses_exact_persisted_frame',
        'unexpected_action_or_frame_fails_closed',
        'exact_lost_response_retry_idempotent'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "durable responder contract '$field' is not verified"
        }
    }
    foreach ($field in @('format_sniffing', 'parse_error_fallback')) {
        if ([bool]$Manifest.contracts.$field) {
            throw "unsafe responder contract '$field' is enabled"
        }
    }
    if (-not [bool]$Manifest.activation.durable_client_adapter_integrated -or
        -not [bool]$Manifest.activation.durable_responder_adapter_integrated -or
        [bool]$Manifest.activation.mutual_capability_selection_integrated -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled -or
        'mutual-authenticated-capability-selection' -cnotin @($Manifest.remaining) -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'canonical sync v2 responder activation boundary changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.69' -or
        [string]$Migration.status -cne
            'dormant-mixed-version-and-downgrade-matrix-verified-not-active' -or
        [string]$Migration.history_compatible_successor.dormant_durable_responder_transition_adapter -cne
            'verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration lost durable responder state'
    }
    $requirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-durable-responder-transition-adapter' }
    )
    if ($requirement.Count -ne 1 -or [string]$requirement[0].status -cne 'verified') {
        throw 'canonical migration durable responder requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.69' -or
        [string]$TransportManifest.status -cne
            'dormant-mixed-version-and-downgrade-matrix-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_durable_responder_transition_adapter -or
        'dormant-server-runtime-transition-integration' -cin @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport sync manifest lost durable responder state'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.69' -or
        [string]$Compatibility.status -cne
            'dormant-mixed-version-and-downgrade-matrix-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.dormant_durable_responder_transition_adapter_integrated) {
        throw 'history compatibility manifest lost durable responder state'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) | Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.69' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-DURABLE-RESPONDER-ADAPTER.json' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-durable-responder-transition-adapter' -cnotin
            @($transportCoverage[0].covered) -or
        'dormant-server-runtime-transition-integration' -cin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include durable responder artifacts'
    }

    foreach ($needle in @(
        'CANONICAL_SYNC_RESPONDER_CHECKPOINT_SIGNATURE_DOMAIN_V2',
        'kilogram:sync-responder-checkpoint-signature:cbor-v2\0',
        'verify_sync_responder_checkpoint_successor_v2',
        'CanonicalSyncResponderNextActionV2::ReceiveInventory'
    )) {
        if (-not $Protocol.Contains($needle)) {
            throw "responder checkpoint source marker missing: $needle"
        }
    }
    foreach ($needle in @(
        'pub fn responder_runtime_state(',
        'pub fn install_legacy_import_responder_bootstrap(',
        'pub fn commit_responder_runtime_transition(',
        'CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE'
    )) {
        if (-not $Store.Contains($needle)) {
            throw "responder store source marker missing: $needle"
        }
    }
    foreach ($needle in @(
        'pub fn restore_durable_responder_runtime(',
        'pub fn durable_responder_action(',
        'pub fn accept_durable_inventory(',
        'pub fn mark_durable_diff_sent(',
        'pub fn accept_durable_event_batch(',
        'pub fn mark_durable_complete_sent(',
        'durable_responder_transitions_resume_exact_frames_and_commit_batch_atomically'
    )) {
        if (-not $Adapter.Contains($needle)) {
            throw "responder adapter source marker missing: $needle"
        }
    }
    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('durable_responder_action(') -or
            $inactiveSource.Contains('accept_durable_inventory(') -or
            $inactiveSource.Contains('restore_durable_responder_runtime(')) {
            throw 'durable canonical responder leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'separate signature and identifier domains',
        'one immediate redb transaction',
        'drops and reopens redb',
        'remains unregistered and undialed'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "durable responder RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $protocolPath, $adapterPath, $storePath, $wirePath,
    $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 durable responder file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$protocol = Get-Content -LiteralPath $protocolPath -Raw
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$store = Get-Content -LiteralPath $storePath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $protocol $adapter $store $wire $transport $cli $rfc

if ($SelfTest) {
    $premature = Copy-JsonObject $manifest
    $premature.activation.mutual_capability_selection_integrated = $true
    Assert-Rejected {
        Assert-Contract $premature $migration $transportManifest $compatibility $coverage `
            $protocol $adapter $store $wire $transport $cli $rfc
    } 'premature_capability_activation'

    $sharedDomain = $protocol.Replace(
        'kilogram:sync-responder-checkpoint-signature:cbor-v2\0',
        'kilogram:sync-session-checkpoint-signature:cbor-v2\0'
    )
    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $sharedDomain $adapter $store $wire $transport $cli $rfc
    } 'shared_checkpoint_signature_domain'

    $unsafeFallback = Copy-JsonObject $manifest
    $unsafeFallback.contracts.parse_error_fallback = $true
    Assert-Rejected {
        Assert-Contract $unsafeFallback $migration $transportManifest $compatibility $coverage `
            $protocol $adapter $store $wire $transport $cli $rfc
    } 'parse_error_fallback'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $protocol $adapter $store $wire ($transport + ' durable_responder_action(') $cli $rfc
    } 'adapter_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_durable_responder_adapter_self_test=passed'
}

Write-Output 'canonical_sync_v2_durable_responder_adapter_boundary=verified'
Write-Output 'responder_transitions=inventory+diff+event-batch+complete'
Write-Output 'restart_dispatch=exact-persisted-server-frame'
Write-Output 'atomic_received_carrier_transition=true'
Write-Output 'mutual_capability_selection=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
