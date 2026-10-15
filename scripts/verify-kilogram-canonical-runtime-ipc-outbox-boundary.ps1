[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-OUTBOX-VECTORS.json'
$coreVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$queueVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_outbox_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-outbox-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0159-m1133-canonical-runtime-ipc-snapshot-outbox.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.42' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-RUNTIME-IPC.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-runtime-ipc-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC outbox schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_outbox_request_bytes,
        [int]$Schema.limits.maximum_outbox_response_bytes,
        [int]$Schema.limits.maximum_outbox_page_size,
        [int]$Schema.limits.maximum_outbox_snapshot_items,
        [int]$Schema.limits.maximum_outbox_contacts,
        [int]$Schema.limits.maximum_outbox_retry_records
    ) -join ','
    if ($limits -cne '65536,131072,128,4096,256,4096') {
        throw 'canonical runtime IPC outbox limits changed'
    }
    if (@($Schema.domains).Count -ne 62 -or @($Schema.objects).Count -ne 31 -or
        @($Schema.covered_commands).Count -ne 27 -or @($Schema.covered_results).Count -ne 26 -or
        @($Schema.semantic_rules).Count -ne 122 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 3) {
        throw 'canonical runtime IPC outbox inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-outbox-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-outbox-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-outbox-response-id:cbor-v1\0',
        'kilogram:runtime-ipc-outbox-snapshot-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC outbox domain is missing: $domain"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_OUTBOX_REQUEST_BYTES: usize = 64 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_OUTBOX_RESPONSE_BYTES: usize = 128 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_OUTBOX_PAGE_SIZE: u16 = 128',
        'MAX_CANONICAL_RUNTIME_IPC_OUTBOX_SNAPSHOT_ITEMS: u32 = 4_096',
        'kilogram:runtime-ipc-outbox-snapshot-id:cbor-v1\0',
        'decode_and_verify_runtime_ipc_outbox_request_v1',
        'runtime_ipc_outbox_snapshot_id_v1',
        'decode_and_verify_signed_runtime_ipc_outbox_response_v1',
        'verify_runtime_ipc_outbox_response_for_request_v1',
        'sign_runtime_ipc_outbox_response_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC outbox Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    $expectedDependencies = @(
        'CANONICAL-RUNTIME-IPC-VECTORS.json',
        'CANONICAL-RUNTIME-IPC-QUEUE-VECTORS.json')
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne ($expectedDependencies -join ',')) {
        throw 'canonical runtime IPC outbox vector header changed'
    }
    foreach ($entry in @(
        $Vectors.expected.account_id_hex,
        $Vectors.expected.device_id_hex,
        $Vectors.expected.snapshot.object_id_hex,
        $Vectors.expected.request.object_id_hex,
        $Vectors.expected.response.object_id_hex,
        $Vectors.expected.page.snapshot_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'runtime IPC outbox identifier'
    }
    foreach ($entry in @(
        $Vectors.expected.snapshot.cbor_hex,
        $Vectors.expected.request.cbor_hex,
        $Vectors.expected.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$entry) -Label 'runtime IPC outbox object'
    }
    Assert-LowerHex ([string]$Vectors.expected.response.signature_hex) 64 `
        'runtime IPC outbox response signature'
    if ([int]$Vectors.expected.page.item_count -ne 2 -or
        [int]$Vectors.expected.page.next_start_index -ne 2 -or
        [string]$Vectors.expected.page.snapshot_id_hex -cne
            [string]$Vectors.expected.snapshot.object_id_hex -or
        -not [bool]$Vectors.expected.context.snapshot_is_content_derived -or
        -not [bool]$Vectors.expected.context.pagination_is_snapshot_bound -or
        -not [bool]$Vectors.expected.context.response_binds_complete_request_digest -or
        [bool]$Vectors.expected.context.active_runtime_changed) {
        throw 'canonical runtime IPC outbox vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $coreVectorsPath, $queueVectorsPath,
    $cddlPath, $rustPath, $libPath, $activeIpcPath, $compatibilityPath,
    $coveragePath, $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC outbox file is missing: $path"
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

foreach ($required in @('runtime-ipc-outbox-command-v1', 'runtime-ipc-outbox-request-v1',
    'runtime-ipc-outbox-counts-v1', 'runtime-ipc-outbox-item-v1',
    'runtime-ipc-outbox-page-v1', 'runtime-ipc-outbox-result-v1',
    'signed-runtime-ipc-outbox-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC outbox CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_outbox_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC outbox module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_outbox_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical runtime IPC outbox model leaked into active implementation'
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
    'exact-network-ticket-publication-and-device-observation',
    'content-addressed-root-authorized-publication-conflict-ceremony',
    'content-addressed-own-device-announcement-automation',
    'pairwise-own-device-ticket-discovery',
    'roster-wide-own-device-automation')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC outbox aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-outbox-status-page-request-v1',
    'signed-runtime-ipc-outbox-status-page-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-OUTBOX-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical runtime IPC outbox mutation fixtures changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_outbox_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC outbox reference is missing '$required'"
    }
}
foreach ($required in @('content-derived snapshot ID', 'strictly increasing Queue ID',
    'Continuation arithmetic is exact', 'must fail explicitly as a',
    'does not activate canonical CBOR', 'Postcard IPC version 26', 'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC outbox RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --core-vectors $coreVectorsPath --queue-vectors $queueVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_outbox_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC outbox independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_outbox_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_outbox_page_size = 127
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_outbox_page_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.expected.page.next_start_index = 1
    Assert-Rejected { Assert-Vectors $changedVector } 'changed_outbox_continuation'
    Write-Output 'canonical_runtime_ipc_outbox_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_outbox_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'snapshot_content_derived=true'
Write-Output 'pagination_snapshot_bound=true'
Write-Output 'stale_snapshot_rebuild=forbidden'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
