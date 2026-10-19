[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-CONTAINMENT-RECOVERY.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\containment_recovery.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0227-m1203-explicit-containment-recovery.md'

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
    if ([string]$Manifest.milestone -cne 'M1.2.3' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-explicit-containment-recovery-verified-local' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.recovery_authority.explicit_local_approval_required -or
        -not [bool]$Manifest.recovery_authority.device_signature_required -or
        [bool]$Manifest.recovery_authority.healthy_release_policy_is_authority -or
        [bool]$Manifest.recovery_authority.automatic_containment_clear -or
        -not [bool]$Manifest.recovery_authority.target_preserved -or
        -not [bool]$Manifest.recovery_authority.canonical_evidence_preserved -or
        [bool]$Manifest.recovery_authority.canonical_required_managed -or
        [int]$Manifest.replay_and_failure.maximum_approval_bytes -ne 4096 -or
        -not [bool]$Manifest.replay_and_failure.exact_replay_idempotent -or
        [bool]$Manifest.replay_and_failure.filesystem_side_effects -or
        [bool]$Manifest.replay_and_failure.runtime_ipc_side_effects -or
        [bool]$Manifest.replay_and_failure.network_side_effects -or
        [bool]$Manifest.remaining_boundaries.activation_persistence_integrated -or
        [bool]$Manifest.remaining_boundaries.desktop_consumer_exists -or
        [bool]$Manifest.remaining_boundaries.runtime_consumer_exists) {
        throw 'containment recovery authority boundary changed'
    }
    foreach ($name in @('changed_policy_health_or_head', 'wrong_device_or_tamper')) {
        if ([string]$Manifest.replay_and_failure.$name -cne 'fail-closed') {
            throw "containment recovery '$name' is not fail-closed"
        }
    }
    Assert-ContainsAll $Source @(
        'UserOrTestOperatorApproved',
        'CanonicalSyncV2ActivationActor::ExplicitLocal',
        'CanonicalSyncV2ActivationDisposition::Normal',
        'PolicyBelowContainedState',
        'HealthyRecoveryEvidenceRequired',
        'candidate.migration_state() != current.migration_state()',
        'candidate.canonical_evidence_id() != current.canonical_evidence_id()',
        'UnmanagedCanonicalRequired',
        'fn changed_health_device_and_head_fail_closed'
    ) 'containment recovery implementation'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'necessary preconditions, not recovery authority',
        'preserves the migration state and canonical evidence byte-for-byte',
        'Only the byte-identical already-applied candidate is an idempotent replay',
        'public `legacy-only` default remain unchanged'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "containment recovery RFC is missing '$needle'"
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
    $mutated.recovery_authority.automatic_containment_clear = $true
    try { Assert-Contract $mutated $source $rfc } catch {
        Write-Output 'automatic_containment_clear=rejected'
        Write-Output 'canonical_sync_v2_containment_recovery_self_test=passed'
        exit 0
    }
    throw 'containment recovery silently became automatic'
}

Write-Output 'explicit_device_recovery=true'
Write-Output 'automatic_containment_clear=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_containment_recovery_boundary=verified'
