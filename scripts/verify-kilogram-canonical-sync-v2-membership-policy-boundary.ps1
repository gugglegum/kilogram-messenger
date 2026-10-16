[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-MEMBERSHIP-POLICY.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0185-m1159-dormant-cross-format-membership-policy.md'

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
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.59' -or
        [string]$Manifest.status -cne
            'dormant-cross-format-membership-policy-verified-not-active' -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2' -or
        [string]$Manifest.implementation.crate -cne 'kilogram-session' -or
        [string]$Manifest.implementation.rust_type -cne
            'CanonicalSyncV2MembershipPolicy' -or
        [string]$Manifest.implementation.policy_trait -cne
            'CanonicalSyncV2EventPolicy' -or
        [string]$Manifest.implementation.legacy_snapshot_type -cne
            'ConversationMembershipSnapshot' -or
        [string]$Manifest.implementation.canonical_snapshot_type -cne
            'CanonicalSignedConversationMembershipV1') {
        throw 'canonical sync v2 membership policy header changed'
    }

    foreach ($field in @(
        'snapshot_signature_verified_at_construction',
        'snapshot_signature_reverified_during_event_check',
        'exact_conversation_binding_required',
        'snapshot_revision_preserved',
        'legacy_snapshot_authorizes_legacy_carrier',
        'legacy_snapshot_authorizes_canonical_carrier',
        'canonical_snapshot_authorizes_legacy_carrier',
        'canonical_snapshot_authorizes_canonical_carrier',
        'carrier_authorization_reverified',
        'membership_checked_before_store'
    )) {
        if (-not [bool]$Manifest.contracts.$field) {
            throw "canonical sync v2 membership policy contract '$field' is not verified"
        }
    }
    if ([bool]$Manifest.contracts.format_sniffing -or
        [bool]$Manifest.contracts.parse_error_fallback) {
        throw 'canonical sync v2 membership policy enabled unsafe format fallback'
    }
    foreach ($field in @(
        'two_snapshot_by_two_carrier_matrix',
        'nonmember_rejected',
        'conversation_mismatch_rejected',
        'invalid_snapshot_signature_rejected',
        'policy_rejection_prevents_storage'
    )) {
        if (-not [bool]$Manifest.regression.$field) {
            throw "canonical sync v2 membership policy regression '$field' is not verified"
        }
    }
    foreach ($field in @(
        'active_runtime_policy',
        'legacy_event_store_import_integrated',
        'durable_checkpoint_runtime_integrated',
        'endpoint_registered',
        'canonical_dial_enabled'
    )) {
        if ([bool]$Manifest.activation.$field) {
            throw "canonical sync v2 membership policy was prematurely activated through '$field'"
        }
    }
    if ([bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'canonical sync v2 membership policy scope expanded unexpectedly'
    }

    $expectedRemaining = @(
        'legacy-event-store-import',
        'durable-checkpoint-runtime-integration',
        'mutual-authenticated-capability-selection',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    )
    if ((@($Manifest.remaining) -join ',') -cne ($expectedRemaining -join ',')) {
        throw 'canonical sync v2 membership policy remaining-work boundary changed'
    }

    if ([string]$Migration.milestone -cne 'M1.1.60' -or
        [string]$Migration.status -cne
            'dormant-bounded-resumable-legacy-import-verified-not-active' -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [bool]$Migration.candidate_identifier_registered -or
        [string]$Migration.history_compatible_successor.dormant_cross_format_membership_policy -cne
            'verified' -or
        [string]$Migration.history_compatible_successor.dormant_bounded_resumable_legacy_import -cne
            'verified') {
        throw 'canonical migration omitted the dormant cross-format membership policy'
    }
    $policyRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'dormant-cross-format-membership-policy'
    })
    $runtimeRequirement = @($Migration.activation_requirements | Where-Object {
        [string]$_.id -ceq 'dual-reader-runtime-implementation'
    })
    if ($policyRequirement.Count -ne 1 -or
        [string]$policyRequirement[0].status -cne 'verified' -or
        $runtimeRequirement.Count -ne 1 -or
        [string]$runtimeRequirement[0].status -cne 'pending') {
        throw 'canonical migration confused dormant policy with active runtime support'
    }

    if ([string]$TransportManifest.milestone -cne 'M1.1.60' -or
        [string]$TransportManifest.status -cne
            'dormant-bounded-resumable-legacy-import-verified-not-active' -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_cross_format_membership_policy -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_bounded_resumable_legacy_import -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed -or
        [bool]$TransportManifest.authenticated_capability.canonical_dial_enabled -or
        [string]$TransportManifest.selection.current_state -cne 'legacy-only' -or
        (@($TransportManifest.remaining)[0]) -cne
            'durable-import-checkpoint-and-capability-integration') {
        throw 'canonical transport manifest does not preserve dormant-only policy'
    }

    if ([string]$Compatibility.milestone -cne 'M1.1.60' -or
        [string]$Compatibility.status -cne
            'dormant-bounded-resumable-legacy-import-verified-not-active' -or
        -not [bool]$Compatibility.required_successor.dormant_cross_format_membership_policy_integrated -or
        -not [bool]$Compatibility.required_successor.dormant_bounded_resumable_legacy_import_integrated -or
        [string]$Compatibility.required_successor.status -cne
            'dormant-store-policy-and-legacy-import-verified-not-active' -or
        -not [bool]$Compatibility.active.registered -or
        -not [bool]$Compatibility.active.dialed) {
        throw 'history compatibility manifest omitted dormant cross-format policy'
    }

    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.60' -or
        $surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
        'dormant-cross-format-membership-policy' -cnotin @($surface[0].covered) -or
        'dormant-bounded-resumable-legacy-import' -cnotin @($surface[0].covered) -or
        'durable-import-checkpoint-and-capability-integration' -cnotin
            @($surface[0].remaining)) {
        throw 'canonical coverage omitted dormant cross-format policy'
    }
    foreach ($artifact in @(
        'CANONICAL-SYNC-V2-MEMBERSHIP-POLICY.json',
        'crates/kilogram-session/src/canonical_sync_v2_adapter.rs',
        'scripts/verify-kilogram-canonical-sync-v2-membership-policy-boundary.ps1',
        'docs/RFC-0185-m1159-dormant-cross-format-membership-policy.md'
    )) {
        if ($artifact -cnotin @($surface[0].artifacts)) {
            throw "canonical coverage is missing membership policy artifact '$artifact'"
        }
    }

    foreach ($required in @(
        'pub struct CanonicalSyncV2MembershipPolicy',
        'CanonicalSyncV2MembershipSnapshot::Legacy',
        'CanonicalSyncV2MembershipSnapshot::Canonical',
        'pub fn from_legacy',
        'pub fn from_canonical',
        'verify_signed_conversation_membership_v1',
        'verify_authorized_event_for_membership_v1',
        '.verify_for_membership(membership)',
        '.require_member(AccountId::from_bytes(',
        '.binary_search(legacy.author_account_id())',
        'policy.verify_event(conversation_id, event)?;',
        'concrete_membership_policy_accepts_both_carriers_with_both_snapshot_formats',
        'concrete_membership_policy_rejects_nonmembers_mismatch_and_invalid_snapshot',
        'membership_policy_rejection_prevents_storage'
    )) {
        if ($Adapter.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync v2 membership policy source is missing '$required'"
        }
    }
    foreach ($activeSource in @($Wire, $Transport, $Cli)) {
        if ($activeSource.IndexOf('CanonicalSyncV2MembershipPolicy',
                [StringComparison]::Ordinal) -ge 0) {
            throw 'dormant canonical sync v2 membership policy leaked into active runtime source'
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($required in @(
        'concrete pre-storage authorization policy',
        'One owner-signed snapshot format authorizes both carrier namespaces',
        'not required to re-sign old membership state',
        'rejected before the state machine invokes the atomic typed store',
        'does not sniff membership bytes',
        '`/cbor/2` remains unregistered and undialed'
    )) {
        if ($normalizedRfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "RFC-0185 is missing '$required'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $adapterPath, $wirePath, $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync v2 membership policy file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $adapter $wire $transport $cli $rfc

if ($SelfTest) {
    $oneWay = Copy-JsonObject $manifest
    $oneWay.contracts.canonical_snapshot_authorizes_legacy_carrier = $false
    Assert-Rejected {
        Assert-Contract $oneWay $migration $transportManifest $compatibility $coverage `
            $adapter $wire $transport $cli $rfc
    } 'incomplete_cross_format_matrix'

    $fallback = Copy-JsonObject $manifest
    $fallback.contracts.parse_error_fallback = $true
    Assert-Rejected {
        Assert-Contract $fallback $migration $transportManifest $compatibility $coverage `
            $adapter $wire $transport $cli $rfc
    } 'membership_parse_error_fallback'

    $activated = Copy-JsonObject $manifest
    $activated.activation.active_runtime_policy = $true
    Assert-Rejected {
        Assert-Contract $activated $migration $transportManifest $compatibility $coverage `
            $adapter $wire $transport $cli $rfc
    } 'premature_active_membership_policy'

    $leakedCli = $cli + "`nCanonicalSyncV2MembershipPolicy`n"
    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $adapter $wire $transport $leakedCli $rfc
    } 'membership_policy_leaked_into_active_cli'
    Write-Output 'canonical_sync_v2_membership_policy_self_test=passed'
}

Write-Output 'canonical_sync_v2_membership_policy_boundary=verified'
Write-Output 'snapshot_formats=legacy-postcard,canonical-cbor'
Write-Output 'carrier_formats=legacy-postcard,canonical-cbor'
Write-Output 'cross_format_matrix=verified-2x2'
Write-Output 'nonmember=fail-closed'
Write-Output 'conversation_mismatch=fail-closed'
Write-Output 'invalid_snapshot_signature=fail-closed'
Write-Output 'membership_checked_before_store=true'
Write-Output 'active_runtime_policy=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
