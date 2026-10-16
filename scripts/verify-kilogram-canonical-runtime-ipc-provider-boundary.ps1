[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json'
$coreVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_provider_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-provider-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0160-m1134-canonical-runtime-ipc-volunteer-providers.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.45' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v27-with-json-descriptor-v8' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC provider schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_provider_request_bytes,
        [int]$Schema.limits.maximum_provider_response_bytes,
        [int]$Schema.limits.maximum_active_providers,
        [int]$Schema.limits.maximum_selected_providers
    ) -join ','
    if ($limits -cne '65536,262144,4096,8') {
        throw 'canonical runtime IPC provider limits changed'
    }
    if (@($Schema.domains).Count -ne 73 -or @($Schema.objects).Count -ne 36 -or
        @($Schema.covered_commands).Count -ne 32 -or @($Schema.covered_results).Count -ne 31 -or
        @($Schema.semantic_rules).Count -ne 158 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 1) {
        throw 'canonical runtime IPC provider inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-provider-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-provider-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-provider-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC provider domain is missing: $domain"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_PROVIDER_REQUEST_BYTES: usize = 64 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_PROVIDER_RESPONSE_BYTES: usize = 256 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_ACTIVE_PROVIDERS: u32 = 4_096',
        'MAX_CANONICAL_RUNTIME_IPC_SELECTED_PROVIDERS: u8 = 8',
        'decode_and_verify_runtime_ipc_provider_request_v1',
        'decode_and_verify_signed_runtime_ipc_provider_response_v1',
        'verify_runtime_ipc_provider_response_for_request_v1',
        'verify_mailbox_storage_offer_admission_work_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC provider Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    $expectedDependencies = @('CANONICAL-RUNTIME-IPC-VECTORS.json',
        'CANONICAL-MAILBOX-VECTORS.json', 'CANONICAL-MAILBOX-GOSSIP-VECTORS.json')
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne ($expectedDependencies -join ',')) {
        throw 'canonical runtime IPC provider vector header changed'
    }
    foreach ($entry in @($Vectors.expected.account_id_hex, $Vectors.expected.device_id_hex,
        $Vectors.expected.offer_id_hex, $Vectors.expected.request.object_id_hex,
        $Vectors.expected.response.object_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'runtime IPC provider identifier'
    }
    foreach ($entry in @($Vectors.input.offer_signed_cbor_hex,
        $Vectors.expected.request.cbor_hex, $Vectors.expected.response.signed_cbor_hex,
        $Vectors.expected.selection_request.cbor_hex,
        $Vectors.expected.selection_response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$entry) -Label 'runtime IPC provider object'
    }
    foreach ($signature in @($Vectors.expected.response.signature_hex,
        $Vectors.expected.selection_response.signature_hex)) {
        Assert-LowerHex ([string]$signature) 64 'runtime IPC provider response signature'
    }
    if (-not [bool]$Vectors.expected.context.provider_properties_derive_from_signed_offer -or
        [bool]$Vectors.expected.context.mailbox_capabilities_in_payload -or
        -not [bool]$Vectors.expected.context.response_binds_complete_request_digest -or
        [bool]$Vectors.expected.context.active_runtime_changed) {
        throw 'canonical runtime IPC provider vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $coreVectorsPath, $cddlPath, $rustPath,
    $libPath, $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC provider file is missing: $path"
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

foreach ($required in @('runtime-ipc-provider-command-v1', 'runtime-ipc-provider-request-v1',
    'runtime-ipc-volunteer-provider-v1', 'runtime-ipc-provider-result-v1',
    'signed-runtime-ipc-provider-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC provider CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_provider_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC provider module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 27;',
    'kilogram:runtime-ipc-descriptor:v8\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_provider_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical runtime IPC provider model leaked into active implementation'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
    [string]$source[0].compatibility -cne 'exact-version' -or
    [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 27) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v27'
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
    'roster-wide-own-device-automation',
    'sas-confirmed-device-signed-recovery-plan-custody',
    'content-addressed-recovery-plan-status',
    'device-signed-recovery-plan-cancellation-tombstone',
    'recipient-device-signed-recovery-scheduler-chain',
    'deterministic-retry-and-attempt-lease',
    'policy-derived-foreground-recovery-readiness',
    'checkpoint-bound-recovery-progress-and-completion',
    'recovery-execution-start-and-status',
    'active-postcard-conversation-list-pagination-migration')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC provider aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-provider-import-request-v1',
    'signed-runtime-ipc-provider-import-response-v1',
    'runtime-ipc-provider-selection-request-v1',
    'signed-runtime-ipc-provider-selection-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 30843 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical runtime IPC provider mutation fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_provider_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC provider reference is missing '$required'"
    }
}
foreach ($required in @('original signed offer', 'contains no mailbox ID',
    'between one and eight providers', 'bootstrap-safe rules',
    'does not activate canonical CBOR', 'Postcard IPC version 26', 'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC provider RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --core-vectors $coreVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_provider_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC provider independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_provider_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_selected_providers = 9
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_provider_selection_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.expected.context.mailbox_capabilities_in_payload = $true
    Assert-Rejected { Assert-Vectors $changedVector } 'provider_capability_leak'
    Write-Output 'canonical_runtime_ipc_provider_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_provider_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'provider_offer_original_signature_retained=true'
Write-Output 'mailbox_capabilities_in_payload=false'
Write-Output 'bounded_decoder_mutations=30843'
Write-Output 'active_postcard_v27_changed_by_canonical_candidate=false'
Write-Output 'network_executed=false'
