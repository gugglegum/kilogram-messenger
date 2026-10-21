[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-READ-ADAPTER.json'
$executionManifestPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\Cargo.toml'
$executionLibPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$integrationManifestPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$integrationLibPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$adapterPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_read_adapter.rs'
$intentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_intent.rs'
$runtimeLibPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0258-m1234-dormant-recipient-import-read-adapter.md'

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
        [string]$ExecutionManifest,
        [string]$ExecutionLib,
        [string]$Adapter,
        [string]$Intent,
        [string]$RuntimeLib,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.34' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-read-adapter-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import read adapter identity or scope changed'
    }

    $adapterBoundary = $Manifest.adapter
    if (-not [bool]$adapterBoundary.separate_integration_crate -or
        -not [bool]$adapterBoundary.execution_engine_has_no_production_runtime_ipc_dependency -or
        -not [bool]$adapterBoundary.transport_independent -or
        [bool]$adapterBoundary.active_runtime_registration -or
        (@($adapterBoundary.handled_commands) -join ',') -cne 'plan,status' -or
        (@($adapterBoundary.rejected_commands) -join ',') -cne 'confirm,apply-next,apply-bounded' -or
        -not [bool]$adapterBoundary.returns_privacy_safe_outcome -or
        -not [bool]$adapterBoundary.returns_ephemeral_in_process_plan -or
        [bool]$adapterBoundary.persists_execution_plan -or
        [bool]$adapterBoundary.creates_intent -or
        [bool]$adapterBoundary.creates_confirmation -or
        [bool]$adapterBoundary.imports_history -or
        [bool]$adapterBoundary.consumes_intent) {
        throw 'recipient import read adapter command or mutation boundary changed'
    }

    foreach ($property in @(
        'caller_owned_loader', 'reloads_each_exact_reference',
        'authority_reference_echo_checked', 'authenticated_context_echo_checked',
        'membership_digest_checked', 'ordered_source_ordinal_preserved',
        'missing_reference_rejected', 'ambiguous_reference_rejected',
        'substituted_reference_rejected'
    )) {
        if (-not [bool]$Manifest.membership_authority.$property) {
            throw "recipient import membership authority '$property' changed"
        }
    }
    foreach ($property in @(
        'membership_snapshot_persisted', 'membership_snapshot_returned',
        'authority_inferred_from_recovery_database'
    )) {
        if ([bool]$Manifest.membership_authority.$property) {
            throw "recipient import membership authority escaped through '$property'"
        }
    }

    foreach ($property in @(
        'exact_intent_id_required', 'active_unconsumed_intent_required',
        'exact_coordinator_id_and_sequence_required', 'bounded_m1232_planner_reused',
        'read_only', 'confirmation_required_before_future_mutation'
    )) {
        if (-not [bool]$Manifest.plan_boundary.$property) {
            throw "recipient import plan boundary '$property' changed"
        }
    }
    foreach ($property in @(
        'durable_intent_lookup_read_only', 'unplanned_status_requires_original_coordinator',
        'known_plan_is_ephemeral_caller_context', 'known_plan_memberships_reloaded',
        'known_plan_replanned_and_exact_matched', 'foreign_or_stale_plan_rejected_before_loader'
    )) {
        if (-not [bool]$Manifest.status_boundary.$property) {
            throw "recipient import status boundary '$property' changed"
        }
    }
    if ([bool]$Manifest.status_boundary.status_creates_database) {
        throw 'recipient import status unexpectedly creates a database'
    }

    foreach ($property in @('opaque_content_addressed_ids', 'closed_counts_and_states')) {
        if (-not [bool]$Manifest.privacy_projection.$property) {
            throw "recipient import privacy property '$property' changed"
        }
    }
    foreach ($property in @(
        'raw_account_id_exposed', 'raw_device_id_exposed',
        'root_or_device_secret_exposed', 'filesystem_path_exposed',
        'membership_snapshot_exposed', 'message_body_or_plaintext_exposed',
        'ciphertext_history_artifact_exposed'
    )) {
        if ([bool]$Manifest.privacy_projection.$property) {
            throw "recipient import read projection leaked '$property'"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.targeted_adapter_tests -cne '8/8' -or
        [string]$verification.full_execution_crate_tests -cne '42/42' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '160/160' -or
        [bool]$verification.release_build -or
        [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or
        [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or
        [bool]$verification.network_executed) {
        throw 'recipient import read adapter verification claim changed'
    }

    $productionExecutionManifest = ($ExecutionManifest -split '(?m)^\[dev-dependencies\]\s*$', 2)[0]
    if ($productionExecutionManifest.IndexOf('kilogram-runtime-ipc', [StringComparison]::Ordinal) -ge 0) {
        throw 'offline recovery execution gained a production runtime IPC dependency'
    }
    Assert-ContainsAll $ExecutionManifest @(
        'kilogram-runtime-ipc = { path = "../kilogram-runtime-ipc" }'
    ) 'recipient import execution test manifest'
    Assert-ContainsAll $IntegrationSurface @(
        'name = "kilogram-peer-history-recovery-runtime"',
        'kilogram-peer-history-recovery-execution = { path = "../kilogram-peer-history-recovery-execution" }',
        'kilogram-runtime-ipc = { path = "../kilogram-runtime-ipc" }',
        'mod execution_api',
        'mod recipient_import_read_adapter;'
    ) 'recipient import runtime integration crate'
    Assert-ContainsAll $ExecutionLib @(
        '#[cfg(test)]',
        'kilogram-peer-history-recovery-runtime/src/recipient_import_read_adapter.rs',
        'recipient_import_read_adapter_plans_without_mutation_or_persistence',
        'recipient_import_read_adapter_observes_unplanned_status_without_loading_membership',
        'recipient_import_read_adapter_reloads_known_plan_for_exact_status',
        'recipient_import_read_adapter_rejects_changed_membership_binding',
        'recipient_import_read_adapter_rejects_missing_and_ambiguous_memberships',
        'recipient_import_read_adapter_rejects_stale_coordinator_before_loader',
        'recipient_import_read_adapter_rejects_foreign_known_plan_before_reload',
        'recipient_import_read_adapter_never_dispatches_mutation_commands'
    ) 'recipient import execution library'
    Assert-ContainsAll $Adapter @(
        'pub trait RecipientImportMembershipLoader',
        'pub enum RecipientImportMembershipLoad',
        'pub struct RecipientImportReadKnownPlan',
        'pub struct RecipientImportReadAdapterDispatch',
        'pub struct RecipientHistoryRecoveryImportReadAdapter',
        'RuntimeIpcRecipientImportExecutionCommand::Plan',
        'RuntimeIpcRecipientImportExecutionCommand::Status',
        'RuntimeIpcRecipientImportExecutionCommand::Confirm',
        'RuntimeIpcRecipientImportExecutionCommand::ApplyNext',
        'RuntimeIpcRecipientImportExecutionCommand::ApplyBounded',
        'RecipientHistoryRecoveryImportRoundExecutor::plan',
        'load_exact_memberships',
        'authority_reference_id != reference.authority_reference_id',
        'authenticated_context_id != reference.authenticated_context_id',
        'recipient_import_membership_digest(&exact.membership)',
        'persisted_execution_plan(&self) -> bool',
        'granted_import_authority(&self) -> bool'
    ) 'recipient import read adapter'
    Assert-ContainsAll $Intent @(
        'pub fn load_by_intent_id(',
        'ReadOnlyDatabase::open(path)',
        'RecipientRecoveryImportRoundIntentError::IntentFork'
    ) 'recipient import intent store'
    Assert-ContainsAll $RuntimeLib @(
        'const IPC_VERSION: u8 = 34;',
        'kilogram:runtime-ipc-descriptor:v15'
    ) 'runtime IPC library'

    foreach ($forbidden in @(
        'tokio::', 'reqwest::', 'iroh::', 'TcpListener', 'TcpStream',
        'std::fs::write', 'fs::write', 'Database::create',
        'RecipientHistoryRecoveryImportRoundExecutor::apply_next',
        'RecipientHistoryRecoveryImportRoundExecutor::apply_bounded',
        'RecipientRecoveryImportRoundIntentStore::commit',
        'RecipientRecoveryImportRoundIntentStore::consume'
    )) {
        if ($Adapter.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import read adapter gained forbidden surface '$forbidden'"
        }
    }
    if ([regex]::IsMatch($Adapter, 'RuntimeIpcCommand::RecipientImport|RuntimeIpcResponse::RecipientImport')) {
        throw 'recipient import read adapter was registered in active runtime IPC'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'connects only its `Plan` and `Status` requests',
        'not registered in `RuntimeIpcCommand`',
        'caller-owned loader',
        'Missing, ambiguous, substituted or digest-mismatched',
        'Loaded membership snapshots live only in a transient vector',
        'reruns M1.2.32 planning with the same budget',
        '`Confirm`, `ApplyNext` and `ApplyBounded` are always rejected',
        'public default remains `legacy-only`',
        'Full execution regression passes 42/42',
        'Aggregate accepted-tag readiness contains 160 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import read adapter RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $executionManifestPath, $executionLibPath,
    $integrationManifestPath, $integrationLibPath, $adapterPath,
    $intentPath, $runtimeLibPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$executionManifest = Get-Content -LiteralPath $executionManifestPath -Raw
$executionLib = Get-Content -LiteralPath $executionLibPath -Raw
$integrationSurface = (Get-Content -LiteralPath $integrationManifestPath -Raw) + "`n" +
    (Get-Content -LiteralPath $integrationLibPath -Raw)
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$intent = Get-Content -LiteralPath $intentPath -Raw
$runtimeLib = Get-Content -LiteralPath $runtimeLibPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $executionManifest $executionLib $adapter $intent $runtimeLib $rfc

if ($SelfTest) {
    $mutations = @()

    $changed = Copy-JsonObject $manifest
    $changed.milestone = 'M1.2.35'
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.adapter.rejected_commands = @('apply-next')
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.adapter.imports_history = $true
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.membership_authority.ambiguous_reference_rejected = $false
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.membership_authority.membership_snapshot_persisted = $true
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.status_boundary.known_plan_replanned_and_exact_matched = $false
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.privacy_projection.raw_device_id_exposed = $true
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.verification.targeted_adapter_tests = '7/8'
    $mutations += { Assert-Boundary $changed $executionManifest $executionLib $adapter $intent $runtimeLib $rfc }

    $mutations += {
        Assert-Boundary $manifest $executionManifest $executionLib ($adapter + "`nDatabase::create") $intent $runtimeLib $rfc
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import read adapter boundary rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'canonical_recipient_import_read_adapter_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'canonical_recipient_import_read_adapter_boundary=verified'
Write-Output 'active_runtime_registration=false'
Write-Output 'mutation_commands_dispatched=false'
Write-Output 'membership_snapshot_persisted=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
