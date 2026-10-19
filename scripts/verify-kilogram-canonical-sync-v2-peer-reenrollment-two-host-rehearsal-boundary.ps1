[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-REENROLLMENT-TWO-HOST-REHEARSAL.json'
$evidencePath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-REENROLLMENT-TWO-HOST-EVIDENCE.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0237-m1213-peer-reenrollment-two-host-rehearsal.md'
$fieldPath = Join-Path $workspace 'scripts\invoke-kilogram-peer-reenrollment-two-host-rehearsal.ps1'

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param([object]$Manifest, [object]$Evidence, [string]$Rfc, [string]$Field)

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.13' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-reenrollment-two-host-rehearsal-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'Peer re-enrollment two-host identity or scope changed'
    }

    foreach ($property in @(
        'exact_same_binary_on_two_physical_hosts',
        'predecessor_quarantine_retained',
        'replacement_device_root_authorized',
        'predecessor_readable_history_rewrapped_offline',
        'explicit_offline_plan_approval_acceptance',
        'descriptor_substitution_rejected_before_network',
        'fresh_start_consumes_successor_before_endpoint',
        'ordinary_scheduler_delivers_queued_item_once',
        'restart_replay_is_idempotent',
        'complete_histories_converge'
    )) {
        if (-not [bool]$Manifest.rehearsal_boundary.$property) {
            throw "Rehearsal property '$property' changed"
        }
    }

    $acceptance = $Manifest.acceptance
    if ([string]$acceptance.two_host_exact_binary_rehearsal -cne 'verified' -or
        [string]$acceptance.field_evidence -cne 'CANONICAL-SYNC-V2-PEER-REENROLLMENT-TWO-HOST-EVIDENCE.json' -or
        [string]$acceptance.implementation_revision -cne 'd307e965f338e11ee786af75a8e3f71309b42fa3' -or
        [string]$acceptance.binary_sha256 -cne '31e3b6e1a925526b5fee1d1bbe4205eb3ed732106e5b1ca479c3e5c5855d4f0f' -or
        [int64]$acceptance.binary_bytes -ne 62375424 -or
        [string]$acceptance.preceding_identical_binary_cli_regression -cne '101/101 binary + 1/1 library' -or
        [string]$acceptance.runtime_store_tests -cne '5/5' -or
        [string]$acceptance.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$acceptance.rustfmt -cne 'verified' -or
        [string]$acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [string]$acceptance.aggregate_readiness -cne '139/139' -or
        [bool]$acceptance.release_build -or
        [bool]$acceptance.archive_created -or
        [bool]$acceptance.git_tag_created -or
        [bool]$acceptance.automatic_github_build) {
        throw 'Peer re-enrollment two-host acceptance claim changed'
    }

    if ([string]$Manifest.residual_limits.transport_topology -cne 'direct-lan-only-for-this-rehearsal' -or
        [string]$Manifest.residual_limits.readable_predecessor_history_source -cne 'required-for-old-message-rewrap' -or
        [string]$Manifest.residual_limits.automatic_peer_reenrollment -cne 'not-implemented' -or
        [string]$Manifest.residual_limits.runtime_ipc_or_desktop_ceremony -cne 'not-implemented' -or
        [string]$Manifest.residual_limits.generic_first_contact_capability_stripping -cne 'not-detectable-without-prior-authenticated-state' -or
        [string]$Manifest.residual_limits.independent_security_review -cne 'required-before-accept-canary') {
        throw 'Peer re-enrollment two-host residual limits changed'
    }

    if ([int]$Evidence.schema_version -ne 1 -or
        [string]$Evidence.milestone -cne 'M1.2.13' -or
        [string]$Evidence.status -cne 'canonical-sync-v2-peer-reenrollment-two-host-rehearsal-verified' -or
        [string]$Evidence.run_id -cne 'm1213-field-20261019T171026Z' -or
        [string]$Evidence.source_revision -cne 'd307e965f338e11ee786af75a8e3f71309b42fa3' -or
        [string]$Evidence.public_default -cne 'legacy-only' -or
        [bool]$Evidence.changes_connection_ticket_wire -or
        [bool]$Evidence.changes_sync_wire -or
        [bool]$Evidence.changes_runtime_ipc -or
        [bool]$Evidence.changes_public_default) {
        throw 'Peer re-enrollment two-host evidence identity or scope changed'
    }

    if ([string]$Evidence.binary.name -cne 'kilogram-cli.exe' -or
        [string]$Evidence.binary.stable_local_path -cne '.tmp/kilogram-field-runner/kilogram-cli.exe' -or
        [string]$Evidence.binary.stable_remote_path -cne 'KilogramLab/field-kit-current/kilogram-cli.exe' -or
        [string]$Evidence.binary.sha256 -cne '31e3b6e1a925526b5fee1d1bbe4205eb3ed732106e5b1ca479c3e5c5855d4f0f' -or
        [int64]$Evidence.binary.bytes -ne 62375424 -or
        [string]$Evidence.binary.profile -cne 'debug' -or
        [string]$Evidence.binary.target -cne 'x86_64-pc-windows-msvc' -or
        -not [bool]$Evidence.topology.distinct_physical_hosts -or
        -not [bool]$Evidence.topology.binary_hashes_equal -or
        [string]$Evidence.topology.transport -cne 'iroh-direct-lan' -or
        -not [bool]$Evidence.topology.ssh_is_control_channel_only) {
        throw 'Peer re-enrollment two-host binary or topology evidence changed'
    }

    if ([string]$Evidence.prerequisite.status -cne 'canonical-sync-v2-peer-capability-two-host-rehearsal-verified' -or
        [string]$Evidence.prerequisite.old_peer_floor -cne 'quarantined' -or
        [string]$Evidence.prerequisite.old_peer_quarantine_reason -cne 'pinned-canonical-capability-missing') {
        throw 'Peer re-enrollment prerequisite evidence changed'
    }

    $reenrollment = $Evidence.reenrollment
    foreach ($property in @(
        'predecessor_contact_enrolled',
        'replacement_device_root_authorized',
        'predecessor_history_rewrapped_offline',
        'replacement_endpoint_candidate_enrolled',
        'explicit_plan_approval_acceptance',
        'transition_prepared_before_restart',
        'descriptor_substitution_rejected',
        'fresh_start_consumed',
        'exact_replay_already_consumed',
        'histories_equal',
        'predecessor_contact_byte_exact',
        'old_quarantine_retained'
    )) {
        if (-not [bool]$reenrollment.$property) {
            throw "Re-enrollment evidence '$property' changed"
        }
    }
    if ([bool]$reenrollment.ceremony_network_executed -or
        [bool]$reenrollment.transition_consumed_before_restart -or
        [bool]$reenrollment.descriptor_substitution_peer_network_attempted -or
        [int]$reenrollment.predecessor_history_rewrapped_event_count -ne 6 -or
        [string]$reenrollment.consumption_phase -cne 'before-network-endpoint' -or
        [string]$reenrollment.queued_delivery_status -cne 'delivered' -or
        [string]$reenrollment.queued_delivery_path -cne 'direct' -or
        [int]$reenrollment.queued_delivery_count -ne 1 -or
        [int]$reenrollment.replay_outbound_actions -ne 0 -or
        [int]$reenrollment.complete_history_event_count -ne 8 -or
        [int]$reenrollment.queued_message_occurrences -ne 1) {
        throw 'Peer re-enrollment semantic evidence changed'
    }

    foreach ($property in @(
        'raw_logs_tracked', 'state_tracked', 'ticket_bodies_tracked',
        'account_or_device_ids_tracked', 'endpoint_addresses_tracked',
        'event_or_queue_ids_tracked', 'secrets_tracked'
    )) {
        if ([bool]$Evidence.privacy.$property) {
            throw "Evidence privacy property '$property' changed"
        }
    }

    Assert-ContainsAll $Field @(
        "invoke-kilogram-peer-capability-two-host-rehearsal.ps1",
        "history-rewrap-export",
        "history-rewrap-import",
        "source_inventory_complete' 'true'",
        "runtime_outbound_network_attempted' 'false'",
        'accepted peer re-enrollment endpoint descriptor changed',
        "peer_reenrollment_runtime_consumption_phase' 'before-network-endpoint'",
        "runtime_outbound_status' 'delivered'",
        "peer_reenrollment_runtime_transitions_already_consumed' '1'",
        "runtime_outbound_actions' '0'",
        'post-re-enrollment histories differ across physical hosts',
        'predecessor contact file changed after consumption',
        'status=canonical-sync-v2-peer-reenrollment-two-host-rehearsal-verified'
    ) 'Peer re-enrollment two-host field harness'

    foreach ($forbidden in @('cargo test', '--release', 'Compress-Archive', 'git tag', 'git push')) {
        if ($Field.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "Peer re-enrollment field harness gained forbidden surface '$forbidden'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'history-rewrap mechanism before network sync',
        'fail closed with zero peer network attempt',
        'consume the successor before construction of the Iroh endpoint',
        'delivers the pre-existing sealed queue item once',
        'converges eight events',
        'public default remains `legacy-only`',
        'not an independent security review'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Peer re-enrollment two-host RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $evidencePath, $rfcPath, $fieldPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$evidence = Get-Content -LiteralPath $evidencePath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$field = Get-Content -LiteralPath $fieldPath -Raw
Assert-Contract $manifest $evidence $rfc $field

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.rehearsal_boundary.descriptor_substitution_rejected_before_network = $false
    $mutations += [pscustomobject]@{ Label = 'descriptor_substitution_reached_network'; Manifest = $changed; Evidence = $evidence }

    $changed = Copy-JsonObject $manifest
    $changed.public_default = 'dual-reader-prefer-canonical'
    $mutations += [pscustomobject]@{ Label = 'public_default_changed'; Manifest = $changed; Evidence = $evidence }

    $changedEvidence = Copy-JsonObject $evidence
    $changedEvidence.reenrollment.predecessor_history_rewrapped_offline = $false
    $mutations += [pscustomobject]@{ Label = 'history_rewrap_omitted'; Manifest = $manifest; Evidence = $changedEvidence }

    $changedEvidence = Copy-JsonObject $evidence
    $changedEvidence.reenrollment.descriptor_substitution_peer_network_attempted = $true
    $mutations += [pscustomobject]@{ Label = 'substitution_network_attempt'; Manifest = $manifest; Evidence = $changedEvidence }

    $changedEvidence = Copy-JsonObject $evidence
    $changedEvidence.reenrollment.replay_outbound_actions = 1
    $mutations += [pscustomobject]@{ Label = 'duplicate_restart_action'; Manifest = $manifest; Evidence = $changedEvidence }

    $changedEvidence = Copy-JsonObject $evidence
    $changedEvidence.privacy.account_or_device_ids_tracked = $true
    $mutations += [pscustomobject]@{ Label = 'identity_leak_in_evidence'; Manifest = $manifest; Evidence = $changedEvidence }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            Assert-Contract $mutation.Manifest $mutation.Evidence $rfc $field
        }
        catch {
            Write-Output "$($mutation.Label)=rejected"
            $rejected++
            continue
        }
        throw "$($mutation.Label) mutation was unexpectedly accepted"
    }
    if ($rejected -ne 6) {
        throw "Peer re-enrollment two-host mutation matrix incomplete: $rejected/6"
    }
    Write-Output 'peer_reenrollment_two_host_negative_mutations=6/6'
    Write-Output 'canonical_sync_v2_peer_reenrollment_two_host_rehearsal_self_test=passed'
    exit 0
}

Write-Output 'descriptor_substitution=rejected-before-network'
Write-Output 'predecessor_history_rewrap=6/6-offline'
Write-Output 'queued_delivery=exactly-once-direct'
Write-Output 'history_convergence=8-events-equal'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_reenrollment_two_host_rehearsal_boundary=verified'
