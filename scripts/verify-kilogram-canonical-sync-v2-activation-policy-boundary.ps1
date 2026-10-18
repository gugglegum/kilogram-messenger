[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ACTIVATION-POLICY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0205-m1181-canonical-sync-v2-activation-semantics.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactStrings {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $values = @($Actual | ForEach-Object { [string]$_ })
    if (($values -join "`n") -cne ($Expected -join "`n")) {
        throw "$Label changed"
    }
}

function Assert-State {
    param(
        [object]$State,
        [string]$Id,
        [string]$RustVariant,
        [bool]$Legacy,
        [bool]$Publish,
        [bool]$Register,
        [bool]$Accept,
        [bool]$Initiate,
        [bool]$Resume
    )
    if ([string]$State.id -cne $Id -or
        [string]$State.rust_variant -cne $RustVariant -or
        [bool]$State.legacy_reader_enabled -ne $Legacy -or
        [bool]$State.publish_canonical_capability -ne $Publish -or
        [bool]$State.register_canonical_alpn -ne $Register -or
        [bool]$State.accept_new_canonical_session -ne $Accept -or
        [bool]$State.initiate_new_canonical_session -ne $Initiate -or
        [bool]$State.resume_selected_canonical_session -ne $Resume) {
        throw "activation state '$Id' changed"
    }
    if ([bool]$State.publish_canonical_capability -and
        -not [bool]$State.register_canonical_alpn) {
        throw "activation state '$Id' publishes capability without a registered reader"
    }
    if (([bool]$State.accept_new_canonical_session -or
            [bool]$State.initiate_new_canonical_session -or
            [bool]$State.resume_selected_canonical_session) -and
        -not [bool]$State.register_canonical_alpn) {
        throw "activation state '$Id' authorizes a canonical session without a registered ALPN"
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Migration,
        [object]$TransportManifest,
        [string]$Adapter,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.81' -or
        [string]$Manifest.status -cne
            'canonical-sync-v2-activation-semantics-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [string]$Manifest.legacy_protocol_identifier -cne 'kilogram/m0/sync/8' -or
        [string]$Manifest.canonical_protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'canonical activation policy header changed'
    }

    Assert-ExactStrings @($Manifest.policy_dimensions) @(
        'legacy-reader-enabled',
        'publish-canonical-capability',
        'register-canonical-alpn',
        'accept-new-canonical-session',
        'initiate-new-canonical-session',
        'resume-selected-canonical-session'
    ) 'canonical activation policy dimensions'

    $states = @($Manifest.states)
    if ($states.Count -ne 5) { throw 'canonical activation state count changed' }
    Assert-State $states[0] 'legacy-only' 'LegacyOnly' $true $false $false $false $false $false
    Assert-State $states[1] 'advertise-cbor-v2' 'AdvertiseCanonicalV2' $true $true $true $false $false $false
    Assert-State $states[2] 'dual-reader-prefer-legacy' 'DualReaderPreferLegacy' $true $true $true $true $false $true
    Assert-State $states[3] 'dual-reader-prefer-canonical' 'DualReaderPreferCanonical' $true $true $true $true $true $true
    Assert-State $states[4] 'canonical-required' 'CanonicalRequired' $false $true $true $true $true $true

    $containment = $Manifest.emergency_containment
    if ([bool]$containment.migration_state_changed -or
        -not [bool]$containment.persisted_canonical_evidence_required -or
        -not [bool]$containment.legacy_reader_follows_migration_state -or
        [bool]$containment.publish_canonical_capability -or
        -not [bool]$containment.register_canonical_alpn -or
        [bool]$containment.accept_new_canonical_session -or
        [bool]$containment.initiate_new_canonical_session -or
        -not [bool]$containment.resume_selected_canonical_session -or
        -not [bool]$containment.automatic_parse_or_network_trigger_forbidden -or
        -not [bool]$containment.legacy_fallback_forbidden -or
        -not [bool]$containment.durable_canonical_evidence_retained) {
        throw 'canonical emergency containment contract changed'
    }

    $invariants = $Manifest.invariants
    if (-not [bool]$invariants.publication_requires_registered_reader -or
        -not [bool]$invariants.new_canonical_session_requires_mutual_authenticated_selection -or
        -not [bool]$invariants.resume_requires_exact_persisted_selection -or
        [bool]$invariants.selected_canonical_failure_may_fallback_to_legacy -or
        [bool]$invariants.missing_policy_activates_canonical -or
        [bool]$invariants.malformed_policy_activates_canonical -or
        [bool]$invariants.migration_epoch_rollback_activates_canonical) {
        throw 'canonical activation invariants changed'
    }

    if (-not [bool]$Manifest.activation.semantic_policy_integrated -or
        [bool]$Manifest.activation.durable_local_authority_integrated -or
        [bool]$Manifest.activation.active_runtime_integrated -or
        [bool]$Manifest.activation.candidate_endpoint_registered_by_default -or
        [bool]$Manifest.activation.candidate_dialed_by_default -or
        [bool]$Manifest.activation.network_executed) {
        throw 'canonical activation boundary changed prematurely'
    }
    Assert-ExactStrings @($Manifest.next) @(
        'durable-device-bound-local-activation-authority',
        'production-runtime-dark-launch-integration',
        'local-and-two-host-activation-rehearsal'
    ) 'canonical activation next work'

    if ([string]$Migration.current_state -cne 'legacy-only' -or
        [bool]$Migration.changes_active_wire -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'existing public transport is no longer legacy-only'
    }

    foreach ($needle in @(
        'pub enum CanonicalSyncV2ActivationDisposition',
        'pub struct CanonicalSyncV2ActivationPermissions',
        'pub fn evaluate_canonical_sync_v2_activation_permissions(',
        'publish_canonical_capability',
        'register_canonical_alpn',
        'accept_new_canonical_session',
        'initiate_new_canonical_session',
        'resume_selected_canonical_session',
        'ActivationStateConflictsWithCanonicalEvidence',
        'ContainmentRequiresCanonicalEvidence',
        'activation_permissions_separate_publication_registration_and_session_rights',
        'activation_containment_preserves_only_exact_canonical_resume'
    )) {
        if (-not $Adapter.Contains($needle)) {
            throw "canonical activation source marker missing: $needle"
        }
    }
    foreach ($inactiveSource in @($Transport, $Cli)) {
        if ($inactiveSource.Contains('evaluate_canonical_sync_v2_activation_permissions') -or
            $inactiveSource.Contains('CanonicalSyncV2ActivationPermissions')) {
            throw 'canonical activation policy leaked into active transport or CLI prematurely'
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'separates those actions before any production integration',
        'Capability publication requires the canonical ALPN and reader to be ready',
        'Emergency containment is an overlay, not a migration state',
        'stops capability publication and all new canonical accepts and dials',
        'never turns a selected canonical failure into a Postcard retry',
        'does not persist an activation decision',
        'Postcard v6 remains the public default'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "canonical activation RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $adapterPath,
    $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical activation policy file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $adapter $transport $cli $rfc

if ($SelfTest) {
    $unregistered = Copy-JsonObject $manifest
    $unregistered.states[1].register_canonical_alpn = $false
    Assert-Rejected {
        Assert-Contract $unregistered $migration $transportManifest $adapter $transport $cli $rfc
    } 'published_without_registered_reader'

    $fallback = Copy-JsonObject $manifest
    $fallback.invariants.selected_canonical_failure_may_fallback_to_legacy = $true
    Assert-Rejected {
        Assert-Contract $fallback $migration $transportManifest $adapter $transport $cli $rfc
    } 'selected_canonical_legacy_fallback'

    $containment = Copy-JsonObject $manifest
    $containment.emergency_containment.persisted_canonical_evidence_required = $false
    Assert-Rejected {
        Assert-Contract $containment $migration $transportManifest $adapter $transport $cli $rfc
    } 'containment_without_canonical_evidence'

    $activated = Copy-JsonObject $manifest
    $activated.activation.active_runtime_integrated = $true
    Assert-Rejected {
        Assert-Contract $activated $migration $transportManifest $adapter $transport $cli $rfc
    } 'premature_runtime_activation'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $adapter `
            ($transport + ' evaluate_canonical_sync_v2_activation_permissions') $cli $rfc
    } 'activation_policy_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_activation_policy_self_test=passed'
}

Write-Output 'canonical_sync_v2_activation_policy_boundary=verified'
Write-Output 'public_default=legacy-only'
Write-Output 'candidate_identifier_registered_by_default=false'
Write-Output 'candidate_identifier_dialed_by_default=false'
Write-Output 'containment_preserves_exact_resume=true'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
