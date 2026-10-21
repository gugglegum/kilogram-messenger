[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-MULTI-SOURCE-SCHEDULER.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-ACCEPTOR.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\lib.rs'
$schedulerPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\scheduler.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0251-m1227-bounded-multi-source-recovery-scheduler.md'

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
        [string]$Scheduler,
        [string]$CliCargo,
        [string]$DesktopCargo,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.27' -or
        [string]$Manifest.status -cne 'explicit-bounded-multi-source-history-recovery-scheduler-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.26' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'multi-source scheduler identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.26' -or
        [string]$Predecessor.status -cne 'explicit-bounded-history-recovery-recipient-acceptor-verified') {
        throw 'multi-source scheduler predecessor changed'
    }

    $schedule = $Manifest.schedule
    if (-not [bool]$schedule.explicit_invocation_only -or
        -not [bool]$schedule.caller_supplied_candidates_only -or
        [bool]$schedule.automatic_source_discovery -or
        [int]$schedule.minimum_sources -ne 1 -or
        [int]$schedule.maximum_sources -ne 8 -or
        [int]$schedule.default_rounds -ne 3 -or
        [int]$schedule.maximum_rounds -ne 8 -or
        [int]$schedule.default_total_attempts -ne 24 -or
        [int]$schedule.maximum_total_attempts -ne 64 -or
        [int]$schedule.default_connect_timeout_seconds -ne 30 -or
        [int]$schedule.maximum_in_flight -ne 1 -or
        -not [bool]$schedule.one_attempt_per_pending_source_per_round -or
        -not [bool]$schedule.partial_round_never_started -or
        -not [bool]$schedule.deterministic_input_order_round_robin -or
        -not [bool]$schedule.all_candidates_preflighted_before_network -or
        -not [bool]$schedule.same_account_required -or
        -not [bool]$schedule.same_recipient_required -or
        -not [bool]$schedule.distinct_source_devices_required -or
        -not [bool]$schedule.distinct_transfer_intents_required -or
        -not [bool]$schedule.exact_durable_intents_only -or
        [bool]$schedule.creates_request -or
        [bool]$schedule.modifies_request -or
        [bool]$schedule.imports_history) {
        throw 'multi-source schedule contract changed'
    }

    foreach ($required in @(
        'pre_connect_retryable', 'post_connect_ambiguous_retryable_with_exact_request',
        'retryable_failure_yields_to_next_source', 'round_exhaustion_leaves_intents_pending',
        'total_attempt_exhaustion_leaves_intents_pending',
        'unexpected_local_failure_stops_invocation', 'earlier_receipts_remain_durable',
        'next_invocation_skips_acknowledged_sources'
    )) {
        if (-not [bool]$Manifest.failure_and_resume.$required) {
            throw "failure/resume property '$required' changed"
        }
    }
    if ([bool]$Manifest.failure_and_resume.scheduler_state_persisted) {
        throw 'scheduler gained persistent retry state'
    }
    if (-not [bool]$Manifest.report.source_ordinal_only -or
        -not [bool]$Manifest.report.bounded_counts_and_closed_states -or
        [bool]$Manifest.report.identity_visible -or
        [bool]$Manifest.report.transfer_id_visible -or
        [bool]$Manifest.report.network_coordinates_visible -or
        [bool]$Manifest.report.message_bodies_visible -or
        [bool]$Manifest.report.filesystem_paths_visible -or
        [bool]$Manifest.report.import_authority_granted) {
        throw 'multi-source scheduler observation boundary changed'
    }
    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_worker_added', 'spawned_task_added', 'persistent_retry_schedule_added',
        'remote_orchestration_added', 'history_import_authority_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) { throw "authority non-claim '$forbidden' changed" }
    }

    if ([int]$Manifest.verification.focused_scheduler_tests -ne 7 -or
        [int]$Manifest.verification.combined_transport_outbox_executor_acceptor_scheduler_tests -ne 26 -or
        -not [bool]$Manifest.verification.durable_candidate_preflight_test -or
        -not [bool]$Manifest.verification.deterministic_fairness_and_budget_tests -or
        -not [bool]$Manifest.verification.reuses_previously_verified_single_transfer_iroh_loopback -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '153/153' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'multi-source scheduler verification boundary changed'
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
        'kilogram-peer-history-recovery-transport'
    ) 'active app manifests'
    Assert-ContainsAll $Library @(
        'mod scheduler;', 'schedule_pending_history_recovery_transfers',
        'MAX_HISTORY_RECOVERY_SCHEDULE_TOTAL_ATTEMPTS'
    ) 'history transfer library surface'

    $productionScheduler = ($Scheduler -split '#\[cfg\(test\)\]', 2)[0]
    Assert-ContainsAll $productionScheduler @(
        'pub const MAX_HISTORY_RECOVERY_SCHEDULE_SOURCES: usize = 8;',
        'pub const MAX_HISTORY_RECOVERY_SCHEDULE_ROUNDS: u8 = 8;',
        'pub const MAX_HISTORY_RECOVERY_SCHEDULE_TOTAL_ATTEMPTS: u8 = 64;',
        'const DEFAULT_SCHEDULE_ROUNDS: u8 = 3;',
        'const DEFAULT_SCHEDULE_TOTAL_ATTEMPTS: u8 = 24;',
        'preflight_candidates(candidates)?',
        'HistoryRecoveryTransferExecutionPolicy::new(1, policy.connect_timeout())',
        'execute_pending_history_recovery_transfer(',
        'source_device_ids.contains(&prepared.source_device_id())',
        'transfer_ids.contains(&prepared.transfer_id())',
        "'rounds: for _ in 0..policy.max_rounds()",
        'if remaining_attempts < pending_in_round',
        'for (source_index, source) in sources.iter_mut().enumerate()',
        'contains_identity: false', 'contains_network_coordinates: false',
        'contains_message_bodies: false', 'grants_import_authority: false'
    ) 'multi-source scheduler implementation'
    Assert-OmitsAll $productionScheduler @(
        'tokio::spawn', 'tokio::time::sleep', 'tokio::time::interval',
        'PreparedHistoryRecoveryTransferSession::create', 'DeviceIdentity',
        'RuntimeIpcCommand', 'kilogram_windows', 'import_staged'
    ) 'multi-source scheduler production authority'
    Assert-ContainsAll $Scheduler @(
        'schedule_policy_is_strictly_bounded',
        'round_robin_yields_after_failure_and_reaches_all_sources',
        'total_attempt_budget_never_starts_a_partial_round',
        'acknowledged_sources_are_skipped_without_attempts',
        'round_budget_leaves_exact_sources_pending_for_next_invocation',
        'empty_and_fatal_schedules_fail_closed',
        'candidate_preflight_rejects_endpoint_and_duplicate_source_before_network'
    ) 'multi-source scheduler tests'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'caller-supplied slice of one to eight already durable transfer intents',
        'Before the first network attempt it loads and revalidates every request',
        'hard limits are eight rounds and 64 total invocations',
        'at most one attempt is made for each still-pending source',
        'yields immediately to the next source',
        'partial round can therefore never favor an earlier input',
        'scheduler itself persists no retry state',
        'readiness contains 153 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "multi-source scheduler RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $schedulerPath, $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$scheduler = Get-Content -LiteralPath $schedulerPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library,
    $scheduler, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.schedule.maximum_sources = 9
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.schedule.maximum_total_attempts = 65
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.schedule.all_candidates_preflighted_before_network = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.schedule.partial_round_never_started = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.schedule.creates_request = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.failure_and_resume.retryable_failure_yields_to_next_source = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authority.background_worker_added = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '152/152'
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedScheduler = $scheduler.Replace('HistoryRecoveryTransferExecutionPolicy::new(1, policy.connect_timeout())', 'HistoryRecoveryTransferExecutionPolicy::default()')
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedScheduler $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedRfc = $rfc -replace 'hard limits are eight rounds and 64 total invocations', 'limits are unbounded'
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $scheduler $cliCargo $desktopCargo $changedRfc }.GetNewClosure())

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
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_multi_source_scheduler_boundary=verified'
Write-Output 'milestone=M1.2.27'
Write-Output 'maximum_sources=8'
Write-Output 'maximum_rounds=8'
Write-Output 'maximum_total_attempts=64'
Write-Output 'background_worker=false'
Write-Output 'active_runtime_registration=false'
Write-Output 'public_default=legacy-only'
