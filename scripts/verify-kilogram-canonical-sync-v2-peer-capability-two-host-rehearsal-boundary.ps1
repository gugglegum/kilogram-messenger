[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-CAPABILITY-TWO-HOST-REHEARSAL.json'
$evidencePath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-CAPABILITY-TWO-HOST-EVIDENCE.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0231-m1207-peer-capability-two-host-rehearsal.md'
$fieldPath = Join-Path $workspace 'scripts\invoke-kilogram-peer-capability-two-host-rehearsal.ps1'
$pairHarnessPath = Join-Path $workspace 'scripts\invoke-kilogram-canonical-sync-v2-dark-launch-two-host.ps1'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Get-UniqueScenario {
    param([object]$Evidence, [string]$Name)
    $matches = @($Evidence.scenarios | Where-Object { [string]$_.name -ceq $Name })
    if ($matches.Count -ne 1) { throw "Evidence must contain one scenario named '$Name'" }
    return $matches[0]
}

function Assert-Contract {
    param([object]$Manifest, [object]$Evidence, [string]$Rfc, [string]$Field, [string]$PairHarness)
    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.7' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-capability-two-host-rehearsal-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        [string]$Manifest.peer_capability_enforcement.extension_free_first_contact -cne 'legacy-no-create' -or
        [string]$Manifest.peer_capability_enforcement.signed_candidate -cne 'durably-pinned-before-network' -or
        [string]$Manifest.peer_capability_enforcement.stripping_after_pin -cne 'durably-quarantined-before-endpoint' -or
        [string]$Manifest.peer_capability_enforcement.selected_session_resume -cne 'persisted-selection-no-fallback' -or
        [bool]$Manifest.peer_capability_enforcement.generic_first_contact_stripping_detection -or
        [string]$Manifest.acceptance.two_host_exact_binary_rehearsal -cne 'verified' -or
        [string]$Manifest.acceptance.base_pair_scenarios -cne '3/3' -or
        [string]$Manifest.acceptance.field_scenarios -cne '3/3' -or
        [string]$Manifest.acceptance.field_evidence -cne 'CANONICAL-SYNC-V2-PEER-CAPABILITY-TWO-HOST-EVIDENCE.json' -or
        [string]$Manifest.acceptance.implementation_revision -cne '120fa77ff7e832fb9a285f72dfd377ee39825296' -or
        [string]$Manifest.acceptance.binary_sha256 -cne '1b58772af5ebbee4c46c929f78b2a822fc39c9dcfef47a3f50dbf0d3fb2c300b' -or
        [string]$Manifest.acceptance.preceding_identical_binary_cli_regression -cne '96/96+1/1' -or
        [string]$Manifest.acceptance.boundary_self_test -cne 'verified' -or
        [string]$Manifest.acceptance.rustfmt -cne 'verified' -or
        [string]$Manifest.acceptance.locked_offline_strict_clippy -cne 'verified' -or
        [string]$Manifest.acceptance.aggregate_readiness -cne '133/133' -or
        [bool]$Manifest.acceptance.release_build -or
        [bool]$Manifest.acceptance.archive_created -or
        [bool]$Manifest.acceptance.git_tag_created -or
        [string]$Manifest.residual_limits.generic_first_contact_capability_stripping -cne 'not-detectable-without-prior-authenticated-state' -or
        [string]$Manifest.residual_limits.automatic_runtime_sync_enforcement -cne 'not-yet-integrated' -or
        [string]$Manifest.residual_limits.automatic_quarantine_recovery -cne 'not-implemented' -or
        [string]$Manifest.residual_limits.independent_security_review -cne 'required-before-accept-canary') {
        throw 'Peer capability two-host rehearsal contract changed'
    }

    if ([int]$Evidence.schema_version -ne 1 -or
        [string]$Evidence.milestone -cne 'M1.2.7' -or
        [string]$Evidence.status -cne 'canonical-sync-v2-peer-capability-two-host-rehearsal-verified' -or
        [string]$Evidence.run_id -cne 'm1207-capability-20261019T101809Z' -or
        [string]$Evidence.source_revision -cne '120fa77ff7e832fb9a285f72dfd377ee39825296' -or
        [string]$Evidence.public_default -cne 'legacy-only' -or
        [bool]$Evidence.changes_connection_ticket_wire -or
        [bool]$Evidence.changes_sync_wire -or
        [bool]$Evidence.changes_runtime_ipc -or
        [bool]$Evidence.changes_public_default -or
        [string]$Evidence.binary.name -cne 'kilogram-cli.exe' -or
        [string]$Evidence.binary.stable_local_path -cne '.tmp/kilogram-field-runner/kilogram-cli.exe' -or
        [string]$Evidence.binary.stable_remote_path -cne 'KilogramLab/field-kit-current/kilogram-cli.exe' -or
        [string]$Evidence.binary.sha256 -cne '1b58772af5ebbee4c46c929f78b2a822fc39c9dcfef47a3f50dbf0d3fb2c300b' -or
        [int64]$Evidence.binary.bytes -ne 61414912 -or
        [string]$Evidence.binary.profile -cne 'debug' -or
        [string]$Evidence.binary.target -cne 'x86_64-pc-windows-msvc' -or
        -not [bool]$Evidence.topology.distinct_physical_hosts -or
        -not [bool]$Evidence.topology.binary_hashes_equal -or
        [string]$Evidence.topology.transport -cne 'iroh-direct-lan' -or
        -not [bool]$Evidence.topology.ssh_is_control_channel_only -or
        [string]$Evidence.base_pair_preparation.status -cne 'canonical-sync-v2-two-host-basic-pairs-prepared' -or
        [int]$Evidence.base_pair_preparation.scenarios -ne 3 -or
        @($Evidence.scenarios).Count -ne 3 -or
        [bool]$Evidence.privacy.raw_logs_tracked -or
        [bool]$Evidence.privacy.state_tracked -or
        [bool]$Evidence.privacy.ticket_bodies_tracked -or
        [bool]$Evidence.privacy.account_or_device_ids_tracked -or
        [bool]$Evidence.privacy.endpoint_addresses_tracked -or
        [bool]$Evidence.privacy.selection_evidence_ids_tracked -or
        [bool]$Evidence.privacy.secrets_tracked) {
        throw 'Peer capability two-host evidence boundary changed'
    }

    $legacy = Get-UniqueScenario $Evidence 'extension-free-first-contact'
    if (-not [bool]$legacy.synchronized -or
        [string]$legacy.selected_alpn -cne 'kilogram/m0/sync/8' -or
        [string]$legacy.semantic_use -cne 'legacy-compatibility-unasserted' -or
        [bool]$legacy.store_created -or
        [int]$legacy.event_count -ne 6 -or
        -not [bool]$legacy.histories_equal) {
        throw 'Extension-free first-contact evidence changed'
    }
    $stripped = Get-UniqueScenario $Evidence 'pin-then-strip'
    if ([string]$stripped.initial_floor -cne 'canonical-candidate-pinned' -or
        [int]$stripped.initial_record_sequence -ne 1 -or
        [bool]$stripped.stripped_sync_succeeded -or
        [bool]$stripped.endpoint_created -or
        [string]$stripped.final_floor -cne 'quarantined' -or
        [string]$stripped.quarantine_reason -cne 'pinned-canonical-capability-missing' -or
        [int]$stripped.final_record_sequence -ne 2) {
        throw 'Pin-then-strip evidence changed'
    }
    $resume = Get-UniqueScenario $Evidence 'selected-canonical-resume-with-extension-free-ticket'
    if ([string]$resume.evidence_source -cne 'persisted-canonical-selection' -or
        [string]$resume.selected_alpn -cne 'kilogram/m1/sync/cbor/2' -or
        [bool]$resume.fallback_after_selection -or
        [bool]$resume.capability_record_changed -or
        [int]$resume.record_sequence -ne 1 -or
        -not [bool]$resume.synchronized -or
        [int]$resume.event_count -ne 6 -or
        -not [bool]$resume.histories_equal -or
        -not [bool]$resume.selection_evidence_consistent) {
        throw 'Selected canonical resume evidence changed'
    }

    Assert-ContainsAll $Field @(
        "-BasicPairPreparationOnly",
        "function New-ExtensionFreeTicket",
        "source ticket is not a valid base64url JSON ticket",
        "peer_capability_transport_evidence_source' 'persisted-canonical-selection'",
        "sync_transport_fallback_after_selection' 'false'",
        "transport_endpoint_id=",
        "status=canonical-sync-v2-peer-capability-two-host-rehearsal-verified"
    ) 'Peer capability field harness'
    Assert-ContainsAll $PairHarness @(
        '[switch] $BasicPairPreparationOnly',
        "status = 'canonical-sync-v2-two-host-basic-pairs-prepared'",
        "M1207-BASIC-PAIR-EVIDENCE.json"
    ) 'Basic pair preparation harness'
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'extension-free first-contact ticket completes legacy sync without creating a capability store',
        'durable sequence-two quarantine and no `transport_endpoint_id`',
        'persisted-selection evidence and never falls back to legacy',
        'public default remains `legacy-only`',
        'not an independent security review'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Peer capability field RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $evidencePath, $rfcPath, $fieldPath, $pairHarnessPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$evidence = Get-Content -LiteralPath $evidencePath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$field = Get-Content -LiteralPath $fieldPath -Raw
$pairHarness = Get-Content -LiteralPath $pairHarnessPath -Raw
Assert-Contract $manifest $evidence $rfc $field $pairHarness

if ($SelfTest) {
    $mutated = $evidence | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    (Get-UniqueScenario $mutated 'pin-then-strip').endpoint_created = $true
    try { Assert-Contract $manifest $mutated $rfc $field $pairHarness } catch {
        Write-Output 'endpoint_before_quarantine=rejected'
        $mutated = $evidence | ConvertTo-Json -Depth 100 | ConvertFrom-Json
        (Get-UniqueScenario $mutated 'extension-free-first-contact').store_created = $true
        try { Assert-Contract $manifest $mutated $rfc $field $pairHarness } catch {
            Write-Output 'legacy_first_contact_store_creation=rejected'
            $mutated = $evidence | ConvertTo-Json -Depth 100 | ConvertFrom-Json
            (Get-UniqueScenario $mutated 'selected-canonical-resume-with-extension-free-ticket').fallback_after_selection = $true
            try { Assert-Contract $manifest $mutated $rfc $field $pairHarness } catch {
                Write-Output 'selected_legacy_fallback=rejected'
                Write-Output 'canonical_sync_v2_peer_capability_two_host_rehearsal_self_test=passed'
                exit 0
            }
        }
    }
    throw 'Peer capability two-host boundary accepted a negative mutation'
}

Write-Output 'extension_free_first_contact=legacy-no-create'
Write-Output 'stripped_after_pin=quarantined-before-endpoint'
Write-Output 'selected_canonical_resume=persisted-selection-no-fallback'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_capability_two_host_rehearsal_boundary=verified'
