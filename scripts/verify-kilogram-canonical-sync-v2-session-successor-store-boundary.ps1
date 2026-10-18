[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-SESSION-SUCCESSOR-STORE.json'
$bootstrapPath = Join-Path $workspace 'CANONICAL-SYNC-V2-SESSION-BOOTSTRAP.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0189-m1163-dormant-signed-session-successor-store.md'

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
        [object]$Bootstrap,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$Store,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.63' -or
        [string]$Manifest.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'canonical sync v2 successor store header changed'
    }
    if ([string]$Manifest.implementation.crate -cne 'kilogram-store' -or
        [string]$Manifest.implementation.store -cne 'CanonicalSyncV2CarrierStore' -or
        [string]$Manifest.implementation.append_api -cne
            'append_session_checkpoint_successor' -or
        [string]$Manifest.implementation.restore_api -cne
            'session_checkpoint_chain' -or
        [UInt64]$Manifest.implementation.maximum_checkpoints_per_session -ne 65536) {
        throw 'canonical sync v2 successor store implementation contract changed'
    }
    foreach ($field in @(
        'device_signature_verified',
        'session_identity_immutable',
        'sequence_exactly_increments',
        'predecessor_exact_checkpoint_id',
        'sync_round_transition_verified',
        'typed_event_evidence_monotonic',
        'next_action_transition_verified',
        'pause_resume_transition_verified',
        'terminal_successor_rejected',
        'current_durable_head_required',
        'import_plan_binding_inherited_from_head',
        'immutable_sequence_journal',
        'successor_and_head_same_redb_transaction',
        'immediate_durability',
        'exact_successor_replay_idempotent',
        'complete_chain_verified_on_restore',
        'initial_bootstrap_shape_reverified_on_restore',
        'missing_intermediate_record_rejected',
        'corrupt_ancestry_rejected'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 successor store contract '$field' is not verified"
        }
    }
    foreach ($field in @('format_sniffing', 'parse_error_fallback')) {
        if ([bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 successor store unsafe contract '$field' is enabled"
        }
    }
    foreach ($field in @(
        'three_checkpoint_chain_restored_after_reopen',
        'exact_successor_replay_idempotent',
        'skipped_sequence_rejected',
        'same_sequence_fork_rejected',
        'corrupt_ancestor_rejected',
        'missing_session_head_rejected'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 successor store regression '$field' is not verified"
        }
    }
    if (-not [bool]$Manifest.activation.signed_successor_store_integrated -or
        [bool]$Manifest.activation.runtime_transition_checkpointing_integrated -or
        [bool]$Manifest.activation.runtime_resume_from_checkpoint_integrated -or
        [bool]$Manifest.activation.mutual_capability_selection_integrated -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled) {
        throw 'canonical sync v2 successor store activation boundary changed'
    }
    if ([string]$Bootstrap.status -cne
            'dormant-signed-session-bootstrap-verified-not-active' -or
        [bool]$Bootstrap.activation.signed_checkpoint_successor_runtime_integrated -or
        [bool]$Bootstrap.activation.mutual_capability_selection_integrated) {
        throw 'initial session bootstrap boundary changed prematurely'
    }
    if ([string]$Migration.milestone -cne 'M1.1.85' -or
        [string]$Migration.status -cne
            'canonical-sync-v2-opt-in-dark-launch-verified' -or
        [string]$Migration.history_compatible_successor.dormant_signed_session_successor_store -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_checkpoint_bound_runtime_frame_journal -cne
            'verified' -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration lost dormant successor store state'
    }
    $successorRequirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-signed-session-successor-store' }
    )
    if ($successorRequirement.Count -ne 1 -or
        [string]$successorRequirement[0].status -cne 'verified') {
        throw 'canonical migration successor store requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.85' -or
        [string]$TransportManifest.status -cne
            'canonical-sync-v2-opt-in-dark-launch-verified' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_successor_store -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_checkpoint_bound_runtime_frame_journal -or
        'dormant-server-runtime-transition-integration' -cin
            @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport sync manifest lost dormant successor store state'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.85' -or
        [string]$Compatibility.status -cne
            'canonical-sync-v2-opt-in-dark-launch-verified' -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_successor_store_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_checkpoint_bound_runtime_frame_journal_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'canonical-sync-v2-opt-in-dark-launch-verified') {
        throw 'history compatibility manifest lost dormant successor store state'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) | Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.85' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-SESSION-SUCCESSOR-STORE.json' -cnotin
            @($transportCoverage[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-session-successor-store-boundary.ps1' -cnotin
            @($transportCoverage[0].artifacts) -or
        'docs/RFC-0189-m1163-dormant-signed-session-successor-store.md' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-signed-session-successor-store' -cnotin
            @($transportCoverage[0].covered) -or
        'dormant-server-runtime-transition-integration' -cin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include dormant successor store artifacts'
    }

    foreach ($needle in @(
        'pub const MAX_CANONICAL_SYNC_V2_SESSION_CHECKPOINTS: u64 = 65_536;',
        'pub fn session_checkpoint_chain(',
        'pub fn append_session_checkpoint_successor(',
        'verify_sync_session_checkpoint_successor_v2(',
        'CanonicalSyncV2SessionCheckpointChainTooLong',
        'CanonicalSyncV2SessionCheckpointChainMissingRecord',
        'canonical_sync_v2_session_successors_restore_the_complete_chain_after_reopen',
        'canonical_sync_v2_session_successor_forks_gaps_and_corrupt_ancestry_fail_closed'
    )) {
        if (-not $Store.Contains($needle)) {
            throw "kilogram-store lost successor store source marker: $needle"
        }
    }
    foreach ($activeSource in @($Wire, $Transport, $Cli)) {
        if ($activeSource.Contains('append_session_checkpoint_successor') -or
            $activeSource.Contains('session_checkpoint_chain')) {
            throw 'dormant successor store leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'complete-chain restore',
        'written in one redb transaction with immediate durability',
        'does not create successors at every frame transition',
        'remains unregistered and undialed'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "successor store RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $bootstrapPath, $migrationPath, $transportManifestPath,
    $compatibilityPath, $coveragePath, $storePath, $wirePath, $transportPath,
    $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 successor store file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$bootstrap = Get-Content -LiteralPath $bootstrapPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$store = Get-Content -LiteralPath $storePath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $bootstrap $migration $transportManifest $compatibility `
    $coverage $store $wire $transport $cli $rfc

if ($SelfTest) {
    $runtime = Copy-JsonObject $manifest
    $runtime.activation.runtime_transition_checkpointing_integrated = $true
    Assert-Rejected {
        Assert-Contract $runtime $bootstrap $migration $transportManifest $compatibility `
            $coverage $store $wire $transport $cli $rfc
    } 'premature_runtime_transition_checkpointing'

    $fallback = Copy-JsonObject $manifest
    $fallback.contracts.parse_error_fallback = $true
    Assert-Rejected {
        Assert-Contract $fallback $bootstrap $migration $transportManifest $compatibility `
            $coverage $store $wire $transport $cli $rfc
    } 'parse_error_fallback'

    $unbounded = Copy-JsonObject $manifest
    $unbounded.implementation.maximum_checkpoints_per_session = 0
    Assert-Rejected {
        Assert-Contract $unbounded $bootstrap $migration $transportManifest $compatibility `
            $coverage $store $wire $transport $cli $rfc
    } 'unbounded_checkpoint_chain'

    Assert-Rejected {
        Assert-Contract $manifest $bootstrap $migration $transportManifest $compatibility `
            $coverage $store $wire $transport ($cli + ' session_checkpoint_chain') $rfc
    } 'successor_store_leaked_into_active_cli'

    Write-Output 'canonical_sync_v2_session_successor_store_self_test=passed'
}

Write-Output 'canonical_sync_v2_session_successor_store_boundary=verified'
Write-Output 'successor_append=atomic-immediate-durability'
Write-Output 'restore=complete-signed-chain'
Write-Output 'maximum_checkpoints_per_session=65536'
Write-Output 'runtime_transition_checkpointing=false'
Write-Output 'mutual_capability_selection=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
