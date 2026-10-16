[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-IMPORT-CHECKPOINT.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0187-m1161-dormant-atomic-legacy-import-checkpoint.md'

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
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.61' -or
        [string]$Manifest.status -cne
            'dormant-atomic-legacy-import-checkpoint-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2' -or
        [string]$Manifest.implementation.crate -cne 'kilogram-store' -or
        [string]$Manifest.implementation.store -cne 'CanonicalSyncV2CarrierStore' -or
        [string]$Manifest.implementation.record_type -cne
            'CanonicalSyncV2LegacyImportCheckpoint' -or
        [string]$Manifest.implementation.database_engine -cne 'redb' -or
        [string]$Manifest.implementation.table -cne
            'canonical-sync-v2-legacy-import-checkpoints-v1' -or
        [UInt64]$Manifest.implementation.record_version -ne 1 -or
        [UInt64]$Manifest.implementation.record_bytes -ne 129) {
        throw 'canonical sync v2 import checkpoint header changed'
    }
    if ((@($Manifest.key) -join ',') -cne
            'conversation-id-32-bytes,legacy-import-plan-id-32-bytes' -or
        (@($Manifest.record) -join ',') -cne
            'version-u8,conversation-id-32-bytes,legacy-import-plan-id-32-bytes,membership-revision-u64-big-endian,membership-digest-32-bytes,total-event-count-u64-big-endian,last-page-start-u64-big-endian,next-index-u64-big-endian') {
        throw 'canonical sync v2 import checkpoint fixed format changed'
    }
    foreach ($field in @(
        'exact_plan_scoped',
        'strict_fixed_length_decode',
        'key_and_value_plan_binding_verified',
        'next_index_bounded_by_plan',
        'carrier_page_and_checkpoint_same_redb_transaction',
        'immediate_durability',
        'durable_cursor_compared_before_advance',
        'last_committed_page_start_bound',
        'stale_or_skipped_cursor_rejected',
        'same_cursor_lost_response_retry_idempotent',
        'page_conflict_does_not_advance_checkpoint',
        'parallel_immutable_plans_have_distinct_keys'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 import checkpoint contract '$field' is not verified"
        }
    }
    foreach ($field in @(
        'signed_sync_checkpoint_synthesized',
        'format_sniffing',
        'parse_error_fallback'
    )) {
        if ([bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 import checkpoint unsafe contract '$field' is enabled"
        }
    }
    foreach ($field in @(
        'checkpoint_survives_reopen',
        'resume_uses_durable_cursor',
        'stale_page_shape_rejected',
        'alternate_same_end_page_shape_rejected',
        'missing_checkpoint_cannot_skip_prefix',
        'checkpoint_corruption_rejected',
        'page_conflict_retains_previous_checkpoint',
        'later_page_items_absent_after_conflict',
        'same_cursor_retry_returns_same_progress'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 import checkpoint regression '$field' is not verified"
        }
    }
    if (-not [bool]$Manifest.activation.durable_local_import_checkpoint_integrated -or
        [bool]$Manifest.activation.signed_sync_checkpoint_runtime_integrated -or
        [bool]$Manifest.activation.active_runtime_import -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled) {
        throw 'canonical sync v2 import checkpoint activation boundary changed'
    }
    foreach ($field in @(
        'network_executed', 'release_build', 'archive_created', 'stable_executable_replaced'
    )) {
        if ([bool]$Manifest.scope.$field) {
            throw "canonical sync v2 import checkpoint scope '$field' unexpectedly changed"
        }
    }
    foreach ($remaining in @(
        'signed-sync-checkpoint-runtime-integration',
        'mutual-authenticated-capability-selection',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    )) {
        if ($remaining -cnotin @($Manifest.remaining)) {
            throw "canonical sync v2 import checkpoint remaining gate '$remaining' disappeared"
        }
    }

    if ([string]$Migration.milestone -cne 'M1.1.62' -or
        [string]$Migration.status -cne
            'dormant-signed-session-bootstrap-verified-not-active' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -cne
            'verified' -or
        [bool]$Migration.candidate_identifier_registered) {
        throw 'canonical migration lost the dormant atomic import checkpoint'
    }
    $checkpointRequirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-atomic-legacy-import-checkpoint' }
    )
    if ($checkpointRequirement.Count -ne 1 -or
        [string]$checkpointRequirement[0].status -cne 'verified') {
        throw 'canonical migration lost the verified atomic import checkpoint requirement'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.62' -or
        [string]$TransportManifest.status -cne
            'dormant-signed-session-bootstrap-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        'signed-checkpoint-successor-runtime-integration' -cnotin
            @($TransportManifest.remaining) -or
        'mutual-authenticated-capability-selection' -cnotin
            @($TransportManifest.remaining)) {
        throw 'transport sync manifest lost the dormant atomic import checkpoint boundary'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.62' -or
        [string]$Compatibility.status -cne
            'dormant-signed-session-bootstrap-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_legacy_import_checkpoint_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_bootstrap_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-policy-import-checkpoint-and-session-bootstrap-verified-not-active') {
        throw 'history compatibility manifest lost the atomic import checkpoint contract'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) | Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.62' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-IMPORT-CHECKPOINT.json' -cnotin
            @($transportCoverage[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-import-checkpoint-boundary.ps1' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-atomic-legacy-import-checkpoint' -cnotin
            @($transportCoverage[0].covered) -or
        'dormant-signed-session-bootstrap' -cnotin
            @($transportCoverage[0].covered) -or
        'signed-checkpoint-successor-runtime-integration' -cnotin
            @($transportCoverage[0].remaining) -or
        'mutual-authenticated-capability-selection' -cnotin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include atomic import checkpoint artifacts'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'pub struct CanonicalSyncV2LegacyImportCheckpoint',
        'canonical-sync-v2-legacy-import-checkpoints-v1',
        'CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_BYTES: usize = 129',
        'pub fn legacy_import_checkpoint(',
        'fn commit_legacy_import_page(',
        'fn canonical_sync_v2_legacy_import_checkpoint_key(',
        'fn encode_canonical_sync_v2_legacy_import_checkpoint(',
        'fn decode_canonical_sync_v2_legacy_import_checkpoint(',
        'self.commit_legacy_import_page(plan, cursor, next_index, &carriers)?',
        'CanonicalSyncV2LegacyImportDurableCursorMismatch',
        'canonical_sync_v2_legacy_import_checkpoint_survives_reopen_and_rejects_stale_pages',
        'canonical_sync_v2_legacy_import_checkpoint_corruption_fails_closed',
        'canonical_sync_v2_legacy_import_conflict_does_not_commit_later_page_items'
    )) {
        if (-not $Store.Contains($needle)) {
            throw "kilogram-store lost atomic import checkpoint source marker: $needle"
        }
    }
    foreach ($activeSource in @($Wire, $Transport, $Cli)) {
        if ($activeSource.Contains('CanonicalSyncV2LegacyImportCheckpoint') -or
            $activeSource.Contains('commit_legacy_import_page')) {
            throw 'dormant import checkpoint leaked into active wire/transport/CLI code'
        }
    }
    foreach ($needle in @(
        'same redb database as the typed carriers',
        'Only then is the transaction committed',
        'exact lost-response retry',
        'not the Device-signed',
        '`/cbor/2` remains unregistered and undialed'
    )) {
        if (-not $normalizedRfc.Contains($needle)) {
            throw "RFC-0187 lost required atomic checkpoint statement: $needle"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $storePath, $wirePath, $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 import checkpoint boundary file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$store = Get-Content -LiteralPath $storePath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ($SelfTest) {
    $nonAtomic = Copy-JsonObject $manifest
    $nonAtomic.contracts.carrier_page_and_checkpoint_same_redb_transaction = $false
    Assert-Rejected {
        Assert-Contract $nonAtomic $migration $transportManifest $compatibility $coverage `
            $store $wire $transport $cli $rfc
    } 'non_atomic_import_checkpoint'

    $skippable = Copy-JsonObject $manifest
    $skippable.contracts.stale_or_skipped_cursor_rejected = $false
    Assert-Rejected {
        Assert-Contract $skippable $migration $transportManifest $compatibility $coverage `
            $store $wire $transport $cli $rfc
    } 'skippable_durable_import_cursor'

    $prematureSignedCheckpoint = Copy-JsonObject $manifest
    $prematureSignedCheckpoint.activation.signed_sync_checkpoint_runtime_integrated = $true
    Assert-Rejected {
        Assert-Contract $prematureSignedCheckpoint $migration $transportManifest `
            $compatibility $coverage $store $wire $transport $cli $rfc
    } 'premature_signed_sync_checkpoint_integration'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $store $wire $transport ($cli + ' CanonicalSyncV2LegacyImportCheckpoint') $rfc
    } 'import_checkpoint_leaked_into_active_cli'

    Write-Output 'canonical_sync_v2_import_checkpoint_self_test=passed'
}

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $store $wire $transport $cli $rfc
Write-Output 'canonical_sync_v2_import_checkpoint_boundary=verified'
Write-Output 'checkpoint_key=conversation-and-plan'
Write-Output 'checkpoint_record_bytes=129'
Write-Output 'carrier_page_and_checkpoint_transaction=atomic'
Write-Output 'same_cursor_lost_response_retry=idempotent'
Write-Output 'signed_sync_checkpoint_runtime=false'
Write-Output 'active_runtime_import=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
