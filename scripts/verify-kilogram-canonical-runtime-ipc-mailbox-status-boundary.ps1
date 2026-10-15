[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-MAILBOX-STATUS-VECTORS.json'
$provisioningVectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-PROVISIONING-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_mailbox_status_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-mailbox-status-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0161-m1135-canonical-runtime-ipc-mailbox-status.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.41' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC mailbox status schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_mailbox_status_request_bytes,
        [int]$Schema.limits.maximum_mailbox_status_response_bytes,
        [int]$Schema.limits.maximum_mailbox_status_heads,
        [int]$Schema.limits.maximum_mailbox_status_page_size
    ) -join ','
    if ($limits -cne '65536,655360,1024,16') {
        throw 'canonical runtime IPC mailbox status limits changed'
    }
    if (@($Schema.domains).Count -ne 57 -or @($Schema.objects).Count -ne 29 -or
        @($Schema.covered_commands).Count -ne 25 -or @($Schema.covered_results).Count -ne 24 -or
        @($Schema.semantic_rules).Count -ne 110 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 4) {
        throw 'canonical runtime IPC mailbox status inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-mailbox-status-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-mailbox-status-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-mailbox-status-response-id:cbor-v1\0',
        'kilogram:runtime-ipc-mailbox-status-snapshot-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC mailbox status domain is missing: $domain"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_REQUEST_BYTES: usize = 64 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_RESPONSE_BYTES: usize = 640 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_HEADS: u32 = 1_024',
        'MAX_CANONICAL_RUNTIME_IPC_MAILBOX_STATUS_PAGE_SIZE: u16 = 16',
        'decode_and_verify_runtime_ipc_mailbox_status_request_v1',
        'decode_and_verify_signed_runtime_ipc_mailbox_status_response_v1',
        'verify_runtime_ipc_mailbox_status_response_for_request_v1',
        'runtime_ipc_mailbox_status_snapshot_id_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC mailbox status Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('CanonicalSignedLocalMailboxBindingV1', 'read_secret:', 'write_secret:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical runtime IPC mailbox status leaks local secret material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne 'CANONICAL-MAILBOX-PROVISIONING-VECTORS.json') {
        throw 'canonical runtime IPC mailbox status vector header changed'
    }
    foreach ($entry in @($Vectors.expected.account_id_hex, $Vectors.expected.device_id_hex,
        $Vectors.expected.descriptor.object_id_hex, $Vectors.expected.request.object_id_hex,
        $Vectors.expected.response.object_id_hex, $Vectors.expected.snapshot_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'runtime IPC mailbox status identifier'
    }
    foreach ($entry in @($Vectors.expected.descriptor.signed_cbor_hex,
        $Vectors.expected.request.cbor_hex, $Vectors.expected.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$entry) -Label 'runtime IPC mailbox status object'
    }
    Assert-LowerHex ([string]$Vectors.expected.response.signature_hex) 64 `
        'runtime IPC mailbox status response signature'
    if ([int]$Vectors.expected.context.head_count -ne 1 -or
        -not [bool]$Vectors.expected.context.signed_updates_and_acknowledgements_only -or
        [bool]$Vectors.expected.context.read_write_secrets_in_payload -or
        -not [bool]$Vectors.expected.context.response_binds_complete_request_digest -or
        [bool]$Vectors.expected.context.active_runtime_changed) {
        throw 'canonical runtime IPC mailbox status vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $provisioningVectorsPath, $cddlPath,
    $rustPath, $libPath, $activeIpcPath, $compatibilityPath, $coveragePath,
    $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC mailbox status file is missing: $path"
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

foreach ($required in @('runtime-ipc-mailbox-status-command-v1',
    'runtime-ipc-mailbox-status-request-v1', 'runtime-ipc-mailbox-capability-head-v1',
    'runtime-ipc-mailbox-status-counts-v1', 'runtime-ipc-mailbox-status-page-v1',
    'signed-runtime-ipc-mailbox-status-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC mailbox status CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_mailbox_status_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC mailbox status module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_mailbox_status_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical mailbox status model leaked into active implementation'
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
    'pairwise-own-device-ticket-discovery')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC mailbox status aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-mailbox-capability-status-page-request-v1',
    'signed-runtime-ipc-mailbox-capability-status-page-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-MAILBOX-STATUS-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 27711 -or
    [int]$mutations.maximum_mutations -ne 30000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 30000) {
    throw 'canonical runtime IPC mailbox status mutation fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_mailbox_status_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC mailbox status reference is missing '$required'"
    }
}
foreach ($required in @('exact signed capability update', 'read secret', 'write secret',
    'immutable snapshot', 'does not activate canonical CBOR', 'Postcard IPC version 26',
    'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC mailbox status RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --provisioning-vectors $provisioningVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_mailbox_status_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC mailbox status independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_mailbox_status_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_mailbox_status_page_size = 17
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_mailbox_status_page_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.expected.context.read_write_secrets_in_payload = $true
    Assert-Rejected { Assert-Vectors $changedVector } 'mailbox_status_secret_leak'
    Write-Output 'canonical_runtime_ipc_mailbox_status_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_mailbox_status_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'status_pagination=snapshot-bound'
Write-Output 'local_binding_secrets_in_payload=false'
Write-Output 'bounded_decoder_mutations=27711'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
