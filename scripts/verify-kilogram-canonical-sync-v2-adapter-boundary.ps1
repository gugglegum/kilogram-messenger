[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-RUNTIME-ADAPTER.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$sessionPath = Join-Path $workspace 'crates\kilogram-session\src\lib.rs'
$activeWirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0183-m1157-dormant-history-compatible-sync-runtime-adapter.md'

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
        [object]$Coverage,
        [string]$Adapter,
        [string]$Session,
        [string]$ActiveWire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    $alpn = 'kilogram/m1/sync/cbor/2'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.57' -or
        [string]$Manifest.status -cne
            'dormant-history-compatible-sync-v2-runtime-state-machine-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne $alpn -or
        [string]$Manifest.module.crate -cne 'kilogram-session' -or
        [string]$Manifest.module.path -cne
            'crates/kilogram-session/src/canonical_sync_v2_adapter.rs' -or
        [string]$Manifest.module.export -cne 'canonical_sync_v2_adapter' -or
        [bool]$Manifest.module.referenced_by_active_cli -or
        [bool]$Manifest.module.referenced_by_iroh_transport) {
        throw 'canonical sync v2 dormant adapter header changed'
    }

    if ([string]$Manifest.contracts.typed_store_trait -cne 'CanonicalSyncV2Store' -or
        [string]$Manifest.contracts.authorization_policy_trait -cne
            'CanonicalSyncV2EventPolicy' -or
        [string]$Manifest.contracts.client_state_machine -cne 'CanonicalSyncV2Client' -or
        [string]$Manifest.contracts.server_state_machine -cne 'CanonicalSyncV2Server' -or
        -not [bool]$Manifest.contracts.full_typed_key_preserved -or
        -not [bool]$Manifest.contracts.exact_requested_key_set_required -or
        -not [bool]$Manifest.contracts.exact_completion_key_set_required -or
        -not [bool]$Manifest.contracts.conversation_and_session_bound -or
        -not [bool]$Manifest.contracts.requester_and_responder_bound -or
        -not [bool]$Manifest.contracts.policy_checked_before_store -or
        [bool]$Manifest.contracts.parse_error_fallback -or
        [bool]$Manifest.contracts.format_sniffing) {
        throw 'canonical sync v2 dormant adapter contract changed'
    }

    foreach ($field in @(
        'closed_frames_encoded_and_decoded_between_state_transitions',
        'legacy_postcard_carrier_bidirectional',
        'canonical_cbor_carrier_bidirectional',
        'policy_rejection_prevents_storage',
        'duplicate_inventory_fails_closed',
        'incomplete_store_read_fails_closed'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 dormant adapter regression '$field' is not verified"
        }
    }

    if ([bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled -or
        [bool]$Manifest.activation.active_event_store_implemented -or
        [bool]$Manifest.activation.active_membership_policy_implemented -or
        [bool]$Manifest.activation.durable_checkpoint_runtime_integrated -or
        [bool]$Manifest.activation.mutual_capability_selection_integrated -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'canonical sync v2 dormant adapter was prematurely activated'
    }

    $expectedRemaining = @(
        'active-typed-store-implementation',
        'legacy-and-canonical-membership-policy-implementation',
        'durable-checkpoint-runtime-integration',
        'mutual-authenticated-capability-selection',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    )
    if ((@($Manifest.remaining) -join ',') -cne ($expectedRemaining -join ',')) {
        throw 'canonical sync v2 dormant adapter remaining-work boundary changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.71' -or
        [string]$Migration.status -cne
            'dormant-deterministic-rollback-drill-verified-not-active' -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [bool]$Migration.candidate_identifier_registered -or
        [string]$Migration.history_compatible_successor.dormant_runtime_state_machine -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_typed_store -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_cross_format_membership_policy -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_bounded_resumable_legacy_import -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_signed_session_bootstrap -cne
            'verified') {
        throw 'canonical migration omitted the dormant runtime state machine'
    }
    $dormantRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'dormant-history-compatible-runtime-state-machine'
    })
    $activeRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'dual-reader-runtime-implementation'
    })
    if ($dormantRequirement.Count -ne 1 -or
        [string]$dormantRequirement[0].status -cne 'verified' -or
        $activeRequirement.Count -ne 1 -or
        [string]$activeRequirement[0].status -cne 'verified') {
        throw 'canonical migration confused dormant verification with active runtime support'
    }

    if ([string]$TransportManifest.milestone -cne 'M1.1.71' -or
        [string]$TransportManifest.status -cne
            'dormant-deterministic-rollback-drill-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_runtime_state_machine -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_typed_store -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_cross_format_membership_policy -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_atomic_legacy_import_checkpoint -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_signed_session_bootstrap -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled -or
        [string]$TransportManifest.selection.current_state -cne 'legacy-only' -or
        (@($TransportManifest.remaining)[0]) -cne 'sustained-fuzzing' -or
        'rollback-drill' -cin @($TransportManifest.remaining) -or
        'dual-reader-runtime-implementation' -cin
            @($TransportManifest.remaining)) {
        throw 'canonical transport manifest does not preserve dormant-only state'
    }

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.71' -or
        $surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
        'dormant-history-compatible-sync-v2-runtime-state-machine' -cnotin
            @($surface[0].covered) -or
        'dormant-atomic-typed-carrier-store' -cnotin @($surface[0].covered) -or
        'dormant-cross-format-membership-policy' -cnotin @($surface[0].covered) -or
        'dormant-bounded-resumable-legacy-import' -cnotin @($surface[0].covered) -or
        'dormant-atomic-legacy-import-checkpoint' -cnotin @($surface[0].covered) -or
        'dormant-signed-session-bootstrap' -cnotin @($surface[0].covered) -or
        'dormant-server-runtime-transition-integration' -cin
            @($surface[0].remaining) -or
        'dual-reader-runtime-implementation' -cin
            @($surface[0].remaining)) {
        throw 'canonical coverage omitted the dormant runtime state machine boundary'
    }
    foreach ($artifact in @(
        'CANONICAL-SYNC-V2-RUNTIME-ADAPTER.json',
        'crates/kilogram-session/src/canonical_sync_v2_adapter.rs',
        'scripts/verify-kilogram-canonical-sync-v2-adapter-boundary.ps1',
        'docs/RFC-0183-m1157-dormant-history-compatible-sync-runtime-adapter.md'
    )) {
        if ($artifact -cnotin @($surface[0].artifacts)) {
            throw "canonical coverage is missing dormant adapter artifact '$artifact'"
        }
    }

    foreach ($required in @(
        'pub trait CanonicalSyncV2Store',
        'pub trait CanonicalSyncV2EventPolicy',
        'pub struct CanonicalSyncV2Client',
        'pub struct CanonicalSyncV2Server',
        'verify_sync_frame_session_v2',
        'UnadvertisedEventRequested',
        'StoreEventKeysMismatch',
        'BatchEventKeysMismatch',
        'CompletionEventKeysMismatch',
        'full_frame_round_converges_legacy_and_canonical_carriers',
        'membership_policy_rejection_prevents_storage',
        'duplicate_inventory_and_incomplete_store_reads_fail_closed'
    )) {
        if ($Adapter.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync v2 dormant adapter source is missing '$required'"
        }
    }
    if ($Session.IndexOf('pub mod canonical_sync_v2_adapter;',
            [StringComparison]::Ordinal) -lt 0) {
        throw 'canonical sync v2 dormant adapter module is not exported'
    }
    foreach ($activeSource in @($ActiveWire, $Transport, $Cli)) {
        if ($activeSource.IndexOf('canonical_sync_v2_adapter',
                [StringComparison]::Ordinal) -ge 0 -or
            $activeSource.IndexOf('CanonicalSyncV2Client',
                [StringComparison]::Ordinal) -ge 0 -or
            $activeSource.IndexOf('CanonicalSyncV2Server',
                [StringComparison]::Ordinal) -ge 0) {
            throw 'dormant canonical sync v2 adapter leaked into active runtime source'
        }
    }
    if ($Cli.IndexOf('CANONICAL_SYNC_ALPN_V2', [StringComparison]::Ordinal) -ge 0 -or
        $Cli.IndexOf($alpn, [StringComparison]::Ordinal) -ge 0) {
        throw 'canonical sync v2 identifier leaked into active CLI registration or dial path'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'dormant runtime adapter, not a dual-reader activation',
        'does not implement the active `EventStore`',
        '`CanonicalSyncV2EventPolicy` is therefore mandatory',
        '`/cbor/2` is not registered or dialed',
        'No format sniffing, parse-error fallback',
        'typed durable session checkpoint'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0183 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $coveragePath,
    $adapterPath, $sessionPath, $activeWirePath, $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 dormant adapter file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$session = Get-Content -LiteralPath $sessionPath -Raw
$activeWire = Get-Content -LiteralPath $activeWirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $coverage $adapter `
    $session $activeWire $transport $cli $rfc

if ($SelfTest) {
    $activated = Copy-JsonObject $manifest
    $activated.activation.endpoint_registered = $true
    Assert-Rejected {
        Assert-Contract $activated $migration $transportManifest $coverage $adapter `
            $session $activeWire $transport $cli $rfc
    } 'premature_adapter_activation'

    $missingPolicy = Copy-JsonObject $manifest
    $missingPolicy.contracts.policy_checked_before_store = $false
    Assert-Rejected {
        Assert-Contract $missingPolicy $migration $transportManifest $coverage $adapter `
            $session $activeWire $transport $cli $rfc
    } 'removed_membership_policy_boundary'

    $leakedCli = $cli + "`nCanonicalSyncV2Client`n"
    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $coverage $adapter `
            $session $activeWire $transport $leakedCli $rfc
    } 'adapter_leaked_into_active_cli'
    Write-Output 'canonical_sync_v2_adapter_self_test=passed'
}

Write-Output 'canonical_sync_v2_adapter_boundary=verified'
Write-Output 'dormant_runtime_state_machine=verified'
Write-Output 'legacy_and_canonical_carrier_regression=verified'
Write-Output 'active_store_implementation=false'
Write-Output 'active_membership_policy=false'
Write-Output 'durable_checkpoint_runtime_integration=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
