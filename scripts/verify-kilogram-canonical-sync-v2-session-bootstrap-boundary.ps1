[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-SESSION-BOOTSTRAP.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$sessionPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0188-m1162-dormant-signed-session-bootstrap.md'

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
        [string]$Session,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.62' -or
        [string]$Manifest.status -cne
            'dormant-signed-session-bootstrap-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2' -or
        [string]$Manifest.implementation.storage_crate -cne 'kilogram-store' -or
        [string]$Manifest.implementation.session_crate -cne 'kilogram-session' -or
        [string]$Manifest.implementation.store -cne 'CanonicalSyncV2CarrierStore' -or
        [string]$Manifest.implementation.client -cne 'CanonicalSyncV2Client' -or
        [string]$Manifest.implementation.record_type -cne
            'CanonicalSyncV2StoredSessionCheckpoint' -or
        [string]$Manifest.implementation.signed_checkpoint_type -cne
            'CanonicalSignedSyncSessionCheckpointV2' -or
        [string]$Manifest.implementation.database_engine -cne 'redb' -or
        [string]$Manifest.implementation.journal_table -cne
            'canonical-sync-v2-session-checkpoints-v1' -or
        [string]$Manifest.implementation.head_table -cne
            'canonical-sync-v2-session-checkpoint-heads-v1' -or
        [UInt64]$Manifest.implementation.record_version -ne 1 -or
        [UInt64]$Manifest.implementation.head_version -ne 1) {
        throw 'canonical sync v2 session bootstrap header changed'
    }
    if ((@($Manifest.identity_key) -join ',') -cne
            'conversation-id-32-bytes,local-device-id-32-bytes,peer-device-id-32-bytes,session-binding-32-bytes' -or
        [string]$Manifest.journal_key_suffix -cne
            'checkpoint-sequence-u64-big-endian' -or
        (@($Manifest.journal_record) -join ',') -cne
            'version-u8,legacy-import-plan-id-32-bytes,signed-checkpoint-id-32-bytes,canonical-signed-checkpoint-cbor' -or
        (@($Manifest.head_record) -join ',') -cne
            'version-u8,legacy-import-plan-id-32-bytes,checkpoint-sequence-u64-big-endian,signed-checkpoint-id-32-bytes') {
        throw 'canonical sync v2 session bootstrap fixed format changed'
    }
    foreach ($field in @(
        'complete_exact_legacy_import_required',
        'import_plan_bound_in_local_record',
        'initial_checkpoint_device_signed',
        'exact_cbor_v2_protocol_identifier',
        'initial_checkpoint_sequence_one',
        'initial_sync_round_one',
        'initial_predecessor_absent',
        'initial_event_evidence_empty',
        'initial_availability_false',
        'initial_action_send_inventory',
        'initial_paused_false',
        'immutable_sequence_journal',
        'journal_and_head_same_redb_transaction',
        'immediate_durability',
        'byte_identical_replay_idempotent',
        'head_record_key_signature_and_id_verified',
        'orphan_record_rejected'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 session bootstrap contract '$field' is not verified"
        }
    }
    foreach ($field in @(
        'import_plan_added_to_peer_visible_checkpoint',
        'format_sniffing',
        'parse_error_fallback'
    )) {
        if ([bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 session bootstrap unsafe contract '$field' is enabled"
        }
    }
    foreach ($field in @(
        'missing_import_checkpoint_rejected',
        'incomplete_import_checkpoint_rejected',
        'wrong_initial_shape_rejected',
        'wrong_conversation_rejected',
        'malformed_head_rejected',
        'corrupt_signed_record_rejected',
        'orphan_record_rejected',
        'bootstrap_survives_reopen',
        'exact_bootstrap_replay_idempotent',
        'concrete_client_begins_signed_inventory_after_bootstrap'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 session bootstrap regression '$field' is not verified"
        }
    }
    if (-not [bool]$Manifest.activation.completed_import_to_signed_bootstrap_integrated -or
        [bool]$Manifest.activation.signed_checkpoint_successor_runtime_integrated -or
        [bool]$Manifest.activation.mutual_capability_selection_integrated -or
        [bool]$Manifest.activation.active_runtime_import -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled) {
        throw 'canonical sync v2 session bootstrap activation boundary changed'
    }
    foreach ($field in @(
        'network_executed', 'release_build', 'archive_created', 'stable_executable_replaced'
    )) {
        if ([bool]$Manifest.scope.$field) {
            throw "canonical sync v2 session bootstrap scope '$field' unexpectedly changed"
        }
    }
    foreach ($remaining in @(
        'signed-checkpoint-successor-runtime-integration',
        'mutual-authenticated-capability-selection',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    )) {
        if ($remaining -cnotin @($Manifest.remaining)) {
            throw "canonical sync v2 session bootstrap remaining gate '$remaining' disappeared"
        }
    }

    if ([string]$Migration.milestone -cne 'M1.1.63' -or
        [string]$Migration.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        [string]$Migration.history_compatible_successor.dormant_signed_session_bootstrap -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_signed_session_successor_store -cne
            'verified' -or
        [bool]$Migration.candidate_identifier_registered) {
        throw 'canonical migration lost the dormant signed session bootstrap'
    }
    $bootstrapRequirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-signed-session-bootstrap' }
    )
    if ($bootstrapRequirement.Count -ne 1 -or
        [string]$bootstrapRequirement[0].status -cne 'verified') {
        throw 'canonical migration lost the verified signed session bootstrap requirement'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.63' -or
        [string]$TransportManifest.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_successor_store -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        'signed-checkpoint-runtime-transition-integration' -cnotin
            @($TransportManifest.remaining) -or
        'mutual-authenticated-capability-selection' -cnotin
            @($TransportManifest.remaining)) {
        throw 'transport sync manifest lost the dormant signed session bootstrap boundary'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.63' -or
        [string]$Compatibility.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_bootstrap_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_successor_store_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-policy-import-checkpoint-session-bootstrap-and-successor-store-verified-not-active') {
        throw 'history compatibility manifest lost the signed session bootstrap contract'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) | Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.63' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-SESSION-BOOTSTRAP.json' -cnotin
            @($transportCoverage[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-session-bootstrap-boundary.ps1' -cnotin
            @($transportCoverage[0].artifacts) -or
        'docs/RFC-0188-m1162-dormant-signed-session-bootstrap.md' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-signed-session-bootstrap' -cnotin
            @($transportCoverage[0].covered) -or
        'signed-checkpoint-runtime-transition-integration' -cnotin
            @($transportCoverage[0].remaining) -or
        'mutual-authenticated-capability-selection' -cnotin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include signed session bootstrap artifacts'
    }

    foreach ($needle in @(
        'pub struct CanonicalSyncV2StoredSessionCheckpoint',
        'canonical-sync-v2-session-checkpoints-v1',
        'canonical-sync-v2-session-checkpoint-heads-v1',
        'pub fn session_checkpoint_head(',
        'pub fn install_legacy_import_session_bootstrap(',
        'validate_canonical_sync_v2_legacy_import_session_bootstrap',
        'CanonicalSyncV2SessionBootstrapImportMissing',
        'CanonicalSyncV2SessionBootstrapImportIncomplete',
        'CanonicalSyncV2SessionCheckpointOrphanRecord',
        'canonical_sync_v2_session_bootstrap_requires_complete_import_and_survives_reopen',
        'canonical_sync_v2_session_checkpoint_corruption_and_orphans_fail_closed'
    )) {
        if (-not $Store.Contains($needle)) {
            throw "kilogram-store lost signed session bootstrap source marker: $needle"
        }
    }
    foreach ($needle in @(
        'pub fn bootstrap_from_completed_legacy_import(',
        'CanonicalSyncNextActionV2::SendInventory',
        'store.install_legacy_import_session_bootstrap(plan, &checkpoint)?',
        'concrete_client_bootstrap_requires_the_completed_exact_legacy_import'
    )) {
        if (-not $Session.Contains($needle)) {
            throw "kilogram-session lost signed session bootstrap source marker: $needle"
        }
    }
    foreach ($activeSource in @($Wire, $Transport, $Cli)) {
        if ($activeSource.Contains('bootstrap_from_completed_legacy_import') -or
            $activeSource.Contains('CanonicalSyncV2StoredSessionCheckpoint')) {
            throw 'dormant signed session bootstrap leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'cannot enter the new local session through the gated constructor before its exact import is complete',
        'Only checkpoint sequence 1 is installed here',
        'public dormant state-machine constructor remains available',
        '`/cbor/2` remains unregistered and undialed'
    )) {
        if (-not $normalizedRfc.Contains($needle)) {
            throw "RFC-0188 lost required signed session bootstrap statement: $needle"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $storePath, $sessionPath, $wirePath, $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 session bootstrap boundary file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$store = Get-Content -LiteralPath $storePath -Raw
$session = Get-Content -LiteralPath $sessionPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ($SelfTest) {
    $premature = Copy-JsonObject $manifest
    $premature.contracts.complete_exact_legacy_import_required = $false
    Assert-Rejected {
        Assert-Contract $premature $migration $transportManifest $compatibility $coverage `
            $store $session $wire $transport $cli $rfc
    } 'premature_session_bootstrap'

    $peerVisiblePlan = Copy-JsonObject $manifest
    $peerVisiblePlan.contracts.import_plan_added_to_peer_visible_checkpoint = $true
    Assert-Rejected {
        Assert-Contract $peerVisiblePlan $migration $transportManifest $compatibility $coverage `
            $store $session $wire $transport $cli $rfc
    } 'invented_peer_visible_import_plan'

    $prematureRuntime = Copy-JsonObject $manifest
    $prematureRuntime.activation.signed_checkpoint_successor_runtime_integrated = $true
    Assert-Rejected {
        Assert-Contract $prematureRuntime $migration $transportManifest $compatibility $coverage `
            $store $session $wire $transport $cli $rfc
    } 'premature_checkpoint_successor_runtime'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $store $session $wire $transport `
            ($cli + ' bootstrap_from_completed_legacy_import') $rfc
    } 'session_bootstrap_leaked_into_active_cli'

    Write-Output 'canonical_sync_v2_session_bootstrap_self_test=passed'
}

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $store $session $wire $transport $cli $rfc
Write-Output 'canonical_sync_v2_session_bootstrap_boundary=verified'
Write-Output 'complete_exact_legacy_import_required=true'
Write-Output 'initial_checkpoint=device-signed-sequence-1'
Write-Output 'journal_and_head_transaction=atomic'
Write-Output 'peer_visible_import_plan=false'
Write-Output 'signed_checkpoint_successor_runtime=false'
Write-Output 'mutual_capability_selection=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
