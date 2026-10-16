[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-RECOVERY-EXECUTION-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_recovery_execution_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-recovery-execution-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0170-m1144-canonical-runtime-ipc-recovery-execution.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-LowerHex {
    param([string]$Value, [int]$Bytes = -1, [string]$Label)
    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -cnotmatch '^[0-9a-f]+$' -or
        $Value.Length % 2 -ne 0 -or ($Bytes -ge 0 -and $Value.Length -ne $Bytes * 2)) {
        throw "$Label is not canonical lowercase hexadecimal"
    }
}

function Assert-Schema {
    param([object]$Schema, [string]$Rust)
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.44' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical recovery-execution schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_recovery_execution_request_bytes,
        [int]$Schema.limits.maximum_recovery_execution_response_bytes,
        [int]$Schema.limits.maximum_recovery_scheduler_state_bytes,
        [int]$Schema.limits.minimum_recovery_retry_base_seconds,
        [int]$Schema.limits.maximum_recovery_retry_base_seconds,
        [int]$Schema.limits.maximum_recovery_retry_seconds,
        [int]$Schema.limits.minimum_recovery_attempt_lease_seconds,
        [int]$Schema.limits.maximum_recovery_attempt_lease_seconds
    ) -join ','
    if ($limits -cne '65536,2097152,524288,1,300,3600,30,7200') {
        throw 'canonical recovery-execution limits changed'
    }
    if (@($Schema.domains).Count -ne 73 -or @($Schema.objects).Count -ne 36 -or
        @($Schema.covered_commands).Count -ne 32 -or @($Schema.covered_results).Count -ne 31 -or
        @($Schema.semantic_rules).Count -ne 154 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 2) {
        throw 'canonical recovery-execution inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-recovery-scheduler-signature:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-scheduler-state-id:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-scheduler-jitter:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-execution-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-execution-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-execution-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical recovery-execution domain is missing: $domain"
        }
    }
    foreach ($required in @('start-history-recovery-execution',
        'history-recovery-execution-status')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical recovery-execution command is missing: $required"
        }
    }
    foreach ($required in @('history-recovery-execution-started',
        'history-recovery-execution-status')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical recovery-execution result is missing: $required"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_REQUEST_BYTES: usize = 64 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_RECOVERY_EXECUTION_RESPONSE_BYTES: usize = 2 * 1024 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_RECOVERY_SCHEDULER_STATE_BYTES: usize = 512 * 1024',
        'StartHistoryRecoveryExecution', 'HistoryRecoveryExecutionStatus',
        'AttemptStarted', 'AttemptFailed', 'AttemptProgressed', 'Completed', 'Cancelled',
        'derive_runtime_ipc_recovery_readiness_v1',
        'verify_runtime_ipc_recovery_scheduler_successor_v1',
        'decode_and_verify_runtime_ipc_recovery_execution_request_v1',
        'decode_and_verify_signed_runtime_ipc_recovery_execution_response_v1',
        'verify_runtime_ipc_recovery_execution_request_for_descriptor_v1',
        'verify_runtime_ipc_recovery_execution_response_for_request_v1',
        'scheduler_successor_rejects_overflow_zero_failure_and_nondeterministic_delay')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical recovery-execution Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub root_secret:', 'pub device_secret:', 'pub read_capability:',
        'pub write_capability:', 'pub history_plaintext:', 'pub recovery_page_payload:',
        'pub local_path:', 'pub filesystem_path:', 'pub hpke_secret:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical recovery-execution leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or [string]$Vectors.milestone -cne 'M1.1.44' -or
        [string]$Vectors.status -cne 'candidate-not-active' -or
        [bool]$Vectors.changes_active_runtime_ipc -or
        [string]$Vectors.encoding -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical recovery-execution vector header changed'
    }
    foreach ($value in @($Vectors.input.descriptor_id_hex, $Vectors.input.account_id_hex,
        $Vectors.input.recipient_device_id_hex, $Vectors.input.plan_id_hex,
        $Vectors.input.plan_record_id_hex, $Vectors.input.conversation_id_hex,
        $Vectors.input.source_device_id_hex, $Vectors.input.confirmed_sas_hex,
        $Vectors.input.start_request_id_hex, $Vectors.input.status_request_id_hex,
        $Vectors.expected.initialized_scheduler.state_id_hex,
        $Vectors.expected.started_scheduler.state_id_hex,
        $Vectors.expected.failed_scheduler.state.state_id_hex,
        $Vectors.expected.start.request.digest_hex, $Vectors.expected.start.response.id_hex,
        $Vectors.expected.status.request.digest_hex, $Vectors.expected.status.response.id_hex)) {
        Assert-LowerHex ([string]$value) 32 'recovery-execution identifier'
    }
    foreach ($value in @($Vectors.expected.initial_checkpoint.signature_hex,
        $Vectors.expected.initialized_scheduler.signature_hex,
        $Vectors.expected.started_scheduler.signature_hex,
        $Vectors.expected.failed_scheduler.state.signature_hex,
        $Vectors.expected.start.response.signature_hex,
        $Vectors.expected.status.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'recovery-execution signature'
    }
    foreach ($value in @($Vectors.input.descriptor_cbor_hex, $Vectors.input.signed_plan_cbor_hex,
        $Vectors.input.plan_state_cbor_hex, $Vectors.expected.initial_checkpoint.signed_cbor_hex,
        $Vectors.expected.initialized_scheduler.signed_cbor_hex,
        $Vectors.expected.started_scheduler.signed_cbor_hex,
        $Vectors.expected.failed_scheduler.state.signed_cbor_hex,
        $Vectors.expected.start.request.cbor_hex,
        $Vectors.expected.start.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'recovery-execution canonical bytes'
    }
    if ([int]$Vectors.input.retry_base_seconds -ne 5 -or
        [int]$Vectors.input.retry_max_seconds -ne 300 -or
        [int]$Vectors.input.attempt_lease_seconds -ne 120 -or
        [int]$Vectors.expected.failed_scheduler.scheduled_delay_seconds -ne 3 -or
        [int]$Vectors.expected.failed_scheduler.next_attempt_at_unix_seconds -ne 1800000069) {
        throw 'canonical recovery-execution retry fixture changed'
    }
    $allIpcHex = @($Vectors.expected.start.request.cbor_hex,
        $Vectors.expected.start.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex) -join ''
    if ($allIpcHex.IndexOf([string]$Vectors.input.device_secret_key_hex,
            [StringComparison]::Ordinal) -ge 0) {
        throw 'Device secret leaked into canonical recovery-execution IPC vectors'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath,
    $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical recovery-execution file is missing: $path"
    }
}

$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$activeIpc = Get-Content -LiteralPath $activeIpcPath -Raw
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$reference = Get-Content -LiteralPath $referencePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('runtime-ipc-recovery-execution-command-v1',
    'start-history-recovery-execution', 'history-recovery-execution-status',
    'runtime-ipc-recovery-scheduler-state-content-v1',
    'signed-runtime-ipc-recovery-scheduler-state-v1',
    'runtime-ipc-recovery-readiness-v1',
    'runtime-ipc-recovery-execution-snapshot-v1',
    'runtime-ipc-recovery-execution-result-v1',
    'signed-runtime-ipc-recovery-execution-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical recovery-execution CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_recovery_execution_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical recovery-execution module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_recovery_execution_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive recovery-execution model leaked into active implementation'
}
$surface = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($surface.Count -ne 1 -or [string]$surface[0].encoding -cne 'postcard' -or
    [string]$surface[0].compatibility -cne 'exact-version' -or
    [int](@($surface[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 26) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v26'
}
$coverageSurface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ([string]$coverage.milestone -cne 'M1.1.44' -or $coverageSurface.Count -ne 1 -or
    [string]$coverageSurface[0].coverage -cne 'partial' -or
    'recipient-device-signed-recovery-scheduler-chain' -cnotin @($coverageSurface[0].covered) -or
    'policy-derived-foreground-recovery-readiness' -cnotin @($coverageSurface[0].covered) -or
    (@($coverageSurface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical recovery-execution aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-recovery-execution-start-request-v1',
    'signed-runtime-ipc-recovery-execution-start-response-v1',
    'runtime-ipc-recovery-execution-status-request-v1',
    'signed-runtime-ipc-recovery-execution-status-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-RECOVERY-EXECUTION-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 30843 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical recovery-execution fixtures or mutation capacity changed'
}
foreach ($required in @('node:crypto', 'schedulerSignature', 'backoffDelay',
    'canonical_runtime_ipc_recovery_execution_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical recovery-execution reference is missing '$required'"
    }
}
foreach ($required in @('approved plan', 'recipient-Device', 'append-only',
    'deterministic equal-jitter', 'attempt lease', 'canonical recovery checkpoint',
    'foreground runtime', 'OS background service', 'Account Root secrets',
    'local filesystem paths', 'does not activate canonical CBOR', 'Postcard IPC version 26')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical recovery-execution RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_recovery_execution_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_blake3=verified' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical recovery-execution independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_recovery_execution_without_migration'
    $unboundedRetry = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $unboundedRetry.limits.maximum_recovery_retry_seconds = 3601
    Assert-Rejected { Assert-Schema $unboundedRetry $rust } 'unbounded_recovery_retry'
    $leakedVector = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $leakedVector.expected.status.response.signed_cbor_hex =
        ([string]$leakedVector.expected.status.response.signed_cbor_hex) +
        ([string]$leakedVector.input.device_secret_key_hex)
    Assert-Rejected { Assert-Vectors $leakedVector } 'device_secret_in_recovery_execution_ipc'
    Write-Output 'canonical_runtime_ipc_recovery_execution_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_recovery_execution_boundary=verified'
Write-Output 'recovery_scheduler_chain=canonical-candidate'
Write-Output 'recovery_execution_start_status=canonical-candidate'
Write-Output 'active_runtime_ipc_unchanged=postcard-v26'
