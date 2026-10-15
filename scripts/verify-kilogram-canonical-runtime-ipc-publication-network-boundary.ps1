[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json'
$coreVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-VECTORS.json'
$contactVectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json'
$ticketVectorsPath = Join-Path $workspace 'CANONICAL-TICKET-PUBLICATION-VECTORS.json'
$conflictVectorsPath = Join-Path $workspace 'CANONICAL-PUBLICATION-CONFLICT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_publication_network_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-publication-network-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0164-m1138-canonical-runtime-ipc-publication-network.md'

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
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC publication-network schema header changed'
    }
    $limits = @([int]$Schema.limits.maximum_publication_network_request_bytes,
        [int]$Schema.limits.maximum_publication_network_response_bytes,
        [int]$Schema.limits.maximum_publication_service_origin_bytes,
        [int]$Schema.limits.maximum_publication_recipient_devices) -join ','
    if ($limits -cne '131072,131072,512,32') {
        throw 'canonical runtime IPC publication-network limits changed'
    }
    if (@($Schema.domains).Count -ne 62 -or @($Schema.objects).Count -ne 31 -or
        @($Schema.covered_commands).Count -ne 27 -or @($Schema.covered_results).Count -ne 26 -or
        @($Schema.semantic_rules).Count -ne 122 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 3) {
        throw 'canonical runtime IPC publication-network inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-publication-network-service-origin-digest:cbor-v1\0',
        'kilogram:runtime-ipc-publication-network-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-publication-network-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-publication-network-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC publication-network domain is missing: $domain"
        }
    }
    foreach ($required in @('publish-own-ticket', 'refresh-contact-ticket')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical runtime IPC publication-network command is missing: $required"
        }
    }
    foreach ($required in @('own-ticket-published', 'contact-ticket-refreshed')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical runtime IPC publication-network result is missing: $required"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_REQUEST_BYTES: usize = 128 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_NETWORK_RESPONSE_BYTES: usize = 128 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_SERVICE_ORIGIN_BYTES: usize = 512',
        'MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_RECIPIENT_DEVICES: u16 = 32',
        'PublishOwnTicket', 'RefreshContactTicket', 'ConfirmedHttpsSuccess',
        'PendingPeerObservation', 'AtomicallyCommitted', 'ObservedByLocalDevice',
        'decode_and_verify_runtime_ipc_publication_network_request_v1',
        'decode_and_verify_signed_runtime_ipc_publication_network_response_v1',
        'verify_runtime_ipc_publication_network_request_for_descriptor_v1',
        'verify_runtime_ipc_publication_network_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC publication-network Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub ticket:', 'pub opaque_envelope:', 'pub device_secret:',
        'pub peer_scope:', 'pub write_secret:', 'pub local_path:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical runtime IPC publication-network leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne
        'CANONICAL-RUNTIME-IPC-VECTORS.json,CANONICAL-RUNTIME-IPC-CONTACT-VECTORS.json,CANONICAL-RUNTIME-IPC-PUBLICATION-VECTORS.json,CANONICAL-TICKET-PUBLICATION-VECTORS.json,CANONICAL-PUBLICATION-CONFLICT-VECTORS.json') {
        throw 'canonical runtime IPC publication-network vector header changed'
    }
    foreach ($value in @($Vectors.expected.account_id_hex, $Vectors.expected.device_id_hex,
        $Vectors.expected.service_origin_digest_hex,
        $Vectors.expected.publish.request.object_id_hex,
        $Vectors.expected.publish.response.object_id_hex,
        $Vectors.expected.refresh.request.object_id_hex,
        $Vectors.expected.refresh.response.object_id_hex,
        $Vectors.input.publish_request_id_hex, $Vectors.input.refresh_request_id_hex,
        $Vectors.input.peer_account_id_hex, $Vectors.input.publish_contact_id_hex,
        $Vectors.input.refresh_contact_id_hex, $Vectors.input.recipient_device_list_digest_hex)) {
        Assert-LowerHex ([string]$value) 32 'runtime IPC publication-network identifier'
    }
    foreach ($value in @($Vectors.expected.publish.response.signature_hex,
        $Vectors.expected.refresh.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'runtime IPC publication-network response signature'
    }
    foreach ($value in @($Vectors.expected.publish.request.cbor_hex,
        $Vectors.expected.publish.response.signed_cbor_hex,
        $Vectors.expected.publish.state_cbor_hex, $Vectors.expected.publish.result_cbor_hex,
        $Vectors.expected.refresh.request.cbor_hex,
        $Vectors.expected.refresh.response.signed_cbor_hex,
        $Vectors.expected.refresh.state_cbor_hex, $Vectors.expected.refresh.result_cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'runtime IPC publication-network canonical object'
    }
    $context = $Vectors.expected.context
    if ([bool]$context.active_runtime_changed -or [bool]$context.network_io_executed -or
        -not [bool]$context.hpke_envelope_fixture_only -or [bool]$context.ticket_plaintext_in_ipc -or
        [bool]$context.hpke_ciphertext_in_ipc -or [bool]$context.write_secret_in_ipc -or
        -not [bool]$context.https_success_is_device_attestation_not_remote_durability_proof -or
        -not [bool]$context.publish_delivery_requires_separate_peer_observation -or
        -not [bool]$context.refresh_install_is_atomic_before_success) {
        throw 'runtime IPC publication-network security context changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $coreVectorsPath, $contactVectorsPath,
    $ticketVectorsPath, $conflictVectorsPath, $cddlPath, $rustPath, $libPath,
    $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath, $referencePath,
    $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC publication-network file is missing: $path"
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

foreach ($required in @('runtime-ipc-publication-network-publish-state-v1',
    'runtime-ipc-publication-network-refresh-state-v1',
    'runtime-ipc-publication-network-command-v1',
    'runtime-ipc-publication-network-request-v1', 'runtime-ipc-own-ticket-publication-v1',
    'runtime-ipc-contact-ticket-refresh-v1', 'runtime-ipc-publication-network-result-v1',
    'signed-runtime-ipc-publication-network-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC publication-network CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_publication_network_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC publication-network module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_publication_network_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical publication-network model leaked into active implementation'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
    [string]$source[0].compatibility -cne 'exact-version' -or
    [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 26) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v26'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'partial' -or
    'exact-network-ticket-publication-and-device-observation' -cnotin @($surface[0].covered) -or
    'pairwise-own-device-ticket-discovery' -cnotin @($surface[0].covered) -or
    'roster-wide-own-device-automation' -cnotin @($surface[0].covered) -or
    (@($surface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical runtime IPC publication-network aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-publication-network-publish-request-v1',
    'signed-runtime-ipc-publication-network-publish-response-v1',
    'runtime-ipc-publication-network-refresh-request-v1',
    'signed-runtime-ipc-publication-network-refresh-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-PUBLICATION-NETWORK-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 28755 -or
    [int]$mutations.maximum_mutations -ne 30000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 30000) {
    throw 'canonical runtime IPC publication-network fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'blake3DeriveKey(',
    'service origin must be an exact canonical root-only HTTPS URL',
    'canonical_runtime_ipc_publication_network_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC publication-network reference is missing '$required'"
    }
}
foreach ($required in @('exact current publication state', 'opaque envelope',
    'ticket plaintext', 'HPKE ciphertext', 'write secret', 'atomic',
    'peer Device observation', 'not proof of remote durability', 'idempotency key',
    'does not activate canonical CBOR', 'Postcard IPC version 26', 'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC publication-network RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --core-vectors $coreVectorsPath --contact-vectors $contactVectorsPath `
    --ticket-vectors $ticketVectorsPath --conflict-vectors $conflictVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_publication_network_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC publication-network independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_publication_network_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_publication_network_request_bytes = 131073
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_publication_network_request_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $changedVector.expected.context.ticket_plaintext_in_ipc = $true
    Assert-Rejected { Assert-Vectors $changedVector } 'ticket_plaintext_in_publication_network_ipc'
    $changedObservation = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $changedObservation.expected.context.publish_delivery_requires_separate_peer_observation = $false
    Assert-Rejected { Assert-Vectors $changedObservation } 'publication_without_peer_observation'
    Write-Output 'canonical_runtime_ipc_publication_network_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_publication_network_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'publish_own_ticket=canonical-candidate'
Write-Output 'refresh_contact_ticket=canonical-candidate'
Write-Output 'ticket_plaintext_in_ipc=false'
Write-Output 'hpke_ciphertext_in_ipc=false'
Write-Output 'write_secret_in_ipc=false'
Write-Output 'https_success_is_remote_durability_proof=false'
Write-Output 'bounded_decoder_mutations=28755'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
