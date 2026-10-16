[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-SESSION-RUNTIME-FRAME.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0190-m1164-checkpoint-bound-runtime-frame-journal.md'

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
        [string]$Manifest.milestone -cne 'M1.1.64' -or
        [string]$Manifest.status -cne
            'dormant-checkpoint-bound-runtime-frame-journal-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'canonical sync v2 session runtime frame header changed'
    }
    if ([string]$Manifest.implementation.crate -cne 'kilogram-store' -or
        [string]$Manifest.implementation.store -cne 'CanonicalSyncV2CarrierStore' -or
        [string]$Manifest.implementation.runtime_type -cne
            'CanonicalSyncV2StoredSessionRuntime' -or
        [string]$Manifest.implementation.runtime_table -cne
            'canonical-sync-v2-session-runtime-v1' -or
        [string]$Manifest.implementation.restore_api -cne 'session_runtime_state' -or
        [string]$Manifest.implementation.install_frame_api -cne
            'set_session_pending_client_frame' -or
        [UInt64]$Manifest.implementation.maximum_pending_frame_bytes -ne 8388608) {
        throw 'canonical sync v2 session runtime frame implementation contract changed'
    }
    foreach ($field in @(
        'runtime_record_bound_to_exact_checkpoint_id',
        'pending_frame_uses_closed_canonical_client_frame_codec',
        'pending_frame_reverified_on_restore',
        'pending_frame_conversation_binding_verified',
        'inventory_local_device_binding_verified',
        'inventory_session_binding_verified',
        'runtime_record_created_with_session_bootstrap',
        'runtime_record_rebound_with_successor_and_head_transaction',
        'runtime_frame_write_once_per_checkpoint',
        'exact_frame_retry_idempotent',
        'different_frame_same_checkpoint_rejected',
        'stale_checkpoint_update_rejected',
        'missing_or_orphan_runtime_record_rejected',
        'malformed_runtime_record_rejected',
        'legacy_bootstrap_exact_replay_backfills_empty_runtime_record'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 session runtime frame contract '$field' is not verified"
        }
    }
    foreach ($field in @('format_sniffing', 'parse_error_fallback')) {
        if ([bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 session runtime frame unsafe contract '$field' is enabled"
        }
    }
    foreach ($field in @(
        'empty_bootstrap_runtime_restored',
        'pending_inventory_survives_reopen',
        'successor_rebind_preserves_exact_frame',
        'exact_frame_replay_idempotent',
        'different_frame_conflict_rejected',
        'stale_checkpoint_update_rejected',
        'cross_session_frame_rejected',
        'corrupt_checkpoint_binding_rejected',
        'invalid_presence_marker_rejected',
        'missing_runtime_record_rejected'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 session runtime frame regression '$field' is not verified"
        }
    }
    if (-not [bool]$Manifest.activation.checkpoint_bound_runtime_frame_journal_integrated -or
        [bool]$Manifest.activation.runtime_transition_checkpointing_integrated -or
        [bool]$Manifest.activation.runtime_resume_from_checkpoint_integrated -or
        [bool]$Manifest.activation.mutual_capability_selection_integrated -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled) {
        throw 'canonical sync v2 session runtime frame activation boundary changed'
    }
    if ('signed-checkpoint-runtime-transition-integration' -cnotin @($Manifest.remaining) -or
        'atomic-received-carrier-and-checkpoint-transition' -cnotin @($Manifest.remaining) -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'canonical sync v2 session runtime frame scope changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.64' -or
        [string]$Migration.status -cne
            'dormant-checkpoint-bound-runtime-frame-journal-verified-not-active' -or
        [string]$Migration.history_compatible_successor.dormant_checkpoint_bound_runtime_frame_journal -cne
            'verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration lost checkpoint-bound runtime frame state'
    }
    $runtimeRequirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-checkpoint-bound-runtime-frame-journal' }
    )
    if ($runtimeRequirement.Count -ne 1 -or
        [string]$runtimeRequirement[0].status -cne 'verified') {
        throw 'canonical migration runtime frame requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.64' -or
        [string]$TransportManifest.status -cne
            'dormant-checkpoint-bound-runtime-frame-journal-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_checkpoint_bound_runtime_frame_journal -or
        'signed-checkpoint-runtime-transition-integration' -cnotin
            @($TransportManifest.remaining) -or
        'atomic-received-carrier-and-checkpoint-transition' -cnotin
            @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport sync manifest lost checkpoint-bound runtime frame state'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.64' -or
        [string]$Compatibility.status -cne
            'dormant-checkpoint-bound-runtime-frame-journal-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.dormant_checkpoint_bound_runtime_frame_journal_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-policy-import-checkpoint-session-bootstrap-successor-store-and-runtime-frame-journal-verified-not-active') {
        throw 'history compatibility manifest lost checkpoint-bound runtime frame state'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) | Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.64' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-SESSION-RUNTIME-FRAME.json' -cnotin
            @($transportCoverage[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-session-runtime-frame-boundary.ps1' -cnotin
            @($transportCoverage[0].artifacts) -or
        'docs/RFC-0190-m1164-checkpoint-bound-runtime-frame-journal.md' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-checkpoint-bound-runtime-frame-journal' -cnotin
            @($transportCoverage[0].covered) -or
        'signed-checkpoint-runtime-transition-integration' -cnotin
            @($transportCoverage[0].remaining) -or
        'atomic-received-carrier-and-checkpoint-transition' -cnotin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include checkpoint-bound runtime frame artifacts'
    }

    foreach ($needle in @(
        'TableDefinition::new("canonical-sync-v2-session-runtime-v1")',
        'pub struct CanonicalSyncV2StoredSessionRuntime',
        'pub fn session_runtime_state(',
        'pub fn set_session_pending_client_frame(',
        'fn encode_canonical_sync_v2_session_runtime_record(',
        'fn decode_canonical_sync_v2_session_runtime_record(',
        'decode_and_verify_sync_client_frame_v2(',
        'canonical_sync_v2_session_runtime_frame_is_checkpoint_bound_and_survives_reopen',
        'canonical_sync_v2_session_runtime_corruption_and_conflicts_fail_closed'
    )) {
        if (-not $Store.Contains($needle)) {
            throw "kilogram-store lost session runtime frame source marker: $needle"
        }
    }
    foreach ($inactiveSource in @($Adapter, $Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('set_session_pending_client_frame') -or
            $inactiveSource.Contains('session_runtime_state') -or
            $inactiveSource.Contains('canonical-sync-v2-session-runtime-v1')) {
            throw 'checkpoint-bound runtime frame journal leaked into runtime/wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'exact signed inventory',
        'write-once for the current checkpoint',
        'does not yet drive the state machine',
        'remains unregistered and undialed'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "session runtime frame RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $storePath, $adapterPath, $wirePath, $transportPath, $cliPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 session runtime frame file is missing: $path"
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
    $runtime = Copy-JsonObject $manifest
    $runtime.activation.runtime_transition_checkpointing_integrated = $true
    Assert-Rejected {
        Assert-Contract $runtime $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'premature_runtime_transition_checkpointing'

    $fallback = Copy-JsonObject $manifest
    $fallback.contracts.parse_error_fallback = $true
    Assert-Rejected {
        Assert-Contract $fallback $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'parse_error_fallback'

    $unbounded = Copy-JsonObject $manifest
    $unbounded.implementation.maximum_pending_frame_bytes = 0
    Assert-Rejected {
        Assert-Contract $unbounded $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'unbounded_pending_frame'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $store ($adapter + ' set_session_pending_client_frame') $wire $transport $cli $rfc
    } 'runtime_frame_journal_leaked_into_adapter'

    Write-Output 'canonical_sync_v2_session_runtime_frame_self_test=passed'
}

Write-Output 'canonical_sync_v2_session_runtime_frame_boundary=verified'
Write-Output 'runtime_frame_binding=exact-signed-checkpoint-id'
Write-Output 'pending_frame_codec=closed-canonical-client-frame-v2'
Write-Output 'maximum_pending_frame_bytes=8388608'
Write-Output 'runtime_transition_checkpointing=false'
Write-Output 'runtime_resume_from_checkpoint=false'
Write-Output 'mutual_capability_selection=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
