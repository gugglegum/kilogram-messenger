[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-CAPABILITY-STORE.json'
$storePath = Join-Path $workspace 'crates\kilogram-peer-capability-store\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0218-m1194-durable-peer-capability-store.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Store, [string]$Cli, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.1.94' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-capability-store-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [bool]$Manifest.store.stores_ticket_or_invitation_body -or
        [bool]$Manifest.store.network_dependency -or
        -not [bool]$Manifest.monotonicity.extension_stripping_after_pin_quarantines -or
        [bool]$Manifest.monotonicity.network_can_clear_quarantine -or
        -not [bool]$Manifest.monotonicity.retained_tail_rollback_detected -or
        [bool]$Manifest.monotonicity.whole_store_rollback_detected_without_external_witness -or
        [bool]$Manifest.activation_boundary.runtime_reads_store -or
        [bool]$Manifest.activation_boundary.rollout_policy_reads_store -or
        [bool]$Manifest.activation_boundary.automatic_enrollment_enabled -or
        [bool]$Manifest.activation_boundary.automatic_quarantine_recovery_enabled) {
        throw 'durable peer capability store boundary changed'
    }
    Assert-ContainsAll $Store @(
        'pub struct PeerCapabilityStore',
        'Durability::Immediate',
        'SignedPeerCapabilityRecord',
        'apply_current_invitation',
        'apply_authenticated_observation',
        'OrphanPeerRecord',
        'PeerSyncCapabilityFloor::Quarantined'
    ) 'peer capability store'
    foreach ($forbidden in @('reqwest', 'iroh::', 'tokio::net', 'clear_quarantine')) {
        if ($Store.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "peer capability store contains forbidden runtime/network surface '$forbidden'"
        }
    }
    Assert-ContainsAll $Cli @(
        'CurrentPeerInvitationImport',
        'PeerCapabilityStatus',
        'apply_current_invitation',
        'apply_authenticated_observation',
        'current peer remains quarantined; refusing to export its embedded ticket',
        'status=current-peer-invitation-imported'
    ) 'peer capability CLI'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Only after the durable pin succeeds',
        'never accepts a detached legacy ticket as a substitute',
        'Coordinated rollback of the entire database',
        'public default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer capability RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $storePath, $cliPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$store = Get-Content -LiteralPath $storePath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $store $cli $rfc

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.monotonicity.network_can_clear_quarantine = $true
    try { Assert-Contract $mutated $store $cli $rfc } catch {
        Write-Output 'network_quarantine_recovery=rejected'
        Write-Output 'canonical_sync_v2_peer_capability_store_self_test=passed'
        exit 0
    }
    throw 'network-driven quarantine recovery was accepted'
}

Write-Output 'network_executed=false'
Write-Output 'runtime_integration_enabled=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_capability_store_boundary=verified'
