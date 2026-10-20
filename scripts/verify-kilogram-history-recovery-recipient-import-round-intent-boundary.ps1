[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-IMPORT-ROUND-INTENT.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-IMPORT-ROUND.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$intentPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round_intent.rs'
$roundPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\recipient_import_round.rs'
$coordinatorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\coordinator.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0255-m1231-durable-recipient-import-round-intent.md'

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
        [object]$Compatibility, [string]$Library, [string]$Intent,
        [string]$Round, [string]$Coordinator, [string]$CliCargo,
        [string]$DesktopCargo, [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.31' -or
        [string]$Manifest.status -cne 'durable-explicit-recipient-import-round-intent-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.30' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient import-round intent identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.30' -or
        [string]$Predecessor.status -cne 'bounded-explicit-multi-source-recipient-import-round-verified') {
        throw 'recipient import-round intent predecessor changed'
    }

    foreach ($required in @(
        'read_only', 'requires_exact_unstarted_round',
        'requires_exact_durable_coordinator'
    )) {
        if (-not [bool]$Manifest.planning.$required) {
            throw "planning property '$required' changed"
        }
    }
    if ([bool]$Manifest.planning.creates_state -or
        [bool]$Manifest.planning.opens_network -or
        [bool]$Manifest.planning.imports_history -or
        [bool]$Manifest.planning.schedules_background_work) {
        throw 'recipient import-round intent planning authority changed'
    }

    foreach ($required in @(
        'recipient_device_signed', 'content_addressed',
        'binds_owner_account_and_device', 'binds_round_id',
        'binds_baseline_coordinator_plan_and_sequence',
        'binds_all_coordinator_source_baselines', 'binds_ordered_source_ordinals',
        'binds_per_source_import_plan_ids', 'binds_exact_candidate_heads_and_ranges',
        'binds_membership_digest_and_revision', 'binds_source_and_event_budgets',
        'commit_timestamp_not_fork_relevant'
    )) {
        if (-not [bool]$Manifest.intent.$required) {
            throw "intent binding '$required' changed"
        }
    }

    if (-not [bool]$Manifest.storage.separate_redb -or
        -not [bool]$Manifest.storage.immediate_durability -or
        -not [bool]$Manifest.storage.state_directory_lock -or
        -not [bool]$Manifest.storage.symlinks_rejected -or
        -not [bool]$Manifest.storage.canonical_postcard_records -or
        [int]$Manifest.storage.maximum_intents -ne 4096 -or
        [int]$Manifest.storage.maximum_record_bytes -ne 65536 -or
        [int]$Manifest.storage.maximum_active_unconsumed_intents -ne 1 -or
        -not [bool]$Manifest.storage.consumed_history_retained -or
        [bool]$Manifest.storage.stores_membership_snapshot -or
        [bool]$Manifest.storage.stores_message_bodies -or
        [bool]$Manifest.storage.stores_plaintext -or
        [bool]$Manifest.storage.stores_ciphertext_artifacts -or
        [bool]$Manifest.storage.whole_database_rollback_detection_supported) {
        throw 'recipient import-round intent storage boundary changed'
    }

    foreach ($required in @(
        'commit_explicit_only', 'inspect_read_only', 'consume_explicit_only',
        'exact_commit_replay_idempotent', 'exact_consume_replay_idempotent',
        'active_competing_round_rejected', 'same_round_fork_rejected',
        'stale_precommit_state_rejected', 'foreign_recipient_rejected'
    )) {
        if (-not [bool]$Manifest.operations.$required) {
            throw "operation property '$required' changed"
        }
    }

    if ((@($Manifest.progress.states) -join ',') -cne 'not-started,partial,complete') {
        throw 'recipient import-round intent progress states changed'
    }
    foreach ($required in @(
        'exact_ordered_prefix_required', 'unplanned_sources_must_remain_unchanged',
        'completed_receipts_and_artifacts_reverified',
        'baseline_coordinator_chain_member_required',
        'import_before_coordinator_refresh_observable',
        'consume_requires_complete_round', 'consume_requires_coordinator_refresh',
        'consumption_recipient_device_signed', 'consumption_binds_final_coordinator'
    )) {
        if (-not [bool]$Manifest.progress.$required) {
            throw "progress property '$required' changed"
        }
    }

    foreach ($forbidden in @(
        'executes_import_round', 'active_runtime_registered', 'cli_command_added',
        'desktop_control_added', 'background_worker_added', 'spawned_task_added',
        'source_discovery_added', 'automatic_round_loop_added',
        'network_authority_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) {
            throw "recipient import-round intent authority non-claim '$forbidden' changed"
        }
    }

    if ([int]$Manifest.verification.focused_intent_tests -ne 4 -or
        [int]$Manifest.verification.combined_execution_crate_tests -ne 31 -or
        -not [bool]$Manifest.verification.read_only_plan_and_commit_replay_test -or
        -not [bool]$Manifest.verification.foreign_competing_and_stale_test -or
        -not [bool]$Manifest.verification.two_source_partial_restart_and_consume_test -or
        -not [bool]$Manifest.verification.pre_refresh_consume_block_test -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '157/157' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recipient import-round intent verification boundary changed'
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
        'mod recipient_import_round_intent;',
        'RecipientRecoveryImportRoundIntentStore',
        'recipient_import_round_intent_plan_is_read_only_and_commit_replays',
        'recipient_import_round_intent_rejects_foreign_competing_and_stale_commit',
        'recipient_import_round_intent_observes_partial_prefix_and_consumes_complete_round',
        'recipient_import_round_intent_blocks_consume_until_coordinator_refresh'
    ) 'recipient import-round intent library surface and tests'
    Assert-ContainsAll $Intent @(
        'recipient-import-round-intents-v1.redb',
        'MAX_DURABLE_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENTS: u64 = 4_096',
        'MAX_RECIPIENT_RECOVERY_IMPORT_ROUND_INTENT_BYTES: usize = 64 * 1024',
        'pub struct RecipientRecoveryImportRoundIntentStore',
        'pub fn plan(', 'pub fn inspect_active(', 'pub fn commit(',
        'pub fn inspect_progress(', 'pub fn consume(',
        'Durability::Immediate', 'StateDirectoryLock::acquire',
        'RecipientRecoveryImportRoundIntentProgressState::NotStarted',
        'RecipientRecoveryImportRoundIntentProgressState::Partial',
        'RecipientRecoveryImportRoundIntentProgressState::Complete',
        'verify_chain_member(', 'UnplannedSourceChanged',
        'CoordinatorRefreshRequired', 'ActiveIntentConflict', 'IntentFork',
        'contains_membership_snapshot: false', 'contains_message_bodies: false',
        'contains_plaintext: false', 'opened_network: false',
        'scheduled_background_work: false'
    ) 'recipient import-round intent implementation'
    Assert-OmitsAll $Intent @(
        'tokio::spawn', 'tokio::time::sleep', 'iroh::', 'Endpoint::',
        'RuntimeIpcCommand', 'kilogram_windows', 'std::process::Command'
    ) 'recipient import-round intent production authority'
    Assert-ContainsAll $Round @(
        'pub(crate) fn verify_exact_unstarted(',
        'pub(crate) fn coordination_plan(', 'pub(crate) fn import_plans('
    ) 'M1.2.30 intent bridge'
    Assert-ContainsAll $Coordinator @(
        'pub(crate) fn verify_exact_observation(',
        'pub(crate) fn verify_chain_member(', 'pub(crate) fn previous_plan_id('
    ) 'durable coordinator observation bridge'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'planning is read-only', 'exact, unstarted M1.2.30 round',
        'exactly one active unconsumed intent', 'exact prefix of the signed caller order',
        'coordinator_refresh_required=true', 'does not execute the M1.2.30 round',
        'full execution crate passes 31/31 tests',
        'readiness contains 157 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient import-round intent RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $intentPath, $roundPath, $coordinatorPath,
    $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$intent = Get-Content -LiteralPath $intentPath -Raw
$round = Get-Content -LiteralPath $roundPath -Raw
$coordinator = Get-Content -LiteralPath $coordinatorPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library, $intent,
    $round, $coordinator, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.planning.read_only = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.intent.binds_ordered_source_ordinals = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.storage.maximum_active_unconsumed_intents = 2
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.storage.stores_membership_snapshot = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.operations.same_round_fork_rejected = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.progress.consume_requires_coordinator_refresh = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authority.executes_import_round = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '156/156'
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $intent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedIntent = $intent.Replace('Durability::Immediate', 'Durability::Eventual')
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedIntent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedIntent = $intent + "`ntokio::spawn(async {});"
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedIntent $round $coordinator $cliCargo $desktopCargo $rfc }.GetNewClosure())

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne $mutations.Count -or $mutations.Count -ne 10) {
        throw "recipient import-round intent self-test rejected $rejected/$($mutations.Count) mutations"
    }
    Write-Output 'history_recovery_recipient_import_round_intent_self_test=passed'
    Write-Output 'negative_mutations_rejected=10/10'
}

Write-Output 'history_recovery_recipient_import_round_intent_boundary=verified'
Write-Output 'milestone=M1.2.31'
Write-Output 'active_unconsumed_intents=1'
Write-Output 'intent_executes_import=false'
Write-Output 'network_authority=false'
Write-Output 'background_authority=false'
Write-Output 'public_default=legacy-only'
