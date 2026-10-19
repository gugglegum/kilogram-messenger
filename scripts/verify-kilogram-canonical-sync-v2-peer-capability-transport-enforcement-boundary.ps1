[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-CAPABILITY-TRANSPORT-ENFORCEMENT.json'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0230-m1206-peer-capability-transport-enforcement.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Cli, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.2.6' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-capability-transport-enforcement-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [bool]$Manifest.outbound_sync_boundary.unknown_extension_free_first_contact_creates_store -or
        -not [bool]$Manifest.outbound_sync_boundary.authenticated_canonical_candidate_is_pinned_before_network -or
        -not [bool]$Manifest.outbound_sync_boundary.extension_stripping_after_pin_is_durably_quarantined -or
        [bool]$Manifest.outbound_sync_boundary.quarantined_peer_reaches_network -or
        -not [bool]$Manifest.resume_boundary.persisted_mutually_signed_selection_is_authenticated_evidence -or
        [bool]$Manifest.resume_boundary.selected_canonical_resume_may_fall_back_to_legacy -or
        -not [bool]$Manifest.resume_boundary.new_canonical_selection_requires_current_ticket_candidate -or
        [bool]$Manifest.activation_boundary.automatic_contact_enrollment_enabled -or
        [bool]$Manifest.activation_boundary.automatic_quarantine_recovery_enabled -or
        [bool]$Manifest.activation_boundary.runtime_ipc_integration_enabled -or
        [bool]$Manifest.activation_boundary.public_canonical_rollout_enabled) {
        throw 'peer capability transport enforcement boundary changed'
    }

    Assert-ContainsAll $Cli @(
        'enum PeerCapabilityTransportEvidenceSource',
        'PersistedCanonicalSelection',
        'fn peer_sync_capability_evidence(',
        'fn apply_peer_capability_floor_before_sync(',
        'durably apply authenticated peer capability before sync transport selection',
        'peer sync capability floor is quarantined:',
        'peer_capability_transport_unknown_legacy_does_not_create_store',
        'peer_capability_transport_pins_authenticated_canonical_before_use',
        'peer_capability_transport_stripping_quarantines_before_network',
        'peer_capability_transport_persisted_selection_is_resume_authority'
    ) 'peer capability transport CLI'

    $syncStart = $Cli.IndexOf('async fn sync(', [StringComparison]::Ordinal)
    $enforcement = $Cli.IndexOf(
        'let peer_capability_authorization = apply_peer_capability_floor_before_sync(',
        $syncStart,
        [StringComparison]::Ordinal
    )
    $endpointBind = $Cli.IndexOf(
        'let endpoint = endpoint_builder_for_remote(',
        $enforcement,
        [StringComparison]::Ordinal
    )
    if ($syncStart -lt 0 -or $enforcement -lt 0 -or $endpointBind -lt 0 -or
        $enforcement -ge $endpointBind) {
        throw 'peer capability floor is not enforced before outbound endpoint bind'
    }

    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'before an Iroh endpoint is bound',
        'does not create an otherwise absent capability database',
        'cannot erase that evidence, force legacy fallback',
        'public policy default remains `legacy-only`',
        'generic first-contact extension stripping remains impossible to detect'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer capability transport RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $cliPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $cli $rfc

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.outbound_sync_boundary.quarantined_peer_reaches_network = $true
    try { Assert-Contract $mutated $cli $rfc } catch {
        Write-Output 'quarantined_peer_network_use=rejected'
        Write-Output 'canonical_sync_v2_peer_capability_transport_enforcement_self_test=passed'
        exit 0
    }
    throw 'quarantined peer was allowed to reach network transport'
}

Write-Output 'network_executed=false'
Write-Output 'runtime_ipc_integration_enabled=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_capability_transport_enforcement_boundary=verified'
