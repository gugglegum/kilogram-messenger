[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-IMPORT-ROUND-EXECUTOR.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-IMPORT-ROUND-INTENT.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$executorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_executor.rs'
$intentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_intent.rs'
$importPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0256-m1232-explicit-durable-recipient-import-round-executor.md'

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

function Assert-OmitsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "$Label unexpectedly contains '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest, [object]$Predecessor, [object]$Completion,
        [object]$Compatibility, [string]$Library, [string]$Executor,
        [string]$Intent, [string]$Import, [string]$CliCargo,
        [string]$DesktopCargo, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.32' -or
        [string]$Manifest.status -cne 'explicit-durable-recipient-import-round-executor-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.31' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import-round executor identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.31' -or
        [string]$Predecessor.status -cne 'durable-explicit-recipient-import-round-intent-verified') {
        throw 'recipient import-round executor predecessor changed'
    }

    foreach ($required in @(
        'read_only', 'requires_one_active_unconsumed_intent',
        'requires_exact_current_coordinator_observation',
        'requires_caller_supplied_live_memberships',
        'requires_exact_signed_caller_order'
    )) {
        if (-not [bool]$Manifest.planning.$required) {
            throw "planning property '$required' changed"
        }
    }
    if ([bool]$Manifest.planning.creates_state -or
        [bool]$Manifest.planning.opens_network -or
        [bool]$Manifest.planning.imports_history -or
        [bool]$Manifest.planning.schedules_background_work) {
        throw 'recipient import-round executor planning authority changed'
    }

    foreach ($required in @(
        'explicit_foreground_only', 'apply_next_supported',
        'apply_bounded_supported', 'uses_existing_single_transfer_importer',
        'compares_exact_signed_candidate_binding',
        'reopens_active_intent_after_every_step',
        'revalidates_progress_after_every_step',
        'reconciles_import_before_coordinator_refresh',
        'exact_complete_replay_is_noop'
    )) {
        if (-not [bool]$Manifest.execution.$required) {
            throw "execution property '$required' changed"
        }
    }
    if ([int]$Manifest.execution.default_max_steps -ne 1 -or
        [int]$Manifest.execution.hard_max_steps -ne 8 -or
        [int]$Manifest.execution.maximum_concurrent_imports -ne 1 -or
        [bool]$Manifest.execution.automatically_consumes_intent) {
        throw 'recipient import-round executor bounded execution changed'
    }

    if ([bool]$Manifest.privacy.stores_new_durable_state -or
        -not [bool]$Manifest.privacy.execution_plan_content_addressed) {
        throw 'recipient import-round executor durable/privacy boundary changed'
    }
    foreach ($forbidden in @(
        'execution_plan_contains_membership_snapshot',
        'execution_plan_contains_message_bodies',
        'execution_plan_contains_plaintext',
        'receipt_contains_membership_snapshot',
        'receipt_contains_message_bodies',
        'receipt_contains_plaintext'
    )) {
        if ([bool]$Manifest.privacy.$forbidden) {
            throw "privacy non-claim '$forbidden' changed"
        }
    }

    foreach ($required in @(
        'missing_active_intent_rejected',
        'consumed_or_changed_active_intent_rejected',
        'missing_changed_or_reordered_membership_rejected',
        'stale_coordinator_rejected_before_import',
        'changed_candidate_rejected_before_import',
        'non_prefix_or_beyond_round_progress_rejected',
        'unplanned_source_change_rejected',
        'post_step_progress_must_advance_exactly_once',
        'intent_fork_and_corruption_rejected_by_predecessor'
    )) {
        if (-not [bool]$Manifest.fail_closed.$required) {
            throw "fail-closed property '$required' changed"
        }
    }

    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_worker_added', 'spawned_task_added', 'timer_added',
        'source_discovery_added', 'automatic_round_loop_added',
        'network_authority_added', 'cross_process_orchestration_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) {
            throw "recipient import-round executor authority non-claim '$forbidden' changed"
        }
    }

    if ([int]$Manifest.verification.focused_executor_tests -ne 3 -or
        [int]$Manifest.verification.combined_execution_crate_tests -ne 34 -or
        -not [bool]$Manifest.verification.read_only_plan_and_membership_rejection_test -or
        -not [bool]$Manifest.verification.next_then_bounded_signed_order_test -or
        -not [bool]$Manifest.verification.crash_after_import_reconciliation_test -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '158/158' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recipient import-round executor verification boundary changed'
    }

    if ([int]$Completion.active_postcard.version -ne 33 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        @($Completion.canonical.commands).Count -ne 38 -or
        @($Completion.canonical.results).Count -ne 37) {
        throw 'active runtime IPC inventory changed'
    }
    $runtimeSurface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ([bool]$Compatibility.changes_wire -or $runtimeSurface.Count -ne 1 -or
        [int](@($runtimeSurface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 33) {
        throw 'compatibility inventory changed'
    }

    Assert-OmitsAll ($CliCargo + "`n" + $DesktopCargo) @(
        'kilogram-peer-history-recovery-execution'
    ) 'active app manifests'
    Assert-ContainsAll $Library @(
        'mod recipient_import_round_executor;',
        'RecipientHistoryRecoveryImportRoundExecutor',
        'recipient_import_round_executor_plan_is_read_only_and_requires_exact_live_membership',
        'recipient_import_round_executor_applies_next_then_bounded_in_signed_order',
        'recipient_import_round_executor_reconciles_crash_after_import_without_duplicate'
    ) 'recipient import-round executor library surface and tests'
    Assert-ContainsAll $Executor @(
        'DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_MAX_STEPS: usize = 1',
        'MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EXECUTION_STEPS',
        'pub struct RecipientRecoveryImportRoundExecutionPlan',
        'pub struct RecipientRecoveryImportRoundExecutionReceipt',
        'pub fn plan(', 'pub fn apply_next(', 'pub fn apply_bounded(',
        'RecipientHistoryRecoveryImporter::plan(',
        'RecipientHistoryRecoveryImporter::apply(',
        'candidate_binding() != expected', 'reopen_exact(',
        'store.inspect_progress(', 'refresh_coordination(',
        'contains_membership_snapshot: false', 'contains_message_bodies: false',
        'contains_plaintext: false', 'opened_network: false',
        'scheduled_background_work: false', 'concurrent_imports: 1'
    ) 'recipient import-round executor implementation'
    Assert-OmitsAll $Executor @(
        'tokio::spawn', 'tokio::time::sleep', 'iroh::', 'Endpoint::',
        'RuntimeIpcCommand', 'kilogram_windows', 'std::process::Command',
        'std::thread::spawn'
    ) 'recipient import-round executor production authority'
    Assert-ContainsAll $Intent @(
        'pub(crate) fn candidate_binding(', 'pub fn inspect_active(',
        'pub fn inspect_progress(',
        'ensure_same_state_directory(&self.state_directory, current.state_directory())?;'
    ) 'M1.2.31 executor bridge'
    Assert-ContainsAll $Import @(
        'pub(crate) fn candidate_binding(',
        'pub(crate) fn preflight_candidate_binding(',
        'pub fn apply('
    ) 'M1.2.29 executor bridge'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Planning opens exactly one active, unconsumed intent',
        'caller-supplied live memberships',
        '`apply_next` executes at most one pending source',
        '`apply_bounded` executes at most the plan budget',
        'at most one import is in flight',
        'reopens the active intent',
        'never consumes the intent automatically',
        'full execution crate passes 34/34 tests',
        'readiness contains 158 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import-round executor RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $executorPath, $intentPath, $importPath,
    $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$executor = Get-Content -LiteralPath $executorPath -Raw
$intent = Get-Content -LiteralPath $intentPath -Raw
$import = Get-Content -LiteralPath $importPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library, $executor,
    $intent, $import, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.planning.read_only = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.planning.requires_caller_supplied_live_memberships = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.execution.hard_max_steps = 9
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.execution.automatically_consumes_intent = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.privacy.execution_plan_contains_membership_snapshot = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.fail_closed.post_step_progress_must_advance_exactly_once = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authority.background_worker_added = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '157/157'
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $executor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedExecutor = $executor.Replace('reopen_exact(', 'reopen_unchecked(')
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedExecutor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedExecutor = $executor + "`ntokio::spawn(async {});"
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedExecutor $intent $import $cliCargo $desktopCargo $rfc }.GetNewClosure())

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import-round executor self-test rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'history_recovery_recipient_import_round_executor_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_recipient_import_round_executor_boundary=verified'
Write-Output 'milestone=M1.2.32'
Write-Output 'default_max_steps=1'
Write-Output 'hard_max_steps=8'
Write-Output 'maximum_concurrent_imports=1'
Write-Output 'automatic_intent_consumption=false'
Write-Output 'network_authority=false'
Write-Output 'background_authority=false'
Write-Output 'public_default=legacy-only'
