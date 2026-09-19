[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0097-replication-ledger-crash-recovery.md'

foreach ($path in @($runtimePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "replication-ledger recovery source is missing: $path"
    }
}

$runtime = Get-Content -LiteralPath $runtimePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

foreach ($required in @(
    'async fn inspect_runtime_mailbox_replication_ledger(',
    'Err(error) if redb_repair_required(&error)',
    'let state_lock = acquire_runtime_state_lock(state_directory)',
    'let vault_guard = VaultDualWriteGuard::prepare(state_directory)?;',
    'let repair_result = runtime_mailbox_replication_ledger(state_directory).map(drop);',
    'combine_operation_and_mirror(repair_result, mirror_result)',
    'runtime_mailbox_replication_ledger_repair_status=repaired-after-interrupted-runtime',
    'inspect().context("inspect repaired mailbox replication ledger read-only")',
    'interrupted_replication_ledger_is_repaired_before_read_only_inspection',
    'REPLICATION_LEDGER_UNCLEAN_EXIT_DIRECTORY_ENV',
    'std::process::exit(73)'
)) {
    if ($runtime.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime replication-ledger recovery boundary is missing '$required'"
    }
}

if ([regex]::Matches($runtime, [regex]::Escape('inspect_runtime_mailbox_replication_ledger(')).Count -ne 3) {
    throw 'replication-ledger recovery helper must have one definition and two bounded call sites (runtime plus regression)'
}

foreach ($forbidden in @(
    'remove_file(runtime_mailbox_replication_ledger',
    'remove_dir_all(runtime_mailbox_replication_ledger',
    'DatabaseError::Corrupted =>',
    'DatabaseError::Io =>'
)) {
    if ($runtime.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "runtime replication-ledger recovery contains forbidden broad/destructive fallback '$forbidden'"
    }
}

foreach ($required in @(
    'typed `redb::DatabaseError::RepairAborted`',
    'prepare `VaultDualWriteGuard`',
    'repeat the original read-only inspection',
    'does not delete, recreate, truncate',
    'actual Redb recovery marker',
    'distinct `M0.9.74` milestone'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "RFC-0097 is missing '$required'"
    }
}

Write-Output 'runtime_mailbox_replication_recovery=verified'
Write-Output 'repair_trigger=typed-redb-repair-aborted-only'
Write-Output 'state_lock=required'
Write-Output 'vault_dual_write=required'
Write-Output 'post_repair_read_only_inspection=required'
Write-Output 'destructive_recreation=false'
Write-Output 'crash_regression=real-child-process-exit'
Write-Output 'new_executable=false'
