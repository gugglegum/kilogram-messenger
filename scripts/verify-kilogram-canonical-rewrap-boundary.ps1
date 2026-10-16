[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-HISTORY-REWRAP-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-HISTORY-REWRAP-VECTORS.json'
$endpointVectorsPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json'
$eventVectorsPath = Join-Path $workspace 'CANONICAL-EVENT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-HISTORY-REWRAP.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_rewrap_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'crates\kilogram-protocol\src\rewrap.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-rewrap-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0151-m1125-canonical-history-rewrap.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.25' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.completes_surface -cne 'history-rewrap' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-HISTORY-REWRAP.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-history-rewrap-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v1-request-bundle-transfer' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical history-rewrap schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_entries_per_bundle,
        [int]$Schema.limits.maximum_inventory_events,
        [int]$Schema.limits.maximum_plaintext_bytes_per_entry,
        [int]$Schema.limits.maximum_ciphertext_bytes_per_entry,
        [int]$Schema.limits.maximum_nested_cbor_bytes,
        [int]$Schema.limits.maximum_bundle_bytes,
        [int]$Schema.limits.maximum_transfer_bytes
    ) -join ','
    if ($limits -cne '256,4096,65536,65552,1048576,8388608,9437184') {
        throw 'canonical history-rewrap limits changed'
    }
    if (@($Schema.objects).Count -ne 8 -or @($Schema.domains).Count -ne 9) {
        throw 'canonical history-rewrap object or domain inventory changed'
    }
    foreach ($required in @(
        'MAX_CANONICAL_HISTORY_REWRAP_ENTRIES: usize = 256',
        'MAX_CANONICAL_HISTORY_REWRAP_INVENTORY_EVENTS: usize = 4096',
        'kilogram:history-rewrap-entry-hpke:cbor-v1\0',
        'kilogram:history-rewrap-entry-aad:cbor-v1\0',
        'decode_and_verify_signed_history_rewrap_request_v1',
        'decode_and_verify_signed_history_rewrap_entry_v1',
        'decode_and_verify_history_rewrap_bundle_v1',
        'decode_and_verify_signed_history_rewrap_transfer_v1',
        'open_history_rewrap_entry_v1',
        'verify_history_rewrap_transfer_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical history-rewrap Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne
        'CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json,CANONICAL-EVENT-VECTORS.json') {
        throw 'canonical history-rewrap vectors header changed'
    }
    foreach ($field in @('account_id_hex', 'source_device_id_hex', 'recipient_device_id_hex',
        'recipient_encryption_public_key_hex', 'conversation_id_hex', 'event_id_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    foreach ($entry in @($Vectors.expected.request, $Vectors.expected.entry, $Vectors.expected.transfer)) {
        Assert-LowerHex ([string]$entry.content_cbor_hex) -Label 'signed content'
        Assert-LowerHex ([string]$entry.signing_preimage_hex) -Label 'signing preimage'
        Assert-LowerHex ([string]$entry.signature_hex) 64 'signature'
        Assert-LowerHex ([string]$entry.signed_cbor_hex) -Label 'signed CBOR'
    }
    foreach ($entry in @($Vectors.expected.manifest, $Vectors.expected.bundle)) {
        Assert-LowerHex ([string]$entry.cbor_hex) -Label 'object CBOR'
        Assert-LowerHex ([string]$entry.id_preimage_hex) -Label 'object ID preimage'
        Assert-LowerHex ([string]$entry.object_id_hex) 32 'object ID'
    }
    Assert-LowerHex ([string]$Vectors.input.session_binding_hex) 32 'session binding'
    Assert-LowerHex ([string]$Vectors.input.encapsulated_key_hex) 32 'encapsulated key'
    Assert-LowerHex ([string]$Vectors.input.ciphertext_hex) -Label 'ciphertext'
    if ([UInt64]$Vectors.input.range_start -ne 0 -or [UInt64]$Vectors.input.max_event_count -ne 1 -or
        [string]$Vectors.input.plaintext_utf8 -cne 'canonical history body') {
        throw 'canonical history-rewrap vector input changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $endpointVectorsPath, $eventVectorsPath, $cddlPath,
    $rustPath, $libPath, $activePath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical history-rewrap file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('signed-history-rewrap-request-v1', 'history-rewrap-manifest-v1',
    'signed-history-rewrap-entry-v1', 'history-rewrap-bundle-v1',
    'signed-history-rewrap-transfer-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "history-rewrap CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_rewrap_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical history-rewrap module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes',
    'kilogram:history-rewrap-hpke:v1\0', 'kilogram:history-rewrap-transfer-signature:v1\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active history-rewrap contract changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_rewrap_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical history-rewrap leaked into active runtime'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'history-rewrap' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard') {
    throw 'active compatibility registry no longer records Postcard history-rewrap'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'history-rewrap' })
$expectedCovered = @('exact-current-account-device-list', 'conversation-membership',
    'recipient-signed-session-bound-range-request', 'sas-and-complete-inventory-digest',
    'source-signed-entry-and-transfer', 'recipient-only-hpke-entry',
    'manifest-index-and-event-associated-data', 'contextual-request-and-session-verification')
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'canonical history-rewrap aggregate coverage boundary changed'
}
if ([int]$mutations.expected_mutations -ne 30843 -or [int]$mutations.maximum_mutations -ne 35000) {
    throw 'canonical history-rewrap decoder mutation bound changed'
}
$expectedMutationIds = @('signed-history-rewrap-request-v1', 'signed-history-rewrap-entry-v1',
    'history-rewrap-bundle-v1', 'signed-history-rewrap-transfer-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-HISTORY-REWRAP-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical history-rewrap mutation fixtures changed'
}
foreach ($required in @('history-recovery ceremony', 'recipient-only encrypted events',
    'exact current Root-signed Account device list', 'real HPKE ciphertext',
    'independent cross-language HPKE', 'changes no active wire bytes')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical history-rewrap RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --endpoint-vectors $endpointVectorsPath --event-vectors $eventVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_history_rewrap_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical history-rewrap independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_protocol_identifier'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_entries_per_bundle = 257
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_entry_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.expected.transfer.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changedVector } 'invalid_transfer_signature_shape'
    Write-Output 'canonical_history_rewrap_self_test=passed'
}

Write-Output 'canonical_history_rewrap_boundary=verified'
Write-Output 'history_rewrap_coverage=complete'
Write-Output 'recipient_only_hpke_entry=verified-by-rust'
Write-Output 'independent_ed25519_and_blake3=verified'
Write-Output 'independent_hpke_conformance=pending-before-activation'
Write-Output 'bounded_decoder_mutations=30843'
Write-Output 'active_postcard_v1_changed=false'
Write-Output 'network_executed=false'
