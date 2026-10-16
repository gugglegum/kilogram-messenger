[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-LEGACY-IMPORT.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$storeCargoPath = Join-Path $workspace 'crates\kilogram-store\Cargo.toml'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0186-m1160-dormant-bounded-legacy-history-import.md'

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
        [string]$StoreCargo,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.60' -or
        [string]$Manifest.status -cne
            'dormant-bounded-resumable-legacy-import-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2' -or
        [string]$Manifest.implementation.crate -cne 'kilogram-store' -or
        [string]$Manifest.implementation.source_store -cne 'EventStore' -or
        [string]$Manifest.implementation.target_store -cne 'CanonicalSyncV2CarrierStore' -or
        [string]$Manifest.implementation.plan_type -cne 'CanonicalSyncV2LegacyImportPlan' -or
        [string]$Manifest.implementation.cursor_type -cne 'CanonicalSyncV2LegacyImportCursor' -or
        [string]$Manifest.implementation.progress_type -cne
            'CanonicalSyncV2LegacyImportProgress') {
        throw 'canonical sync v2 legacy import header changed'
    }
    if ([UInt64]$Manifest.bounds.maximum_source_events -ne 4096 -or
        [UInt64]$Manifest.bounds.maximum_events_per_page -ne 64 -or
        [bool]$Manifest.bounds.empty_page_size_allowed) {
        throw 'canonical sync v2 legacy import bounds changed'
    }
    foreach ($field in @(
        'source_membership_verified',
        'source_authorization_reverified',
        'source_items_strictly_ordered_and_unique',
        'source_byte_change_rejected',
        'membership_change_rejected',
        'cross_plan_cursor_rejected',
        'resume_prefix_must_exist_and_match',
        'same_cursor_retry_idempotent',
        'page_preflighted_and_committed_atomically'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 legacy import contract '$field' is not verified"
        }
    }
    foreach ($field in @(
        'partial_page_commit_on_conflict',
        'native_canonical_events_synthesized',
        'format_sniffing',
        'parse_error_fallback'
    )) {
        if ([bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 legacy import unsafe contract '$field' is enabled"
        }
    }
    foreach ($field in @(
        'bounded_multi_page_import',
        'same_cursor_retry',
        'complete_cursor_retry',
        'skipped_prefix_rejected',
        'changed_membership_rejected',
        'changed_source_claim_rejected',
        'cross_plan_cursor_rejected',
        'conflict_rolls_back_later_page_items'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 legacy import regression '$field' is not verified"
        }
    }
    foreach ($field in @(
        'active_runtime_import',
        'durable_cursor_transaction_integrated',
        'endpoint_registered',
        'canonical_dial_enabled'
    )) {
        if ([bool]$Manifest.activation.$field) {
            throw "canonical sync v2 legacy import activation '$field' was premature"
        }
    }
    foreach ($field in @(
        'network_executed',
        'release_build',
        'archive_created',
        'stable_executable_replaced'
    )) {
        if ([bool]$Manifest.scope.$field) {
            throw "canonical sync v2 legacy import scope '$field' unexpectedly changed"
        }
    }
    foreach ($remaining in @(
        'durable-import-cursor-and-store-transaction',
        'durable-sync-checkpoint-runtime-integration',
        'mutual-authenticated-capability-selection',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    )) {
        if ($remaining -cnotin @($Manifest.remaining)) {
            throw "canonical sync v2 legacy import remaining gate '$remaining' disappeared"
        }
    }

    if ([string]$Migration.milestone -cne 'M1.1.63' -or
        [string]$Migration.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        [string]$Migration.history_compatible_successor.dormant_bounded_resumable_legacy_import -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -cne
            'verified' -or
        [bool]$Migration.candidate_identifier_registered) {
        throw 'canonical migration does not preserve the dormant legacy import state'
    }
    $importRequirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'dormant-bounded-resumable-legacy-import' }
    )
    if ($importRequirement.Count -ne 1 -or
        [string]$importRequirement[0].status -cne 'verified') {
        throw 'canonical migration lost the verified legacy import requirement'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.63' -or
        [string]$TransportManifest.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        'signed-checkpoint-runtime-transition-integration' -cnotin
            @($TransportManifest.remaining) -or
        'mutual-authenticated-capability-selection' -cnotin
            @($TransportManifest.remaining)) {
        throw 'transport sync manifest does not preserve the dormant legacy import boundary'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.63' -or
        [string]$Compatibility.status -cne
            'dormant-signed-session-successor-store-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.dormant_bounded_resumable_legacy_import_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_legacy_import_checkpoint_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_bootstrap_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_signed_session_successor_store_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-policy-import-checkpoint-session-bootstrap-and-successor-store-verified-not-active') {
        throw 'history compatibility manifest lost the dormant legacy import contract'
    }
    $transportCoverage = @(
        @($Coverage.surfaces) |
            Where-Object { [string]$_.id -ceq 'transport-sync' }
    )
    if ([string]$Coverage.milestone -cne 'M1.1.63' -or
        $transportCoverage.Count -ne 1 -or
        'CANONICAL-SYNC-V2-LEGACY-IMPORT.json' -cnotin @($transportCoverage[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-legacy-import-boundary.ps1' -cnotin
            @($transportCoverage[0].artifacts) -or
        'dormant-bounded-resumable-legacy-import' -cnotin @($transportCoverage[0].covered) -or
        'dormant-atomic-legacy-import-checkpoint' -cnotin @($transportCoverage[0].covered) -or
        'dormant-signed-session-bootstrap' -cnotin @($transportCoverage[0].covered) -or
        'signed-checkpoint-runtime-transition-integration' -cnotin
            @($transportCoverage[0].remaining) -or
        'mutual-authenticated-capability-selection' -cnotin
            @($transportCoverage[0].remaining)) {
        throw 'canonical coverage does not include the dormant legacy import artifacts'
    }

    foreach ($needle in @(
        'pub struct CanonicalSyncV2LegacyImportPlan',
        'pub struct CanonicalSyncV2LegacyImportCursor',
        'pub struct CanonicalSyncV2LegacyImportProgress',
        'pub fn canonical_sync_v2_legacy_import_plan(',
        'pub fn import_legacy_page(',
        'fn verify_legacy_import_prefix(',
        'CANONICAL_SYNC_V2_LEGACY_IMPORT_MEMBERSHIP_DIGEST_CONTEXT',
        'CANONICAL_SYNC_V2_LEGACY_IMPORT_EVENT_DIGEST_CONTEXT',
        'CANONICAL_SYNC_V2_LEGACY_IMPORT_PLAN_ID_CONTEXT',
        'maximum_events > MAX_CANONICAL_SYNC_V2_EVENTS_PER_BATCH',
        'CanonicalSyncV2LegacyImportCursorPlanMismatch',
        'CanonicalSyncV2LegacyImportMembershipMismatch',
        'CanonicalSyncV2LegacyImportSourceEventChanged',
        'CanonicalSyncV2LegacyImportPrefixMismatch',
        'CanonicalSyncLegacyEventBridgeV1::from_active_event(&event)',
        'self.commit_legacy_import_page(plan, cursor, next_index, &carriers)?',
        'canonical_sync_v2_legacy_import_is_bounded_resumable_and_idempotent',
        'canonical_sync_v2_legacy_import_rejects_skips_and_changed_claims',
        'canonical_sync_v2_legacy_import_conflict_does_not_commit_later_page_items'
    )) {
        if (-not $Store.Contains($needle)) {
            throw "kilogram-store lost legacy import source marker: $needle"
        }
    }
    if (-not $StoreCargo.Contains('blake3.workspace = true')) {
        throw 'kilogram-store lost its domain-separated BLAKE3 plan dependency'
    }
    foreach ($activeSource in @($Wire, $Transport, $Cli)) {
        if ($activeSource.Contains('CanonicalSyncV2LegacyImportPlan') -or
            $activeSource.Contains('import_legacy_page')) {
            throw 'dormant legacy import leaked into active wire/transport/CLI code'
        }
    }
    foreach ($needle in @(
        'between 1 and 64 events',
        'every prefix before `next_index` must already exist',
        'cursor is not durably stored in this stage',
        '`/cbor/2` remains unregistered and undialed'
    )) {
        if (-not $Rfc.Contains($needle)) {
            throw "RFC-0186 lost required dormant import statement: $needle"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $storePath, $storeCargoPath, $wirePath, $transportPath,
    $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 legacy import boundary file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$store = Get-Content -LiteralPath $storePath -Raw
$storeCargo = Get-Content -LiteralPath $storeCargoPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ($SelfTest) {
    $changedBound = Copy-JsonObject $manifest
    $changedBound.bounds.maximum_events_per_page = 65
    Assert-Rejected {
        Assert-Contract $changedBound $migration $transportManifest $compatibility $coverage `
            $store $storeCargo $wire $transport $cli $rfc
    } 'expanded_legacy_import_page_bound'

    $skippable = Copy-JsonObject $manifest
    $skippable.contracts.resume_prefix_must_exist_and_match = $false
    Assert-Rejected {
        Assert-Contract $skippable $migration $transportManifest $compatibility $coverage `
            $store $storeCargo $wire $transport $cli $rfc
    } 'skippable_legacy_import_prefix'

    $prematureCheckpoint = Copy-JsonObject $manifest
    $prematureCheckpoint.activation.durable_cursor_transaction_integrated = $true
    Assert-Rejected {
        Assert-Contract $prematureCheckpoint $migration $transportManifest $compatibility `
            $coverage $store $storeCargo $wire $transport $cli $rfc
    } 'premature_import_checkpoint_activation'

    $fallback = Copy-JsonObject $manifest
    $fallback.contracts.parse_error_fallback = $true
    Assert-Rejected {
        Assert-Contract $fallback $migration $transportManifest $compatibility $coverage `
            $store $storeCargo $wire $transport $cli $rfc
    } 'legacy_import_parse_error_fallback'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $store $storeCargo $wire $transport ($cli + ' CanonicalSyncV2LegacyImportPlan') $rfc
    } 'legacy_import_leaked_into_active_cli'

    Write-Output 'canonical_sync_v2_legacy_import_self_test=passed'
}

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $store $storeCargo $wire $transport $cli $rfc
Write-Output 'canonical_sync_v2_legacy_import_boundary=verified'
Write-Output 'source_inventory_bound=4096'
Write-Output 'page_event_bound=64'
Write-Output 'exact_source_and_membership_claim=verified'
Write-Output 'resume_prefix=fail-closed'
Write-Output 'same_cursor_retry=idempotent'
Write-Output 'page_commit=atomic'
Write-Output 'durable_cursor_transaction=false'
Write-Output 'active_runtime_import=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
