[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json'
$coreVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_contact_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-contact-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0156-m1130-canonical-runtime-ipc-contact-directory-status.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.35' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-RUNTIME-IPC.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-runtime-ipc-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC contact schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_request_bytes,
        [int]$Schema.limits.maximum_contact_response_bytes,
        [int]$Schema.limits.maximum_endpoint_candidates_per_conversation,
        [int]$Schema.limits.maximum_account_devices,
        [int]$Schema.limits.maximum_text_bytes,
        [int]$Schema.limits.maximum_path_bytes
    ) -join ','
    if ($limits -cne '65536,131072,4,32,4096,4096') {
        throw 'canonical runtime IPC contact limits changed'
    }
    if (@($Schema.domains).Count -ne 30 -or @($Schema.objects).Count -ne 17 -or
        @($Schema.covered_commands).Count -ne 13 -or @($Schema.covered_results).Count -ne 14 -or
        @($Schema.semantic_rules).Count -ne 52 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 6) {
        throw 'canonical runtime IPC contact inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-contact-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-contact-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-contact-response-id:cbor-v1\0',
        'kilogram:runtime-ipc-contact-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC contact domain is missing: $domain"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_CONTACT_RESPONSE_BYTES: usize = 128 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_ACCOUNT_DEVICES: u16 = 32',
        'kilogram:runtime-ipc-contact-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-contact-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-contact-id:cbor-v1\0',
        'decode_and_verify_runtime_ipc_contact_request_v1',
        'verify_runtime_ipc_contact_request_for_descriptor_v1',
        'decode_and_verify_signed_runtime_ipc_contact_response_v1',
        'verify_runtime_ipc_contact_response_for_request_v1',
        'sign_runtime_ipc_contact_response_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC contact Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne 'CANONICAL-RUNTIME-IPC-VECTORS.json') {
        throw 'canonical runtime IPC contact vector header changed'
    }
    foreach ($entry in @(
        $Vectors.expected.account_id_hex,
        $Vectors.expected.device_id_hex,
        $Vectors.expected.peer_account_id_hex,
        $Vectors.expected.conversation_id_hex,
        $Vectors.expected.contact_id_hex,
        $Vectors.expected.add_contact.request.object_id_hex,
        $Vectors.expected.add_contact.response.object_id_hex,
        $Vectors.expected.directory_status.request.object_id_hex,
        $Vectors.expected.directory_status.response.object_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'runtime IPC contact identifier'
    }
    foreach ($entry in @(
        $Vectors.expected.add_contact.request.cbor_hex,
        $Vectors.expected.add_contact.response.signed_cbor_hex,
        $Vectors.expected.directory_status.request.cbor_hex,
        $Vectors.expected.directory_status.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$entry) -Label 'runtime IPC contact object'
    }
    foreach ($entry in @(
        $Vectors.expected.add_contact.response.signature_hex,
        $Vectors.expected.directory_status.response.signature_hex)) {
        Assert-LowerHex ([string]$entry) 64 'runtime IPC contact response signature'
    }
    if ([int]$Vectors.input.endpoint_candidate_count -ne 2 -or
        [int]$Vectors.input.authority_revision -ne 7 -or
        [int]$Vectors.input.active_device_count -ne 2 -or
        [int]$Vectors.input.receipt_generation -ne 3 -or
        [int]$Vectors.input.profile_convergence -ne 1 -or
        [int]$Vectors.input.restart_recovery -ne 0 -or
        [string]$Vectors.expected.context.add_command_result_pair -cne
            'add-contact/contact-added' -or
        [string]$Vectors.expected.context.status_command_result_pair -cne
            'own-device-directory-status/own-device-directory-status' -or
        -not [bool]$Vectors.expected.context.response_binds_complete_request_digest -or
        [bool]$Vectors.expected.context.active_runtime_changed) {
        throw 'canonical runtime IPC contact vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $coreVectorsPath, $cddlPath, $rustPath,
    $libPath, $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC contact file is missing: $path"
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

foreach ($required in @('runtime-ipc-contact-request-v1', 'runtime-ipc-contact-result-v1',
    'runtime-ipc-own-device-directory-status-v1', 'signed-runtime-ipc-contact-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC contact CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_contact_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC contact module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_contact_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical runtime IPC contact model leaked into active implementation'
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
    'snapshot-bound-paginated-mailbox-capability-status')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC contact aggregate coverage boundary changed'
}
$expectedMutationIds = @(
    'runtime-ipc-add-contact-request-v1',
    'signed-runtime-ipc-add-contact-response-v1',
    'runtime-ipc-device-directory-status-request-v1',
    'signed-runtime-ipc-device-directory-status-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical runtime IPC contact mutation fixtures changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_contact_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC contact reference is missing '$required'"
    }
}
foreach ($required in @('contact enrollment', 'own-device-directory status', 'different trust boundaries',
    'does not activate canonical CBOR', 'Postcard IPC version 26')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC contact RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --core-vectors $coreVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_contact_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC contact independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_contact_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_account_devices = 33
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_device_count_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.input.receipt_generation = 0
    Assert-Rejected { Assert-Vectors $changedVector } 'changed_directory_receipt'
    Write-Output 'canonical_runtime_ipc_contact_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_contact_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'contact_enrollment=verified'
Write-Output 'device_directory_status=verified'
Write-Output 'authority_transition=not_in_scope'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
