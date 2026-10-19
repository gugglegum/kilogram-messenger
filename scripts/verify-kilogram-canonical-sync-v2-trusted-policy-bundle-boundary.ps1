[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-TRUSTED-POLICY-BUNDLE.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\trusted_policy.rs'
$rolloutSourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0224-m1200-rotation-aware-policy-bundle.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Source, [string]$RolloutSource, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.2.0' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-rotation-aware-policy-bundle-verified-local' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [int]$Manifest.bundle.maximum_bytes -ne 8192 -or
        -not [bool]$Manifest.bundle.self_contained_keyring -or
        -not [bool]$Manifest.bundle.exact_keyring_id_binding -or
        -not [bool]$Manifest.bundle.exact_signer_key_id_binding -or
        -not [bool]$Manifest.bundle.exact_policy_bytes_binding -or
        -not [bool]$Manifest.rotation.policy_predecessor_binding_preserved -or
        [bool]$Manifest.purpose_boundary.containment_only_can_expand -or
        -not [bool]$Manifest.purpose_boundary.containment_only_can_stop -or
        [bool]$Manifest.activation_boundary.runtime_consumer_exists -or
        [bool]$Manifest.activation_boundary.desktop_consumer_exists -or
        [bool]$Manifest.activation_boundary.cli_consumer_exists -or
        [bool]$Manifest.activation_boundary.durable_bundle_store_exists -or
        [bool]$Manifest.activation_boundary.policy_delivery_exists -or
        [bool]$Manifest.activation_boundary.production_trust_root_exists -or
        [bool]$Manifest.activation_boundary.runtime_activation_authorized) {
        throw 'trusted release-policy bundle boundary changed'
    }
    foreach ($failure in @('foreign_root', 'keyring_skip_or_fork', 'old_revoked_signer_for_new_policy', 'purpose_substitution')) {
        $value = switch ($failure) {
            'foreign_root' { [string]$Manifest.bundle.foreign_root }
            'purpose_substitution' { [string]$Manifest.purpose_boundary.purpose_substitution }
            default { [string]$Manifest.rotation.$failure }
        }
        if ($value -cne 'fail-closed') { throw "trusted policy failure '$failure' is not fail-closed" }
    }
    Assert-ContainsAll $Source @(
        'pub const MAX_TRUSTED_RELEASE_POLICY_BUNDLE_BYTES: usize = 8_192;',
        'pub struct TrustedReleasePolicyBundle',
        'pub fn decode_and_verify',
        'pub fn verify_successor_of',
        'PurposeMismatch',
        'fn policy_chain_crosses_an_exact_operational_key_rotation',
        'fn containment_key_cannot_expand_rollout',
        'fn bundle_rejects_keyring_and_purpose_substitution'
    ) 'trusted policy implementation'
    Assert-ContainsAll $RolloutSource @(
        'pub fn issue_successor_with_previous_authority',
        'previous_authority.ok_or(RolloutError::InvalidPolicyCombination)?'
    ) 'rotated policy implementation'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'binds one signed rollout policy to the exact root-signed keyring',
        'exactly one root-signed keyring successor',
        'cannot sign a normal policy that expands enrollment',
        'public `legacy-only` behavior remain unchanged'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "trusted policy RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $sourcePath, $rolloutSourcePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath $sourcePath -Raw
$rolloutSource = Get-Content -LiteralPath $rolloutSourcePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $source $rolloutSource $rfc

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.purpose_boundary.containment_only_can_expand = $true
    try { Assert-Contract $mutated $source $rolloutSource $rfc } catch {
        Write-Output 'containment_key_rollout_expansion=rejected'
        Write-Output 'canonical_sync_v2_trusted_policy_bundle_self_test=passed'
        exit 0
    }
    throw 'containment-only signer was allowed to expand rollout'
}

Write-Output 'trusted_policy_bundle_max_bytes=8192'
Write-Output 'operational_key_rotation=exact-successor-only'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_trusted_policy_bundle_boundary=verified'
