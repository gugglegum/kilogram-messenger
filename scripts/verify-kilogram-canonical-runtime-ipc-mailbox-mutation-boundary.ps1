[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json'
$provisioningVectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-PROVISIONING-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_mailbox_mutation_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-mailbox-mutation-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0162-m1136-canonical-runtime-ipc-mailbox-mutations.md'

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
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v32-with-json-descriptor-v13' -or
        -not [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC mailbox mutation schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_mailbox_mutation_request_bytes,
        [int]$Schema.limits.maximum_mailbox_mutation_response_bytes,
        [int]$Schema.limits.maximum_selected_providers
    ) -join ','
    if ($limits -cne '524288,131072,8') {
        throw 'canonical runtime IPC mailbox mutation limits changed'
    }
    if (@($Schema.domains).Count -ne 78 -or @($Schema.objects).Count -ne 38 -or
        @($Schema.covered_commands).Count -ne 37 -or @($Schema.covered_results).Count -ne 36 -or
        @($Schema.semantic_rules).Count -ne 185 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 0) {
        throw 'canonical runtime IPC mailbox mutation inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-mailbox-mutation-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-mailbox-mutation-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-mailbox-mutation-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC mailbox mutation domain is missing: $domain"
        }
    }
    foreach ($command in @('create-mailbox-capability', 'rotate-mailbox-capability',
        'revoke-mailbox-capability')) {
        if ($command -cnotin @($Schema.covered_commands | ForEach-Object { [string]$_ })) {
            throw "canonical runtime IPC mailbox mutation command is missing: $command"
        }
    }
    if ('mailbox-capability-transition' -cnotin
        @($Schema.covered_results | ForEach-Object { [string]$_ })) {
        throw 'canonical runtime IPC mailbox mutation result is missing'
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_REQUEST_BYTES: usize = 512 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_MAILBOX_MUTATION_RESPONSE_BYTES: usize = 128 * 1024',
        'CreateMailboxCapability', 'RotateMailboxCapability', 'RevokeMailboxCapability',
        'AlreadyCommitted', 'PendingRecipientAcknowledgement',
        'decode_and_verify_runtime_ipc_mailbox_mutation_request_v1',
        'decode_and_verify_signed_runtime_ipc_mailbox_mutation_response_v1',
        'verify_runtime_ipc_mailbox_mutation_request_for_descriptor_v1',
        'verify_runtime_ipc_mailbox_mutation_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC mailbox mutation Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('CanonicalSignedLocalMailboxBindingV1', 'read_secret:', 'write_secret:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical runtime IPC mailbox mutation leaks local secret material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne
        'CANONICAL-RUNTIME-IPC-VECTORS.json,CANONICAL-RUNTIME-IPC-PROVIDER-VECTORS.json,CANONICAL-MAILBOX-PROVISIONING-VECTORS.json') {
        throw 'canonical runtime IPC mailbox mutation vector header changed'
    }
    Assert-LowerHex ([string]$Vectors.expected.descriptor.object_id_hex) 32 `
        'runtime IPC mailbox mutation descriptor ID'
    Assert-LowerHex ([string]$Vectors.expected.provider_selection.object_id_hex) 32 `
        'runtime IPC mailbox mutation provider selection ID'
    Assert-LowerHex ([string]$Vectors.expected.descriptor.signed_cbor_hex) -Label `
        'runtime IPC mailbox mutation descriptor'
    Assert-LowerHex ([string]$Vectors.expected.provider_selection.signed_cbor_hex) -Label `
        'runtime IPC mailbox mutation provider selection'
    foreach ($operation in @('create', 'rotate', 'revoke')) {
        $request = $Vectors.expected.$operation.request
        $response = $Vectors.expected.$operation.response
        Assert-LowerHex ([string]$request.object_id_hex) 32 "$operation request ID"
        Assert-LowerHex ([string]$response.object_id_hex) 32 "$operation response ID"
        Assert-LowerHex ([string]$response.signature_hex) 64 "$operation response signature"
        Assert-LowerHex ([string]$request.cbor_hex) -Label "$operation request"
        Assert-LowerHex ([string]$response.signed_cbor_hex) -Label "$operation response"
    }
    foreach ($inputId in @($Vectors.input.contact_id_hex, $Vectors.input.create_request_id_hex,
        $Vectors.input.rotate_request_id_hex, $Vectors.input.revoke_request_id_hex,
        $Vectors.input.recipient_device_list_digest_hex)) {
        Assert-LowerHex ([string]$inputId) 32 'runtime IPC mailbox mutation input ID'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $provisioningVectorsPath, $cddlPath,
    $rustPath, $libPath, $activeIpcPath, $compatibilityPath, $coveragePath,
    $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC mailbox mutation file is missing: $path"
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

foreach ($required in @('runtime-ipc-mailbox-mutation-target-v1',
    'runtime-ipc-mailbox-mutation-command-v1', 'runtime-ipc-mailbox-mutation-request-v1',
    'runtime-ipc-mailbox-capability-transition-v1', 'runtime-ipc-mailbox-mutation-result-v1',
    'signed-runtime-ipc-mailbox-mutation-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC mailbox mutation CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_mailbox_mutation_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC mailbox mutation module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 32;',
    'kilogram:runtime-ipc-descriptor:v13\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_mailbox_mutation_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical mailbox mutation model leaked into active implementation'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
    [string]$source[0].compatibility -cne 'exact-version' -or
    [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 32) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v32'
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
    'active-postcard-conversation-list-pagination-migration',
    'exhaustive-command-result-and-active-adapter-inventory',
    'foreground-contact-ticket-automation-policy-and-status',
    'active-postcard-endpoint-bootstrap-helper-retirement',
    'active-postcard-legacy-central-https-mailbox-command-retirement',
    'read-only-peer-capability-quarantine-observability')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC mailbox mutation aggregate coverage boundary changed'
}
$expectedMutationIds = @(
    'runtime-ipc-mailbox-capability-create-request-v1',
    'signed-runtime-ipc-mailbox-capability-create-response-v1',
    'runtime-ipc-mailbox-capability-rotate-request-v1',
    'signed-runtime-ipc-mailbox-capability-rotate-response-v1',
    'runtime-ipc-mailbox-capability-revoke-request-v1',
    'signed-runtime-ipc-mailbox-capability-revoke-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-MAILBOX-MUTATION-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 31851 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical runtime IPC mailbox mutation fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'blake3Hash(',
    'canonical_runtime_ipc_mailbox_mutation_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC mailbox mutation reference is missing '$required'"
    }
}
foreach ($required in @('exact Device-signed provider-selection',
    'active, recipient-acknowledged', 'one atomic transaction', 'idempotency key',
    'mailbox read secret', 'mailbox write secret', 'Independent HPKE interoperability',
    'does not activate canonical CBOR', 'Postcard IPC version 26', 'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC mailbox mutation RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --provisioning-vectors $provisioningVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_mailbox_mutation_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC mailbox mutation independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_mailbox_mutation_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_mailbox_mutation_request_bytes = 524289
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_mailbox_mutation_request_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 40 | ConvertFrom-Json
    $changedVector.expected.create.response.signature_hex =
        ([string]$changedVector.expected.create.response.signature_hex).ToUpperInvariant()
    Assert-Rejected { Assert-Vectors $changedVector } 'noncanonical_mailbox_mutation_signature'
    Write-Output 'canonical_runtime_ipc_mailbox_mutation_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_mailbox_mutation_boundary=verified'
Write-Output 'runtime_ipc_coverage=complete'
Write-Output 'mailbox_mutation_provider_selection=exact-device-signed'
Write-Output 'mailbox_mutation_predecessor=exact-acknowledged-head'
Write-Output 'local_binding_secrets_in_payload=false'
Write-Output 'bounded_decoder_mutations=31851'
Write-Output 'active_postcard_v29_changed_by_canonical_candidate=false'
Write-Output 'network_executed=false'
