[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-AUTOMATED-RUNTIME-ENFORCEMENT.json'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0232-m1208-automated-runtime-enforcement.md'

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Ordered {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    $offset = 0
    foreach ($needle in $Needles) {
        $index = $Source.IndexOf($needle, $offset, [StringComparison]::Ordinal)
        if ($index -lt 0) { throw "$Label is missing ordered token '$needle'" }
        $offset = $index + $needle.Length
    }
}

function Assert-Contract {
    param([object]$Manifest, [string]$Cli, [string]$Rfc)
    if ([string]$Manifest.milestone -cne 'M1.2.8' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-automated-runtime-enforcement-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only' -or
        -not [bool]$Manifest.shared_authority_boundary.one_capability_decision_path -or
        -not [bool]$Manifest.shared_authority_boundary.persisted_canonical_selection_is_resume_authority -or
        [bool]$Manifest.shared_authority_boundary.unknown_extension_free_first_contact_creates_store -or
        -not [bool]$Manifest.runtime_outbox_boundary.capability_floor_before_message_materialization -or
        -not [bool]$Manifest.runtime_outbox_boundary.capability_floor_before_endpoint_or_network -or
        -not [bool]$Manifest.runtime_outbox_boundary.capability_floor_before_mailbox_fallback -or
        [bool]$Manifest.runtime_outbox_boundary.quarantined_item_creates_retry_record -or
        [bool]$Manifest.runtime_outbox_boundary.quarantined_item_reselected_in_same_process -or
        -not [bool]$Manifest.runtime_outbox_boundary.restart_rechecks_durable_floor -or
        -not [bool]$Manifest.scheduled_sync_boundary.capability_floor_before_sync_attempt -or
        [bool]$Manifest.scheduled_sync_boundary.quarantined_contact_reselected_in_same_process -or
        -not [bool]$Manifest.scheduled_sync_boundary.capability_block_stops_candidate_failover -or
        [bool]$Manifest.scheduled_sync_boundary.blocked_contact_starves_other_contacts -or
        -not [bool]$Manifest.scheduled_sync_boundary.restart_rechecks_durable_floor -or
        [bool]$Manifest.activation_boundary.automatic_quarantine_recovery_enabled -or
        [bool]$Manifest.activation_boundary.runtime_ipc_quarantine_mutation_enabled -or
        [bool]$Manifest.activation_boundary.public_canonical_rollout_enabled) {
        throw 'automated runtime capability enforcement boundary changed'
    }

    Assert-ContainsAll $Cli @(
        'capability_blocked_sync_contacts: BTreeSet<RuntimeContactId>',
        'capability_blocked_queue_items: BTreeSet<RuntimeQueueId>',
        'fn authorize_runtime_candidates_before_outbound_network(',
        'runtime_peer_capability_status=blocked',
        'runtime_outbound_network_attempted=false',
        'runtime_outbound_status=capability-blocked',
        'runtime_sync_capability_status=blocked',
        'runtime_sync_network_attempted=false',
        'runtime_sync_status=capability-blocked',
        'runtime_outbox_quarantine_blocks_before_materialization_and_recovers_after_restart',
        'runtime_sync_scheduler_skips_blocked_contact_without_starving_next_peer',
        'peer_capability_transport_unknown_legacy_does_not_create_peer_record_in_existing_store',
        'peer_capability_transport_persisted_selection_is_resume_authority'
    ) 'automated runtime enforcement CLI'

    $deliveryStart = $Cli.IndexOf('async fn prepare_runtime_delivery(', [StringComparison]::Ordinal)
    $deliveryEnd = $Cli.IndexOf('async fn persist_runtime_delivery(', $deliveryStart, [StringComparison]::Ordinal)
    if ($deliveryStart -lt 0 -or $deliveryEnd -lt 0) { throw 'runtime delivery implementation is missing' }
    $delivery = $Cli.Substring($deliveryStart, $deliveryEnd - $deliveryStart)
    Assert-Ordered $delivery @(
        'require_conversation_participants(',
        'authorize_runtime_candidates_before_outbound_network(',
        'pin_peer_authority_primary(',
        'let event = match snapshot.materialized.get(&queue_id)'
    ) 'runtime outbox preflight'

    $schedulerStart = $Cli.IndexOf('async fn attempt_runtime_contact_sync(', [StringComparison]::Ordinal)
    $schedulerEnd = $Cli.IndexOf('async fn wait_for_runtime_event(', $schedulerStart, [StringComparison]::Ordinal)
    if ($schedulerStart -lt 0 -or $schedulerEnd -lt 0) { throw 'scheduled sync implementation is missing' }
    $scheduler = $Cli.Substring($schedulerStart, $schedulerEnd - $schedulerStart)
    Assert-Ordered $scheduler @(
        'authorize_runtime_candidates_before_outbound_network(',
        'RuntimeSyncPreparation::Ready',
        'match sync('
    ) 'scheduled sync preflight'

    $normalized = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'before the message body is opened',
        'creates no delivery retry record',
        'one quarantined peer does not starve unrelated peers',
        'first later attempt re-evaluates that durable record before network use',
        'public policy default remains `legacy-only`'
    )) {
        if ($normalized.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "automated runtime enforcement RFC is missing '$needle'"
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
    $mutated.runtime_outbox_boundary.quarantined_item_creates_retry_record = $true
    try { Assert-Contract $mutated $cli $rfc } catch {
        Write-Output 'quarantined_runtime_retry=rejected'
        Write-Output 'canonical_sync_v2_automated_runtime_enforcement_self_test=passed'
        exit 0
    }
    throw 'quarantined runtime work was allowed to create a retry'
}

Write-Output 'network_executed=false'
Write-Output 'runtime_ipc_changed=false'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_automated_runtime_enforcement_boundary=verified'
