[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-DIRECTORY-VECTORS.json'
$coreVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$recoveryVectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json'
$accountVectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-VECTORS.json'
$authorityVectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_directory_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-directory-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0157-m1131-canonical-runtime-ipc-device-directory-apply.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.32' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-RUNTIME-IPC.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-runtime-ipc-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC directory schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_request_bytes,
        [int]$Schema.limits.maximum_directory_response_bytes,
        [int]$Schema.limits.maximum_account_devices,
        [int]$Schema.limits.maximum_path_bytes
    ) -join ','
    if ($limits -cne '65536,131072,32,4096') {
        throw 'canonical runtime IPC directory limits changed'
    }
    if (@($Schema.domains).Count -ne 19 -or @($Schema.objects).Count -ne 11 -or
        @($Schema.covered_commands).Count -ne 9 -or @($Schema.covered_results).Count -ne 10 -or
        @($Schema.semantic_rules).Count -ne 33 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 7) {
        throw 'canonical runtime IPC directory inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-directory-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-directory-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-directory-response-id:cbor-v1\0',
        'kilogram:runtime-ipc-device-list-digest:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC directory domain is missing: $domain"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_RESPONSE_BYTES: usize = 128 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_PATH_BYTES: usize = 4 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_DIRECTORY_DEVICES: usize = 32',
        'kilogram:runtime-ipc-directory-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-device-list-digest:cbor-v1\0',
        'decode_and_verify_runtime_ipc_directory_request_v1',
        'verify_runtime_ipc_directory_request_for_descriptor_v1',
        'verify_runtime_ipc_device_directory_transition_v1',
        'decode_and_verify_signed_runtime_ipc_directory_response_v1',
        'verify_runtime_ipc_directory_response_for_request_v1',
        'sign_runtime_ipc_directory_response_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC directory Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    $expectedDependencies = @(
        'CANONICAL-RUNTIME-IPC-VECTORS.json',
        'CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json',
        'CANONICAL-ACCOUNT-VECTORS.json',
        'CANONICAL-AUTHORITY-VECTORS.json')
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne ($expectedDependencies -join ',')) {
        throw 'canonical runtime IPC directory vector header changed'
    }
    foreach ($entry in @(
        $Vectors.expected.account_id_hex,
        $Vectors.expected.local_device_id_hex,
        $Vectors.expected.removed_device_id_hex,
        $Vectors.expected.current_device_list.digest_hex,
        $Vectors.expected.candidate_device_list.digest_hex,
        $Vectors.expected.request.object_id_hex,
        $Vectors.expected.response.object_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'runtime IPC directory identifier'
    }
    foreach ($entry in @(
        $Vectors.expected.current_device_list.signed_cbor_hex,
        $Vectors.expected.candidate_device_list.signed_cbor_hex,
        $Vectors.expected.request.cbor_hex,
        $Vectors.expected.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$entry) -Label 'runtime IPC directory object'
    }
    Assert-LowerHex ([string]$Vectors.expected.response.signature_hex) 64 `
        'runtime IPC directory response signature'
    if ([int]$Vectors.expected.previous_authority_revision -ne 10 -or
        [int]$Vectors.expected.authority_revision -ne 11 -or
        [int]$Vectors.expected.active_device_count -ne 1 -or
        [string]$Vectors.expected.context.command_result_pair -cne
            'apply-own-device-directory/own-device-directory-applied' -or
        -not [bool]$Vectors.expected.context.candidate_embedded_in_request -or
        -not [bool]$Vectors.expected.context.exact_current_anchor_required -or
        -not [bool]$Vectors.expected.context.only_existing_root_revoked_devices_may_disappear -or
        -not [bool]$Vectors.expected.context.running_device_must_remain_byte_identical -or
        [bool]$Vectors.expected.context.active_runtime_changed) {
        throw 'canonical runtime IPC directory vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $coreVectorsPath, $recoveryVectorsPath,
    $accountVectorsPath, $authorityVectorsPath, $cddlPath, $rustPath, $libPath,
    $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC directory file is missing: $path"
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

foreach ($required in @('runtime-ipc-directory-command-v1', 'runtime-ipc-directory-request-v1',
    'runtime-ipc-device-directory-applied-v1', 'runtime-ipc-directory-result-v1',
    'signed-runtime-ipc-directory-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC directory CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_directory_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC directory module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_directory_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical runtime IPC directory model leaked into active implementation'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
    [string]$source[0].compatibility -cne 'exact-version' -or
    [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 26) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v26'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
$expectedCovered = @('device-signed-loopback-descriptor',
    'short-lived-bearer-authenticated-core-request', 'request-correlation-and-exact-digest-binding',
    'device-signed-core-response', 'ping-pong', 'wait-for-change', 'shutdown', 'bounded-error',
    'bounded-paginated-conversation-list-read', 'snapshot-bound-history-page-read',
    'device-signed-exact-read-response', 'exact-peer-contact-enrollment',
    'read-only-own-device-directory-status', 'root-revoked-own-device-directory-transition',
    'idempotent-durable-message-queue-insertion')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC directory aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-device-directory-apply-request-v1',
    'signed-runtime-ipc-device-directory-apply-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-DIRECTORY-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical runtime IPC directory mutation fixtures changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_directory_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC directory reference is missing '$required'"
    }
}
foreach ($required in @('time-of-check/time-of-use authority substitution',
    'exact current-state anchor', 'Removal-only live transition',
    'does not activate canonical CBOR', 'Postcard IPC version 26',
    'preexisting ciphertext is immutable', 'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC directory RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --core-vectors $coreVectorsPath --recovery-vectors $recoveryVectorsPath `
    --account-vectors $accountVectorsPath --authority-vectors $authorityVectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_directory_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC directory independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_directory_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_account_devices = 33
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_directory_device_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 40 | ConvertFrom-Json
    $changedVector.expected.previous_authority_revision = 9
    Assert-Rejected { Assert-Vectors $changedVector } 'changed_current_authority_anchor'
    Write-Output 'canonical_runtime_ipc_directory_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_directory_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'root_revoked_device_removal=verified'
Write-Output 'exact_current_authority_anchor=verified'
Write-Output 'authority_transition=verified'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
