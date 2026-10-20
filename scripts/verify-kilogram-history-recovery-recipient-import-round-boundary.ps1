[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-IMPORT-ROUND.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-IMPORT-PLAN.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$roundPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round.rs'
$planPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import.rs'
$importPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\import.rs'
$coordinatorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\coordinator.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0254-m1230-bounded-multi-source-recipient-import-round.md'

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
        [object]$Compatibility, [string]$Library, [string]$Round,
        [string]$Plan, [string]$Import, [string]$Coordinator,
        [string]$CliCargo, [string]$DesktopCargo, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.30' -or
        [string]$Manifest.status -cne 'bounded-explicit-multi-source-recipient-import-round-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.29' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import round identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.29' -or
        [string]$Predecessor.status -cne 'explicit-recipient-history-recovery-import-plan-verified') {
        throw 'recipient import round predecessor changed'
    }

    foreach ($required in @(
        'caller_supplied_source_order_required', 'duplicate_source_ordinals_rejected',
        'exact_durably_applied_coordinator_required',
        'every_source_preflighted_before_first_import',
        'every_membership_verified_and_bound',
        'late_invalid_source_leaves_earlier_sources_unimported',
        'one_transfer_per_source_per_round', 'read_only'
    )) {
        if (-not [bool]$Manifest.planning.$required) {
            throw "planning property '$required' changed"
        }
    }
    if ([bool]$Manifest.planning.creates_state -or
        [bool]$Manifest.planning.opens_network -or
        [bool]$Manifest.planning.imports_history) {
        throw 'recipient import round planning authority changed'
    }

    if ([int]$Manifest.limits.default_max_sources -ne 4 -or
        [int]$Manifest.limits.hard_max_sources -ne 8 -or
        [int]$Manifest.limits.default_max_events -ne 128 -or
        [int]$Manifest.limits.hard_max_events -ne 256 -or
        [int]$Manifest.limits.maximum_concurrent_imports -ne 1 -or
        [bool]$Manifest.limits.automatic_next_round) {
        throw 'recipient import round limits changed'
    }

    foreach ($required in @(
        'explicit_only', 'caller_order_preserved', 'one_import_in_flight',
        'fresh_coordinator_after_each_import',
        'exact_candidate_revalidated_before_each_import',
        'existing_m1229_import_plan_reused', 'existing_atomic_import_pipeline_reused'
    )) {
        if (-not [bool]$Manifest.apply.$required) {
            throw "apply property '$required' changed"
        }
    }
    if ([bool]$Manifest.apply.performs_network_io -or
        [bool]$Manifest.apply.schedules_background_work) {
        throw 'recipient import round apply authority changed'
    }

    foreach ($required in @(
        'initial_preflight_failure_imports_nothing',
        'completed_progress_must_be_caller_order_prefix',
        'same_round_plan_resumes_durable_prefix',
        'post_import_pre_refresh_state_reconciled',
        'already_imported_artifacts_reverified',
        'progress_beyond_one_transfer_per_source_rejected',
        'unplanned_source_change_rejected',
        'concurrent_change_after_first_import_may_leave_verified_prefix',
        'exact_replay_idempotent'
    )) {
        if (-not [bool]$Manifest.failure_and_restart.$required) {
            throw "failure/restart property '$required' changed"
        }
    }
    if ([bool]$Manifest.failure_and_restart.whole_round_atomicity_claimed) {
        throw 'whole-round atomicity was unexpectedly claimed'
    }

    if ([bool]$Manifest.storage.new_database_added -or
        -not [bool]$Manifest.storage.round_plan_is_ephemeral -or
        -not [bool]$Manifest.storage.existing_import_receipts_reused -or
        -not [bool]$Manifest.storage.existing_coordinator_chain_reused -or
        [bool]$Manifest.storage.stores_plaintext -or
        [bool]$Manifest.storage.stores_message_bodies_in_plan -or
        [bool]$Manifest.storage.whole_database_rollback_detection_supported) {
        throw 'recipient import round storage boundary changed'
    }

    if (-not [bool]$Manifest.report.source_ordinals_only -or
        -not [bool]$Manifest.report.bounded_counts -or
        -not [bool]$Manifest.report.closed_step_dispositions -or
        [bool]$Manifest.report.identity_visible -or
        [bool]$Manifest.report.network_coordinates_visible -or
        [bool]$Manifest.report.message_bodies_visible -or
        [bool]$Manifest.report.plaintext_visible -or
        [bool]$Manifest.report.network_authority_granted -or
        [bool]$Manifest.report.background_authority_granted) {
        throw 'recipient import round report boundary changed'
    }
    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_worker_added', 'spawned_task_added', 'source_discovery_added',
        'automatic_round_loop_added', 'network_authority_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) {
            throw "recipient import round authority non-claim '$forbidden' changed"
        }
    }

    if ([int]$Manifest.verification.focused_import_round_tests -ne 5 -or
        [int]$Manifest.verification.combined_execution_crate_tests -ne 27 -or
        -not [bool]$Manifest.verification.two_source_reverse_order_test -or
        -not [bool]$Manifest.verification.single_source_backlog_fairness_test -or
        -not [bool]$Manifest.verification.late_invalid_preflight_test -or
        -not [bool]$Manifest.verification.changed_membership_test -or
        -not [bool]$Manifest.verification.durable_prefix_and_unreconciled_resume_test -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '156/156' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recipient import round verification boundary changed'
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
        'mod recipient_import_round;', 'RecipientHistoryRecoveryImportRound',
        'recipient_import_round_imports_only_one_backlog_generation_per_source',
        'recipient_import_round_preserves_caller_order_across_two_sources',
        'recipient_import_round_preflights_all_inputs_before_mutation',
        'recipient_import_round_rejects_changed_apply_membership_without_import',
        'recipient_import_round_resumes_durable_prefix_and_unreconciled_import'
    ) 'recipient import round library surface and tests'
    Assert-ContainsAll $Round @(
        'DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_MAX_SOURCES: usize = 4',
        'MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_SOURCES',
        'DEFAULT_RECIPIENT_RECOVERY_IMPORT_ROUND_MAX_EVENTS',
        'MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_EVENTS',
        'pub struct RecipientHistoryRecoveryImportRound;',
        'pub fn plan_with_budget(', 'pub fn apply(',
        'verify_exact_current(', 'DuplicateSourceOrdinal',
        'preflight_candidate_binding(', 'completed_prefix',
        'for index in inspection.completed_prefix..plan.import_plans.len()',
        'RecipientHistoryRecoveryImporter::apply(',
        'verify_staged_transfer_imported(',
        'concurrent_imports: 1', 'grants_network_authority: false',
        'grants_background_authority: false'
    ) 'recipient import round implementation'
    Assert-OmitsAll $Round @(
        'tokio::spawn', 'tokio::time::sleep', 'iroh::', 'Endpoint::',
        'RuntimeIpcCommand', 'kilogram_windows', 'std::process::Command'
    ) 'recipient import round production authority'
    Assert-ContainsAll $Plan @(
        'pub(crate) fn preflight_candidate_binding(',
        'pub(crate) fn verify_plan_object('
    ) 'M1.2.29 candidate bridge'
    Assert-ContainsAll $Import @(
        'pub(crate) fn verify_staged_transfer_imported('
    ) 'verified imported-artifact bridge'
    Assert-ContainsAll $Coordinator @(
        'pub(crate) fn verify_exact_current(',
        'pub imported_event_count: u64'
    ) 'coordinator round observation bridge'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'default budget is four sources and 128 events',
        'hard limits are eight sources and 256 events',
        'invalid late source', 'exact prefix of caller order',
        'not one cross-source transaction', 'two real source Devices',
        'full execution crate passes 27/27 tests',
        'readiness contains 156 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import round RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $roundPath, $planPath, $importPath, $coordinatorPath,
    $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$round = Get-Content -LiteralPath $roundPath -Raw
$plan = Get-Content -LiteralPath $planPath -Raw
$import = Get-Content -LiteralPath $importPath -Raw
$coordinator = Get-Content -LiteralPath $coordinatorPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library, $round,
    $plan, $import, $coordinator, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.planning.every_source_preflighted_before_first_import = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.limits.default_max_sources = 8
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.limits.hard_max_events = 512
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.apply.one_import_in_flight = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.failure_and_restart.same_round_plan_resumes_durable_prefix = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.failure_and_restart.whole_round_atomicity_claimed = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authority.background_worker_added = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '155/155'
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $round $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedRound = $round.Replace('preflight_candidate_binding(', 'unchecked_candidate(')
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedRound $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedRound = $round + "`ntokio::spawn(async {});"
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedRound $plan $import $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import round self-test rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'history_recovery_recipient_import_round_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_recipient_import_round_boundary=verified'
Write-Output 'milestone=M1.2.30'
Write-Output 'default_sources=4'
Write-Output 'hard_sources=8'
Write-Output 'maximum_concurrent_imports=1'
Write-Output 'network_authority=false'
Write-Output 'background_authority=false'
Write-Output 'public_default=legacy-only'
