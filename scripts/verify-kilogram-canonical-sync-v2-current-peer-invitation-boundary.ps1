[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-CURRENT-PEER-INVITATION.json'
$capabilityPath = Join-Path $workspace 'crates\kilogram-sync-capability\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0217-m1193-current-peer-invitation.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Capability, [string]$Cli, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.1.93' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-current-peer-invitation-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [string]$Manifest.artifact.signature -cne 'listener-device-ed25519' -or
        [string]$Manifest.artifact.required_protocol -cne 'kilogram/m1/sync/cbor/2' -or
        -not [bool]$Manifest.cli.create_is_no_clobber -or
        -not [bool]$Manifest.cli.inspect_verifies_embedded_ticket_and_capability -or
        [bool]$Manifest.cli.prints_embedded_ticket -or
        [bool]$Manifest.cli.automatically_enrolls_contact -or
        [bool]$Manifest.cli.automatically_connects -or
        [bool]$Manifest.compatibility.current_importer_may_ignore_missing_outer_invitation -or
        [bool]$Manifest.activation_boundary.durable_floor_store_implemented -or
        [bool]$Manifest.activation_boundary.runtime_consumes_invitation -or
        [bool]$Manifest.activation_boundary.rollout_policy_consumes_invitation -or
        [bool]$Manifest.activation_boundary.automatic_enrollment_enabled) {
        throw 'current peer invitation boundary changed'
    }
    Assert-ContainsAll $Capability @(
        'pub struct SignedCurrentPeerInvitation',
        '#[serde(deny_unknown_fields)]',
        'CURRENT_PEER_INVITATION_SIGNATURE_DOMAIN',
        'CURRENT_PEER_INVITATION_ID_DOMAIN',
        'required_capability_id',
        'connection_ticket',
        'pub fn required_floor'
    ) 'current peer invitation library'
    Assert-ContainsAll $Cli @(
        'CurrentPeerInvitationCreate',
        'CurrentPeerInvitationInspect',
        'create_current_peer_invitation',
        'inspect_current_peer_invitation',
        'write_new_authority_file(&output_file, &encoded)',
        'current_peer_invitation_ticket_embedded=true',
        'status=current-peer-invitation-verified'
    ) 'current peer invitation CLI'
    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'embeds the exact base64url M1.0.0-compatible connection ticket',
        'must not fall back',
        'does not enroll a contact',
        'public `legacy-only` default'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "current peer invitation RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $capabilityPath, $cliPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$capability = Get-Content -LiteralPath $capabilityPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $capability $cli $rfc

if ($SelfTest) {
    $mutated = $manifest | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $mutated.cli.automatically_enrolls_contact = $true
    try { Assert-Contract $mutated $capability $cli $rfc } catch {
        Write-Output 'automatic_enrollment=rejected'
        Write-Output 'canonical_sync_v2_current_peer_invitation_self_test=passed'
        exit 0
    }
    throw 'automatic invitation enrollment was accepted'
}

Write-Output 'network_executed=false'
Write-Output 'runtime_integration_enabled=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_current_peer_invitation_boundary=verified'
