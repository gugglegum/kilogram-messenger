[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-OPERATOR-UX.json'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\rollout_operator.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0228-m1204-rollout-operator-ux.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Cli, [string]$Desktop, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.2.4' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-rollout-operator-ux-verified-local' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.observer.trusted_policy_store_read_only -or
        -not [bool]$Manifest.observer.rollout_health_store_read_only -or
        -not [bool]$Manifest.observer.rollout_health_store_root_anchored -or
        -not [bool]$Manifest.observer.rollout_health_store_rotation_aware -or
        [bool]$Manifest.observer.missing_store_created -or
        -not [bool]$Manifest.observer.desktop_view_read_only -or
        [bool]$Manifest.observer.shows_cohort_seed -or
        [bool]$Manifest.observer.shows_cohort_bucket -or
        [bool]$Manifest.observer.shows_secret_key -or
        [bool]$Manifest.observer.writes_state -or
        -not [bool]$Manifest.explicit_ceremonies.trusted_policy_import_requires_confirmation -or
        -not [bool]$Manifest.explicit_ceremonies.trusted_policy_import_requires_rollout_directory -or
        -not [bool]$Manifest.explicit_ceremonies.trusted_policy_import_synchronizes_complete_chain -or
        -not [bool]$Manifest.explicit_ceremonies.approval_plan_requires_device_confirmation -or
        [bool]$Manifest.explicit_ceremonies.approval_plan_writes_activation -or
        -not [bool]$Manifest.explicit_ceremonies.approval_apply_requires_exact_artifact_id -or
        -not [bool]$Manifest.explicit_ceremonies.recovery_is_separate_from_approval -or
        -not [bool]$Manifest.explicit_ceremonies.recovery_apply_requires_exact_artifact_id -or
        [bool]$Manifest.explicit_ceremonies.desktop_one_toggle_activation -or
        [bool]$Manifest.explicit_ceremonies.automatic_enrollment -or
        [bool]$Manifest.explicit_ceremonies.network_triggered_apply -or
        -not [bool]$Manifest.authority_boundary.offline_root_is_verification_only -or
        -not [bool]$Manifest.authority_boundary.release_policy_is_proposal_only -or
        -not [bool]$Manifest.authority_boundary.device_signs_approval -or
        -not [bool]$Manifest.authority_boundary.device_signs_activation) {
        throw 'Rollout operator UX authority boundary changed'
    }
    Assert-ContainsAll $Cli @(
        'RolloutOperatorStatus',
        'inspect_existing_trusted_policy_store',
        'TrustedRolloutHealthStore::open(&rollout_dir, trust_root)',
        'synchronize rollout health with complete trusted-policy chain',
        'rollout_operator_writes_state=false',
        'confirm_import',
        'confirm_device_approval',
        'expected_approval_id',
        'confirm_device_recovery',
        'expected_recovery_id',
        'write_new_authority_file',
        'SignedCanonicalSyncV2ActivationHighWater::issue'
    ) 'Rollout operator CLI'
    Assert-ContainsAll $Desktop @(
        'Read-only release trust',
        'This panel cannot enroll this Device',
        'Refresh rollout trust state',
        'rollout_operator_writes_state',
        'rollout_operator_explicit_device_approval_required',
        'parser_rejects_write_claim_partial_policy_and_implicit_approval'
    ) 'Rollout operator desktop view'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'observation does not create a directory, database, identity or rollout seed',
        'restart-safely synchronizes health across the complete verified bundle chain',
        'no rollout toggle, enrollment action, approval button or recovery button',
        'Plan and apply cannot be collapsed into a single toggle',
        'public default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Rollout operator UX RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $cliPath, $desktopPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $cli $desktop $rfc

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.explicit_ceremonies.desktop_one_toggle_activation = $true
    try { Assert-Contract $mutated $cli $desktop $rfc } catch {
        Write-Output 'desktop_one_toggle_activation=rejected'
        Write-Output 'canonical_sync_v2_rollout_operator_ux_self_test=passed'
        exit 0
    }
    throw 'Desktop silently gained one-toggle rollout authority'
}

Write-Output 'rollout_operator_observer=read-only'
Write-Output 'device_approval=explicit-plan-then-exact-apply'
Write-Output 'containment_recovery=separate-explicit-ceremony'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_rollout_operator_ux_boundary=verified'
