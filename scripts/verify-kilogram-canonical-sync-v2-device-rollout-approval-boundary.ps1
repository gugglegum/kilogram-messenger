[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DEVICE-ROLLOUT-APPROVAL.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\rollout_approval.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0226-m1202-device-signed-rollout-approval.md'

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
    if ([string]$Manifest.milestone -cne 'M1.2.2' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-device-rollout-approval-verified-local' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [bool]$Manifest.authority_boundary.release_policy_can_sign_approval -or
        [bool]$Manifest.authority_boundary.release_policy_can_sign_activation -or
        -not [bool]$Manifest.authority_boundary.explicit_local_approval_required -or
        -not [bool]$Manifest.authority_boundary.approval_signed_by_device -or
        -not [bool]$Manifest.authority_boundary.candidate_activation_signed_by_device -or
        [bool]$Manifest.authority_boundary.automatic_activation -or
        [int]$Manifest.apply_boundary.maximum_approval_bytes -ne 2048 -or
        -not [bool]$Manifest.apply_boundary.exact_replay_idempotent -or
        [bool]$Manifest.apply_boundary.storage_side_effects -or
        [bool]$Manifest.apply_boundary.network_side_effects -or
        [bool]$Manifest.remaining_boundaries.activation_persistence_integrated -or
        [bool]$Manifest.remaining_boundaries.runtime_consumer_exists -or
        [bool]$Manifest.remaining_boundaries.desktop_consumer_exists -or
        [bool]$Manifest.remaining_boundaries.containment_recovery_exists) {
        throw 'Device rollout approval authority boundary changed'
    }
    foreach ($name in @('changed_policy_or_bundle', 'changed_activation_head', 'wrong_device_or_tamper')) {
        if ([string]$Manifest.apply_boundary.$name -cne 'fail-closed') {
            throw "Device rollout approval '$name' is not fail-closed"
        }
    }
    Assert-ContainsAll $Source @(
        'UserOrTestOperatorApproved',
        'release_policy_can_sign_activation: false',
        'CanonicalSyncV2ActivationActor::ReleasePolicy',
        'identity.sign(&approval_signing_bytes(&content)?)',
        'expected_previous_record_id',
        'StaleActivationHead',
        'DeviceRolloutApprovalApplyOutcome::AlreadyApplied',
        'ContainmentEvidenceRequired',
        'fn wrong_device_tamper_and_stale_head_fail_closed'
    ) 'Device rollout approval implementation'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'unable to manufacture either layer',
        'byte-identical already-applied candidate is the only idempotent replay',
        'performs no filesystem write, runtime IPC or network operation',
        'public `legacy-only` default remain unchanged'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Device rollout approval RFC is missing '$needle'"
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
    $mutated.authority_boundary.release_policy_can_sign_activation = $true
    try { Assert-Contract $mutated $source $rfc } catch {
        Write-Output 'release_policy_activation_signature=rejected'
        Write-Output 'canonical_sync_v2_device_rollout_approval_self_test=passed'
        exit 0
    }
    throw 'release policy silently gained Device activation signing authority'
}

Write-Output 'explicit_device_approval=true'
Write-Output 'release_policy_can_sign_activation=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_device_rollout_approval_boundary=verified'
