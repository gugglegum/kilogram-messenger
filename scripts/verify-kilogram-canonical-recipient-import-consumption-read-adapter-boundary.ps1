[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-READ-ADAPTER.json'
$runtimeManifestPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$runtimeLibPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$adapterPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_read_adapter.rs'
$mutationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_mutation_adapter.rs'
$executionLibPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$intentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_intent.rs'
$ipcLibPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0262-m1238-recipient-import-consumption-read-adapter.md'

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

function Assert-Boundary {
    param(
        [object]$Manifest,
        [string]$RuntimeSurface,
        [string]$Adapter,
        [string]$Mutation,
        [string]$ExecutionLib,
        [string]$Intent,
        [string]$IpcLib,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.38' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-consumption-read-adapter-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_runtime_ipc_contract -or
        [int]$Manifest.runtime_ipc_version -ne 33 -or
        [int]$Manifest.runtime_ipc_domain -ne 14 -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import consumption read adapter identity or public scope changed'
    }

    if (-not [bool]$Manifest.adapter_boundary.dormant_library_api -or
        (@($Manifest.adapter_boundary.handled_commands) -join ',') -cne 'plan' -or
        (@($Manifest.adapter_boundary.rejected_commands) -join ',') -cne 'commit') {
        throw 'recipient import consumption read adapter command boundary changed'
    }
    foreach ($property in @(
        'commit_rejected_before_state_reload', 'returns_ephemeral_in_process_plan'
    )) {
        if (-not [bool]$Manifest.adapter_boundary.$property) {
            throw "recipient import consumption read adapter '$property' changed"
        }
    }
    foreach ($property in @(
        'persists_consumption_plan', 'consumes_intent', 'opens_network',
        'schedules_background_work', 'active_runtime_registration'
    )) {
        if ([bool]$Manifest.adapter_boundary.$property) {
            throw "recipient import consumption read adapter escaped through '$property'"
        }
    }

    foreach ($property in @(
        'read_by_exact_mutation_nonce', 'missing_storage_is_not_created',
        'absent_rejected', 'prepared_rejected', 'completed_record_id_reloaded',
        'completed_receipt_reloaded', 'membership_set_digest_reloaded',
        'canonical_evidence_reconstructed', 'changed_evidence_rejected'
    )) {
        if (-not [bool]$Manifest.completed_mutation_boundary.$property) {
            throw "completed mutation boundary '$property' changed"
        }
    }
    foreach ($property in @(
        'current_coordinator_checked_before_membership_loader',
        'active_unconsumed_intent_required', 'caller_owned_membership_loader',
        'exact_ordered_membership_references_required', 'complete_progress_required',
        'coordinator_refresh_pending_rejected', 'source_counts_and_event_count_exact',
        'final_coordinator_exact', 'consumed_intent_rejected',
        'changed_membership_rejected'
    )) {
        if (-not [bool]$Manifest.live_revalidation_boundary.$property) {
            throw "live revalidation boundary '$property' changed"
        }
    }
    foreach ($property in @(
        'membership_snapshot_returned', 'membership_snapshot_persisted',
        'message_body_or_plaintext', 'ciphertext_history_artifact',
        'filesystem_path', 'transport_authority', 'raw_runtime_token'
    )) {
        if ([bool]$Manifest.privacy_boundary.$property) {
            throw "recipient import consumption projection leaked '$property'"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.real_signed_fixture_tests -cne '4/4' -or
        [string]$verification.mutation_ledger_read_projection_tests -cne '1/1' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '164/164' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or
        [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or
        [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or
        [bool]$verification.network_executed) {
        throw 'recipient import consumption read adapter verification claim changed'
    }

    Assert-ContainsAll $RuntimeSurface @(
        'mod recipient_import_consumption_read_adapter;',
        'RecipientHistoryRecoveryImportConsumptionReadAdapter',
        'RecipientImportMutationReadStatus'
    ) 'recipient import recovery runtime surface'
    Assert-ContainsAll $Adapter @(
        'RuntimeIpcRecipientImportConsumptionCommand::Plan',
        'RuntimeIpcRecipientImportConsumptionCommand::Commit',
        'RuntimeIpcRecipientImportConsumptionHazard::CommitRejected',
        'mutations.inspect_by_nonce(&evidence.mutation_nonce)',
        'RuntimeIpcRecipientImportCompletedMutationEvidence::from_completed_receipt',
        'store.load_by_intent_id(evidence.intent_id.to_bytes()?)',
        'store.inspect_active()?.as_ref() != Some(&durable)',
        'load_exact_memberships(memberships, loader)',
        'store.inspect_progress(&durable, current, &sources, inbox, imports)',
        'RecipientRecoveryImportRoundIntentProgressState::Complete',
        'RuntimeIpcRecipientImportConsumptionPlanSummary::issue',
        'pub fn persisted_consumption_plan(&self) -> bool',
        'pub fn consumed_intent(&self) -> bool',
        'pub fn opened_network(&self) -> bool',
        'pub fn scheduled_background_work(&self) -> bool'
    ) 'recipient import consumption read adapter'
    Assert-ContainsAll $Mutation @(
        'pub enum RecipientImportMutationReadStatus',
        'pub fn inspect_by_nonce(',
        'ReadOnlyDatabase::open(&path)',
        'RecipientImportMutationReadStatus::Absent',
        'RecipientImportMutationReadStatus::Prepared',
        'RecipientImportMutationReadStatus::Completed'
    ) 'recipient import mutation read projection'
    Assert-ContainsAll $ExecutionLib @(
        'recipient_import_consumption_read_adapter_plans_from_exact_live_completed_state',
        'recipient_import_consumption_read_adapter_rejects_commit_before_any_reload',
        'recipient_import_consumption_read_adapter_rejects_changed_evidence_membership_and_coordinator',
        'recipient_import_consumption_read_adapter_rejects_missing_ledger_and_consumed_intent'
    ) 'recipient import consumption signed fixtures'
    Assert-ContainsAll $Intent @(
        'pub fn load_by_intent_id(',
        'pub fn inspect_active(',
        'pub fn inspect_progress('
    ) 'recipient import intent store'
    Assert-ContainsAll $IpcLib @(
        'const IPC_VERSION: u8 = 34;',
        'kilogram:runtime-ipc-descriptor:v15'
    ) 'runtime IPC library'

    foreach ($forbidden in @(
        'tokio::', 'reqwest::', 'iroh::', 'TcpListener', 'TcpStream',
        'std::fs::write', 'fs::write', 'Database::create',
        'RecipientRecoveryImportRoundIntentStore::commit',
        'RecipientRecoveryImportRoundIntentStore::consume'
    )) {
        if ($Adapter.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import consumption read adapter gained forbidden surface '$forbidden'"
        }
    }
    if ([regex]::IsMatch($Adapter, 'RuntimeIpcCommand::RecipientImport|RuntimeIpcResponse::RecipientImport')) {
        throw 'recipient import consumption read adapter was registered in active runtime IPC'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'connects only its `Plan` request',
        '`Commit` is rejected before any mutation ledger',
        'Missing storage remains missing',
        'Only completed state projects the exact signed record ID',
        'caller-owned loader',
        'must report `Complete`',
        'writes no consumption plan, clears no intent, opens no network',
        'public default remains `legacy-only`',
        'Four stable-path real signed-fixture tests',
        'Aggregate accepted-tag readiness contains 164 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import consumption read adapter RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $runtimeManifestPath, $runtimeLibPath, $adapterPath,
    $mutationPath, $executionLibPath, $intentPath, $ipcLibPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$runtimeSurface = (Get-Content -LiteralPath $runtimeManifestPath -Raw) + "`n" +
    (Get-Content -LiteralPath $runtimeLibPath -Raw)
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$mutation = Get-Content -LiteralPath $mutationPath -Raw
$executionLib = Get-Content -LiteralPath $executionLibPath -Raw
$intent = Get-Content -LiteralPath $intentPath -Raw
$ipcLib = Get-Content -LiteralPath $ipcLibPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.milestone = 'M1.2.39'
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.adapter_boundary.rejected_commands = @()
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.adapter_boundary.consumes_intent = $true
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.completed_mutation_boundary.missing_storage_is_not_created = $false
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.live_revalidation_boundary.current_coordinator_checked_before_membership_loader = $false
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.privacy_boundary.membership_snapshot_persisted = $true
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.verification.real_signed_fixture_tests = '3/4'
    $mutations += { Assert-Boundary $changed $runtimeSurface $adapter $mutation $executionLib $intent $ipcLib $rfc }

    $mutations += {
        Assert-Boundary $manifest $runtimeSurface ($adapter + "`nRecipientRecoveryImportRoundIntentStore::consume") $mutation $executionLib $intent $ipcLib $rfc
    }
    $mutations += {
        Assert-Boundary $manifest $runtimeSurface $adapter ($mutation -replace 'ReadOnlyDatabase::open\(&path\)', 'Database::create(&path)') $executionLib $intent $ipcLib $rfc
    }

    $rejected = 0
    foreach ($mutationCase in $mutations) {
        try { & $mutationCase } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import consumption read adapter boundary rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'canonical_recipient_import_consumption_read_adapter_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'canonical_recipient_import_consumption_read_adapter_boundary=verified'
Write-Output 'handled_commands=plan'
Write-Output 'rejected_commands=commit'
Write-Output 'active_runtime_registration=false'
Write-Output 'consumes_intent=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
