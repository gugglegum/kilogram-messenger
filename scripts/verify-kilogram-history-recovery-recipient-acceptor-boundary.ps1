[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-RECIPIENT-ACCEPTOR.json'
$predecessorPath = Join-Path $workspace 'CANONICAL-HISTORY-RECOVERY-TRANSFER-EXECUTOR.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$libraryPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\lib.rs'
$acceptorPath = Join-Path $workspace 'crates\kilogram-peer-history-recovery-transport\src\acceptor.rs'
$cliCargoPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'
$desktopCargoPath = Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'
$rfcPath = Join-Path $workspace 'docs\RFC-0250-m1226-explicit-bounded-recipient-acceptor.md'

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
        [string]$Acceptor,
        [string]$CliCargo,
        [string]$DesktopCargo,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.26' -or
        [string]$Manifest.status -cne 'explicit-bounded-history-recovery-recipient-acceptor-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.25' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_desktop_runtime_authority -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'recipient acceptor identity or scope changed'
    }
    if ([string]$Predecessor.milestone -cne 'M1.2.25' -or
        [string]$Predecessor.status -cne 'explicit-restart-safe-history-transfer-executor-verified') {
        throw 'recipient acceptor predecessor changed'
    }

    $accept = $Manifest.acceptor
    if (-not [bool]$accept.explicit_invocation_only -or
        -not [bool]$accept.caller_owned_endpoint -or
        [string]$accept.dedicated_alpn_only -cne 'kilogram/m1/history-recovery/postcard/1' -or
        [int]$accept.default_sessions -ne 1 -or
        [int]$accept.maximum_sessions -ne 8 -or
        [int]$accept.maximum_in_flight -ne 1 -or
        [int]$accept.default_idle_timeout_seconds -ne 30 -or
        [int]$accept.default_request_timeout_seconds -ne 30 -or
        [int]$accept.default_completion_timeout_seconds -ne 210 -or
        [int]$accept.maximum_timeout_seconds -ne 300 -or
        -not [bool]$accept.malformed_session_consumes_slot -or
        -not [bool]$accept.rejected_session_does_not_end_budget -or
        -not [bool]$accept.durable_stage_precedes_success -or
        [bool]$accept.imports_history) {
        throw 'bounded recipient acceptor contract changed'
    }

    if (-not [bool]$Manifest.report.aggregate_counts_only -or
        -not [bool]$Manifest.report.termination_reason_visible -or
        -not [bool]$Manifest.report.last_failure_class_visible -or
        [bool]$Manifest.report.identity_visible -or
        [bool]$Manifest.report.network_coordinates_visible -or
        [bool]$Manifest.report.message_bodies_visible -or
        [bool]$Manifest.report.filesystem_paths_visible -or
        [bool]$Manifest.report.import_authority_granted) {
        throw 'recipient acceptor observation boundary changed'
    }
    foreach ($forbidden in @(
        'active_runtime_registered', 'cli_command_added', 'desktop_control_added',
        'background_service_added', 'spawned_task_added', 'persistent_schedule_added',
        'multi_source_scheduler_added', 'history_import_authority_added'
    )) {
        if ([bool]$Manifest.authority.$forbidden) { throw "authority non-claim '$forbidden' changed" }
    }

    if ([int]$Manifest.verification.focused_acceptor_tests -ne 5 -or
        [int]$Manifest.verification.combined_transport_outbox_executor_acceptor_tests -ne 19 -or
        -not [bool]$Manifest.verification.real_single_session_local_iroh_loopback_test -or
        -not [bool]$Manifest.verification.deterministic_multi_session_control_flow_tests -or
        -not [bool]$Manifest.verification.stable_path_rust_harness_required_on_windows -or
        [string]$Manifest.verification.boundary_self_test_and_negative_mutations -cne 'verified' -or
        [string]$Manifest.verification.m1_readiness -cne '152/152' -or
        [bool]$Manifest.verification.second_host_required -or
        [bool]$Manifest.verification.product_network_executed -or
        [bool]$Manifest.verification.release_build -or
        [bool]$Manifest.verification.archive_created -or
        [bool]$Manifest.verification.git_tag_created -or
        [bool]$Manifest.verification.automatic_github_build) {
        throw 'recipient acceptor verification boundary changed'
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
        'mod acceptor;', 'accept_history_recovery_transfers',
        'MAX_HISTORY_RECOVERY_RECIPIENT_ACCEPT_SESSIONS'
    ) 'history transfer library surface'

    $productionAcceptor = ($Acceptor -split '#\[cfg\(test\)\]', 2)[0]
    Assert-ContainsAll $productionAcceptor @(
        'pub const MAX_HISTORY_RECOVERY_RECIPIENT_ACCEPT_SESSIONS: u8 = 8;',
        'const DEFAULT_ACCEPT_SESSIONS: u8 = 1;',
        'const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);',
        'const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);',
        'const DEFAULT_COMPLETION_TIMEOUT: Duration = Duration::from_secs(210);',
        'const MAX_ACCEPT_TIMEOUT: Duration = Duration::from_secs(300);',
        'run_bounded_accept_loop(policy', 'endpoint.accept()',
        'connection.alpn() != HISTORY_RECOVERY_TRANSFER_ALPN',
        'read_history_recovery_transfer_request', 'complete_history_recovery_transfer',
        'report.slots_consumed = report.slots_consumed.saturating_add(1);',
        'contains_identity: false', 'contains_network_coordinates: false',
        'contains_message_bodies: false', 'grants_import_authority: false'
    ) 'recipient acceptor implementation'
    Assert-OmitsAll $productionAcceptor @(
        'tokio::spawn', 'tokio::time::sleep', 'tokio::time::interval',
        'RuntimeIpcCommand', 'kilogram_windows', 'import_staged'
    ) 'recipient acceptor production authority'
    Assert-ContainsAll $Acceptor @(
        'accept_policy_is_strictly_bounded',
        'rejected_sessions_do_not_block_later_valid_transfer',
        'request_timeout_does_not_block_later_valid_transfer',
        'real_loopback_stages_one_transfer',
        'idle_deadline_returns_body_free_report'
    ) 'recipient acceptor tests'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'default is one session and the hard cap is eight',
        'exactly one session in flight',
        'Every accepted connection consumes one slot',
        'does not end the remaining budget',
        'never imports history',
        'no active runtime registration, CLI command, desktop control',
        'readiness contains 152 closed gates',
        'public default remains `legacy-only`'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "recipient acceptor RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $predecessorPath, $completionPath, $compatibilityPath,
    $libraryPath, $acceptorPath, $cliCargoPath, $desktopCargoPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$predecessor = Get-Content -LiteralPath $predecessorPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$library = Get-Content -LiteralPath $libraryPath -Raw
$acceptor = Get-Content -LiteralPath $acceptorPath -Raw
$cliCargo = Get-Content -LiteralPath $cliCargoPath -Raw
$desktopCargo = Get-Content -LiteralPath $desktopCargoPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$arguments = @(
    $manifest, $predecessor, $completion, $compatibility, $library,
    $acceptor, $cliCargo, $desktopCargo, $rfc
)
Assert-Contract @arguments

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest; $changed.acceptor.maximum_sessions = 9
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.acceptor.maximum_in_flight = 2
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.acceptor.malformed_session_consumes_slot = $false
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.acceptor.imports_history = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.report.identity_visible = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.authority.background_service_added = $true
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changed = Copy-JsonObject $manifest; $changed.verification.m1_readiness = '151/151'
    $mutations += ({ Assert-Contract $changed $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedAcceptor = $acceptor.Replace('connection.alpn() != HISTORY_RECOVERY_TRANSFER_ALPN', 'connection.alpn() != fallback_alpn')
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedAcceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedAcceptor = $acceptor.Replace('#[cfg(test)]', "tokio::spawn(async {});`n#[cfg(test)]")
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $changedAcceptor $cliCargo $desktopCargo $rfc }.GetNewClosure())
    $changedRfc = $rfc -replace 'hard\r?\ncap is eight', 'has no hard cap'
    $mutations += ({ Assert-Contract $manifest $predecessor $completion $compatibility $library $acceptor $cliCargo $desktopCargo $changedRfc }.GetNewClosure())

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

Write-Output 'history_recovery_recipient_acceptor_boundary=verified'
Write-Output 'milestone=M1.2.26'
Write-Output 'maximum_sessions=8'
Write-Output 'maximum_in_flight=1'
Write-Output 'background_service=false'
Write-Output 'active_runtime_registration=false'
Write-Output 'public_default=legacy-only'
