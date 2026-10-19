[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-RELEASE-KEYRING.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\trust.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0223-m1199-offline-release-keyring.md'

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
    if ([string]$Manifest.milestone -cne 'M1.1.99' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-offline-release-keyring-verified-local' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [bool]$Manifest.trust_model.offline_root_embedded -or
        [bool]$Manifest.trust_model.private_key_embedded -or
        -not [bool]$Manifest.trust_model.root_signed_complete_snapshots -or
        [int]$Manifest.trust_model.maximum_operational_keys -ne 8 -or
        -not [bool]$Manifest.trust_model.monotonic_epoch -or
        -not [bool]$Manifest.trust_model.exact_predecessor_binding -or
        -not [bool]$Manifest.trust_model.revocation_is_irreversible -or
        -not [bool]$Manifest.purpose_separation.containment_only_key_cannot_sign_normal_policy -or
        [bool]$Manifest.activation_boundary.runtime_consumer_exists -or
        [bool]$Manifest.activation_boundary.desktop_consumer_exists -or
        [bool]$Manifest.activation_boundary.cli_consumer_exists -or
        [bool]$Manifest.activation_boundary.policy_delivery_exists -or
        [bool]$Manifest.activation_boundary.production_trust_root_exists -or
        [bool]$Manifest.activation_boundary.automatic_enrollment_enabled -or
        [bool]$Manifest.activation_boundary.runtime_activation_authorized) {
        throw 'offline release keyring boundary changed'
    }
    foreach ($failure in @(
        'foreign_root', 'signature_tamper', 'epoch_gap', 'forked_predecessor',
        'key_removal_or_mutation', 'duplicate_or_unsorted_key',
        'unknown_or_repeated_revocation', 'oversized_keyring'
    )) {
        if ([string]$Manifest.negative_matrix.$failure -cne 'fail-closed') {
            throw "release keyring failure '$failure' is not fail-closed"
        }
    }
    Assert-ContainsAll $Source @(
        'pub const MAX_RELEASE_SIGNING_KEYS: usize = 8;',
        'pub struct ReleaseTrustRoot',
        'pub struct SignedReleaseKeyring',
        'pub fn verify_successor_of',
        'SigningKeyRemoved',
        'InvalidRevocationTransition',
        'NoActiveContainmentKey',
        'SigningPurposeDenied',
        'fn rotation_revokes_old_key_without_removing_history',
        'fn successor_validation_rejects_fork_and_epoch_skip'
    ) 'release keyring implementation'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'separates a long-lived offline trust root',
        'at most eight operational Ed25519 public keys',
        'containment-only key cannot authorize a normal rollout policy',
        'public default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "release keyring RFC is missing '$needle'"
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
    $mutated.purpose_separation.containment_only_key_cannot_sign_normal_policy = $false
    try { Assert-Contract $mutated $source $rfc } catch {
        Write-Output 'wrong_purpose_signer=rejected'
        Write-Output 'canonical_sync_v2_release_keyring_self_test=passed'
        exit 0
    }
    throw 'containment-only signer was allowed to authorize normal rollout'
}

Write-Output 'release_keyring_bound=8'
Write-Output 'runtime_consumer=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_release_keyring_boundary=verified'
