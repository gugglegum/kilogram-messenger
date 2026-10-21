[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-HISTORY-IMPORT.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-OLD-HISTORY-RECOVERY-EXECUTION.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\Cargo.toml'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$importPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\import.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0245-m1221-atomic-recipient-history-store-import.md'

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
        [string]$Library,
        [string]$Import,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.21' -or
        [string]$Manifest.status -cne 'atomic-recipient-history-store-import-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.20' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient history import identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.20' -or
        [string]$Predecessor.status -cne 'bounded-single-source-history-recovery-execution-verified') {
        throw 'recipient history import predecessor changed'
    }

    foreach ($required in @(
        'requires_exact_committed_execution_record', 'reverifies_execution_artifacts',
        'requires_current_execution_head_before_first_import',
        'requires_current_recipient_device', 'requires_exact_conversation_membership',
        'stale_unimported_execution_record_rejected',
        'exact_already_imported_record_replay_allowed'
    )) {
        if (-not [bool]$Manifest.input_gate.$required) {
            throw "input gate '$required' changed"
        }
    }

    $importBoundary = $Manifest.import
    if ([string]$importBoundary.mode -cne 'local-offline-single-source' -or
        [int]$importBoundary.max_events_per_chunk -ne 32 -or
        -not [bool]$importBoundary.ordinary_event_store_mutated -or
        -not [bool]$importBoundary.ordinary_encrypted_projection_store_mutated -or
        [bool]$importBoundary.plaintext_stored -or
        -not [bool]$importBoundary.state_transaction_used -or
        -not [bool]$importBoundary.state_directory_lock_used -or
        -not [bool]$importBoundary.vault_primary_dual_write_used_when_present -or
        -not [bool]$importBoundary.checkpoint_before_and_after_persisted -or
        -not [bool]$importBoundary.content_addressed_short_checkpoint_names -or
        [bool]$importBoundary.network_opened -or
        [bool]$importBoundary.multi_source_scheduler_added -or
        [bool]$importBoundary.active_ipc_command_added -or
        [bool]$importBoundary.desktop_control_added) {
        throw 'recipient history import mutation boundary changed'
    }

    $receipt = $Manifest.receipt
    if ([string]$receipt.directory -cne 'history-recovery/import-receipts' -or
        [string]$receipt.checkpoint_directory -cne 'history-recovery/import-checkpoints' -or
        [int]$receipt.max_receipt_bytes -ne 65536 -or
        [int]$receipt.max_receipts -ne 16384 -or
        -not [bool]$receipt.recipient_device_signed -or
        -not [bool]$receipt.execution_record_bound -or
        -not [bool]$receipt.bundle_checkpoint_range_event_projection_bound -or
        -not [bool]$receipt.append_only_linear_chain -or
        -not [bool]$receipt.single_execution_record_consumption) {
        throw 'recipient history import receipt boundary changed'
    }

    foreach ($required in @(
        'exact_replay_is_no_op', 'exact_replay_reverifies_imported_artifacts',
        'event_projection_checkpoint_conflict_rejected', 'partial_failure_rolls_back',
        'active_transaction_marker_blocks_status', 'crashed_transaction_recovered_before_retry',
        'receipt_corruption_rejected', 'symlink_store_roots_rejected'
    )) {
        if (-not [bool]$Manifest.restart_and_replay.$required) {
            throw "restart/replay property '$required' changed"
        }
    }
    if ([bool]$Manifest.restart_and_replay.whole_database_rollback_detected_without_external_witness) {
        throw 'recipient history import gained an unsupported rollback claim'
    }

    if (-not [bool]$Manifest.observation.read_only_no_create -or
        -not [bool]$Manifest.observation.opaque_selection_token -or
        -not [bool]$Manifest.observation.bounded_chunk_event_progress_visible -or
        [bool]$Manifest.observation.message_bodies_visible -or
        [bool]$Manifest.observation.plaintext_visible -or
        [bool]$Manifest.observation.raw_identity_visible -or
        [bool]$Manifest.observation.filesystem_paths_visible -or
        [bool]$Manifest.observation.network_coordinates_visible -or
        [bool]$Manifest.observation.network_authority_granted -or
        [bool]$Manifest.observation.execution_authority_granted) {
        throw 'recipient history import observation boundary changed'
    }

    if ([int]$Manifest.verification.focused_import_tests -ne 4 -or
        [int]$Manifest.verification.combined_execution_tests -ne 10 -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '147/147' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recipient history import verification boundary changed'
    }
    foreach ($nonClaim in @(
        'whole-database-rollback-detection-without-external-witness',
        'network-history-transfer', 'multi-source-recovery-scheduling',
        'runtime-ipc-history-recovery-authority', 'desktop-history-recovery-authority'
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
        throw 'recipient history import changed active runtime IPC inventory'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
        throw 'recipient history import changed compatibility binding'
    }

    Assert-ContainsAll $Cargo @(
        'kilogram-state', 'kilogram-store', 'tempfile.workspace = true'
    ) 'recipient history import Cargo manifest'
    $productionCargo = ($Cargo -split '(?m)^\[dev-dependencies\]\s*$', 2)[0]
    foreach ($forbidden in @('iroh', 'reqwest', 'tokio', 'eframe', 'kilogram-runtime-ipc')) {
        if ($productionCargo.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient history import gained forbidden dependency '$forbidden'"
        }
    }
    Assert-ContainsAll $Library @(
        'mod import;', 'RecipientHistoryImportStore', 'pub fn committed_chunk(',
        'pub fn committed_head_chunk(', 'CommittedHeadMismatch',
        'fn recipient_import_atomically_publishes_events_projections_checkpoint_and_vault',
        'fn recipient_import_rollback_and_crash_recovery_leave_no_partial_page',
        'fn recipient_import_requires_current_head_sequential_chain_and_current_device',
        'fn recipient_import_checkpoint_and_receipt_corruption_fail_closed'
    ) 'recipient history import library'
    Assert-ContainsAll $Import @(
        'pub struct RecipientHistoryImportStore', 'pub fn import_committed_chunk(',
        'StateDirectoryLock::acquire', 'StateTransaction::begin',
        'put_authorized_batch_with_receipt', 'put_with_receipt',
        'register_append_only_receipt_path', 'vault.begin_dual_write()',
        'vault.commit_primary_transaction', 'vault.confirm_primary_shadow()',
        'RecipientHistoryImportOutcome::AlreadyImported',
        'RecipientHistoryImportError::StateTransactionInProgress',
        'contains_message_bodies: false', 'contains_plaintext: false',
        'grants_network_authority: false', 'grants_execution_authority: false',
        'whole_database_rollback_supported: false',
        'format!("{}.checkpoint", hex_id(checkpoint_id))'
    ) 'recipient history import source'
    foreach ($forbidden in @(
        'use std::net', 'use iroh', 'use reqwest', 'use tokio',
        'RuntimeIpcCommand', 'kilogram_windows'
    )) {
        if ($Import.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient history import source contains forbidden authority '$forbidden'"
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'An import accepts only an exact committed M1.2.20 execution record',
        'one state-directory transaction publishes',
        'An error after events, projections, checkpoints, or receipt creation rolls the whole page back',
        'privacy-safe status refuses to describe that partial state',
        'exact record which already has a valid recipient import receipt may be replayed',
        'does not claim whole-database rollback detection without an external monotonic witness',
        'Aggregate accepted-tag readiness contains 147 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient history import RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $schemaPath,
    $compatibilityPath, $cargoPath, $libraryPath, $importPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$cargo = Get-Content -LiteralPath $cargoPath -Raw
$library = Get-Content -LiteralPath $libraryPath -Raw
$import = Get-Content -LiteralPath $importPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $schema, $compatibility,
    $cargo, $library, $import, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.input_gate.requires_exact_committed_execution_record = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.input_gate.stale_unimported_execution_record_rejected = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.import.max_events_per_chunk = 33
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.import.network_opened = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.receipt.recipient_device_signed = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.restart_and_replay.partial_failure_rolls_back = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.observation.plaintext_visible = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.verification.product_network_executed = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $import $rfc }
    $changedImport = "$import`nuse iroh;"
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $library $changedImport $rfc }
    $changedRfc = $rfc.Replace('accepted-tag readiness contains 147', 'accepted-tag readiness is unknown')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $library $import $changedRfc }

    $rejected = 0
    $mutationIndex = 0
    foreach ($mutation in $mutations) {
        $mutationIndex++
        try { & $mutation; throw "negative mutation $mutationIndex unexpectedly passed" }
        catch {
            if ($_.Exception.Message -ceq "negative mutation $mutationIndex unexpectedly passed") { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'recipient_history_import_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
    exit 0
}

Write-Output 'recipient_history_import_boundary=verified'
Write-Output 'input=exact-committed-execution-head'
Write-Output 'publication=atomic-events-projections-checkpoints-receipt'
Write-Output 'receipt=recipient-device-signed-linear-chain'
Write-Output 'exact_replay=no-op-and-reverified'
Write-Output 'plaintext_stored=false'
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
