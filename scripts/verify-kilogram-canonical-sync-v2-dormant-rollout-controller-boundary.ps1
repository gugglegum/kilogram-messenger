[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-DORMANT-ROLLOUT-CONTROLLER.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-rollout\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0219-m1195-dormant-rollout-controller.md'

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
    if ([string]$Manifest.milestone -cne 'M1.1.95' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-dormant-rollout-controller-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [bool]$Manifest.controller.writes_activation_authority -or
        [bool]$Manifest.controller.runtime_activation_authorized -or
        [bool]$Manifest.controller.release_policy_can_sign_activation -or
        -not [bool]$Manifest.controller.requires_explicit_device_approval -or
        [int]$Manifest.controller.maximum_normal_advance_per_decision -ne 1 -or
        [bool]$Manifest.controller.supports_canonical_required_target -or
        -not [bool]$Manifest.containment.preserves_current_activation_target -or
        -not [bool]$Manifest.containment.requires_existing_authenticated_canonical_evidence -or
        [bool]$Manifest.containment.can_enroll_legacy_only_installation -or
        [bool]$Manifest.containment.can_clear_itself -or
        [bool]$Manifest.activation_boundary.runtime_consumer_exists -or
        [bool]$Manifest.activation_boundary.desktop_consumer_exists -or
        [bool]$Manifest.activation_boundary.cli_consumer_exists -or
        [bool]$Manifest.activation_boundary.automatic_enrollment_enabled) {
        throw 'dormant rollout controller boundary changed'
    }
    Assert-ContainsAll $Source @(
        'pub enum RolloutControllerDecision',
        'pub fn evaluate_rollout_controller',
        'requires_explicit_device_approval: true',
        'release_policy_can_sign_activation: false',
        'pub const fn runtime_activation_authorized(self) -> bool',
        'ContainmentWithoutCanonicalEvidence',
        'current.target.next()'
    ) 'rollout controller'
    foreach ($forbidden in @('kilogram_session', 'kilogram_runtime_ipc', 'kilogram_transport_iroh')) {
        if ($Source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "dormant rollout controller contains forbidden consumer '$forbidden'"
        }
    }
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'advance by exactly one activation state',
        'cannot use containment as an enrollment path',
        'no automatic Device approval',
        'public default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "rollout controller RFC is missing '$needle'"
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
    $mutated.controller.release_policy_can_sign_activation = $true
    try { Assert-Contract $mutated $source $rfc } catch {
        Write-Output 'release_policy_activation_signature=rejected'
        Write-Output 'canonical_sync_v2_dormant_rollout_controller_self_test=passed'
        exit 0
    }
    throw 'release policy was allowed to sign activation'
}

Write-Output 'network_executed=false'
Write-Output 'runtime_integration_enabled=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_dormant_rollout_controller_boundary=verified'
