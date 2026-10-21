[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-PLAN-SESSION-CACHE.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-RECIPIENT-IMPORT-CONSUMPTION-RESPONSE-DISPATCHER.json'
$sourcePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_session_cache.rs'
$dispatcherPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\recipient_import_consumption_response_dispatcher.rs'
$libPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\src\lib.rs'
$cratePath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-runtime\Cargo.toml'
$executionPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-execution\src\lib.rs'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0266-m1242-bounded-recipient-import-consumption-plan-session-cache.md'

function Copy-JsonObject { param([object]$Value) $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json }
function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Boundary {
    param([object]$Manifest, [object]$Predecessor, [string]$Source,
          [string]$Dispatcher, [string]$Lib, [string]$Crate, [string]$Execution,
          [string]$Ipc, [string]$Cli, [string]$Desktop, [string]$Rfc)
    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.42' -or
        [string]$Manifest.status -cne 'canonical-recipient-import-consumption-plan-session-cache-verified' -or
        [string]$Manifest.predecessor -cne 'M1.2.41' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_history_transfer_wire -or
        [bool]$Manifest.changes_runtime_ipc_contract -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [int]$Manifest.runtime_ipc_version -ne 33 -or
        [int]$Manifest.runtime_ipc_domain -ne 14 -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'Plan session cache identity or public scope changed'
    }
    if ([string]$Predecessor.status -cne 'canonical-recipient-import-consumption-response-dispatcher-verified') {
        throw 'Plan session cache predecessor is not the exact M1.2.41 dispatcher'
    }
    $session = $Manifest.session_boundary
    foreach ($property in @(
        'process_local_only', 'rejects_capacity_without_eviction',
        'expired_sessions_purged', 'restart_requires_fresh_plan',
        'stores_opaque_dispatcher_stage', 'exact_plan_id_and_summary_required',
        'exact_plan_request_digest_retained', 'commit_revalidates_descriptor_and_device',
        'commit_revalidates_live_durable_state', 'same_process_exact_replay',
        'stage_skipping_rejected'
    )) {
        if (-not [bool]$session.$property) { throw "Plan session cache property '$property' changed" }
    }
    if ([bool]$session.durable_or_serializable -or
        [bool]$session.commit_authority_granted_by_cache -or
        [int]$session.maximum_live_sessions -ne 32) {
        throw 'Plan session cache persistence, authority, or bound changed'
    }
    foreach ($property in @(
        'active_runtime_registration', 'runtime_socket_handler', 'cli_or_desktop_control',
        'background_worker_or_timer', 'network', 'automatic_consumption',
        'history_or_membership_deletion'
    )) {
        if ([bool]$Manifest.activation_boundary.$property) {
            throw "Plan session cache gained forbidden authority '$property'"
        }
    }
    $verification = $Manifest.verification
    if (-not [bool]$verification.stable_path_rust_harness_required_on_windows -or
        [string]$verification.focused_durable_fixture_tests -cne '1/1' -or
        [string]$verification.boundary_negative_mutations -cne '10/10' -or
        [string]$verification.m1_readiness -cne '168/168' -or
        -not [bool]$verification.strict_clippy -or
        [bool]$verification.release_build -or [bool]$verification.archive_created -or
        [bool]$verification.git_tag_created -or [bool]$verification.automatic_github_build -or
        [bool]$verification.second_host_required -or [bool]$verification.network_executed) {
        throw 'Plan session cache verification claim changed'
    }

    Assert-ContainsAll $Source @(
        'MAX_RECIPIENT_IMPORT_CONSUMPTION_PLAN_SESSIONS: usize = 32',
        'pub struct RecipientImportConsumptionPlanSessionCache',
        'VecDeque<RecipientImportConsumptionPlanSession>',
        'dispatch: RecipientImportConsumptionResponseDispatch',
        'plan_request_digest: [u8; 32]',
        'RecipientImportConsumptionPlanSessionCacheError::CapacityReached',
        'RecipientImportConsumptionPlanSessionCacheError::PlanCollision',
        'RecipientImportConsumptionPlanSessionCacheError::MissingPlanStage',
        'self.purge_expired(now_unix_seconds)',
        '.dispatch_commit(',
        'pub fn is_durable(&self) -> bool',
        'pub fn grants_commit_authority(&self) -> bool',
        'pub fn opened_network(&self) -> bool'
    ) 'Plan session cache source'
    Assert-ContainsAll $Dispatcher @(
        'RuntimeIpcRecipientImportConsumptionRequest::decode_and_validate',
        'RecipientImportConsumptionAuthorizationLedger::new',
        'authorizations.commit(',
        'RecipientHistoryRecoveryImportConsumptionMutationAdapter::new'
    ) 'M1.2.41 dispatcher source'
    Assert-ContainsAll $Lib @(
        'mod recipient_import_consumption_session_cache;',
        'RecipientImportConsumptionPlanSessionCache',
        'MAX_RECIPIENT_IMPORT_CONSUMPTION_PLAN_SESSIONS'
    ) 'runtime integration library'
    Assert-ContainsAll $Execution @(
        'recipient_import_consumption_plan_session_cache_requires_same_process_plan_and_expires',
        'RecipientImportConsumptionPlanSessionInsertOutcome::ExactReplay',
        'RecipientImportConsumptionPlanSessionCacheError::MissingPlanStage',
        'RecipientImportConsumptionPlanSessionCacheError::ExpiredPlan',
        'RuntimeIpcRecipientImportConsumptionOutcome::AlreadyConsumed'
    ) 'durable fixture test'
    Assert-ContainsAll $Ipc @(
        'const IPC_VERSION: u8 = 34;',
        'kilogram:runtime-ipc-descriptor:v15\0'
    ) 'active runtime IPC source'
    Assert-ContainsAll $Crate @('name = "kilogram-peer-history-recovery-runtime"') 'runtime crate'
    foreach ($dependency in @('iroh.workspace', 'tokio.workspace', 'reqwest.workspace')) {
        if ($Crate.IndexOf($dependency, [StringComparison]::Ordinal) -ge 0) {
            throw "Plan session cache crate gained '$dependency'"
        }
    }
    foreach ($app in @($Cli, $Desktop)) {
        if ($app.IndexOf('kilogram-peer-history-recovery-runtime', [StringComparison]::Ordinal) -ge 0) {
            throw 'Plan session cache was wired into an active application'
        }
    }
    foreach ($forbidden in @('serde::', 'redb::', 'tokio::', 'iroh::', 'reqwest::',
                              'TcpListener', 'TcpStream', 'RuntimeIpcCommand::RecipientImport')) {
        if ($Source.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "Plan session cache gained forbidden surface '$forbidden'"
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'plain process-local Rust value',
        'Restart constructs an empty cache',
        'at most 32 live opaque',
        'rejects the new stage rather than silently evicting',
        'does not grant Commit authority',
        'version 34 / domain v15',
        'no active runtime command',
        'contains 168 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "Plan session cache RFC is missing '$needle'"
        }
    }
}

foreach ($path in @($manifestPath, $predecessorPath, $sourcePath, $dispatcherPath,
    $libPath, $cratePath, $executionPath, $ipcPath, $cliPath, $desktopPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$source = Get-Content -LiteralPath $sourcePath -Raw
$dispatcher = Get-Content -LiteralPath $dispatcherPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$crate = Get-Content -LiteralPath $cratePath -Raw
$execution = Get-Content -LiteralPath $executionPath -Raw
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @($manifest, $predecessor, $source, $dispatcher, $lib, $crate,
    $execution, $ipc, $cli, $desktop, $rfc)
Assert-Boundary @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.milestone = 'M1.2.41'
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.changes_active_runtime_ipc = $true
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.session_boundary.maximum_live_sessions = 0
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.session_boundary.durable_or_serializable = $true
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.session_boundary.rejects_capacity_without_eviction = $false
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.session_boundary.restart_requires_fresh_plan = $false
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.session_boundary.commit_authority_granted_by_cache = $true
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.session_boundary.stage_skipping_rejected = $false
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest; $changed.activation_boundary.network = $true
    $mutations += { Assert-Boundary $changed $predecessor $source $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $changedSource = $source.Replace('RecipientImportConsumptionPlanSessionCacheError::CapacityReached', 'removed_capacity_failure')
    $mutations += { Assert-Boundary $manifest $predecessor $changedSource $dispatcher $lib $crate $execution $ipc $cli $desktop $rfc }
    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation; throw 'mutation was accepted' } catch {
            if ($_.Exception.Message -eq 'mutation was accepted') { throw }
            $rejected++
        }
    }
    if ($rejected -ne 10) { throw "expected 10 rejected mutations, got $rejected" }
    Write-Output 'negative_mutations=10/10'
}

Write-Output 'canonical_recipient_import_consumption_plan_session_cache_boundary=verified'
Write-Output 'changes_active_runtime_ipc=false'
Write-Output 'cache_lifetime=process-local-bounded'
Write-Output 'restart_requires_fresh_plan=true'
Write-Output 'network_executed=false'
