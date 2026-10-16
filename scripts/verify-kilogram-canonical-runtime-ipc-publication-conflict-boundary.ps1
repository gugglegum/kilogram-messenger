[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json'
$conflictVectorsPath = Join-Path $workspace 'CANONICAL-PUBLICATION-CONFLICT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_publication_conflict_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-publication-conflict-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0165-m1139-canonical-runtime-ipc-publication-conflict.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.43' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical runtime IPC publication-conflict schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_publication_conflict_request_bytes,
        [int]$Schema.limits.maximum_publication_conflict_response_bytes,
        [int]$Schema.limits.maximum_publication_conflict_request_artifact_bytes,
        [int]$Schema.limits.maximum_publication_conflict_response_artifact_bytes,
        [int]$Schema.limits.maximum_publication_conflict_replacement_ticket_artifact_bytes
    ) -join ','
    if ($limits -cne '131072,131072,9437184,9437184,8388608') {
        throw 'canonical runtime IPC publication-conflict limits changed'
    }
    if (@($Schema.domains).Count -ne 67 -or @($Schema.objects).Count -ne 33 -or
        @($Schema.covered_commands).Count -ne 30 -or @($Schema.covered_results).Count -ne 29 -or
        @($Schema.semantic_rules).Count -ne 136 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 3) {
        throw 'canonical runtime IPC publication-conflict inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-publication-conflict-artifact-digest:cbor-v1\0',
        'kilogram:runtime-ipc-publication-conflict-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-publication-conflict-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-publication-conflict-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical runtime IPC publication-conflict domain is missing: $domain"
        }
    }
    foreach ($required in @('create-publication-conflict-request',
        'apply-publication-conflict-response')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical runtime IPC publication-conflict command is missing: $required"
        }
    }
    foreach ($required in @('publication-conflict-request-created',
        'publication-conflict-response-applied')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical runtime IPC publication-conflict result is missing: $required"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_REQUEST_BYTES: usize = 128 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_PUBLICATION_CONFLICT_RESPONSE_BYTES: usize = 128 * 1024',
        'CreatePublicationConflictRequest', 'ApplyPublicationConflictResponse',
        'StagedForOfflineRootExport', 'AwaitingOfflineAuthorization',
        'VerifiedExactCurrentAuthority', 'AtomicallyReplaced', 'PendingPropagation',
        'decode_and_verify_runtime_ipc_publication_conflict_request_v1',
        'decode_and_verify_signed_runtime_ipc_publication_conflict_response_v1',
        'verify_runtime_ipc_publication_conflict_request_for_descriptor_v1',
        'verify_runtime_ipc_publication_conflict_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime IPC publication-conflict Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub replacement_ticket:', 'pub publication_write_key:',
        'pub root_secret:', 'pub local_path:', 'pub request_artifact:',
        'pub response_artifact:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical runtime IPC publication-conflict leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne 'CANONICAL-PUBLICATION-CONFLICT-VECTORS.json') {
        throw 'canonical runtime IPC publication-conflict vector header changed'
    }
    foreach ($value in @($Vectors.expected.account_id_hex, $Vectors.expected.device_id_hex,
        $Vectors.expected.descriptor_id_hex, $Vectors.expected.endpoint_ticket_artifact_digest_hex,
        $Vectors.expected.replacement_ticket_artifact_digest_hex,
        $Vectors.expected.request_artifact_digest_hex, $Vectors.expected.response_artifact_digest_hex,
        $Vectors.expected.create.request.object_id_hex, $Vectors.expected.create.response.object_id_hex,
        $Vectors.expected.apply.request.object_id_hex, $Vectors.expected.apply.response.object_id_hex,
        $Vectors.input.create_request_id_hex, $Vectors.input.apply_request_id_hex,
        $Vectors.input.device_list_digest_hex)) {
        Assert-LowerHex ([string]$value) 32 'runtime IPC publication-conflict identifier'
    }
    foreach ($value in @($Vectors.expected.create.response.signature_hex,
        $Vectors.expected.apply.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'runtime IPC publication-conflict response signature'
    }
    foreach ($value in @($Vectors.expected.descriptor_signed_cbor_hex,
        $Vectors.expected.create.current_cbor_hex, $Vectors.expected.create.request.cbor_hex,
        $Vectors.expected.create.response.signed_cbor_hex, $Vectors.expected.create.result_cbor_hex,
        $Vectors.expected.apply.current_cbor_hex, $Vectors.expected.apply.request.cbor_hex,
        $Vectors.expected.apply.response.signed_cbor_hex, $Vectors.expected.apply.result_cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'runtime IPC publication-conflict canonical object'
    }
    $context = $Vectors.expected.context
    if ([bool]$context.active_runtime_changed -or [bool]$context.artifact_broker_activated -or
        [bool]$context.network_io_executed -or [bool]$context.local_path_in_ipc -or
        [bool]$context.root_secret_in_ipc -or [bool]$context.publication_write_key_in_ipc -or
        [bool]$context.replacement_ticket_plaintext_in_ipc -or
        [bool]$context.root_resolution_plaintext_in_ipc -or
        -not [bool]$context.request_artifact_staged_before_success -or
        -not [bool]$context.response_verified_before_atomic_install -or
        -not [bool]$context.conflict_proof_retained_after_resolution -or
        -not [bool]$context.sibling_propagation_still_pending) {
        throw 'runtime IPC publication-conflict security context changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $conflictVectorsPath, $cddlPath,
    $rustPath, $libPath, $activeIpcPath, $compatibilityPath, $coveragePath,
    $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime IPC publication-conflict file is missing: $path"
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

foreach ($required in @('runtime-ipc-publication-conflict-current-v1',
    'runtime-ipc-publication-conflict-pending-request-v1',
    'runtime-ipc-publication-conflict-command-v1',
    'runtime-ipc-publication-conflict-request-v1',
    'runtime-ipc-publication-conflict-request-created-v1',
    'runtime-ipc-publication-conflict-resolution-applied-v1',
    'runtime-ipc-publication-conflict-result-v1',
    'signed-runtime-ipc-publication-conflict-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "runtime IPC publication-conflict CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_publication_conflict_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime IPC publication-conflict module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_publication_conflict_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical publication-conflict model leaked into active implementation'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
    [string]$source[0].compatibility -cne 'exact-version' -or
    [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 26) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v26'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ([string]$coverage.milestone -cne 'M1.1.43' -or $surface.Count -ne 1 -or
    [string]$surface[0].coverage -cne 'partial' -or
    'content-addressed-root-authorized-publication-conflict-ceremony' -cnotin @($surface[0].covered) -or
    'content-addressed-own-device-announcement-automation' -cnotin @($surface[0].covered) -or
    'pairwise-own-device-ticket-discovery' -cnotin @($surface[0].covered) -or
    'roster-wide-own-device-automation' -cnotin @($surface[0].covered) -or
    (@($surface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical runtime IPC publication-conflict aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-publication-conflict-create-request-v1',
    'signed-runtime-ipc-publication-conflict-create-response-v1',
    'runtime-ipc-publication-conflict-apply-request-v1',
    'signed-runtime-ipc-publication-conflict-apply-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-PUBLICATION-CONFLICT-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 29799 -or
    [int]$mutations.maximum_mutations -ne 30000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 30000) {
    throw 'canonical runtime IPC publication-conflict fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'descriptor_signed_cbor_hex',
    'requestArtifact', 'responseArtifact', 'protected publication-conflict artifact leaked into IPC',
    'canonical_runtime_ipc_publication_conflict_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime IPC publication-conflict reference is missing '$required'"
    }
}
foreach ($required in @('exact current state', 'content-addressed staging area',
    'Root secret', 'publication write keys', 'replacement-ticket plaintext',
    'local filesystem paths', 'atomic transaction', 'sibling propagation is still pending',
    'idempotency key', 'does not activate canonical CBOR', 'Postcard IPC version 26',
    'No network was executed')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime IPC publication-conflict RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --conflict-vectors $conflictVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_publication_conflict_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime IPC publication-conflict independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_publication_conflict_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_publication_conflict_request_bytes = 131073
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_publication_conflict_request_limit'
    $leakedVector = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $leakedVector.expected.context.root_resolution_plaintext_in_ipc = $true
    Assert-Rejected { Assert-Vectors $leakedVector } 'root_resolution_plaintext_in_ipc'
    $prematureVector = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $prematureVector.expected.context.sibling_propagation_still_pending = $false
    Assert-Rejected { Assert-Vectors $prematureVector } 'false_remote_convergence_claim'
    Write-Output 'canonical_runtime_ipc_publication_conflict_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_publication_conflict_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'create_publication_conflict_request=canonical-candidate'
Write-Output 'apply_publication_conflict_response=canonical-candidate'
Write-Output 'root_secret_in_ipc=false'
Write-Output 'publication_write_key_in_ipc=false'
Write-Output 'replacement_ticket_plaintext_in_ipc=false'
Write-Output 'root_resolution_plaintext_in_ipc=false'
Write-Output 'artifact_broker_activated=false'
Write-Output 'sibling_propagation_pending=true'
Write-Output 'bounded_decoder_mutations=29799'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
