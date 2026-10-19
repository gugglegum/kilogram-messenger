[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-REENROLLMENT-RUNTIME-CONSUMPTION.json'
$workspaceManifestPath = Join-Path $workspace 'Cargo.toml'
$runtimeManifestPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-runtime\Cargo.toml'
$runtimePath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-runtime\src\lib.rs'
$cliManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0236-m1212-fresh-start-peer-reenrollment-consumption.md'

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
    param(
        [object]$Manifest,
        [string]$WorkspaceManifest,
        [string]$RuntimeManifest,
        [string]$Runtime,
        [string]$CliManifest,
        [string]$Cli,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.12' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-reenrollment-runtime-consumption-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'peer re-enrollment runtime consumption identity or scope changed'
    }

    $durable = $Manifest.durable_transition
    foreach ($property in @(
        'separate_runtime_transition_database',
        'prepared_and_consumed_phases_are_distinct',
        'transition_signed_by_current_local_device',
        'prepared_record_contains_complete_successor',
        'consumed_head_references_exact_transition_id',
        'immediate_durability',
        'owner_account_and_device_bound',
        'full_store_verified_on_open'
    )) {
        if (-not [bool]$durable.$property) {
            throw "durable transition property '$property' changed"
        }
    }
    if ([int]$durable.maximum_transition_bytes -ne 331776 -or
        [int]$durable.maximum_contact_artifact_bytes -ne 16384 -or
        [int]$durable.maximum_transitions -ne 4096) {
        throw 'durable transition bounds changed'
    }

    foreach ($property in @(
        'acceptance_receipt_and_quarantine_record_ids',
        'old_and_replacement_peer_devices',
        'contact_and_conversation_ids',
        'predecessor_contact_digest',
        'successor_contact_bytes_and_digest',
        'successor_endpoint_candidate_bytes_and_digest',
        'exact_invitation_bytes_and_id',
        'successor_capability_floor',
        'local_device_signature'
    )) {
        if (-not [bool]$Manifest.transition_binding.$property) {
            throw "transition binding '$property' changed"
        }
    }

    $fresh = $Manifest.fresh_start_boundary
    foreach ($property in @(
        'prepared_transition_does_not_change_snapshot',
        'all_transitions_preflighted_before_any_consumption',
        'consumption_occurs_only_during_runtime_start',
        'consumption_occurs_before_network_endpoint_creation',
        'consumed_transition_applied_as_in_memory_overlay',
        'exact_restart_replay_is_idempotent',
        'consumed_replay_allows_monotonic_authority_advancement',
        'consumed_replay_rejects_replacement_revocation',
        'changed_descriptor_or_context_fails_closed'
    )) {
        if (-not [bool]$fresh.$property) {
            throw "fresh-start property '$property' changed"
        }
    }
    if ([bool]$fresh.hot_apply_enabled) {
        throw 'runtime hot apply became enabled'
    }

    $retained = $Manifest.retained_evidence
    foreach ($property in @(
        'predecessor_contact_file_retained_byte_exact',
        'old_quarantine_retained',
        'acceptance_receipt_retained',
        'prepared_transition_retained',
        'candidate_source_artifact_retained'
    )) {
        if (-not [bool]$retained.$property) {
            throw "retained evidence property '$property' changed"
        }
    }
    if ([bool]$retained.clear_or_delete_api_added) {
        throw 'retained evidence gained clear or delete authority'
    }

    $queue = $Manifest.queue_boundary
    if (-not [bool]$queue.queued_item_retained_unmaterialized_during_consumption -or
        [bool]$queue.automatic_delivery_retry_triggered -or
        -not [bool]$queue.ordinary_scheduler_must_reevaluate_successor -or
        -not [bool]$queue.capability_floor_not_lowered) {
        throw 'queue or capability boundary changed'
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.runtime_store_tests -cne '5/5' -or
        [string]$verification.cli_fresh_start_integration_tests -cne '1/1' -or
        [string]$verification.full_cli_stable_regression -cne '101/101 binary + 1/1 library' -or
        [string]$verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$verification.m1_readiness -cne '138/138' -or
        [bool]$verification.release_build -or
        [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or
        [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or
        [bool]$verification.network_executed) {
        throw 'peer re-enrollment runtime verification claim changed'
    }

    Assert-ContainsAll $WorkspaceManifest @(
        '"crates/kilogram-peer-reenrollment-runtime"'
    ) 'workspace manifest'
    Assert-ContainsAll $RuntimeManifest @(
        'name = "kilogram-peer-reenrollment-runtime"',
        'kilogram-peer-reenrollment-acceptance =',
        'kilogram-sync-capability =',
        'redb.workspace = true'
    ) 'runtime transition crate manifest'
    foreach ($forbidden in @(
        'reqwest', 'iroh', 'tokio', 'kilogram-runtime-ipc',
        'kilogram-transport-iroh', 'kilogram-cli', 'kilogram-windows'
    )) {
        if ($RuntimeManifest.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "runtime transition crate gained forbidden dependency '$forbidden'"
        }
    }

    Assert-ContainsAll $Runtime @(
        'peer-reenrollment-runtime-v1.redb',
        'peer-reenrollment-runtime-prepared-v1',
        'peer-reenrollment-runtime-consumed-v1',
        'pub struct PeerReenrollmentRuntimeBinding',
        'pub struct SignedPeerReenrollmentRuntimeTransition',
        'pub enum PeerReenrollmentRuntimePrepareOutcome',
        'pub enum PeerReenrollmentRuntimeConsumeOutcome',
        'set_durability(Durability::Immediate)',
        'pub fn prepare(',
        'pub fn consume(',
        'ConsumedHeadConflict',
        'prepare_consume_and_reopen_preserve_exact_transition',
        'exact_prepare_and_consume_replay_are_idempotent',
        'changed_successor_for_same_contact_fails_closed',
        'pre_commit_failure_leaves_no_prepared_or_consumed_record',
        'wrong_transition_id_and_corrupt_record_are_rejected'
    ) 'runtime transition implementation'
    foreach ($forbidden in @(
        'reqwest::', 'iroh::', 'tokio::net', 'RuntimeIpc',
        'remove_file(', 'remove_dir_all(', 'clear_quarantine'
    )) {
        if ($Runtime.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "runtime transition implementation gained forbidden surface '$forbidden'"
        }
    }

    Assert-ContainsAll $CliManifest @(
        'kilogram-peer-reenrollment-runtime ='
    ) 'CLI manifest'
    Assert-ContainsAll $Cli @(
        'fn apply_consumed_peer_reenrollment_transitions(',
        'fn validate_peer_reenrollment_runtime_transition(',
        'fn consume_peer_reenrollment_transitions_at_runtime_start(',
        'fn prepare_peer_reenrollment_runtime_transition(',
        'load_runtime_state_snapshot_base(',
        'if !durable.consumed()',
        'Validate the complete set before publishing any new consumed head.',
        'consume accepted peer re-enrollment transitions before network startup',
        'peer_reenrollment_runtime_consumption_phase=before-network-endpoint',
        'accepted peer re-enrollment endpoint descriptor changed',
        'retained peer quarantine changed after re-enrollment acceptance',
        '(!durable.consumed()).then_some(local_authority)',
        'replacement peer Device is no longer authorized after re-enrollment',
        'peer_reenrollment_consumes_only_at_fresh_start_and_retains_old_evidence',
        'assert!(prepared.materialized.is_empty())',
        'assert!(activated.materialized.is_empty())',
        'assert_eq!(fs::read(&contact_path)?, predecessor_bytes)'
    ) 'CLI fresh-start integration'

    $preflight = $Cli.IndexOf('let peer_reenrollment_startup = with_locked_state', [StringComparison]::Ordinal)
    $endpoint = $Cli.IndexOf('let endpoint_builder = endpoint_builder_with_relay', [StringComparison]::Ordinal)
    if ($preflight -lt 0 -or $endpoint -lt 0 -or $preflight -ge $endpoint) {
        throw 'peer re-enrollment consumption no longer precedes network endpoint construction'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'prepared transition alone does not change the runtime snapshot',
        'preflights the complete set of transitions',
        'before construction or binding of the Iroh endpoint',
        'monotonic authority advances after consumption',
        'original predecessor contact file is not overwritten',
        'queued item stays sealed and unmaterialized',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer re-enrollment runtime RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $workspaceManifestPath, $runtimeManifestPath, $runtimePath,
    $cliManifestPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$workspaceManifest = Get-Content -LiteralPath $workspaceManifestPath -Raw
$runtimeManifest = Get-Content -LiteralPath $runtimeManifestPath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$cliManifest = Get-Content -LiteralPath $cliManifestPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $workspaceManifest $runtimeManifest $runtime $cliManifest $cli $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.durable_transition.prepared_and_consumed_phases_are_distinct = $false
    $mutations += [pscustomobject]@{ Label = 'collapsed_transition_phases'; Manifest = $changed; RuntimeManifest = $runtimeManifest; Runtime = $runtime; Cli = $cli }

    $changed = Copy-JsonObject $manifest
    $changed.fresh_start_boundary.all_transitions_preflighted_before_any_consumption = $false
    $mutations += [pscustomobject]@{ Label = 'partial_preflight'; Manifest = $changed; RuntimeManifest = $runtimeManifest; Runtime = $runtime; Cli = $cli }

    $changed = Copy-JsonObject $manifest
    $changed.fresh_start_boundary.hot_apply_enabled = $true
    $mutations += [pscustomobject]@{ Label = 'hot_apply'; Manifest = $changed; RuntimeManifest = $runtimeManifest; Runtime = $runtime; Cli = $cli }

    $changed = Copy-JsonObject $manifest
    $changed.fresh_start_boundary.consumed_replay_allows_monotonic_authority_advancement = $false
    $mutations += [pscustomobject]@{ Label = 'authority_advancement_bricks_replay'; Manifest = $changed; RuntimeManifest = $runtimeManifest; Runtime = $runtime; Cli = $cli }

    $changed = Copy-JsonObject $manifest
    $changed.retained_evidence.old_quarantine_retained = $false
    $mutations += [pscustomobject]@{ Label = 'quarantine_removed'; Manifest = $changed; RuntimeManifest = $runtimeManifest; Runtime = $runtime; Cli = $cli }

    $changed = Copy-JsonObject $manifest
    $changed.queue_boundary.automatic_delivery_retry_triggered = $true
    $mutations += [pscustomobject]@{ Label = 'hidden_delivery_retry'; Manifest = $changed; RuntimeManifest = $runtimeManifest; Runtime = $runtime; Cli = $cli }

    $changed = Copy-JsonObject $manifest
    $changed.public_default = 'dual-reader-prefer-canonical'
    $mutations += [pscustomobject]@{ Label = 'public_default_change'; Manifest = $changed; RuntimeManifest = $runtimeManifest; Runtime = $runtime; Cli = $cli }

    $mutations += [pscustomobject]@{
        Label = 'network_dependency'; Manifest = $manifest
        RuntimeManifest = "$runtimeManifest`r`niroh.workspace = true"
        Runtime = $runtime; Cli = $cli
    }
    $mutations += [pscustomobject]@{
        Label = 'non_immediate_durability'; Manifest = $manifest
        RuntimeManifest = $runtimeManifest
        Runtime = $runtime.Replace('Durability::Immediate', 'Durability::None')
        Cli = $cli
    }
    $mutations += [pscustomobject]@{
        Label = 'startup_preflight_removed'; Manifest = $manifest
        RuntimeManifest = $runtimeManifest; Runtime = $runtime
        Cli = $cli.Replace('Validate the complete set before publishing any new consumed head.', 'Consume records independently.')
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            Assert-Contract $mutation.Manifest $workspaceManifest $mutation.RuntimeManifest `
                $mutation.Runtime $cliManifest $mutation.Cli $rfc
        }
        catch {
            Write-Output "$($mutation.Label)=rejected"
            $rejected++
            continue
        }
        throw "$($mutation.Label) mutation was unexpectedly accepted"
    }
    if ($rejected -ne 10) {
        throw "peer re-enrollment runtime mutation matrix incomplete: $rejected/10"
    }
    Write-Output 'peer_reenrollment_runtime_negative_mutations=10/10'
    Write-Output 'canonical_sync_v2_peer_reenrollment_runtime_consumption_self_test=passed'
    exit 0
}

Write-Output 'network_executed=false'
Write-Output 'runtime_hot_apply_enabled=false'
Write-Output 'automatic_delivery_retry_triggered=false'
Write-Output 'old_quarantine_retained=true'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_reenrollment_runtime_consumption_boundary=verified'
