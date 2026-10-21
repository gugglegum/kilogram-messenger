[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-COORDINATOR.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-MULTI-SOURCE-SCHEDULER.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$coordinatorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\coordinator.rs'
$transferPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\transfer.rs'
$importPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\import.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0252-m1228-durable-recipient-recovery-coordinator.md'

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
        [object]$Manifest,
        [object]$Predecessor,
        [object]$Completion,
        [object]$Compatibility,
        [string]$Library,
        [string]$Coordinator,
        [string]$Transfer,
        [string]$Import,
        [string]$CliCargo,
        [string]$DesktopCargo,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.28' -or
        [string]$Manifest.status -cne 'durable-recipient-history-recovery-coordinator-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.27' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient coordinator identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.27' -or
        [string]$Predecessor.status -cne 'explicit-bounded-multi-source-history-recovery-scheduler-verified') {
        throw 'recipient coordinator predecessor changed'
    }

    $planning = $Manifest.planning
    if (-not [bool]$planning.explicit_caller_supplied_approved_sources_only -or
        [int]$planning.minimum_sources -ne 1 -or
        [int]$planning.maximum_sources -ne 8 -or
        -not [bool]$planning.same_account_required -or
        -not [bool]$planning.same_recipient_required -or
        -not [bool]$planning.distinct_source_devices_required -or
        -not [bool]$planning.durable_consent_revalidated -or
        -not [bool]$planning.reads_verified_transfer_heads -or
        -not [bool]$planning.reads_verified_import_receipt_heads -or
        -not [bool]$planning.read_only -or
        [bool]$planning.creates_state -or
        [bool]$planning.opens_network -or
        [bool]$planning.imports_history) {
        throw 'recipient coordinator planning boundary changed'
    }
    $apply = $Manifest.apply
    if (-not [bool]$apply.explicit_only -or
        -not [bool]$apply.same_state_directory_required -or
        -not [bool]$apply.exact_observation_revalidated_under_state_lock -or
        -not [bool]$apply.stale_plan_rejected -or
        -not [bool]$apply.immediate_durability -or
        -not [bool]$apply.all_sources_committed_atomically -or
        -not [bool]$apply.exact_replay_idempotent -or
        [bool]$apply.performs_network_io -or
        [bool]$apply.performs_history_import) {
        throw 'recipient coordinator apply boundary changed'
    }
    $storage = $Manifest.storage
    if (-not [bool]$storage.separate_coordinator_database -or
        [int]$storage.maximum_durable_plans -ne 16384 -or
        [int]$storage.maximum_durable_source_heads -ne 16384 -or
        -not [bool]$storage.append_only_content_addressed_plan_records -or
        -not [bool]$storage.single_monotonic_plan_chain -or
        -not [bool]$storage.opaque_source_keys_only -or
        -not [bool]$storage.opaque_transfer_head_ids_only -or
        -not [bool]$storage.opaque_import_receipt_head_ids_only -or
        -not [bool]$storage.monotonic_counts_and_ranges_only -or
        [bool]$storage.stores_ciphertext_artifacts -or
        [bool]$storage.stores_plaintext -or
        [bool]$storage.stores_message_bodies -or
        [bool]$storage.whole_database_rollback_detection_supported) {
        throw 'recipient coordinator storage boundary changed'
    }
    foreach ($required in @(
        'crash_before_commit_leaves_no_plan_progress', 'restart_revalidates_full_plan_chain',
        'forked_plan_chain_rejected', 'noncanonical_records_rejected',
        'head_substitution_rejected', 'progress_regression_rejected',
        'completed_source_regression_rejected', 'new_progress_requires_fresh_exact_plan'
    )) {
        if (-not [bool]$Manifest.failure_and_restart.$required) {
            throw "failure/restart property '$required' changed"
        }
    }
    if (-not [bool]$Manifest.report.source_ordinal_only -or
        -not [bool]$Manifest.report.closed_progress_states -or
        -not [bool]$Manifest.report.bounded_counts -or
        [bool]$Manifest.report.identity_visible -or
        [bool]$Manifest.report.network_coordinates_visible -or
        [bool]$Manifest.report.message_bodies_visible -or
        [bool]$Manifest.report.plaintext_visible -or
        [bool]$Manifest.report.network_authority_granted -or
        [bool]$Manifest.report.import_authority_granted -or
        [bool]$Manifest.report.background_authority_granted) {
        throw 'recipient coordinator report boundary changed'
    }
    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_worker_added', 'spawned_task_added',
        'automatic_source_discovery_added', 'history_import_authority_added',
        'network_authority_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) {
            throw "recipient coordinator authority non-claim '$forbidden' changed"
        }
    }
    if ([int]$Manifest.verification.focused_coordinator_tests -ne 4 -or
        [int]$Manifest.verification.combined_execution_crate_tests -ne 18 -or
        -not [bool]$Manifest.verification.read_only_plan_and_exact_replay_test -or
        -not [bool]$Manifest.verification.real_staged_transfer_and_import_progression_test -or
        -not [bool]$Manifest.verification.stale_plan_and_precommit_rollback_test -or
        -not [bool]$Manifest.verification.completed_source_regression_test -or
        -not [bool]$Manifest.verification.duplicate_source_preflight_test -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '154/154' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recipient coordinator verification boundary changed'
    }

    if ([int]$Completion.active_postcard.version -ne 34 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v15\0' -or
        @($Completion.canonical.commands).Count -ne 39 -or
        @($Completion.canonical.results).Count -ne 38) {
        throw 'active runtime IPC inventory changed'
    }
    $runtimeSurface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ([bool]$Compatibility.changes_wire -or
        $runtimeSurface.Count -ne 1 -or
        [int](@($runtimeSurface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
        throw 'compatibility inventory changed'
    }

    Assert-OmitsAll ($CliCargo + "`n" + $DesktopCargo) @(
        'kilogram-peer-history-recovery-execution'
    ) 'active app manifests'
    Assert-ContainsAll $Library @(
        'mod coordinator;', 'RecipientHistoryRecoveryCoordinator',
        'recipient_coordinator_plan_is_read_only_and_apply_replays_exactly',
        'recipient_coordinator_tracks_staged_and_imported_heads_without_acting',
        'recipient_coordinator_rejects_stale_plan_and_rolls_back_failed_apply',
        'recipient_coordinator_rejects_duplicate_sources_before_state_creation'
    ) 'recipient coordinator library surface and tests'
    Assert-ContainsAll $Transfer @('pub(crate) fn coordinator_head(') 'transfer exact-head surface'
    Assert-ContainsAll $Import @(
        'pub(crate) fn coordinator_head(', 'pub(crate) fn state_directory(&self) -> &Path'
    ) 'import exact-head surface'

    $productionCoordinator = $Coordinator
    Assert-ContainsAll $productionCoordinator @(
        'pub const MAX_RECIPIENT_RECOVERY_COORDINATOR_SOURCES: usize = 8;',
        'pub const MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_PLANS: u64 = 16_384;',
        'pub const MAX_DURABLE_RECIPIENT_RECOVERY_COORDINATOR_SOURCES: u64 = 16_384;',
        'pub fn from_durable_consent(', 'consent.signed_plan()?',
        'pub fn plan(', 'pub fn apply(',
        'let state_lock = StateDirectoryLock::acquire(&plan.state_directory)?;',
        'let current = build_plan(', 'if current != *plan || !plan.requires_apply',
        '.set_durability(Durability::Immediate)',
        'validate_monotonic_observation(',
        'CompletedSourceRegression', 'HeadFork', 'StalePlan',
        'contains_message_bodies: false', 'contains_plaintext: false',
        'grants_network_authority: false', 'grants_import_authority: false',
        'background_work_authorized: false',
        'whole_database_rollback_detection_supported: false'
    ) 'recipient coordinator implementation'
    Assert-OmitsAll $productionCoordinator @(
        'tokio::spawn', 'tokio::time::sleep', 'iroh::', 'Endpoint::',
        'import_staged_transfer(', 'stage_envelope(', 'RuntimeIpcCommand',
        'kilogram_windows', 'std::process::Command'
    ) 'recipient coordinator production authority'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'caller supplies one to eight approved sources',
        'Planning is read-only and creates no state',
        're-runs the entire observation',
        'single redb transaction with immediate durability',
        'completed source cannot become pending again',
        'stores only opaque content-addressed IDs and monotonic state',
        'cannot prove whole-database rollback without an external witness',
        'full execution crate passes 18/18 tests',
        'readiness contains 154 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient coordinator RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $coordinatorPath, $transferPath, $importPath,
    $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$coordinator = Get-Content -LiteralPath $coordinatorPath -Raw
$transfer = Get-Content -LiteralPath $transferPath -Raw
$import = Get-Content -LiteralPath $importPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library,
    $coordinator, $transfer, $import, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.planning.maximum_sources = 9
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.planning.read_only = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.apply.exact_observation_revalidated_under_state_lock = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.storage.stores_plaintext = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.storage.whole_database_rollback_detection_supported = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.failure_and_restart.completed_source_regression_rejected = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authority.background_worker_added = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '153/153'
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $coordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedCoordinator = $coordinator.Replace('let state_lock = StateDirectoryLock::acquire(&plan.state_directory)?;', 'let state_lock = fake_lock()?;')
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedCoordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedCoordinator = $coordinator + "`ntokio::spawn(async {});"
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedCoordinator $transfer $import $cliCargo $desktopCargo $rfc }.GetNewClosure())

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient coordinator self-test rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'history_recovery_recipient_coordinator_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_recipient_coordinator_boundary=verified'
Write-Output 'milestone=M1.2.28'
Write-Output 'maximum_approved_sources=8'
Write-Output 'planning_read_only=true'
Write-Output 'apply_exact_head_bound=true'
Write-Output 'network_authority=false'
Write-Output 'import_authority=false'
Write-Output 'public_default=legacy-only'
