[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-OLD-HISTORY-RECOVERY-EXECUTION.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-OLD-HISTORY-RECOVERY-CONSENT-STORE.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\Cargo.toml'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$plannerSourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery\src\lib.rs'
$protocolSourcePath = Join-Path $workspace 'crates\kilogram-protocol\src\rewrap.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0244-m1220-bounded-single-source-history-recovery-execution.md'

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
        [object]$Predecessor,
        [object]$Completion,
        [object]$Schema,
        [object]$Compatibility,
        [string]$Cargo,
        [string]$Source,
        [string]$PlannerSource,
        [string]$ProtocolSource,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.20' -or
        [string]$Manifest.status -cne 'bounded-single-source-history-recovery-execution-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.19' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'old-history execution identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.19' -or
        [string]$Predecessor.status -cne 'durable-old-history-recovery-consent-store-verified') {
        throw 'old-history execution predecessor changed'
    }

    foreach ($required in @(
        'requires_exact_durable_consent_record', 'reverifies_consent_record_and_head',
        'reverifies_signed_plan', 'reverifies_committed_live_context_before_prepare',
        'reverifies_committed_live_context_before_commit', 'reverifies_owner_and_recipient',
        'committed_consent_remains_resumable_after_plan_expiry'
    )) {
        if (-not [bool]$Manifest.consent_gate.$required) {
            throw "consent gate '$required' changed"
        }
    }
    if ([bool]$Manifest.consent_gate.reenrollment_acceptance_is_history_consent) {
        throw 're-enrollment acceptance became history consent'
    }

    $execution = $Manifest.execution
    if ([string]$execution.mode -cne 'local-offline-single-source' -or
        [int]$execution.max_events_per_chunk -ne 32 -or
        -not [bool]$execution.existing_history_rewrap_format_reused -or
        -not [bool]$execution.source_signed_bundle -or
        -not [bool]$execution.recipient_decrypts_bundle -or
        -not [bool]$execution.recipient_signed_checkpoint_transition -or
        -not [bool]$execution.encrypted_local_projections_produced -or
        [bool]$execution.plaintext_stored_in_journal -or
        [bool]$execution.ordinary_event_store_mutated -or
        [bool]$execution.ordinary_projection_store_mutated -or
        [bool]$execution.network_opened -or
        [bool]$execution.multi_source_scheduler_added -or
        [bool]$execution.active_ipc_command_added -or
        [bool]$execution.desktop_control_added) {
        throw 'old-history execution boundary changed'
    }

    $journal = $Manifest.journal
    if ([string]$journal.database_file -cne 'old-history-recovery-execution-v1.redb' -or
        -not [bool]$journal.owner_account_bound -or
        -not [bool]$journal.owner_source_device_bound -or
        -not [bool]$journal.immediate_durability -or
        -not [bool]$journal.append_only_source_signed_records -or
        -not [bool]$journal.single_head_per_consent_selection -or
        -not [bool]$journal.bundle_checkpoint_projection_artifacts_exact_bound -or
        [int]$journal.max_chunks -ne 16384 -or
        [int]$journal.max_record_bytes -ne 16384 -or
        [int]$journal.max_head_bytes -ne 2048 -or
        [int]$journal.max_bundle_bytes -ne 4194304 -or
        [int]$journal.max_checkpoint_bytes -ne 65536 -or
        [int]$journal.max_projection_bytes -ne 4194304) {
        throw 'old-history execution journal boundary changed'
    }

    foreach ($required in @(
        'bounded_checkpoint_resume', 'exact_committed_replay_idempotent',
        'divergent_output_for_consumed_checkpoint_rejected',
        'pre_commit_failure_leaves_no_partial_state', 'fork_and_orphan_rejected',
        'artifact_corruption_rejected', 'new_existing_database_never_repaired'
    )) {
        if (-not [bool]$Manifest.restart_and_replay.$required) {
            throw "restart/replay property '$required' changed"
        }
    }

    if (-not [bool]$Manifest.observation.read_only_open_existing -or
        [bool]$Manifest.observation.missing_store_created -or
        -not [bool]$Manifest.observation.opaque_selection_token -or
        -not [bool]$Manifest.observation.bounded_progress_visible -or
        [bool]$Manifest.observation.message_bodies_visible -or
        [bool]$Manifest.observation.plaintext_visible -or
        [bool]$Manifest.observation.raw_identity_visible -or
        [bool]$Manifest.observation.filesystem_paths_visible -or
        [bool]$Manifest.observation.network_coordinates_visible -or
        [bool]$Manifest.observation.network_authority_granted -or
        [bool]$Manifest.observation.store_import_authority_granted) {
        throw 'old-history execution observation boundary changed'
    }

    if ([int]$Manifest.verification.focused_execution_tests -ne 6 -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '146/146' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'old-history execution verification boundary changed'
    }
    foreach ($nonClaim in @(
        'whole-database-rollback-detection-without-external-witness',
        'atomic-import-into-production-history-store',
        'network-history-transfer', 'multi-source-recovery-scheduling'
    )) {
        if ($nonClaim -notin @($Manifest.non_claims)) {
            throw "required non-claim '$nonClaim' is missing"
        }
    }

    if ([int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v15\0' -or
        @($Completion.canonical.commands).Count -ne 39 -or
        @($Completion.canonical.results).Count -ne 38 -or
        @($Completion.active_postcard.commands).Count -ne 34 -or
        @($Completion.active_postcard.responses).Count -ne 33 -or
        @($Schema.semantic_rules).Count -ne 192) {
        throw 'local execution stage changed active runtime IPC inventory'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
        throw 'local execution stage changed compatibility binding'
    }

    Assert-ContainsAll $Cargo @(
        'name = "kilogram-peer-history-recovery-execution"',
        'kilogram-peer-history-recovery-consent', 'kilogram-protocol',
        'redb.workspace = true', 'postcard.workspace = true'
    ) 'old-history execution Cargo manifest'
    $productionCargo = ($Cargo -split '(?m)^\[dev-dependencies\]\s*$', 2)[0]
    foreach ($forbidden in @('iroh', 'reqwest', 'tokio', 'eframe', 'kilogram-runtime-ipc')) {
        if ($productionCargo.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "old-history execution gained forbidden dependency '$forbidden'"
        }
    }

    Assert-ContainsAll $Source @(
        'const DATABASE_FILE: &str = "old-history-recovery-execution-v1.redb"',
        'pub const MAX_HISTORY_RECOVERY_EXECUTION_CHUNK_EVENTS: usize = 32',
        'pub struct OldHistoryRecoveryExecutionStore',
        'pub struct OldHistoryRecoveryExecutionReadStore',
        'pub fn prepare_next_chunk(', 'HistoryRewrapBundle::seal(',
        'pub fn apply_prepared_chunk(', 'LocalTextProjection::from_history_rewrap(',
        'pub fn commit_chunk(', 'Durability::Immediate',
        'verify_committed_context(', 'DivergentCommittedChunk',
        'contains_message_bodies: false', 'contains_plaintext: false',
        'grants_network_authority: false', 'grants_store_import_authority: false',
        'fn atomic_chunk_commit_reopen_and_privacy_safe_status',
        'fn bounded_multi_chunk_resume_reaches_exact_completion',
        'fn exact_replay_is_idempotent_and_divergent_output_fails_closed',
        'fn interrupted_atomic_commit_leaves_no_partial_state',
        'fn foreign_identity_recipient_and_context_drift_fail_closed',
        'fn corruption_and_orphaned_record_fail_closed_on_reopen'
    ) 'old-history execution source'
    foreach ($forbidden in @(
        'use std::net', 'use iroh', 'use reqwest', 'use tokio',
        'RuntimeIpcCommand', 'kilogram_windows'
    )) {
        if ($Source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "old-history execution source contains forbidden authority '$forbidden'"
        }
    }
    Assert-ContainsAll $PlannerSource @(
        'pub struct BoundHistoryRecoverySelection',
        'pub fn selection(&self, index: usize)',
        'pub fn verify_committed_context('
    ) 'old-history planner execution view'
    Assert-ContainsAll $ProtocolSource @(
        'impl HistoryRewrapId', 'pub fn as_bytes(&self) -> &[u8; 32]'
    ) 'history rewrap ID accessor'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Execution can start only from the exact durable consent record and its unique transition head',
        'An already committed exact consent remains resumable after that expiry',
        'The source prepares at most 32 events',
        'One immediate-durability transaction inserts the source-signed record',
        'Replaying an already committed checkpoint returns the exact durable chunk',
        'Detecting whole-database rollback requires an external monotonic witness',
        'it does not write them into the ordinary event or projection stores',
        'Aggregate accepted-tag readiness contains 146 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "old-history execution RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $schemaPath,
    $compatibilityPath, $cargoPath, $sourcePath, $plannerSourcePath,
    $protocolSourcePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$cargo = Get-Content -LiteralPath $cargoPath -Raw
$source = Get-Content -LiteralPath $sourcePath -Raw
$plannerSource = Get-Content -LiteralPath $plannerSourcePath -Raw
$protocolSource = Get-Content -LiteralPath $protocolSourcePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $schema, $compatibility,
    $cargo, $source, $plannerSource, $protocolSource, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.consent_gate.requires_exact_durable_consent_record = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.consent_gate.committed_consent_remains_resumable_after_plan_expiry = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.execution.max_events_per_chunk = 33
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.execution.ordinary_projection_store_mutated = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.journal.immediate_durability = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.restart_and_replay.divergent_output_for_consumed_checkpoint_rejected = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.observation.plaintext_visible = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.verification.product_network_executed = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $rfc }
    $changedCargo = "$cargo`niroh.workspace = true"
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $changedCargo $source $plannerSource $protocolSource $rfc }
    $changedRfc = $rfc.Replace('accepted-tag readiness contains 146 closed gates', 'accepted-tag readiness is unknown')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $source $plannerSource $protocolSource $changedRfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'negative mutation unexpectedly passed' }
        catch {
            if ($_.Exception.Message -ceq 'negative mutation unexpectedly passed') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'old_history_recovery_execution_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
    exit 0
}

Write-Output 'old_history_recovery_execution_boundary=verified'
Write-Output 'execution=bounded-local-offline-single-source'
Write-Output 'chunk_bound=32-events'
Write-Output 'journal=immediate-append-only-source-signed'
Write-Output 'resume=recipient-signed-checkpoint'
Write-Output 'plaintext_stored=false'
Write-Output 'ordinary_history_store_mutated=false'
Write-Output 'network_authority=false'
Write-Output 'active_runtime_ipc=postcard-v34-json-descriptor-v15'
Write-Output 'changes_active_wire=false'
Write-Output 'changes_public_default=false'
Write-Output 'public_default=legacy-only'
Write-Output 'second_host_required=false'
Write-Output 'product_network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
Write-Output 'automatic_github_build=false'
