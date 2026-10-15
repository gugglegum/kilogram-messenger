[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$authorityVectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0154-m1128-canonical-runtime-ipc-core.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.38' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-RUNTIME-IPC.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-runtime-ipc-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_descriptor_bytes,
        [int]$Schema.limits.maximum_request_bytes,
        [int]$Schema.limits.maximum_response_bytes,
        [int]$Schema.limits.maximum_error_bytes,
        [int]$Schema.limits.maximum_descriptor_validity_seconds,
        [int]$Schema.limits.maximum_request_validity_seconds,
        [int]$Schema.limits.maximum_wait_milliseconds,
        [int]$Schema.limits.clock_skew_seconds
    ) -join ','
    if ($limits -cne '2048,65536,65536,4096,86400,30,120000,30') {
        throw 'canonical runtime IPC limits changed'
    }
    if (@($Schema.domains).Count -ne 42 -or @($Schema.objects).Count -ne 23 -or
        @($Schema.covered_commands).Count -ne 19 -or @($Schema.covered_results).Count -ne 18 -or
        @($Schema.semantic_rules).Count -ne 77 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 5) {
        throw 'canonical runtime IPC inventory changed'
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_DESCRIPTOR_BYTES: usize = 2 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_REQUEST_VALIDITY_SECONDS: u64 = 30',
        'kilogram:runtime-ipc-descriptor-signature:cbor-v1\0',
        'kilogram:runtime-ipc-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-response-signature:cbor-v1\0',
        'decode_and_verify_signed_runtime_ipc_descriptor_v1',
        'verify_runtime_ipc_core_request_for_descriptor_v1',
        'decode_and_verify_signed_runtime_ipc_core_response_v1',
        'verify_runtime_ipc_core_response_for_request_v1',
        'sign_runtime_ipc_core_response_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne 'CANONICAL-AUTHORITY-VECTORS.json') {
        throw 'canonical runtime IPC vector header changed'
    }
    foreach ($entry in @($Vectors.expected.account_id_hex, $Vectors.expected.device_id_hex,
        $Vectors.expected.descriptor.object_id_hex, $Vectors.expected.request.object_id_hex,
        $Vectors.expected.response.object_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'runtime IPC identifier'
    }
    Assert-LowerHex ([string]$Vectors.expected.descriptor.signature_hex) 64 'descriptor signature'
    Assert-LowerHex ([string]$Vectors.expected.response.signature_hex) 64 'response signature'
    Assert-LowerHex ([string]$Vectors.expected.descriptor.signed_cbor_hex) -Label 'descriptor'
    Assert-LowerHex ([string]$Vectors.expected.request.cbor_hex) -Label 'request'
    Assert-LowerHex ([string]$Vectors.expected.response.signed_cbor_hex) -Label 'response'
    if ([int]$Vectors.input.port -ne 43125 -or
        [UInt64]$Vectors.input.descriptor_expires_at_unix_seconds -
            [UInt64]$Vectors.input.descriptor_issued_at_unix_seconds -ne 3600 -or
        [UInt64]$Vectors.input.request_expires_at_unix_seconds -
            [UInt64]$Vectors.input.request_issued_at_unix_seconds -ne 30 -or
        [int]$Vectors.input.timeout_milliseconds -ne 30000 -or
        -not [bool]$Vectors.expected.context.descriptor_matches_request -or
        -not [bool]$Vectors.expected.context.request_digest_matches_response -or
        -not [bool]$Vectors.expected.context.responder_matches_descriptor -or
        [string]$Vectors.expected.context.command_result_pair -cne 'wait-for-change/change-state') {
        throw 'canonical runtime IPC vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $authorityVectorsPath, $cddlPath, $rustPath,
    $libPath, $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC file is missing: $path"
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

foreach ($required in @('signed-runtime-ipc-descriptor-v1', 'runtime-ipc-core-request-v1',
    'signed-runtime-ipc-core-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical runtime IPC leaked into active implementation'
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
    'idempotent-durable-message-queue-insertion', 'snapshot-bound-paginated-outbox-status',
    'signed-volunteer-provider-offer-import', 'capability-free-volunteer-provider-selection',
    'snapshot-bound-paginated-mailbox-capability-status',
    'exact-provider-and-predecessor-bound-mailbox-capability-mutation',
    'exact-current-state-publication-channel-rotation',
    'exact-network-ticket-publication-and-device-observation')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC aggregate coverage boundary changed'
}
if ([int]$mutations.expected_mutations -ne 24579 -or
    [int]$mutations.maximum_mutations -ne 30000) {
    throw 'canonical runtime IPC decoder mutation bound changed'
}
$expectedMutationIds = @('signed-runtime-ipc-descriptor-v1', 'runtime-ipc-core-request-v1',
    'signed-runtime-ipc-core-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical runtime IPC mutation fixtures changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC reference is missing '$required'"
    }
}
foreach ($required in @('stale-descriptor port-reuse race', 'partial',
    'does not activate canonical CBOR', 'Postcard IPC version 26')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --authority-vectors $authorityVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_request_validity_seconds = 31
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_request_validity'
    $changedVector = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedVector.input.port = 0
    Assert-Rejected { Assert-Vectors $changedVector } 'non_loopback_descriptor_port'
    Write-Output 'canonical_runtime_ipc_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'signed_loopback_descriptor=verified'
Write-Output 'bearer_authenticated_request=verified'
Write-Output 'device_signed_exact_response=verified'
Write-Output 'bounded_decoder_mutations=24579'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
