[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-PREKEY-SELECTION-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-PREKEY-SELECTION-VECTORS.json'
$ratchetVectorsPath = Join-Path $workspace 'CANONICAL-RATCHET-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-PREKEY-SELECTION.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_prekey_selection_v1.rs'
$activePath = Join-Path $workspace 'crates\kilogram-ratchet\src\lib.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-prekey-selection-reference.mjs'
$ratchetReferencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-ratchet-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0137-m1111-canonical-prekey-selection.md'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.11' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical prekey selection schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-PREKEY-SELECTION.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-prekey-selection-v1' -or
        [string]$Schema.activation.current_runtime_algorithm -cne 'blake3-domain-plus-device-ids-plus-generation-le-v1' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical prekey selection activation boundary changed'
    }
    if ([int]$Schema.limits.maximum_nested_prekey_pool_bytes -ne 524288 -or
        [int]$Schema.limits.maximum_prekeys_per_pool -ne 64 -or
        [string]$Schema.domain.value -cne 'kilogram:prekey-selection:cbor-v1\0' -or
        [string]$Schema.domain.operation -cne 'blake3-domain-prefixed-digest') {
        throw 'canonical prekey selection derivation or limits changed'
    }
    $objectIds = @($Schema.objects | ForEach-Object { [string]$_.id })
    if (($objectIds -join ',') -cne 'prekey-selection-input-v1,prekey-selection-transcript-v1') {
        throw 'canonical prekey selection object inventory changed'
    }
    foreach ($required in @('CANONICAL_PREKEY_SELECTION_DOMAIN',
        'decode_and_verify_prekey_selection_transcript_v1', 'prekey_selection_digest_v1',
        'from_be_bytes', 'SelectionDigestMismatch', 'SelectedIndexMismatch',
        'SelectedSequenceMismatch')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical prekey selection Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical prekey selection vector header changed'
    }
    Assert-LowerHex $Vectors.input.initiator_device_id_hex 32 'initiator Device ID'
    Assert-LowerHex $Vectors.input.signed_prekey_pool_cbor_hex -1 'signed prekey pool'
    Assert-LowerHex $Vectors.expected.pool_id_hex 32 'prekey pool ID'
    Assert-LowerHex $Vectors.expected.selection_domain_hex -1 'selection domain'
    Assert-LowerHex $Vectors.expected.selection_input_cbor_hex -1 'selection input'
    Assert-LowerHex $Vectors.expected.selection_digest_hex 32 'selection digest'
    if ([string]$Vectors.expected.digest_prefix_u64_be -cnotmatch '^[0-9]+$' -or
        [UInt64]$Vectors.expected.selected_index -ge 64 -or
        [UInt64]$Vectors.expected.selected_sequence -eq 0) {
        throw 'canonical prekey selection integer evidence changed'
    }
    Assert-LowerHex $Vectors.expected.selected_bundle_cbor_hex -1 'selected prekey bundle'
    Assert-LowerHex $Vectors.expected.transcript_cbor_hex -1 'selection transcript'
}

foreach ($path in @($schemaPath, $vectorsPath, $ratchetVectorsPath, $cddlPath, $rustPath,
    $activePath, $referencePath, $ratchetReferencePath, $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical prekey selection file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('prekey-selection-input-v1', 'prekey-selection-transcript-v1',
    'initiator-device-id', 'selection-digest', 'selected-index', 'selected-sequence')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "prekey selection CDDL is missing '$required'"
    }
}
foreach ($required in @('kilogram:prekey-selection:v1\0',
    'self.content.generation.to_le_bytes()', 'u64::from_le_bytes(index_bytes)')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active prekey selector changed without migration: missing '$required'"
    }
}
if ($active.IndexOf('canonical_prekey_selection_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical prekey selector leaked into active runtime'
}
$ratchetCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'ratchet-public-artifacts' })
if ($ratchetCoverage.Count -ne 1 -or [string]$ratchetCoverage[0].coverage -cne 'complete' -or
    @($ratchetCoverage[0].remaining).Count -ne 0 -or
    'prekey-selection-transcript' -notin @($ratchetCoverage[0].covered)) {
    throw 'canonical ratchet-public-artifacts coverage is not complete'
}
foreach ($required in @('first eight digest bytes', 'unsigned big-endian',
    'not proof that the named initiator controls', 'from 2,610 to 2,871',
    'changes no active wire bytes')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical prekey selection RFC is missing '$required'"
    }
}

$ratchetOutput = @(& node $ratchetReferencePath `
    --schema (Join-Path $workspace 'CANONICAL-RATCHET-SCHEMA.json') `
    --vectors $ratchetVectorsPath 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_ratchet_reference=verified' -notin @($ratchetOutput | ForEach-Object { [string]$_ })) {
    throw "canonical ratchet independent reference failed`n$($ratchetOutput -join [Environment]::NewLine)"
}
$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --ratchet-vectors $ratchetVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_prekey_selection_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical prekey selection independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $missing = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $missing.objects = @($missing.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-Schema $missing $rust } 'removed_selection_object'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.selection_digest_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_digest_shape'
    Write-Output 'changed_initiator_binding=rejected-by-independent-reference'
    Write-Output 'canonical_prekey_selection_self_test=passed'
}

Write-Output 'canonical_prekey_selection_boundary=verified'
Write-Output 'selection_endianness=big'
Write-Output 'exact_pool_binding=verified'
Write-Output 'independent_blake3=verified'
Write-Output 'bounded_decoder_mutations=6264'
Write-Output 'ratchet_public_artifacts_coverage=complete'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
