[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_recovery_plan_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-recovery-plan-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0169-m1143-canonical-runtime-ipc-recovery-plan-custody.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.49' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v34-with-json-descriptor-v15' -or
        -not [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical recovery-plan schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_recovery_plan_request_bytes,
        [int]$Schema.limits.maximum_recovery_plan_response_bytes,
        [int]$Schema.limits.maximum_recovery_plan_cancellation_bytes,
        [int]$Schema.limits.minimum_recovery_plan_validity_seconds
    ) -join ','
    if ($limits -cne '2097152,2097152,1024,3600') {
        throw 'canonical recovery-plan limits changed'
    }
    if (@($Schema.domains).Count -ne 78 -or @($Schema.objects).Count -ne 38 -or
        @($Schema.covered_commands).Count -ne 39 -or @($Schema.covered_results).Count -ne 38 -or
        @($Schema.semantic_rules).Count -ne 192 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 0) {
        throw 'canonical recovery-plan inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-recovery-plan-cancellation-signature:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-plan-record-id:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-plan-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-plan-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-recovery-plan-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical recovery-plan domain is missing: $domain"
        }
    }
    foreach ($required in @('approve-history-recovery-plan', 'history-recovery-plan-status',
        'cancel-history-recovery-plan')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical recovery-plan command is missing: $required"
        }
    }
    foreach ($required in @('history-recovery-plan-approved', 'history-recovery-plan-status',
        'history-recovery-plan-cancelled')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical recovery-plan result is missing: $required"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_REQUEST_BYTES: usize = 2 * 1024 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_RESPONSE_BYTES: usize = 2 * 1024 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_CANCELLATION_BYTES: usize = 1024',
        'MIN_CANONICAL_RUNTIME_IPC_RECOVERY_PLAN_VALIDITY_SECONDS: u64 = 60 * 60',
        'ApproveHistoryRecoveryPlan', 'HistoryRecoveryPlanStatus', 'CancelHistoryRecoveryPlan',
        'ForegroundRuntimeOnly', 'runtime_ipc_recovery_plan_record_id_v1',
        'decode_and_verify_runtime_ipc_recovery_plan_request_v1',
        'decode_and_verify_signed_runtime_ipc_recovery_plan_response_v1',
        'verify_runtime_ipc_recovery_plan_request_for_descriptor_v1',
        'verify_runtime_ipc_recovery_plan_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical recovery-plan Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub root_secret:', 'pub device_secret:', 'pub read_capability:',
        'pub write_capability:', 'pub history_plaintext:', 'pub recovery_page_payload:',
        'pub local_path:', 'pub filesystem_path:', 'pub hpke_secret:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical recovery-plan leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or [string]$Vectors.milestone -cne 'M1.1.43' -or
        [string]$Vectors.status -cne 'candidate-not-active' -or
        [bool]$Vectors.changes_active_runtime_ipc -or
        [string]$Vectors.encoding -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical recovery-plan vector header changed'
    }
    foreach ($value in @($Vectors.input.descriptor_id_hex, $Vectors.input.account_id_hex,
        $Vectors.input.recipient_device_id_hex, $Vectors.input.confirmed_sas_hex,
        $Vectors.input.approve_request_id_hex, $Vectors.input.status_request_id_hex,
        $Vectors.input.cancel_request_id_hex, $Vectors.expected.plan.plan_id_hex,
        $Vectors.expected.plan.approved_record_id_hex,
        $Vectors.expected.cancellation.cancelled_record_id_hex,
        $Vectors.expected.approve.request.digest_hex, $Vectors.expected.approve.response.id_hex,
        $Vectors.expected.status.request.digest_hex, $Vectors.expected.status.response.id_hex,
        $Vectors.expected.cancel.request.digest_hex, $Vectors.expected.cancel.response.id_hex)) {
        Assert-LowerHex ([string]$value) 32 'recovery-plan identifier'
    }
    foreach ($value in @($Vectors.expected.cancellation.signature_hex,
        $Vectors.expected.approve.response.signature_hex,
        $Vectors.expected.status.response.signature_hex,
        $Vectors.expected.cancel.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'recovery-plan signature'
    }
    foreach ($name in @('signed_link_cbor_hex')) {
        Assert-LowerHex ([string]$Vectors.input.$name) -Label "recovery-plan $name"
    }
    foreach ($case in @('approve', 'status', 'cancel')) {
        Assert-LowerHex ([string]$Vectors.expected.$case.request.cbor_hex) -Label "$case request"
        Assert-LowerHex ([string]$Vectors.expected.$case.response.signed_cbor_hex) -Label "$case response"
    }
    if ([UInt64]$Vectors.input.valid_for_seconds -ne 86400 -or
        -not [bool]$Vectors.input.allow_ethernet -or -not [bool]$Vectors.input.allow_wifi -or
        [bool]$Vectors.input.allow_mobile -or [bool]$Vectors.input.allow_unknown_network -or
        [bool]$Vectors.input.require_external_power) {
        throw 'canonical recovery-plan fixture policy changed'
    }
    $allIpcHex = @($Vectors.expected.approve.request.cbor_hex,
        $Vectors.expected.approve.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex,
        $Vectors.expected.cancel.request.cbor_hex,
        $Vectors.expected.cancel.response.signed_cbor_hex) -join ''
    if ($allIpcHex.IndexOf([string]$Vectors.input.device_secret_key_hex,
            [StringComparison]::Ordinal) -ge 0) {
        throw 'Device secret leaked into canonical recovery-plan IPC vectors'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath,
    $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical recovery-plan file is missing: $path"
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

foreach ($required in @('runtime-ipc-recovery-execution-policy-v1',
    'runtime-ipc-recovery-plan-command-v1', 'approve-history-recovery-plan',
    'history-recovery-plan-status', 'cancel-history-recovery-plan',
    'runtime-ipc-recovery-plan-cancellation-content-v1',
    'runtime-ipc-recovery-plan-state-v1', 'runtime-ipc-recovery-plan-custody-v1',
    'runtime-ipc-recovery-plan-result-v1',
    'signed-runtime-ipc-recovery-plan-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical recovery-plan CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_recovery_plan_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical recovery-plan module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 34;',
    'kilogram:runtime-ipc-descriptor:v15\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_recovery_plan_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive recovery-plan model leaked into active implementation'
}
$surface = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($surface.Count -ne 1 -or [string]$surface[0].encoding -cne 'postcard' -or
    [string]$surface[0].compatibility -cne 'exact-version' -or
    [int](@($surface[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 34) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v33'
}
$coverageSurface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($coverageSurface.Count -ne 1 -or
    [string]$coverageSurface[0].coverage -cne 'complete' -or
    'sas-confirmed-device-signed-recovery-plan-custody' -cnotin @($coverageSurface[0].covered) -or
    'device-signed-recovery-plan-cancellation-tombstone' -cnotin @($coverageSurface[0].covered) -or
    (@($coverageSurface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical recovery-plan aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-recovery-plan-approve-request-v1',
    'signed-runtime-ipc-recovery-plan-approve-response-v1',
    'runtime-ipc-recovery-plan-cancel-request-v1',
    'signed-runtime-ipc-recovery-plan-cancel-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-RECOVERY-PLAN-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 31851 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical recovery-plan fixtures or mutation capacity changed'
}
foreach ($required in @('node:crypto', 'planSignature', 'cancellationSignature',
    'canonical_runtime_ipc_recovery_plan_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical recovery-plan reference is missing '$required'"
    }
}
foreach ($required in @('full SAS digest', 'content-addressed', 'cancellation tombstone',
    'foreground-runtime-only', 'OS background service', 'Device-link enrollment',
    'Account Root secrets', 'local filesystem paths', 'idempotency key',
    'does not activate canonical CBOR', 'Postcard IPC version 26',
    'executes no network operation')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical recovery-plan RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_recovery_plan_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_blake3=verified' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical recovery-plan independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_recovery_plan_without_migration'
    $shortPlan = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $shortPlan.limits.minimum_recovery_plan_validity_seconds = 3599
    Assert-Rejected { Assert-Schema $shortPlan $rust } 'short_recovery_plan_validity'
    $leakedVector = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $leakedVector.expected.status.response.signed_cbor_hex =
        ([string]$leakedVector.expected.status.response.signed_cbor_hex) +
        ([string]$leakedVector.input.device_secret_key_hex)
    Assert-Rejected { Assert-Vectors $leakedVector } 'device_secret_in_recovery_plan_ipc'
    Write-Output 'canonical_runtime_ipc_recovery_plan_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_recovery_plan_boundary=verified'
Write-Output 'runtime_ipc_coverage=complete'
Write-Output 'approve_history_recovery_plan=canonical-candidate'
Write-Output 'history_recovery_plan_status=canonical-candidate'
Write-Output 'cancel_history_recovery_plan=canonical-candidate'
Write-Output 'sas_confirmation=full-digest'
Write-Output 'cancellation=device-signed-tombstone'
Write-Output 'device_link=bootstrap-helper-not-runtime-ipc'
Write-Output 'private_recovery_capability_in_ipc=false'
Write-Output 'local_path_in_ipc=false'
Write-Output 'os_background_service_enabled=false'
Write-Output 'foreground_runtime_only=true'
Write-Output 'bounded_decoder_mutations=31851'
Write-Output 'active_postcard_v29_changed_by_canonical_candidate=false'
Write-Output 'network_executed=false'
