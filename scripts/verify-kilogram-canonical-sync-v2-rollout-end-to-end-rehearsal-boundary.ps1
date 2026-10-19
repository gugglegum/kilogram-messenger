[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-REHEARSAL.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0229-m1205-rollout-end-to-end-rehearsal.md'
$rehearsalPath = Join-Path $workspace 'crates\kilogram-rollout\src\end_to_end_rehearsal.rs'
$healthPath = Join-Path $workspace 'crates\kilogram-rollout\src\trusted_health_store.rs'
$policyStorePath = Join-Path $workspace 'crates\kilogram-rollout\src\trusted_policy_store.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$fieldPath = Join-Path $workspace 'scripts\invoke-kilogram-rollout-two-host-portability.ps1'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [string]$Rfc,
        [string]$Rehearsal,
        [string]$Health,
        [string]$PolicyStore,
        [string]$Cli,
        [string]$Field
    )
    if ([string]$Manifest.milestone -cne 'M1.2.5' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.control_plane.offline_root_anchor -or
        -not [bool]$Manifest.control_plane.operational_key_rotation -or
        -not [bool]$Manifest.control_plane.complete_policy_chain_required -or
        -not [bool]$Manifest.control_plane.trusted_health_rotation_aware -or
        -not [bool]$Manifest.control_plane.explicit_device_approval -or
        -not [bool]$Manifest.control_plane.emergency_containment -or
        -not [bool]$Manifest.control_plane.explicit_device_recovery -or
        [bool]$Manifest.control_plane.automatic_enrollment -or
        [bool]$Manifest.control_plane.network_triggered_apply -or
        -not [bool]$Manifest.crash_boundary.record_before_high_water_repair -or
        -not [bool]$Manifest.crash_boundary.exact_device_signed_artifact_required -or
        -not [bool]$Manifest.crash_boundary.current_policy_health_and_predecessor_rechecked -or
        -not [bool]$Manifest.crash_boundary.altered_candidate_rejected -or
        [string]$Manifest.acceptance.rollout_unit_tests -cne '56/56' -or
        [string]$Manifest.acceptance.named_end_to_end_rehearsal -cne 'verified-local' -or
        [string]$Manifest.acceptance.cli_interrupted_apply_repair -cne 'verified-local' -or
        [string]$Manifest.acceptance.boundary_self_test -cne 'verified' -or
        [string]$Manifest.acceptance.rustfmt -cne 'verified' -or
        [string]$Manifest.acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [string]$Manifest.acceptance.stable_full_workspace_regression -cne 'verified' -or
        [string]$Manifest.acceptance.aggregate_readiness -cne '131/131' -or
        [string]$Manifest.acceptance.legacy_decoder_mutations -cne '31851/31851' -or
        [string]$Manifest.acceptance.canonical_sync_v2_decoder_mutations -cne '2863/2863' -or
        [bool]$Manifest.acceptance.release_build -or
        [bool]$Manifest.acceptance.archive_created -or
        [bool]$Manifest.acceptance.git_tag_created) {
        throw 'Rollout end-to-end security boundary changed'
    }
    foreach ($name in @('foreign_root', 'stale', 'truncated', 'fork')) {
        if ([string]$Manifest.negative_matrix.$name -cne 'rejected') {
            throw "Rollout negative matrix no longer rejects $name"
        }
    }
    Assert-ContainsAll $Rehearsal @(
        'canonical_sync_v2_rollout_end_to_end_rehearsal',
        'trusted_policy_chain=5/5',
        'device_approval_transitions=3/3',
        'explicit_recovery=verified',
        'negative_stale_truncated_fork=3/3',
        'default_public_activation=legacy-only'
    ) 'Rollout end-to-end rehearsal'
    Assert-ContainsAll $Health @(
        'pub fn synchronize_bundle_chain(',
        'validate_bundle_chain(bundles, self.trust_root)',
        'full_verified_chain_bootstraps_and_resumes_idempotently'
    ) 'Rotation-aware trusted health store'
    Assert-ContainsAll $PolicyStore @('pub fn bundle_chain(&self)') 'Trusted policy store'
    Assert-ContainsAll $Cli @(
        'load complete verified trusted-policy chain',
        'synchronize rollout health with complete trusted-policy chain',
        'fn inspect_interrupted_rollout_activation(',
        'fn repair_interrupted_rollout_approval(',
        'fn repair_interrupted_rollout_recovery(',
        '"atomic-repair"',
        'interrupted_rollout_approval_repairs_only_the_exact_device_signed_candidate'
    ) 'Rollout CLI'
    Assert-ContainsAll $Field @(
        '[string] $WorkingDirectory',
        "'end_to_end_rehearsal::canonical_sync_v2_rollout_end_to_end_rehearsal'",
        'rollout_test_harness_profile = ''verification''',
        'cli_profile = ''debug''',
        'p2p_network_executed = $false',
        'raw_logs_tracked = $false',
        "status = 'canonical-sync-v2-rollout-end-to-end-two-host-verified'"
    ) 'Two-host rollout harness'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'complete verified trusted-policy bundle chain',
        'same exact artifact may repair only that missing high-water',
        'public default remains `legacy-only`',
        'not an independent security review',
        'First-contact capability stripping remains unresolved'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Rollout end-to-end RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $rfcPath, $rehearsalPath, $healthPath, $policyStorePath, $cliPath, $fieldPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$rehearsal = Get-Content -LiteralPath $rehearsalPath -Raw
$health = Get-Content -LiteralPath $healthPath -Raw
$policyStore = Get-Content -LiteralPath $policyStorePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$field = Get-Content -LiteralPath $fieldPath -Raw
Assert-Contract $manifest $rfc $rehearsal $health $policyStore $cli $field

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.control_plane.automatic_enrollment = $true
    try { Assert-Contract $mutated $rfc $rehearsal $health $policyStore $cli $field } catch {
        Write-Output 'automatic_enrollment=rejected'
        Write-Output 'canonical_sync_v2_rollout_end_to_end_rehearsal_self_test=passed'
        exit 0
    }
    throw 'End-to-end rehearsal silently enabled automatic enrollment'
}

Write-Output 'trusted_policy_chain=complete-root-verified'
Write-Output 'device_apply_repair=exact-artifact-only'
Write-Output 'two_host_exact_binary_rehearsal=pending'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_rollout_end_to_end_rehearsal_boundary=verified'
