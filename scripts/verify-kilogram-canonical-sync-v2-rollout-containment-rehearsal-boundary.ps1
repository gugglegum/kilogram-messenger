[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-CONTAINMENT-REHEARSAL.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0221-m1197-rollout-containment-rehearsal.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Source, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.1.97' -or
        [string]$Manifest.status -cne
            'canonical-sync-v2-rollout-containment-rehearsal-verified-local' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [UInt64]$Manifest.controller_matrix.evaluated_combinations -ne 640 -or
        [bool]$Manifest.controller_matrix.normal_proposal_from_existing_containment -or
        -not [bool]$Manifest.controller_matrix.containment_preserves_current_target -or
        -not [bool]$Manifest.controller_matrix.proposal_requires_explicit_device_approval -or
        [bool]$Manifest.controller_matrix.release_policy_can_sign_activation -or
        [bool]$Manifest.controller_matrix.runtime_activation_authorized -or
        [bool]$Manifest.controller_matrix.canonical_required_proposed -or
        -not [bool]$Manifest.health_binding.exact_release_artifact_id -or
        [bool]$Manifest.activation_boundary.runtime_consumer_exists -or
        [bool]$Manifest.activation_boundary.desktop_consumer_exists -or
        [bool]$Manifest.activation_boundary.cli_observer_can_apply -or
        -not [bool]$Manifest.activation_boundary.explicit_recovery_required_to_clear_containment) {
        throw 'rollout containment rehearsal boundary changed'
    }
    foreach ($failure in @(
        'foreign_release_authority',
        'tampered_health_record',
        'missing_current_health_record',
        'truncated_database',
        'symlink_directory_or_database'
    )) {
        if ([string]$Manifest.read_only_failure_matrix.$failure -cne 'fail-closed') {
            throw "rollout failure '$failure' is not fail-closed"
        }
    }
    Assert-ContainsAll $Source @(
        'ContainmentRequiresExplicitRecovery',
        'health.release_artifact_id != evaluation.release_artifact_id',
        'fn controller_never_clears_existing_containment',
        'fn exhaustive_controller_matrix_preserves_authority_invariants',
        'assert_eq!(evaluated, 640)',
        'fn read_only_inspection_rejects_missing_current_health',
        'fn read_only_inspection_rejects_a_truncated_database'
    ) 'rollout containment implementation'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'implicit containment-clear authority',
        'Only a separate explicit Device recovery ceremony may clear it',
        'all 640 combinations',
        'public `legacy-only` default are unchanged'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "containment rehearsal RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $sourcePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath $sourcePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $source $rfc

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.controller_matrix.normal_proposal_from_existing_containment = $true
    try { Assert-Contract $mutated $source $rfc } catch {
        Write-Output 'implicit_containment_clear=rejected'
        Write-Output 'canonical_sync_v2_rollout_containment_rehearsal_self_test=passed'
        exit 0
    }
    throw 'controller was allowed to clear containment implicitly'
}

Write-Output 'controller_matrix=640/640'
Write-Output 'network_executed=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_rollout_containment_rehearsal_boundary=verified'
