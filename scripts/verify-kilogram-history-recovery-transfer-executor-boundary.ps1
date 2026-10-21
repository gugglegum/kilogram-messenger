[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-TRANSFER-EXECUTOR.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-TRANSFER-OUTBOX.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\lib.rs'
$executorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\executor.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0249-m1225-explicit-restart-safe-history-transfer-executor.md'

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
        [string]$Executor,
        [string]$CliCargo,
        [string]$DesktopCargo,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.25' -or
        [string]$Manifest.status -cne 'explicit-restart-safe-history-transfer-executor-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.24' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'history transfer executor identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.24' -or
        [string]$Predecessor.status -cne 'durable-history-transfer-outbox-verified') {
        throw 'history transfer executor predecessor changed'
    }

    foreach ($required in @(
        'explicit_invocation_only', 'loads_durable_pending_intent',
        'exact_request_bytes_each_attempt', 'source_endpoint_revalidated_before_network',
        'recipient_endpoint_revalidated_before_network',
        'verified_receipt_committed_atomically',
        'acknowledged_short_circuit_without_network'
    )) {
        if (-not [bool]$Manifest.execution.$required) {
            throw "execution property '$required' changed"
        }
    }
    if ([bool]$Manifest.execution.creates_request -or
        [bool]$Manifest.execution.modifies_request -or
        [string]$Manifest.execution.dedicated_alpn_only -cne 'kilogram/m1/history-recovery/postcard/1') {
        throw 'exact request or ALPN boundary changed'
    }

    if ([int]$Manifest.retry.default_attempts -ne 3 -or
        [int]$Manifest.retry.maximum_attempts -ne 8 -or
        [int]$Manifest.retry.default_connect_timeout_seconds -ne 30) {
        throw 'bounded retry constants changed'
    }
    foreach ($required in @(
        'zero_attempts_rejected', 'zero_connect_timeout_rejected',
        'pre_connect_classification', 'post_connect_failure_conservatively_ambiguous',
        'exhaustion_leaves_intent_pending', 'restart_reuses_exact_intent'
    )) {
        if (-not [bool]$Manifest.retry.$required) { throw "retry property '$required' changed" }
    }
    foreach ($forbidden in @(
        'new_nonce_on_retry', 'new_request_on_retry', 'background_retry',
        'persistent_retry_schedule'
    )) {
        if ([bool]$Manifest.retry.$forbidden) { throw "retry non-claim '$forbidden' changed" }
    }
    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_worker_added', 'multi_source_scheduler_added',
        'recipient_accept_service_added', 'history_import_authority_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) { throw "authority non-claim '$forbidden' changed" }
    }

    if ([int]$Manifest.verification.focused_executor_tests -ne 6 -or
        [int]$Manifest.verification.combined_transport_outbox_executor_tests -ne 14 -or
        -not [bool]$Manifest.verification.real_local_iroh_loopback_test -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '151/151' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'history transfer executor verification boundary changed'
    }
    foreach ($nonClaim in @(
        'active-runtime-history-transfer', 'background-transfer-worker',
        'automatic-network-resume', 'persistent-retry-schedule',
        'recipient-accept-service', 'multi-source-recovery-scheduling',
        'runtime-ipc-history-recovery-authority', 'desktop-history-recovery-authority'
    )) {
        if ($nonClaim -notin @($Manifest.non_claims)) { throw "required non-claim '$nonClaim' is missing" }
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
        'mod executor;', 'execute_pending_history_recovery_transfer',
        'MAX_HISTORY_RECOVERY_TRANSFER_EXECUTION_ATTEMPTS'
    ) 'history transfer library surface'

    $productionExecutor = ($Executor -split '#\[cfg\(test\)\]', 2)[0]
    Assert-ContainsAll $productionExecutor @(
        'pub const MAX_HISTORY_RECOVERY_TRANSFER_EXECUTION_ATTEMPTS: u8 = 8;',
        'const DEFAULT_EXECUTION_ATTEMPTS: u8 = 3;',
        'const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);',
        'HistoryRecoveryTransferAttemptFailureClass::PreConnect',
        'HistoryRecoveryTransferAttemptFailureClass::PostConnectAmbiguous',
        'AlreadyAcknowledgedWithoutNetwork',
        'let prepared = outbox', '.load(transfer_id)?',
        'prepared.source_endpoint_id() != endpoint_bytes(source_endpoint_id)',
        'prepared.recipient_endpoint_id() != endpoint_bytes(recipient_endpoint_id)',
        'endpoint.connect(target, HISTORY_RECOVERY_TRANSFER_ALPN)',
        'attempt(prepared.clone()).await',
        'outbox.acknowledge(transfer_id, receipt.encoded_receipt())?'
    ) 'history transfer executor implementation'
    Assert-OmitsAll $productionExecutor @(
        'tokio::spawn', 'tokio::time::sleep', 'tokio::time::interval',
        'loop {', 'DeviceIdentity', 'PreparedHistoryRecoveryTransferSession::create'
    ) 'history transfer executor production authority'
    Assert-ContainsAll $Executor @(
        'execution_policy_is_strictly_bounded',
        'retries_exact_bytes_then_commits_verified_receipt',
        'exhausted_attempt_classes_leave_exact_intent_pending_across_restart',
        'acknowledged_intent_never_invokes_network_attempt',
        'endpoint_substitution_fails_before_network_attempt',
        'explicit_iroh_executor_uses_dedicated_alpn_and_commits_receipt'
    ) 'history transfer executor tests'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'accepts only a transfer ID already present',
        'Every retry clones the same verified prepared object',
        'hard-capped at eight',
        'conservatively classified `post-connect-ambiguous`',
        'AlreadyAcknowledgedWithoutNetwork',
        'no loop, timer, persistent retry schedule, background worker',
        'readiness contains 151 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "history transfer executor RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $executorPath, $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$executor = Get-Content -LiteralPath $executorPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library,
    $executor, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.execution.creates_request = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.execution.acknowledged_short_circuit_without_network = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.retry.maximum_attempts = 9
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.retry.new_nonce_on_retry = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.retry.post_connect_failure_conservatively_ambiguous = $false
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.authority.background_worker_added = $true
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $rfc }
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '150/150'
    $mutations += { Assert-Contract $changed $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $rfc }
    $changedExecutor = $executor.Replace('attempt(prepared.clone()).await', 'attempt(new_request()).await')
    $mutations += { Assert-Contract $manifest $predecessor $completion $compatibility $library $changedExecutor $cliCargo $desktopCargo $rfc }
    $changedExecutor = $executor.Replace('endpoint.connect(target, HISTORY_RECOVERY_TRANSFER_ALPN)', 'endpoint.connect(target, fallback_alpn)')
    $mutations += { Assert-Contract $manifest $predecessor $completion $compatibility $library $changedExecutor $cliCargo $desktopCargo $rfc }
    $changedRfc = $rfc.Replace('hard-capped at eight', 'unbounded')
    $mutations += { Assert-Contract $manifest $predecessor $completion $compatibility $library $executor $cliCargo $desktopCargo $changedRfc }

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

Write-Output 'history_recovery_transfer_executor_boundary=verified'
Write-Output 'milestone=M1.2.25'
Write-Output 'maximum_attempts=8'
Write-Output 'background_retry=false'
Write-Output 'active_runtime_registration=false'
Write-Output 'public_default=legacy-only'
