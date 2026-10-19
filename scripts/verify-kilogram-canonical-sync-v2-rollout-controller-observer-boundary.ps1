[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-ROLLOUT-CONTROLLER-OBSERVER.json'
$rolloutPath = Join-Path $workspace 'crates\kilogram-rollout\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0220-m1196-read-only-rollout-controller-observer.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Rollout, [string]$Cli, [string]$CliCargo, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.1.96' -or
        [string]$Manifest.status -cne
            'canonical-sync-v2-rollout-controller-observer-verified-read-only' -or
        [bool]$Manifest.changes_peer_network_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.observer.opens_existing_rollout_database_read_only -or
        -not [bool]$Manifest.observer.missing_directory_is_hold_without_creation -or
        -not [bool]$Manifest.observer.requires_explicit_release_authority -or
        [bool]$Manifest.observer.writes_rollout_state -or
        [bool]$Manifest.observer.writes_device_activation_state -or
        [bool]$Manifest.observer.uses_runtime_ipc -or
        [bool]$Manifest.observer.executes_network -or
        [bool]$Manifest.privacy.prints_installation_seed -or
        [bool]$Manifest.privacy.prints_cohort_bucket -or
        [bool]$Manifest.privacy.prints_message_or_history_data -or
        [bool]$Manifest.authority_boundary.runtime_activation_authorized -or
        -not [bool]$Manifest.authority_boundary.explicit_device_approval_still_required -or
        [bool]$Manifest.authority_boundary.release_policy_can_sign_activation -or
        [bool]$Manifest.authority_boundary.production_release_key_exists -or
        [bool]$Manifest.authority_boundary.policy_delivery_exists -or
        [bool]$Manifest.authority_boundary.automatic_enrollment_enabled) {
        throw 'read-only rollout observer boundary changed'
    }
    Assert-ContainsAll $Rollout @(
        'pub fn inspect_existing_rollout_store',
        'ReadOnlyDatabase::open',
        'Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None)',
        'health_table.iter()',
        'RolloutReadOnlySnapshot'
    ) 'rollout read-only inspection'
    Assert-ContainsAll $Cli @(
        'RolloutControllerStatus',
        'fn show_rollout_controller_status',
        'rollout_runtime_activation_authorized={}',
        'status=rollout-controller-observed-read-only',
        'canonical-required is outside the bounded public rollout controller'
    ) 'CLI rollout observer'
    Assert-ContainsAll $CliCargo @(
        'kilogram-rollout = { path = "../../crates/kilogram-rollout" }'
    ) 'CLI rollout dependency'
    foreach ($forbidden in @(
        'RuntimeIpcCommand::Rollout',
        'runtime_rollout_activation_authorized=true',
        'rollout_installation_seed='
    )) {
        if ($Cli.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "CLI rollout observer contains forbidden surface '$forbidden'"
        }
    }
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'offline observer rather than a rollout control plane',
        'without creating either',
        'existing explicit Device plan/apply ceremony remains mandatory',
        'public `legacy-only` default are unchanged'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "rollout observer RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $rolloutPath, $cliPath, $cliCargoPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$rollout = Get-Content -LiteralPath $rolloutPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $rollout $cli $cliCargo $rfc

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.observer.writes_device_activation_state = $true
    try { Assert-Contract $mutated $rollout $cli $cliCargo $rfc } catch {
        Write-Output 'observer_device_write=rejected'
        Write-Output 'canonical_sync_v2_rollout_controller_observer_self_test=passed'
        exit 0
    }
    throw 'observer was allowed to write Device activation state'
}

Write-Output 'network_executed=false'
Write-Output 'runtime_ipc_changed=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_rollout_controller_observer_boundary=verified'
