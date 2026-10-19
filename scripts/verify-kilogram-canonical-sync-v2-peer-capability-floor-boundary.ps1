[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-CAPABILITY-FLOOR.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-sync-capability\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0216-m1192-peer-capability-floor.md'

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
    if ([string]$Manifest.milestone -cne 'M1.1.92' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-capability-floor-verified-dormant' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'peer capability-floor scope changed'
    }
    if ([bool]$Manifest.first_contact_boundary.legacy_ticket_without_extension_proves_old_client -or
        [bool]$Manifest.first_contact_boundary.generic_network_detection_of_stripped_optional_extension -or
        -not [bool]$Manifest.first_contact_boundary.current_invitation_can_raise_floor_before_ticket_use -or
        [bool]$Manifest.monotonic_rules.ordinary_network_input_can_lower_floor -or
        [bool]$Manifest.monotonic_rules.pinned_capability_may_disappear -or
        [bool]$Manifest.monotonic_rules.authority_revision_may_regress -or
        [bool]$Manifest.monotonic_rules.ticket_publication_epoch_may_regress -or
        [bool]$Manifest.monotonic_rules.quarantine_self_recovers_from_network_input -or
        [bool]$Manifest.activation_boundary.state_machine_performs_io -or
        [bool]$Manifest.activation_boundary.state_machine_grants_transport_permission -or
        [bool]$Manifest.activation_boundary.durable_store_implemented -or
        [bool]$Manifest.activation_boundary.runtime_integration_enabled -or
        [bool]$Manifest.activation_boundary.automatic_enrollment_enabled) {
        throw 'peer capability-floor safety boundary changed'
    }
    Assert-ContainsAll $Source @(
        'pub enum PeerSyncCapabilityFloor',
        'LegacyUnasserted',
        'CurrentInvitationRequired',
        'CanonicalCandidatePinned',
        'Quarantined',
        'pub fn evaluate_peer_sync_capability_floor',
        'PinnedCanonicalCapabilityMissing',
        'AuthenticatedAuthorityRevisionRegression',
        'AuthenticatedTicketEpochRegression'
    ) 'peer capability-floor implementation'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'cannot tell a genuine old ticket from a current ticket',
        '`legacy-unasserted`',
        'Quarantine never clears itself from network input',
        'does not claim universal first-contact stripping detection',
        'public `legacy-only` default'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer capability-floor RFC is missing '$needle'"
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
    $mutated.monotonic_rules.quarantine_self_recovers_from_network_input = $true
    try { Assert-Contract $mutated $source $rfc } catch {
        Write-Output 'quarantine_self_recovery=rejected'
        Write-Output 'canonical_sync_v2_peer_capability_floor_self_test=passed'
        exit 0
    }
    throw 'unsafe quarantine self-recovery was accepted'
}

Write-Output 'network_executed=false'
Write-Output 'runtime_activation_enabled=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_capability_floor_boundary=verified'
