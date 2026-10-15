[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-READ-VECTORS.json'
$coreVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_read_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-read-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0155-m1129-canonical-runtime-ipc-read-model.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.39' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-RUNTIME-IPC.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-runtime-ipc-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC read schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_read_response_bytes,
        [int]$Schema.limits.maximum_conversation_page_size,
        [int]$Schema.limits.maximum_endpoint_candidates_per_conversation,
        [int]$Schema.limits.maximum_history_page_size,
        [int]$Schema.limits.maximum_history_page_body_bytes,
        [int]$Schema.limits.maximum_preview_bytes,
        [int]$Schema.limits.maximum_text_bytes,
        [int]$Schema.limits.maximum_path_bytes
    ) -join ','
    if ($limits -cne '262144,64,4,100,196608,96,4096,4096') {
        throw 'canonical runtime IPC read limits changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-read-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-read-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-read-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC read domain is missing: $domain"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_READ_RESPONSE_BYTES: usize = 256 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_CONVERSATION_PAGE_SIZE: u16 = 64',
        'MAX_CANONICAL_RUNTIME_IPC_ENDPOINT_CANDIDATES: usize = 4',
        'MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_SIZE: u16 = 100',
        'MAX_CANONICAL_RUNTIME_IPC_HISTORY_PAGE_BODY_BYTES: usize = 192 * 1024',
        'kilogram:runtime-ipc-read-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-read-response-signature:cbor-v1\0',
        'decode_and_verify_runtime_ipc_read_request_v1',
        'verify_runtime_ipc_read_request_for_descriptor_v1',
        'decode_and_verify_signed_runtime_ipc_read_response_v1',
        'verify_runtime_ipc_read_response_for_request_v1',
        'sign_runtime_ipc_read_response_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC read Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne 'CANONICAL-RUNTIME-IPC-VECTORS.json') {
        throw 'canonical runtime IPC read vector header changed'
    }
    foreach ($entry in @(
        $Vectors.expected.account_id_hex,
        $Vectors.expected.device_id_hex,
        $Vectors.expected.descriptor_id_hex,
        $Vectors.expected.conversation_id_hex,
        $Vectors.expected.conversation_list.request.object_id_hex,
        $Vectors.expected.conversation_list.response.object_id_hex,
        $Vectors.expected.history_page.request.object_id_hex,
        $Vectors.expected.history_page.response.object_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'runtime IPC read identifier'
    }
    foreach ($entry in @(
        $Vectors.expected.conversation_list.request.cbor_hex,
        $Vectors.expected.conversation_list.response.signed_cbor_hex,
        $Vectors.expected.history_page.request.cbor_hex,
        $Vectors.expected.history_page.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$entry) -Label 'runtime IPC read object'
    }
    foreach ($entry in @(
        $Vectors.expected.conversation_list.response.signature_hex,
        $Vectors.expected.history_page.response.signature_hex)) {
        Assert-LowerHex ([string]$entry) 64 'runtime IPC read response signature'
    }
    if ([int]$Vectors.input.conversation_page_limit -ne 2 -or
        [int]$Vectors.input.history_page_limit -ne 2 -or
        [int]$Vectors.input.total_messages -ne 3 -or
        [int]$Vectors.input.history_next_before_index -ne 1 -or
        @($Vectors.input.endpoint_candidates).Count -ne 2 -or
        @($Vectors.input.history_messages).Count -ne 2 -or
        [string]$Vectors.expected.context.list_command_result_pair -cne
            'conversation-list-page/conversation-list-page' -or
        [string]$Vectors.expected.context.history_command_result_pair -cne
            'history-page/history-page' -or
        -not [bool]$Vectors.expected.context.response_binds_complete_request_digest -or
        -not [bool]$Vectors.expected.context.history_snapshot_cursor_bound -or
        [bool]$Vectors.expected.context.active_runtime_changed) {
        throw 'canonical runtime IPC read vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $coreVectorsPath, $cddlPath, $rustPath,
    $libPath, $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC read file is missing: $path"
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

foreach ($required in @('runtime-ipc-read-request-v1', 'runtime-ipc-conversation-summary-v1',
    'runtime-ipc-history-page-v1', 'signed-runtime-ipc-read-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC read CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_read_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC read module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_read_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical runtime IPC read model leaked into active implementation'
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
    'content-addressed-root-authorized-publication-conflict-ceremony')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC read aggregate coverage boundary changed'
}
$expectedMutationIds = @(
    'runtime-ipc-conversation-list-page-request-v1',
    'signed-runtime-ipc-conversation-list-page-response-v1',
    'runtime-ipc-history-page-request-v1',
    'signed-runtime-ipc-history-page-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-READ-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical runtime IPC read mutation fixtures changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_read_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC read reference is missing '$required'"
    }
}
foreach ($required in @('paginated conversation', 'snapshot', 'partial',
    'does not activate canonical CBOR', 'Postcard IPC version 26')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC read RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --core-vectors $coreVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_read_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC read independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_read_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_history_page_size = 101
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_history_page_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.input.history_next_before_index = 2
    Assert-Rejected { Assert-Vectors $changedVector } 'changed_history_cursor_arithmetic'
    Write-Output 'canonical_runtime_ipc_read_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_read_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'conversation_list_page=verified'
Write-Output 'history_snapshot_page=verified'
Write-Output 'device_signed_exact_read_response=verified'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
