[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-TRANSFER-INBOX.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-RECIPIENT-HISTORY-IMPORT.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$cargoPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\Cargo.toml'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$transferPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\transfer.rs'
$importPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\import.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0246-m1222-authenticated-history-recovery-transfer-inbox.md'

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
        [string]$Transfer,
        [string]$Import,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.22' -or
        [string]$Manifest.status -cne 'authenticated-history-recovery-transfer-inbox-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.21' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'history recovery transfer identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.21' -or
        [string]$Predecessor.status -cne 'atomic-recipient-history-store-import-verified') {
        throw 'history recovery transfer predecessor changed'
    }

    $envelope = $Manifest.envelope
    if ([int]$envelope.version -ne 1 -or
        [string]$envelope.encoding -cne 'canonical-postcard' -or
        [int64]$envelope.max_bytes -ne 9437184 -or
        -not [bool]$envelope.source_device_signed -or
        -not [bool]$envelope.content_addressed_transfer_id -or
        -not [bool]$envelope.exact_committed_execution_record_bound -or
        -not [bool]$envelope.owner_account_bound -or
        -not [bool]$envelope.source_device_bound -or
        -not [bool]$envelope.recipient_device_bound -or
        -not [bool]$envelope.consent_selection_bound -or
        -not [bool]$envelope.generation_and_predecessor_bound -or
        -not [bool]$envelope.conversation_and_range_bound -or
        -not [bool]$envelope.bundle_and_checkpoints_bound -or
        -not [bool]$envelope.encrypted_projections_bound -or
        [bool]$envelope.message_bodies_in_plaintext -or
        [bool]$envelope.network_transport_added) {
        throw 'history recovery transfer envelope boundary changed'
    }

    foreach ($required in @(
        'requires_exact_current_committed_head', 'requires_matching_source_device_secret',
        'reverifies_inner_execution_artifacts', 'stale_execution_record_rejected'
    )) {
        if (-not [bool]$Manifest.export_gate.$required) {
            throw "export gate '$required' changed"
        }
    }

    $inbox = $Manifest.recipient_inbox
    if ([string]$inbox.directory -cne 'history-recovery/transfer-inbox' -or
        [int]$inbox.max_envelopes -ne 16384) {
        throw 'history recovery transfer inbox limits changed'
    }
    foreach ($required in @(
        'append_only', 'state_directory_lock_used', 'state_transaction_used',
        'vault_primary_dual_write_used_when_present', 'generation_zero_required_first',
        'linear_generation_chain_required', 'gaps_forks_and_stale_candidates_rejected',
        'exact_byte_replay_idempotent', 'bounded_backlog_before_import_supported',
        'foreign_account_or_recipient_rejected',
        'symlink_unexpected_entry_and_corruption_rejected'
    )) {
        if (-not [bool]$inbox.$required) { throw "inbox property '$required' changed" }
    }

    $bridge = $Manifest.import_bridge
    foreach ($required in @(
        'requires_exact_staged_transfer', 'requires_same_canonical_state_directory',
        'requires_current_recipient_device', 'requires_exact_conversation_membership',
        'requires_recipient_signed_receipt_predecessor', 'out_of_order_import_rejected',
        'exact_already_imported_replay_allowed'
    )) {
        if (-not [bool]$bridge.$required) { throw "import bridge property '$required' changed" }
    }
    if ([bool]$bridge.inbox_itself_grants_import_authority) {
        throw 'transfer inbox gained import authority'
    }

    $observation = $Manifest.restart_and_observation
    foreach ($required in @(
        'partial_failure_rolls_back', 'active_transaction_marker_blocks_status',
        'crashed_transaction_recovered_before_retry', 'read_only_status_no_create',
        'opaque_selection_token', 'bounded_progress_visible'
    )) {
        if (-not [bool]$observation.$required) {
            throw "restart/observation property '$required' changed"
        }
    }
    foreach ($forbidden in @(
        'message_bodies_visible', 'plaintext_visible', 'raw_identity_visible',
        'filesystem_paths_visible', 'network_coordinates_visible',
        'network_authority_granted', 'import_authority_granted',
        'whole_database_rollback_detected_without_external_witness'
    )) {
        if ([bool]$observation.$forbidden) {
            throw "restart/observation non-claim '$forbidden' changed"
        }
    }

    if ([int]$Manifest.verification.focused_transfer_tests -ne 4 -or
        [int]$Manifest.verification.combined_execution_import_transfer_tests -ne 14 -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '148/148' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'history recovery transfer verification boundary changed'
    }
    foreach ($nonClaim in @(
        'whole-database-rollback-detection-without-external-witness',
        'network-history-transfer-session', 'network-resume-and-retry',
        'multi-source-recovery-scheduling', 'runtime-ipc-history-recovery-authority',
        'desktop-history-recovery-authority'
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
        throw 'history recovery transfer changed active runtime IPC inventory'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
        throw 'history recovery transfer changed compatibility binding'
    }

    Assert-ContainsAll $Cargo @(
        'kilogram-state', 'kilogram-store', 'postcard.workspace = true',
        'tempfile.workspace = true'
    ) 'history recovery transfer Cargo manifest'
    $productionCargo = ($Cargo -split '(?m)^\[dev-dependencies\]\s*$', 2)[0]
    foreach ($forbidden in @('iroh', 'reqwest', 'tokio', 'eframe', 'kilogram-runtime-ipc')) {
        if ($productionCargo.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "history recovery transfer gained forbidden dependency '$forbidden'"
        }
    }
    Assert-ContainsAll $Library @(
        'mod transfer;', 'HistoryRecoveryTransferEnvelope',
        'RecipientHistoryTransferInbox',
        'fn transfer_envelope_stages_replays_and_drives_atomic_import',
        'fn transfer_inbox_requires_a_linear_generation_chain',
        'fn transfer_inbox_rejects_tamper_foreign_recipient_and_corruption',
        'fn transfer_inbox_rollback_and_crash_recovery_are_fail_closed'
    ) 'history recovery transfer library'
    Assert-ContainsAll $Transfer @(
        'MAX_HISTORY_RECOVERY_TRANSFER_ENVELOPE_BYTES: usize = 9 * 1024 * 1024',
        'TRANSFER_SIGNATURE_DOMAIN', 'pub fn export_current(',
        'committed_head_chunk(', 'pub fn decode_and_verify(',
        '.verify(&transfer_signing_bytes(&self.content)?', 'pub fn stage_envelope_bytes(',
        'verify_candidate_successor', 'verify_transfer_chains',
        'StateDirectoryLock::acquire', 'StateTransaction::begin',
        'register_append_only_receipt_path', 'vault.begin_dual_write()',
        'vault.commit_primary_transaction', 'vault.confirm_primary_shadow()',
        'contains_message_bodies: false', 'contains_plaintext: false',
        'grants_network_authority: false', 'grants_import_authority: false'
    ) 'history recovery transfer source'
    Assert-ContainsAll $Import @(
        'pub fn import_staged_transfer(', 'same_state_directory(',
        'inbox.load_staged_locked', 'MissingPreviousReceipt',
        'chunk.previous_record_id() != Some(previous.execution_record_id())'
    ) 'history recovery transfer import bridge'
    foreach ($source in @($Transfer, $Import)) {
        foreach ($forbidden in @(
            'use std::net', 'use iroh', 'use reqwest', 'use tokio',
            'RuntimeIpcCommand', 'kilogram_windows'
        )) {
            if ($source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
                throw "history recovery transfer source contains forbidden authority '$forbidden'"
            }
        }
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'source exports only the exact current committed execution head',
        'recipient verifies the complete envelope before mutation',
        'bounded linear backlog may arrive before import',
        'inbox alone grants neither network nor import authority',
        'Aggregate accepted-tag readiness contains 148 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "history recovery transfer RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $schemaPath,
    $compatibilityPath, $cargoPath, $libraryPath, $transferPath, $importPath, $rfcPath
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
$transfer = Get-Content -LiteralPath $transferPath -Raw
$import = Get-Content -LiteralPath $importPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $schema, $compatibility,
    $cargo, $library, $transfer, $import, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.envelope.source_device_signed = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.envelope.max_bytes = 9437185
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.export_gate.requires_exact_current_committed_head = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.recipient_inbox.linear_generation_chain_required = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.import_bridge.out_of_order_import_rejected = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.import_bridge.inbox_itself_grants_import_authority = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.restart_and_observation.plaintext_visible = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.verification.product_network_executed = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $schema $compatibility $cargo $library $transfer $import $rfc }
    $changedTransfer = "$transfer`nuse iroh;"
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $library $changedTransfer $import $rfc }
    $changedRfc = $rfc.Replace('accepted-tag readiness contains 148', 'accepted-tag readiness is unknown')
    $mutations += { Assert-Contract $manifest $predecessor $completion $schema $compatibility $cargo $library $transfer $import $changedRfc }

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
    Write-Output 'history_recovery_transfer_inbox_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
    exit 0
}

Write-Output 'history_recovery_transfer_inbox_boundary=verified'
Write-Output 'envelope=bounded-canonical-source-device-signed'
Write-Output 'inbox=append-only-linear-backlog'
Write-Output 'import_order=recipient-signed-receipt-chain'
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
