[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json'
$coreVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$contactVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json'
$ticketVectorsPath = Join-Path $workspace 'CANONICAL-TICKET-PUBLICATION-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_publication_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-publication-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0163-m1137-canonical-runtime-ipc-publication-rotation.md'

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
        throw 'canonical runtime IPC publication schema header changed'
    }
    $limits = @([int]$Schema.limits.maximum_publication_request_bytes,
        [int]$Schema.limits.maximum_publication_response_bytes) -join ','
    if ($limits -cne '65536,65536') { throw 'canonical runtime IPC publication limits changed' }
    if (@($Schema.domains).Count -ne 73 -or @($Schema.objects).Count -ne 36 -or
        @($Schema.covered_commands).Count -ne 32 -or @($Schema.covered_results).Count -ne 31 -or
        @($Schema.semantic_rules).Count -ne 154 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 2) {
        throw 'canonical runtime IPC publication inventory changed'
    }
    foreach ($domain in @('kilogram:runtime-ipc-publication-peer-scope-digest:cbor-v1\0',
        'kilogram:runtime-ipc-publication-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-publication-rotation-id:cbor-v1\0',
        'kilogram:runtime-ipc-publication-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-publication-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC publication domain is missing: $domain"
        }
    }
    if ('rotate-publication-channel' -cnotin @($Schema.covered_commands) -or
        'publication-channel-rotated' -cnotin @($Schema.covered_results)) {
        throw 'canonical runtime IPC publication command or result is missing'
    }
    foreach ($required in @('MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_REQUEST_BYTES: usize = 64 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RESPONSE_BYTES: usize = 64 * 1024',
        'RotatePublicationChannel', 'AlreadyCommitted', 'AwaitingFirstPublication',
        'PendingPeerDelivery', 'NotRequired',
        'canonical_runtime_ipc_publication_peer_scope_digest_v1',
        'derive_runtime_ipc_publication_rotation_v1',
        'decode_and_verify_runtime_ipc_publication_request_v1',
        'decode_and_verify_signed_runtime_ipc_publication_response_v1',
        'verify_runtime_ipc_publication_request_for_descriptor_v1',
        'verify_runtime_ipc_publication_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC publication Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub device_secret:', 'pub peer_scope:', 'pub private_write_key:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical runtime IPC publication leaks secret material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne
        'CANONICAL-RUNTIME-IPC-VECTORS.json,CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json,CANONICAL-TICKET-PUBLICATION-VECTORS.json') {
        throw 'canonical runtime IPC publication vector header changed'
    }
    foreach ($value in @($Vectors.expected.account_id_hex, $Vectors.expected.device_id_hex,
        $Vectors.expected.peer_scope_digest_hex, $Vectors.expected.rotation_id_hex,
        $Vectors.expected.request.object_id_hex, $Vectors.expected.response.object_id_hex,
        $Vectors.input.contact_id_hex, $Vectors.input.peer_account_id_hex,
        $Vectors.input.latest_publication_id_hex, $Vectors.input.request_id_hex)) {
        Assert-LowerHex ([string]$value) 32 'runtime IPC publication identifier'
    }
    Assert-LowerHex ([string]$Vectors.expected.response.signature_hex) 64 `
        'runtime IPC publication response signature'
    foreach ($value in @($Vectors.expected.request.cbor_hex,
        $Vectors.expected.response.signed_cbor_hex, $Vectors.expected.rotation_cbor_hex,
        $Vectors.expected.previous_capability.cbor_hex, $Vectors.expected.capability.cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'runtime IPC publication canonical object'
    }
    if ([UInt64]$Vectors.input.next_channel_epoch -ne
        [UInt64]$Vectors.input.current_channel_epoch + 1 -or
        -not [bool]$Vectors.expected.context.successor_derived_inside_runtime -or
        [bool]$Vectors.expected.context.local_device_secret_in_payload -or
        [bool]$Vectors.expected.context.network_publication_executed -or
        [bool]$Vectors.expected.context.peer_delivery_executed -or
        [bool]$Vectors.expected.context.active_runtime_changed) {
        throw 'runtime IPC publication transition context changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $coreVectorsPath, $contactVectorsPath,
    $ticketVectorsPath, $cddlPath, $rustPath, $libPath, $activeIpcPath,
    $compatibilityPath, $coveragePath, $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC publication file is missing: $path"
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

foreach ($required in @('runtime-ipc-publication-channel-state-v1',
    'runtime-ipc-publication-command-v1', 'runtime-ipc-publication-request-v1',
    'runtime-ipc-publication-channel-rotation-v1', 'runtime-ipc-publication-result-v1',
    'signed-runtime-ipc-publication-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC publication CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_publication_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC publication module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_publication_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical publication model leaked into active implementation'
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
    'roster-wide-own-device-automation',
    'sas-confirmed-device-signed-recovery-plan-custody',
    'content-addressed-recovery-plan-status',
    'device-signed-recovery-plan-cancellation-tombstone',
    'recipient-device-signed-recovery-scheduler-chain',
    'deterministic-retry-and-attempt-lease',
    'policy-derived-foreground-recovery-readiness',
    'checkpoint-bound-recovery-progress-and-completion',
    'recovery-execution-start-and-status')
$expectedRemaining = @($schema.remaining_runtime_ipc_surface | ForEach-Object { [string]$_ })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    (@($surface[0].remaining) -join ',') -cne ($expectedRemaining -join ',')) {
    throw 'canonical runtime IPC publication aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-publication-rotate-request-v1',
    'signed-runtime-ipc-publication-rotate-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 30843 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical runtime IPC publication fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'blake3DeriveKey(',
    'next publication channel epoch must advance exactly once',
    'canonical_runtime_ipc_publication_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC publication reference is missing '$required'"
    }
}
foreach ($required in @('exact expected current public state', 'Device secret',
    'successor capability is never caller supplied', 'one state lock', 'idempotency key',
    'no network publication', 'no peer delivery',
    'Root-authorized conflict resolution is not included',
    'does not activate canonical CBOR', 'Postcard IPC version 26',
    'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC publication RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --core-vectors $coreVectorsPath --contact-vectors $contactVectorsPath `
    --ticket-vectors $ticketVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_publication_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC publication independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_publication_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_publication_request_bytes = 65537
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_publication_request_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 40 | ConvertFrom-Json
    $changedVector.input.next_channel_epoch = [UInt64]$changedVector.input.next_channel_epoch + 1
    Assert-Rejected { Assert-Vectors $changedVector } 'skipped_publication_epoch'
    $changedSecretFlag = $vectors | ConvertTo-Json -Depth 40 | ConvertFrom-Json
    $changedSecretFlag.expected.context.local_device_secret_in_payload = $true
    Assert-Rejected { Assert-Vectors $changedSecretFlag } 'device_secret_in_publication_payload'
    Write-Output 'canonical_runtime_ipc_publication_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_publication_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'successor_derivation=runtime-internal'
Write-Output 'device_secret_in_payload=false'
Write-Output 'network_publication_executed=false'
Write-Output 'peer_delivery_executed=false'
Write-Output 'bounded_decoder_mutations=30843'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
