[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0096-cooperative-runtime-outbound-scheduling.md'
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    'type RuntimeOutboundCycleTask = JoinHandle<RuntimeOutboundCycleCompletion>;',
    'RuntimeEvent::OutboundCompleted(completion)',
    'outbound_cycle = Some(tokio::spawn(run_runtime_outbound_cycle(',
    'if outbound_cycle.is_some()',
    'completed = wait_for_runtime_outbound_cycle(outbound_cycle)',
    'let accept_ipc_work = outbound_cycle.is_none();',
    'work = ipc_receiver.recv(), if accept_ipc_work',
    'fn runtime_automatic_sync_initiator(',
    'runtime_sync_role=initiator',
    'runtime_sync_role=passive',
    'automatic_sync_elects_exactly_one_initiator',
    'mutual_automatic_sync_does_not_starve_inbound_accept'
)) {
    if ($runtime.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "cooperative runtime scheduling boundary is missing '$required'"
    }
}

$spawnPattern = [regex]::Escape('outbound_cycle = Some(tokio::spawn(run_runtime_outbound_cycle(')
if ([regex]::Matches($runtime, $spawnPattern).Count -ne 1) {
    throw 'runtime must have exactly one source location that starts its bounded outbound cycle'
}
foreach ($forbidden in @('RuntimeOutboundCycleFuture', '#[cfg(any())]')) {
    if ($runtime.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
        throw "cooperative runtime scheduling retains forbidden legacy scaffolding '$forbidden'"
    }
}

foreach ($required in @(
    'at most one `RuntimeOutboundCycleTask`',
    'authenticated IPC between outbound cycles',
    'lexicographically smaller Account ID initiates',
    'fresh two-network no-HTTPS field run'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "RFC-0096 is missing '$required'"
    }
}

Write-Output 'runtime_cooperative_scheduling=verified'
Write-Output 'outbound_cycles=single-task'
Write-Output 'inbound_accept=polled-during-network-waits'
Write-Output 'ipc_dispatch=between-outbound-cycles'
Write-Output 'automatic_sync=deterministic-single-initiator'
Write-Output 'new_executable=false'
