[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-IMPORT-PLAN.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-COORDINATOR.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$planPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import.rs'
$coordinatorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\coordinator.rs'
$transferPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\transfer.rs'
$importPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\import.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0253-m1229-explicit-recipient-recovery-import-plan.md'

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
        [object]$Compatibility, [string]$Library, [string]$Plan,
        [string]$Coordinator, [string]$Transfer, [string]$Import,
        [string]$CliCargo, [string]$DesktopCargo, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.29' -or
        [string]$Manifest.status -cne 'explicit-recipient-history-recovery-import-plan-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.28' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import plan identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.28' -or
        [string]$Predecessor.status -cne 'durable-recipient-history-recovery-coordinator-verified') {
        throw 'recipient import plan predecessor changed'
    }

    $planning = $Manifest.planning
    foreach ($required in @(
        'explicit_source_ordinal_required', 'durably_applied_coordinator_required',
        'exact_current_coordinator_required', 'next_unimported_generation_selected',
        'linear_backlog_supported', 'membership_signature_verified',
        'membership_digest_bound', 'expected_inbox_head_bound',
        'expected_import_head_bound', 'read_only'
    )) {
        if (-not [bool]$planning.$required) { throw "planning property '$required' changed" }
    }
    if ([bool]$planning.creates_state -or [bool]$planning.opens_network -or
        [bool]$planning.imports_history) {
        throw 'recipient import planning authority changed'
    }

    $apply = $Manifest.apply
    foreach ($required in @(
        'explicit_only', 'imports_exactly_one_transfer', 'same_state_directory_required',
        'coordinator_and_import_revalidated_under_one_state_lock',
        'stale_inbox_head_rejected', 'changed_membership_rejected',
        'existing_verified_import_pipeline_reused', 'recipient_signed_receipt_required',
        'coordinator_refreshed_after_import', 'post_import_pre_refresh_restart_resumable',
        'exact_replay_idempotent'
    )) {
        if (-not [bool]$apply.$required) { throw "apply property '$required' changed" }
    }
    if ([bool]$apply.performs_network_io -or [bool]$apply.schedules_background_work) {
        throw 'recipient import apply authority changed'
    }

    if ([bool]$Manifest.storage.new_database_added -or
        -not [bool]$Manifest.storage.plan_is_ephemeral -or
        -not [bool]$Manifest.storage.existing_atomic_import_transaction_reused -or
        -not [bool]$Manifest.storage.existing_append_only_import_receipts_reused -or
        -not [bool]$Manifest.storage.existing_append_only_coordinator_chain_reused -or
        [bool]$Manifest.storage.stores_plaintext -or
        [bool]$Manifest.storage.stores_message_bodies_in_plan -or
        [bool]$Manifest.storage.whole_database_rollback_detection_supported) {
        throw 'recipient import storage boundary changed'
    }
    if ([string]$Manifest.backlog.selection_rule -cne 'transfer_ordinal_equals_imported_chunk_count' -or
        [bool]$Manifest.backlog.skipped_generations_allowed -or
        [bool]$Manifest.backlog.latest_head_used_as_next_when_earlier_pending -or
        -not [bool]$Manifest.backlog.one_transfer_per_apply -or
        [bool]$Manifest.backlog.automatic_loop_added) {
        throw 'recipient import backlog rule changed'
    }
    foreach ($required in @(
        'failure_before_import_leaves_no_new_import', 'import_commit_precedes_coordinator_refresh',
        'same_plan_can_resume_after_import_commit',
        'resume_verifies_exact_already_imported_artifacts',
        'coordinator_refresh_failure_is_explicit', 'stale_plan_fails_closed'
    )) {
        if (-not [bool]$Manifest.failure_and_restart.$required) {
            throw "failure/restart property '$required' changed"
        }
    }
    if (-not [bool]$Manifest.report.source_ordinal_only -or
        -not [bool]$Manifest.report.bounded_counts -or
        [bool]$Manifest.report.identity_visible -or
        [bool]$Manifest.report.network_coordinates_visible -or
        [bool]$Manifest.report.message_bodies_visible -or
        [bool]$Manifest.report.plaintext_visible -or
        [bool]$Manifest.report.network_authority_granted -or
        [bool]$Manifest.report.background_authority_granted) {
        throw 'recipient import report boundary changed'
    }
    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_worker_added', 'spawned_task_added',
        'automatic_source_selection_added', 'automatic_import_loop_added',
        'network_authority_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) {
            throw "recipient import authority non-claim '$forbidden' changed"
        }
    }
    if ([int]$Manifest.verification.focused_import_plan_tests -ne 4 -or
        [int]$Manifest.verification.combined_execution_crate_tests -ne 22 -or
        -not [bool]$Manifest.verification.three_generation_backlog_test -or
        -not [bool]$Manifest.verification.unapplied_coordinator_and_membership_test -or
        -not [bool]$Manifest.verification.stale_newer_inbox_head_test -or
        -not [bool]$Manifest.verification.post_import_pre_refresh_resume_test -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '155/155' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recipient import verification boundary changed'
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
        'mod recipient_import;', 'RecipientHistoryRecoveryImporter',
        'recipient_import_plan_consumes_backlog_in_exact_generation_order',
        'recipient_import_plan_requires_applied_coordinator_and_exact_membership',
        'recipient_import_plan_rejects_a_newer_staged_head',
        'recipient_import_plan_resumes_after_import_before_coordinator_refresh'
    ) 'recipient import library surface and tests'
    Assert-ContainsAll $Plan @(
        'pub struct RecipientHistoryRecoveryImporter;', 'pub fn plan(', 'pub fn apply(',
        'StateDirectoryLock::acquire(plan.coordination_plan.state_directory())?',
        'verify_durable_current_for_stores(', 'coordinator_transfer_at(',
        'import_staged_transfer_with_state_lock(',
        'CoordinatorRefreshAfterImport', 'InjectedAfterImport',
        'contains_message_bodies: false', 'contains_plaintext: false',
        'grants_network_authority: false', 'grants_background_authority: false',
        'imports_exactly_one_transfer: true'
    ) 'recipient import implementation'
    Assert-OmitsAll $Plan @(
        'tokio::spawn', 'tokio::time::sleep', 'iroh::', 'Endpoint::',
        'RuntimeIpcCommand', 'kilogram_windows', 'std::process::Command'
    ) 'recipient import production authority'
    Assert-ContainsAll $Coordinator @(
        'pub(crate) fn verify_exact_current(',
        'pub(crate) fn verify_durable_current_for_stores('
    ) 'coordinator import bridge'
    Assert-ContainsAll $Transfer @('pub(crate) fn coordinator_transfer_at(') 'transfer backlog selector'
    Assert-ContainsAll $Import @(
        'pub(crate) fn plan_staged_transfer(',
        'pub(crate) fn import_staged_transfer_with_state_lock('
    ) 'import planning and locked bridge'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Planning is read-only and creates no state',
        'next transfer ordinal equals the coordinator''s imported chunk count',
        'imports exactly one transfer', 'under that one lock revalidates',
        'retrying the same import plan verifies the exact already-imported artifacts',
        'no batch loop', 'full execution crate passes 22/22 tests',
        'readiness contains 155 closed gates', 'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $planPath, $coordinatorPath, $transferPath, $importPath,
    $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$plan = Get-Content -LiteralPath $planPath -Raw
$coordinator = Get-Content -LiteralPath $coordinatorPath -Raw
$transfer = Get-Content -LiteralPath $transferPath -Raw
$import = Get-Content -LiteralPath $importPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library, $plan,
    $coordinator, $transfer, $import, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.planning.read_only = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.planning.next_unimported_generation_selected = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.apply.imports_exactly_one_transfer = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.apply.coordinator_and_import_revalidated_under_one_state_lock = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.backlog.skipped_generations_allowed = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.storage.stores_plaintext = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authority.background_worker_added = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '154/154'
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $plan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedPlan = $plan.Replace('StateDirectoryLock::acquire(plan.coordination_plan.state_directory())?', 'fake_lock()?')
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedPlan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedPlan = $plan + "`ntokio::spawn(async {});"
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedPlan $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import self-test rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'history_recovery_recipient_import_plan_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_recipient_import_plan_boundary=verified'
Write-Output 'milestone=M1.2.29'
Write-Output 'planning_read_only=true'
Write-Output 'imports_per_apply=1'
Write-Output 'backlog_generation_skipping=false'
Write-Output 'network_authority=false'
Write-Output 'background_authority=false'
Write-Output 'public_default=legacy-only'
