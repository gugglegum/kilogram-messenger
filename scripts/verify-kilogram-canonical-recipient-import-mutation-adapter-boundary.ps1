[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-MUTATION-ADAPTER.json'
$crateManifestPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$crateLibPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_mutation_adapter.rs'
$confirmationPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_confirmation.rs'
$executorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_executor.rs'
$intentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_intent.rs'
$cliManifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopManifestPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0260-m1236-explicit-bounded-recipient-import-mutation-adapter.md'

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
        [string]$CrateManifest,
        [string]$CrateLib,
        [string]$Source,
        [string]$Confirmation,
        [string]$Executor,
        [string]$Intent,
        [string]$CliManifest,
        [string]$DesktopManifest,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.36' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-mutation-adapter-verified' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_runtime_ipc_contract -or
        [int]$Manifest.runtime_ipc_version -ne 33 -or
        [int]$Manifest.runtime_ipc_domain -ne 14 -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import mutation identity or public scope changed'
    }

    foreach ($property in @(
        'apply_next_and_apply_bounded_only', 'exact_durable_confirmation_required',
        'current_owner_device_required', 'live_membership_reload_required',
        'exact_membership_reference_and_digest_required',
        'initial_execution_plan_exact_match_required', 'active_intent_revalidated',
        'coordinator_progress_revalidated', 'one_import_in_flight'
    )) {
        if (-not [bool]$Manifest.authority_boundary.$property) {
            throw "recipient import mutation authority '$property' changed"
        }
    }
    if ([int]$Manifest.authority_boundary.default_steps -ne 1 -or
        [int]$Manifest.authority_boundary.maximum_steps -ne 8 -or
        [bool]$Manifest.authority_boundary.intent_consumed_automatically) {
        throw 'recipient import mutation step or consume boundary changed'
    }

    if ([string]$Manifest.mutation_durability.separate_database -cne 'history-recovery/recipient-import-mutations-v1.redb' -or
        [int]$Manifest.mutation_durability.maximum_mutations -ne 4096 -or
        [int]$Manifest.mutation_durability.maximum_records -ne 8192) {
        throw 'recipient import mutation durable bounds changed'
    }
    foreach ($property in @(
        'state_directory_lock_required', 'immediate_durability',
        'prepared_before_first_import', 'completed_receipt_before_response',
        'device_signed_content_addressed_records', 'single_monotonic_chain',
        'nonce_index_exact', 'exact_completed_replay_returns_same_receipt',
        'reused_nonce_for_other_binding_rejected', 'parallel_mutation_rejected',
        'internal_fork_rollback_corruption_rejected'
    )) {
        if (-not [bool]$Manifest.mutation_durability.$property) {
            throw "recipient import mutation durability '$property' changed"
        }
    }
    if ([bool]$Manifest.mutation_durability.whole_database_rollback_witnessed) {
        throw 'recipient import mutation overclaims whole-database rollback detection'
    }

    foreach ($property in @(
        'import_before_coordinator_refresh_repaired',
        'coordinator_repair_imports_no_additional_source',
        'durable_progress_may_only_advance_within_original_budget',
        'recovered_steps_reported_as_already_imported',
        'recovered_event_count_exactly_reconciled',
        'completed_receipt_replay_after_expiry'
    )) {
        if (-not [bool]$Manifest.crash_recovery.$property) {
            throw "recipient import mutation crash rule '$property' changed"
        }
    }
    if ([bool]$Manifest.crash_recovery.additional_import_after_confirmation_expiry) {
        throw 'recipient import mutation allows a new import after confirmation expiry'
    }

    foreach ($property in @(
        'owner_account_and_device', 'intent_execution_confirmation_and_coordinator_ids',
        'membership_set_digest', 'mutation_nonce_sequence_and_previous_record_id',
        'bounded_receipt_counts_and_step_metadata'
    )) {
        if (-not [bool]$Manifest.stored_content.$property) {
            throw "recipient import mutation stored binding '$property' changed"
        }
    }
    foreach ($property in @(
        'membership_snapshot', 'message_body_or_plaintext',
        'ciphertext_history_artifact', 'filesystem_path', 'transport_authority'
    )) {
        if ([bool]$Manifest.stored_content.$property) {
            throw "recipient import mutation stores forbidden content '$property'"
        }
    }
    if (-not [bool]$Manifest.activation_boundary.dormant_library_api) {
        throw 'recipient import mutation is no longer a dormant library API'
    }
    foreach ($property in @(
        'active_runtime_registration', 'runtime_server_dispatch',
        'cli_or_desktop_control', 'background_worker_or_timer', 'network',
        'automatic_import', 'automatic_intent_consumption'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "recipient import mutation gained active authority '$property'"
        }
    }

    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.mutation_ledger_tests -cne '3/3' -or
        [string]$verification.real_signed_fixture_tests -cne '1/1' -or
        [string]$verification.import_before_refresh_reconciliation_tests -cne '1/1' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '162/162' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or [bool]$verification.network_executed) {
        throw 'recipient import mutation verification claim changed'
    }

    Assert-ContainsAll $CrateManifest @(
        'name = "kilogram-peer-history-recovery-runtime"',
        'kilogram-state = { path = "../kilogram-state" }',
        'postcard.workspace = true', 'redb.workspace = true', 'serde.workspace = true'
    ) 'recipient import runtime manifest'
    foreach ($forbiddenDependency in @('iroh.workspace', 'tokio.workspace', 'reqwest.workspace')) {
        if ($CrateManifest.IndexOf($forbiddenDependency, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import mutation crate gained '$forbiddenDependency'"
        }
    }
    Assert-ContainsAll $CrateLib @(
        'mod recipient_import_mutation_adapter;',
        'RecipientHistoryRecoveryImportMutationAdapter',
        'MAX_DURABLE_RECIPIENT_IMPORT_MUTATIONS'
    ) 'recipient import runtime library'
    Assert-ContainsAll $Source @(
        'recipient-import-mutations-v1.redb',
        'MAX_DURABLE_RECIPIENT_IMPORT_MUTATIONS: u64 = 4_096',
        'MAX_DURABLE_RECIPIENT_IMPORT_MUTATION_RECORDS',
        'StateDirectoryLock::acquire', 'Durability::Immediate',
        'RuntimeIpcRecipientImportExecutionCommand::ApplyNext',
        'RuntimeIpcRecipientImportExecutionCommand::ApplyBounded',
        'RecipientHistoryRecoveryImportRoundExecutor::apply_next',
        'RecipientHistoryRecoveryImportRoundExecutor::apply_bounded',
        'RecipientHistoryRecoveryImportRoundExecutor::reconcile_pending',
        'RecipientImportMutationPhase::Prepared',
        'RecipientImportMutationPhase::Completed',
        'RuntimeIpcRecipientImportStepDisposition::AlreadyImported',
        'confirmation.expired(now_unix_seconds)',
        'persisted_receipt_before_response: true',
        'consumed_intent(&self) -> bool',
        'mutation_ledger_prepares_completes_and_replays_exact_receipt',
        'mutation_ledger_rejects_parallel_and_rebound_nonce',
        'mutation_ledger_rejects_foreign_device_and_corrupt_head'
    ) 'recipient import mutation adapter'
    Assert-ContainsAll $Confirmation @(
        'pub fn load_by_confirmation_id',
        'pub fn owner_account_id(&self)',
        'pub fn owner_device_id(&self)'
    ) 'recipient import confirmation bridge'
    Assert-ContainsAll $Executor @(
        'pub fn reconcile_pending(',
        'if progress != plan.progress || !progress.coordinator_refresh_required',
        'next.progress.completed_source_count != progress.completed_source_count'
    ) 'recipient import executor reconciliation'
    Assert-ContainsAll $Intent @(
        'pub fn planned_source_event_counts(&self)',
        'usize::from(entry.event_count)'
    ) 'recipient import signed intent metadata'

    $productionSource = ($Source -split '(?m)^#\[cfg\(test\)\]\s*$', 2)[0]
    foreach ($forbidden in @(
        'tokio::', 'iroh::', 'reqwest::', 'TcpListener', 'TcpStream',
        '.consume(', 'RuntimeIpcCommand::RecipientImport'
    )) {
        if ($productionSource.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "recipient import mutation gained forbidden surface '$forbidden'"
        }
    }
    $successorDependency = 'kilogram-peer-history-recovery-runtime = { path = "../../crates/kilogram-peer-history-recovery-runtime" }'
    if ($CliManifest.IndexOf($successorDependency, [StringComparison]::Ordinal) -lt 0 -or
        [regex]::Matches($CliManifest, 'kilogram-peer-history-recovery-runtime\s*=').Count -ne 1 -or
        $DesktopManifest.IndexOf('kilogram-peer-history-recovery-runtime', [StringComparison]::Ordinal) -ge 0) {
        throw 'recipient import mutation successor application boundary changed'
    }

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'smallest mutation boundary',
        'not registered in the runtime server',
        'Prepared` record',
        'before returning success',
        'exact completed nonce replay returns the same persisted receipt',
        'imports no additional source',
        'No additional import is started after the confirmation expires',
        'intent is never consumed automatically',
        'stores no membership snapshot',
        'accepted-tag readiness contains 162 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import mutation RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $crateManifestPath, $crateLibPath, $sourcePath,
    $confirmationPath, $executorPath, $intentPath, $cliManifestPath,
    $desktopManifestPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing file: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$crateManifest = Get-Content -LiteralPath $crateManifestPath -Raw
$crateLib = Get-Content -LiteralPath $crateLibPath -Raw
$source = Get-Content -LiteralPath $sourcePath -Raw
$confirmation = Get-Content -LiteralPath $confirmationPath -Raw
$executor = Get-Content -LiteralPath $executorPath -Raw
$intent = Get-Content -LiteralPath $intentPath -Raw
$cliManifest = Get-Content -LiteralPath $cliManifestPath -Raw
$desktopManifest = Get-Content -LiteralPath $desktopManifestPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Boundary $manifest $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.milestone = 'M1.2.37'
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.authority_boundary.maximum_steps = 64
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.authority_boundary.intent_consumed_automatically = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.mutation_durability.prepared_before_first_import = $false
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.mutation_durability.whole_database_rollback_witnessed = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.crash_recovery.additional_import_after_confirmation_expiry = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.stored_content.membership_snapshot = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $changed = Copy-JsonObject $manifest
    $changed.activation_boundary.runtime_server_dispatch = $true
    $mutations += { Assert-Boundary $changed $crateManifest $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc }

    $mutations += {
        Assert-Boundary $manifest ($crateManifest + "`niroh.workspace = true") $crateLib $source $confirmation $executor $intent $cliManifest $desktopManifest $rfc
    }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import mutation boundary rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'canonical_recipient_import_mutation_adapter_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'canonical_recipient_import_mutation_adapter_boundary=verified'
Write-Output 'mutation_receipt_persisted_before_response=true'
Write-Output 'intent_consumed_automatically=false'
Write-Output 'membership_snapshot_persisted=false'
Write-Output 'active_runtime_registration=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
