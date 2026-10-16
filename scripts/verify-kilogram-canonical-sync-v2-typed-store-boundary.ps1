[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-TYPED-STORE.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$storePath = Join-Path $workspace 'crates\kilogram-store\src\lib.rs'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0184-m1158-dormant-atomic-typed-sync-store.md'

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

    $alpn = 'kilogram/m1/sync/cbor/2'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.58' -or
        [string]$Manifest.status -cne
            'dormant-atomic-typed-carrier-store-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne $alpn -or
        [string]$Manifest.implementation.crate -cne 'kilogram-store' -or
        [string]$Manifest.implementation.rust_type -cne 'CanonicalSyncV2CarrierStore' -or
        [string]$Manifest.implementation.database_engine -cne 'redb' -or
        [string]$Manifest.implementation.database_file -cne 'canonical-sync-v2.redb' -or
        [string]$Manifest.implementation.schema_marker -cne
            'kilogram:canonical-sync-v2-carrier-store:v1\0kilogram/m1/sync/cbor/2' -or
        [string]$Manifest.implementation.session_trait_implementation -cne
            'CanonicalSyncV2Store for CanonicalSyncV2CarrierStore') {
        throw 'canonical sync v2 typed store header changed'
    }

    $expectedStorageKey = @(
        'conversation-id-32-bytes',
        'event-namespace-u64-big-endian',
        'event-id-32-bytes'
    )
    if ((@($Manifest.storage_key) -join ',') -cne ($expectedStorageKey -join ',')) {
        throw 'canonical sync v2 typed storage key changed'
    }
    foreach ($field in @(
        'full_typed_key_preserved',
        'legacy_postcard_namespace_supported',
        'canonical_cbor_namespace_supported',
        'strict_carrier_decode_on_read',
        'database_key_carrier_key_match_required',
        'database_conversation_carrier_conversation_match_required',
        'byte_identical_replay_idempotent',
        'immutable_key_conflict_fails_closed',
        'whole_batch_preflight_before_insert',
        'whole_batch_single_transaction',
        'immediate_durability',
        'bounded_inventory'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 typed store contract '$field' is not verified"
        }
    }
    foreach ($field in @(
        'both_namespaces_persist_across_reopen',
        'trait_round_trip_persists_across_reopen',
        'whole_batch_aborts_on_conflict',
        'corrupt_value_fails_closed',
        'cross_conversation_write_fails_closed'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 typed store regression '$field' is not verified"
        }
    }
    foreach ($field in @(
        'active_runtime_store',
        'legacy_event_store_import_integrated',
        'active_membership_policy_implemented',
        'durable_checkpoint_runtime_integrated',
        'endpoint_registered',
        'canonical_dial_enabled'
    )) {
        if ([bool]$Manifest.activation.$field) {
            throw "canonical sync v2 typed store was prematurely activated through '$field'"
        }
    }
    if ([bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'canonical sync v2 typed store scope expanded unexpectedly'
    }

    $expectedRemaining = @(
        'legacy-event-store-import',
        'legacy-and-canonical-membership-policy-implementation',
        'durable-checkpoint-runtime-integration',
        'mutual-authenticated-capability-selection',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    )
    if ((@($Manifest.remaining) -join ',') -cne ($expectedRemaining -join ',')) {
        throw 'canonical sync v2 typed store remaining-work boundary changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.61' -or
        [string]$Migration.status -cne 'dormant-atomic-legacy-import-checkpoint-verified-not-active' -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [bool]$Migration.candidate_identifier_registered -or
        [string]$Migration.history_compatible_successor.dormant_atomic_typed_store -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_cross_format_membership_policy -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_bounded_resumable_legacy_import -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -cne
            'verified') {
        throw 'canonical migration omitted the dormant atomic typed store'
    }
    $storeRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'dormant-atomic-typed-carrier-store'
    })
    $runtimeRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'dual-reader-runtime-implementation'
    })
    if ($storeRequirement.Count -ne 1 -or
        [string]$storeRequirement[0].status -cne 'verified' -or
        $runtimeRequirement.Count -ne 1 -or
        [string]$runtimeRequirement[0].status -cne 'pending') {
        throw 'canonical migration confused dormant storage with active runtime support'
    }

    if ([string]$TransportManifest.milestone -cne 'M1.1.61' -or
        [string]$TransportManifest.status -cne
            'dormant-atomic-legacy-import-checkpoint-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_typed_store -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_cross_format_membership_policy -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled -or
        [string]$TransportManifest.selection.current_state -cne 'legacy-only' -or
        (@($TransportManifest.remaining)[0]) -cne
            'signed-sync-checkpoint-and-capability-integration') {
        throw 'canonical transport manifest does not preserve dormant-only storage'
    }

    if ([string]$Compatibility.milestone -cne 'M1.1.61' -or
        [string]$Compatibility.status -cne
            'dormant-atomic-legacy-import-checkpoint-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_typed_store_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_cross_format_membership_policy_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_bounded_resumable_legacy_import_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_atomic_legacy_import_checkpoint_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-policy-import-and-checkpoint-verified-not-active' -or
        [bool]$Compatibility.required_successor.{must_preserve_no-sniffing-and-no-parse-fallback} -ne
            $true -or
        -not [bool]$Compatibility.active.registered -or
        -not [bool]$Compatibility.active.dialed) {
        throw 'history compatibility manifest omitted dormant typed storage'
    }

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.61' -or
        $surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
        'dormant-atomic-typed-carrier-store' -cnotin @($surface[0].covered) -or
        'dormant-cross-format-membership-policy' -cnotin @($surface[0].covered) -or
        'dormant-bounded-resumable-legacy-import' -cnotin @($surface[0].covered) -or
        'dormant-atomic-legacy-import-checkpoint' -cnotin @($surface[0].covered) -or
        'signed-sync-checkpoint-and-capability-integration' -cnotin
            @($surface[0].remaining)) {
        throw 'canonical coverage omitted the dormant atomic typed store boundary'
    }
    foreach ($artifact in @(
        'CANONICAL-SYNC-V2-TYPED-STORE.json',
        'crates/kilogram-store/src/lib.rs',
        'crates/kilogram-session/src/canonical_sync_v2_adapter.rs',
        'scripts/verify-kilogram-canonical-sync-v2-typed-store-boundary.ps1',
        'docs/RFC-0184-m1158-dormant-atomic-typed-sync-store.md'
    )) {
        if ($artifact -cnotin @($surface[0].artifacts)) {
            throw "canonical coverage is missing typed store artifact '$artifact'"
        }
    }

    foreach ($required in @(
        'pub struct CanonicalSyncV2CarrierStore',
        'pub const CANONICAL_SYNC_V2_STORE_FILE',
        'kilogram:canonical-sync-v2-carrier-store:v1\0kilogram/m1/sync/cbor/2',
        'TableDefinition::new("canonical-sync-v2-carriers-v1")',
        'set_durability(Durability::Immediate)',
        'let mut missing = Vec::new();',
        'write.commit().map_err(canonical_sync_v2_database_error)',
        'decode_canonical_sync_v2_storage_key',
        'validate_canonical_sync_v2_stored_carrier',
        'CanonicalSyncV2CarrierConflict',
        'canonical_sync_v2_store_persists_both_namespaces_across_reopen',
        'canonical_sync_v2_store_conflict_aborts_the_whole_batch',
        'canonical_sync_v2_store_rejects_corruption_and_cross_conversation_writes'
    )) {
        if ($Store.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync v2 typed store source is missing '$required'"
        }
    }
    foreach ($required in @(
        'impl CanonicalSyncV2Store for CanonicalSyncV2CarrierStore',
        'durable_typed_store_trait_survives_reopen'
    )) {
        if ($Adapter.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync v2 adapter omitted typed store integration '$required'"
        }
    }

    foreach ($activeSource in @($Wire, $Transport, $Cli)) {
        foreach ($forbidden in @(
            'CanonicalSyncV2CarrierStore',
            'CANONICAL_SYNC_V2_STORE_FILE',
            'canonical-sync-v2.redb'
        )) {
            if ($activeSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
                throw "dormant canonical sync v2 typed store leaked into active runtime source: $forbidden"
            }
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'dormant durable store',
        'conversation ID, the Event namespace and the Event ID',
        'preflights every existing key before inserting any missing value',
        'conflict in any element therefore aborts the whole batch',
        'does not make the database the active runtime store',
        '`/cbor/2` remains unregistered and undialed'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0184 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $storePath, $adapterPath, $wirePath, $transportPath, $cliPath,
    $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 typed store file is missing: $path"
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
    $nonAtomic = Copy-JsonObject $manifest
    $nonAtomic.contracts.whole_batch_single_transaction = $false
    Assert-Rejected {
        Assert-Contract $nonAtomic $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'non_atomic_batch_contract'

    $activated = Copy-JsonObject $manifest
    $activated.activation.active_runtime_store = $true
    Assert-Rejected {
        Assert-Contract $activated $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'premature_active_runtime_store'

    $changedSchema = Copy-JsonObject $manifest
    $changedSchema.implementation.schema_marker = 'changed'
    Assert-Rejected {
        Assert-Contract $changedSchema $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $cli $rfc
    } 'changed_store_schema_marker'

    $leakedCli = $cli + "`nCanonicalSyncV2CarrierStore`n"
    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $store $adapter $wire $transport $leakedCli $rfc
    } 'typed_store_leaked_into_active_cli'
    Write-Output 'canonical_sync_v2_typed_store_self_test=passed'
}

Write-Output 'canonical_sync_v2_typed_store_boundary=verified'
Write-Output 'full_typed_storage_key=verified'
Write-Output 'atomic_batch_commit=verified'
Write-Output 'immutable_conflict=fail-closed'
Write-Output 'stored_carrier_validation=fail-closed'
Write-Output 'active_runtime_store=false'
Write-Output 'legacy_event_store_import=false'
Write-Output 'active_membership_policy=false'
Write-Output 'durable_checkpoint_runtime_integration=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
