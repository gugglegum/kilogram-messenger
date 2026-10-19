[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-PEER-REENROLLMENT-ACCEPTANCE.json'
$workspaceManifestPath = Join-Path $workspace 'Cargo.toml'
$plannerManifestPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment\Cargo.toml'
$plannerPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment\src\lib.rs'
$acceptanceManifestPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-acceptance\Cargo.toml'
$acceptancePath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-acceptance\src\lib.rs'
$cliManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0235-m1211-durable-peer-reenrollment-acceptance.md'

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
        [string]$PlannerManifest,
        [string]$Planner,
        [string]$AcceptanceManifest,
        [string]$Acceptance,
        [string]$CliManifest,
        [string]$Cli,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.11' -or
        [string]$Manifest.status -cne 'canonical-sync-v2-peer-reenrollment-acceptance-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'peer re-enrollment acceptance identity or scope changed'
    }

    $durable = $Manifest.durable_acceptance
    foreach ($property in @(
        'separate_acceptance_database',
        'receipt_and_successor_head_share_one_transaction',
        'immediate_durability',
        'receipt_signed_by_current_local_device',
        'receipt_canonical_and_bounded',
        'owner_account_and_device_bound',
        'full_store_verified_on_open',
        'partial_receipt_or_head_rejected',
        'pre_commit_failure_leaves_no_partial_state'
    )) {
        if (-not [bool]$durable.$property) {
            throw "durable acceptance property '$property' changed"
        }
    }
    if ([int]$durable.maximum_receipt_bytes -ne 4096 -or
        [int]$durable.maximum_head_bytes -ne 1024 -or
        [int]$durable.maximum_acceptances -ne 4096) {
        throw 'durable acceptance bounds changed'
    }

    $binding = $Manifest.receipt_binding
    foreach ($property in @(
        'old_quarantine_record_sequence_reason',
        'old_verified_predecessor_record_and_floor',
        'exact_tombstone_plan_id_and_artifact_digest',
        'local_and_peer_authority_revisions_and_digests',
        'replacement_peer_device',
        'successor_capability_floor',
        'invitation_and_required_capability_ids',
        'ticket_publication_epoch',
        'preparation_expiry_and_acceptance_times'
    )) {
        if (-not [bool]$binding.$property) {
            throw "acceptance receipt binding '$property' changed"
        }
    }

    $replay = $Manifest.replay_and_conflict
    foreach ($property in @(
        'exact_replay_is_idempotent',
        'exact_committed_replay_allowed_after_plan_expiry',
        'changed_live_context_rejected',
        'authority_or_publication_rollback_rejected',
        'second_successor_rejected',
        'existing_successor_head_never_replaced'
    )) {
        if (-not [bool]$replay.$property) {
            throw "replay or conflict property '$property' changed"
        }
    }

    $quarantine = $Manifest.quarantine_boundary
    if (-not [bool]$quarantine.old_quarantine_retained -or
        -not [bool]$quarantine.old_capability_database_opened_read_only_by_cli -or
        -not [bool]$quarantine.old_capability_database_byte_preservation_tested -or
        [bool]$quarantine.quarantine_clear_or_rewrite_api -or
        [bool]$quarantine.successor_lowers_old_floor) {
        throw 'old quarantine preservation boundary changed'
    }

    $operator = $Manifest.operator_boundary
    foreach ($property in @(
        'manual_plan_command', 'exact_confirmation_phrase_required',
        'explicit_approval_flag_required', 'manual_accept_command',
        'read_only_status_command', 'no_clobber_artifact_files',
        'runtime_restart_required'
    )) {
        if (-not [bool]$operator.$property) {
            throw "operator ceremony property '$property' changed"
        }
    }
    foreach ($property in @(
        'network_executed', 'automatic_retry_enabled', 'runtime_hot_apply_enabled',
        'runtime_consumer_enabled', 'desktop_one_click_enabled'
    )) {
        if ([bool]$operator.$property) {
            throw "forbidden operator/runtime authority '$property' became enabled"
        }
    }

    foreach ($property in @(
        'contains_connection_ticket_body', 'contains_endpoint_or_address',
        'contains_relay_url', 'contains_message_or_ciphertext',
        'contains_filesystem_path', 'contains_account_root_or_device_secret'
    )) {
        if ([bool]$Manifest.privacy_boundary.$property) {
            throw "acceptance artifact gained forbidden content '$property'"
        }
    }

    Assert-ContainsAll $WorkspaceManifest @(
        '"crates/kilogram-peer-reenrollment"',
        '"crates/kilogram-peer-reenrollment-acceptance"'
    ) 'workspace manifest'
    Assert-ContainsAll $PlannerManifest @(
        'name = "kilogram-peer-reenrollment"',
        'kilogram-peer-capability-store ='
    ) 'planner crate manifest'
    foreach ($forbidden in @('redb', 'reqwest', 'iroh', 'tokio', 'kilogram-runtime-ipc')) {
        if ($PlannerManifest.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "pure planner gained forbidden dependency '$forbidden'"
        }
    }
    foreach ($forbidden in @('Database::create', 'begin_write(', 'pub fn accept(')) {
        if ($Planner.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "pure planner gained durable apply authority '$forbidden'"
        }
    }

    Assert-ContainsAll $AcceptanceManifest @(
        'name = "kilogram-peer-reenrollment-acceptance"',
        'kilogram-peer-reenrollment =',
        'kilogram-peer-capability-store =',
        'redb.workspace = true'
    ) 'acceptance crate manifest'
    foreach ($forbidden in @(
        'reqwest', 'iroh', 'tokio', 'kilogram-runtime-ipc',
        'kilogram-transport-iroh', 'kilogram-cli', 'kilogram-windows'
    )) {
        if ($AcceptanceManifest.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "acceptance crate gained forbidden dependency '$forbidden'"
        }
    }

    Assert-ContainsAll $Acceptance @(
        'peer-reenrollment-acceptance-v1.redb',
        'pub struct SignedPeerReenrollmentAcceptanceReceipt',
        'pub struct PeerReenrollmentAcceptanceStore',
        'pub struct PeerReenrollmentAcceptanceReadStore',
        'pub enum PeerReenrollmentAcceptanceOutcome',
        'AlreadyAccepted(DurablePeerReenrollmentAcceptance)',
        'set_durability(Durability::Immediate)',
        'let mut receipts = write.open_table(RECEIPT_TABLE)',
        'let mut heads = write.open_table(HEAD_TABLE)',
        'write.commit().map_err(database_error)?;',
        'tombstone.verify_context(',
        'tombstone.verify_bound_context(',
        'PeerReenrollmentAcceptanceStoreError::SuccessorAlreadyAccepted',
        'MAX_PEER_REENROLLMENT_ACCEPTANCE_RECEIPT_BYTES: usize = 4 * 1024',
        'MAX_PEER_REENROLLMENT_ACCEPTANCE_HEAD_BYTES: usize = 1024',
        'MAX_DURABLE_PEER_REENROLLMENT_ACCEPTANCES: u64 = 4_096',
        'acceptance_atomically_commits_signed_receipt_and_preserves_quarantine',
        'exact_replay_is_idempotent_even_after_short_plan_expiry',
        'changed_context_and_second_successor_fail_closed_without_replacing_head',
        'pre_commit_failure_leaves_no_receipt_or_successor_head',
        'owner_mismatch_and_corrupt_receipt_are_rejected_on_reopen'
    ) 'acceptance implementation'
    foreach ($forbidden in @(
        'reqwest::', 'iroh::', 'tokio::net', 'RuntimeIpc',
        'remove_file(', 'remove_dir_all(', 'clear_quarantine'
    )) {
        if ($Acceptance.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "acceptance implementation gained forbidden surface '$forbidden'"
        }
    }

    Assert-ContainsAll $CliManifest @(
        'kilogram-peer-reenrollment =',
        'kilogram-peer-reenrollment-acceptance ='
    ) 'CLI manifest'
    Assert-ContainsAll $Cli @(
        'PeerReenrollmentPlan {',
        'PeerReenrollmentApprove {',
        'PeerReenrollmentAccept {',
        'PeerReenrollmentStatus {',
        'fn plan_peer_reenrollment(',
        'fn approve_peer_reenrollment(',
        'fn accept_peer_reenrollment(',
        'fn show_peer_reenrollment_status(',
        'ensure!(approve, "peer re-enrollment approval flag is required")',
        'write_new_authority_file(&output_file',
        'PeerCapabilityReadStore::open_existing(',
        'old_quarantine_retained=true',
        'runtime_restart_required=true',
        'runtime_consumer_enabled=false',
        'network_executed=false',
        'status=peer-reenrollment-acceptance-durable'
    ) 'manual CLI ceremony'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'commits both records or neither',
        'old quarantined capability chain as immutable audit evidence',
        'exact replay against the same still-verifiable authority',
        'A second successor for the same peer Account fails closed',
        'runtime consumer is disabled',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer re-enrollment acceptance RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $workspaceManifestPath, $plannerManifestPath, $plannerPath,
    $acceptanceManifestPath, $acceptancePath, $cliManifestPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$workspaceManifest = Get-Content -LiteralPath $workspaceManifestPath -Raw
$plannerManifest = Get-Content -LiteralPath $plannerManifestPath -Raw
$planner = Get-Content -LiteralPath $plannerPath -Raw
$acceptanceManifest = Get-Content -LiteralPath $acceptanceManifestPath -Raw
$acceptance = Get-Content -LiteralPath $acceptancePath -Raw
$cliManifest = Get-Content -LiteralPath $cliManifestPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $workspaceManifest $plannerManifest $planner `
    $acceptanceManifest $acceptance $cliManifest $cli $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.durable_acceptance.receipt_and_successor_head_share_one_transaction = $false
    $mutations += [pscustomobject]@{ Label = 'split_transaction'; Manifest = $changed; AcceptanceManifest = $acceptanceManifest; Acceptance = $acceptance }

    $changed = Copy-JsonObject $manifest
    $changed.quarantine_boundary.old_quarantine_retained = $false
    $mutations += [pscustomobject]@{ Label = 'quarantine_removed'; Manifest = $changed; AcceptanceManifest = $acceptanceManifest; Acceptance = $acceptance }

    $changed = Copy-JsonObject $manifest
    $changed.replay_and_conflict.exact_replay_is_idempotent = $false
    $mutations += [pscustomobject]@{ Label = 'non_idempotent_replay'; Manifest = $changed; AcceptanceManifest = $acceptanceManifest; Acceptance = $acceptance }

    $changed = Copy-JsonObject $manifest
    $changed.replay_and_conflict.second_successor_rejected = $false
    $mutations += [pscustomobject]@{ Label = 'second_successor'; Manifest = $changed; AcceptanceManifest = $acceptanceManifest; Acceptance = $acceptance }

    $changed = Copy-JsonObject $manifest
    $changed.operator_boundary.runtime_consumer_enabled = $true
    $mutations += [pscustomobject]@{ Label = 'runtime_hot_apply'; Manifest = $changed; AcceptanceManifest = $acceptanceManifest; Acceptance = $acceptance }

    $changed = Copy-JsonObject $manifest
    $changed.public_default = 'dual-reader-prefer-canonical'
    $mutations += [pscustomobject]@{ Label = 'public_default_change'; Manifest = $changed; AcceptanceManifest = $acceptanceManifest; Acceptance = $acceptance }

    $mutations += [pscustomobject]@{
        Label = 'runtime_dependency'; Manifest = $manifest
        AcceptanceManifest = "$acceptanceManifest`r`nkilogram-runtime-ipc = { path = '../kilogram-runtime-ipc' }"
        Acceptance = $acceptance
    }
    $mutations += [pscustomobject]@{
        Label = 'non_immediate_durability'; Manifest = $manifest
        AcceptanceManifest = $acceptanceManifest
        Acceptance = $acceptance.Replace('Durability::Immediate', 'Durability::None')
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try {
            Assert-Contract $mutation.Manifest $workspaceManifest $plannerManifest $planner `
                $mutation.AcceptanceManifest $mutation.Acceptance $cliManifest $cli $rfc
        }
        catch {
            Write-Output "$($mutation.Label)=rejected"
            $rejected++
            continue
        }
        throw "$($mutation.Label) mutation was unexpectedly accepted"
    }
    if ($rejected -ne 8) {
        throw "peer re-enrollment acceptance mutation matrix incomplete: $rejected/8"
    }
    Write-Output 'peer_reenrollment_acceptance_negative_mutations=8/8'
    Write-Output 'canonical_sync_v2_peer_reenrollment_acceptance_self_test=passed'
    exit 0
}

Write-Output 'network_executed=false'
Write-Output 'runtime_consumer_enabled=false'
Write-Output 'old_quarantine_retained=true'
Write-Output 'public_default=legacy-only'
Write-Output 'canonical_sync_v2_peer_reenrollment_acceptance_boundary=verified'
