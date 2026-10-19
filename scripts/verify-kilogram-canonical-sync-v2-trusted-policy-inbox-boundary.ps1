[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-TRUSTED-POLICY-INBOX.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\trusted_policy_store.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0225-m1201-atomic-policy-inbox.md'

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
    if ([string]$Manifest.milestone -cne 'M1.2.1' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-atomic-policy-inbox-verified-local' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.input_boundary.explicit_user_or_test_operator_request_required -or
        [int]$Manifest.input_boundary.maximum_input_bytes -ne 8192 -or
        [bool]$Manifest.input_boundary.automatic_download -or
        [bool]$Manifest.input_boundary.background_polling -or
        [bool]$Manifest.input_boundary.remote_administration -or
        [int]$Manifest.durability.maximum_retained_bundles -ne 32 -or
        -not [bool]$Manifest.durability.record_and_head_same_transaction -or
        -not [bool]$Manifest.durability.exact_replay_idempotent -or
        -not [bool]$Manifest.durability.full_bounded_chain_reverified_on_open -or
        [string]$Manifest.durability.whole_database_rollback_without_external_witness -cne 'not-detectable-non-claim' -or
        [bool]$Manifest.activation_boundary.runtime_consumer_exists -or
        [bool]$Manifest.activation_boundary.desktop_consumer_exists -or
        [bool]$Manifest.activation_boundary.cli_consumer_exists -or
        [bool]$Manifest.activation_boundary.store_grants_activation_authority -or
        [bool]$Manifest.activation_boundary.production_trust_root_exists -or
        [bool]$Manifest.activation_boundary.automatic_enrollment_enabled) {
        throw 'trusted policy inbox boundary changed'
    }
    if ([string]$Manifest.input_boundary.symlink_input -cne 'fail-closed' -or
        [string]$Manifest.input_boundary.changed_during_read -cne 'fail-closed') {
        throw 'unsafe trusted-policy input is not fail-closed'
    }
    foreach ($failure in @(
        'malformed_or_tampered_bundle', 'foreign_trust_root',
        'stale_or_forked_policy', 'keyring_skip_or_fork'
    )) {
        if ([string]$Manifest.quarantine_matrix.$failure -cne 'reject-without-head-change') {
            throw "trusted-policy quarantine '$failure' can change the durable head"
        }
    }
    Assert-ContainsAll $Source @(
        'pub const MAX_DURABLE_TRUSTED_POLICY_BUNDLES: u64 = 32;',
        'UserOrTestOperatorRequested',
        'pub fn import_explicit_file',
        'metadata.file_type().is_symlink()',
        'Durability::Immediate',
        'candidate.verify_successor_of(current)?;',
        'ConcurrentHeadChanged',
        'HeadIsNotChainTip',
        'fn explicit_import_is_atomic_idempotent_and_restart_stable',
        'fn invalid_stale_and_unauthorized_inputs_never_change_head'
    ) 'trusted policy inbox implementation'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'explicit `UserOrTestOperatorRequested` authorization',
        'record and its replacement head are committed in one immediate-durability transaction',
        'requires an external monotonic witness',
        'public `legacy-only` behavior remain unchanged'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "trusted policy inbox RFC is missing '$needle'"
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
    $mutated.input_boundary.automatic_download = $true
    try { Assert-Contract $mutated $source $rfc } catch {
        Write-Output 'automatic_policy_download=rejected'
        Write-Output 'canonical_sync_v2_trusted_policy_inbox_self_test=passed'
        exit 0
    }
    throw 'trusted policy inbox silently gained automatic download authority'
}

Write-Output 'trusted_policy_history_bound=32'
Write-Output 'policy_download=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_trusted_policy_inbox_boundary=verified'
