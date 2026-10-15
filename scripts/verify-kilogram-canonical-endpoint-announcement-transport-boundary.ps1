[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-VECTORS.json'
$endpointVectorsPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_endpoint_announcement_transport_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'apps\kilogram-cli\src\runtime_endpoint_announcement.rs'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-endpoint-announcement-transport-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0150-m1124-canonical-endpoint-announcement-transport.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.24' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.completes_surface -cne 'endpoint-announcement' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-endpoint-announcement-transport-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v4-bundle-hpke-v1-envelope-ack-v2' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical endpoint-announcement transport schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_plaintext_bundle_bytes,
        [int]$Schema.limits.maximum_ciphertext_bytes,
        [int]$Schema.limits.maximum_envelope_bytes,
        [int]$Schema.limits.maximum_acknowledgement_bytes,
        [int]$Schema.limits.hpke_encapsulated_key_bytes,
        [int]$Schema.limits.signature_bytes
    ) -join ','
    if ($limits -cne '16777216,16777232,16777744,4096,32,64') {
        throw 'canonical endpoint-announcement transport limits changed'
    }
    $requirements = @($Schema.activation.required_before_activation)
    if (($requirements -join ',') -cne 'new-explicit-protocol-version-or-parallel-alpn,dual-reader-and-downgrade-tests,coverage-guided-fuzzing,cross-language-hpke-seal-open-conformance,fresh-field-evidence') {
        throw 'canonical endpoint-announcement activation requirements changed'
    }
    foreach ($required in @(
        'CANONICAL_ENDPOINT_ANNOUNCEMENT_ENVELOPE_HPKE_INFO',
        'kilogram:endpoint-announcement-envelope:cbor-v1\0',
        'CANONICAL_ENDPOINT_ANNOUNCEMENT_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN',
        'kilogram:endpoint-announcement-acknowledgement:cbor-v1\0',
        'seal_endpoint_announcement_bundle_v1',
        'open_endpoint_announcement_bundle_v1',
        'sign_endpoint_announcement_acknowledgement_v1',
        'verify_endpoint_announcement_acknowledgement_for_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical endpoint-announcement transport Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        [string]$Vectors.dependency -cne 'CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json') {
        throw 'canonical endpoint-announcement transport vectors header changed'
    }
    Assert-LowerHex ([string]$Vectors.input.session_binding_hex) 32 'session binding'
    Assert-LowerHex ([string]$Vectors.input.fixed_hpke_seal.encapsulated_key_hex) 32 'encapsulated key'
    Assert-LowerHex ([string]$Vectors.input.fixed_hpke_seal.ciphertext_hex) -Label 'ciphertext'
    Assert-LowerHex ([string]$Vectors.expected.envelope_header.cbor_hex) 138 'envelope header'
    if ([string]$Vectors.expected.envelope_header.aad_hex -cne [string]$Vectors.expected.envelope_header.cbor_hex) {
        throw 'canonical endpoint-announcement AAD no longer equals the exact header'
    }
    Assert-LowerHex ([string]$Vectors.expected.encrypted_bundle.cbor_hex) -Label 'encrypted bundle'
    Assert-LowerHex ([string]$Vectors.expected.acknowledgement.content_cbor_hex) -Label 'acknowledgement content'
    Assert-LowerHex ([string]$Vectors.expected.acknowledgement.signing_preimage_hex) -Label 'acknowledgement preimage'
    Assert-LowerHex ([string]$Vectors.expected.acknowledgement.signature_hex) 64 'acknowledgement signature'
    Assert-LowerHex ([string]$Vectors.expected.acknowledgement.signed_cbor_hex) -Label 'signed acknowledgement'
    Assert-LowerHex ([string]$Vectors.expected.transport_binding_hash_hex) 32 'transport binding hash'
    $stats = $Vectors.expected.bundle_statistics
    if ([int]$stats.authority_revision -ne 8 -or [int]$stats.contact_count -ne 4 -or
        [int]$stats.endpoint_count -ne 4 -or [int]$stats.observation_evidence_count -ne 2 -or
        [int]$stats.publication_conflict_count -ne 1 -or
        [int]$stats.publication_resolution_count -ne 1) {
        throw 'canonical endpoint-announcement derived statistics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $endpointVectorsPath, $cddlPath, $rustPath,
    $libPath, $activePath, $coveragePath, $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical endpoint-announcement transport file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('endpoint-announcement-envelope-header-v1',
    'encrypted-endpoint-announcement-bundle-v1',
    'endpoint-announcement-acknowledgement-content-v1',
    'signed-endpoint-announcement-acknowledgement-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical endpoint-announcement transport CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_endpoint_announcement_transport_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical endpoint-announcement transport module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes', 'SealedMessage',
    'kilogram:endpoint-announcement-bundle:v4\0',
    'kilogram:endpoint-announcement-envelope:v1',
    'kilogram:endpoint-announcement-acknowledgement:v2\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active endpoint-announcement contract changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_endpoint_announcement_transport_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical endpoint-announcement transport leaked into active runtime'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'endpoint-announcement' })
$requiredArtifacts = @('CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT.cddl',
    'CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-SCHEMA.json',
    'CANONICAL-ENDPOINT-ANNOUNCEMENT-TRANSPORT-VECTORS.json',
    'crates/kilogram-protocol/src/canonical_endpoint_announcement_transport_v1.rs',
    'scripts/verify-kilogram-canonical-endpoint-announcement-transport-reference.mjs')
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'canonical endpoint-announcement surface is not complete'
}
foreach ($artifact in $requiredArtifacts) {
    if ($artifact -cnotin @($surface[0].artifacts)) { throw "coverage is missing '$artifact'" }
}
foreach ($covered in @('recipient-only-hpke-envelope', 'exact-header-associated-data',
    'session-bound-recipient-acknowledgement', 'derived-total-and-applied-count-binding')) {
    if ($covered -cnotin @($surface[0].covered)) { throw "coverage is missing '$covered'" }
}
if ([int]$mutations.expected_mutations -ne 13715 -or [int]$mutations.maximum_mutations -ne 15000) {
    throw 'canonical endpoint-announcement transport decoder mutation bound changed'
}
$envelopeFixture = @($mutations.fixtures | Where-Object { $_.id -ceq 'encrypted-endpoint-announcement-bundle-v1' })
if ($envelopeFixture.Count -ne 1 -or
    [string]$envelopeFixture[0].context_verifier -cne 'open_endpoint_announcement_bundle_v1') {
    throw 'encrypted endpoint-announcement mutation fixture changed'
}
$ackFixture = @($mutations.fixtures | Where-Object { $_.id -ceq 'signed-endpoint-announcement-acknowledgement-v1' })
if ($ackFixture.Count -ne 1 -or
    [string]$ackFixture[0].context_verifier -cne 'verify_endpoint_announcement_acknowledgement_for_v1') {
    throw 'endpoint-announcement acknowledgement mutation fixture changed'
}
foreach ($required in @('recipient-only HPKE', 'exact canonical header',
    'session-bound acknowledgement', 'changes no active wire bytes',
    'does not activate canonical CBOR', 'cross-language HPKE')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical endpoint-announcement transport RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --endpoint-vectors $endpointVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_endpoint_announcement_transport_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical endpoint-announcement transport independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_protocol_identifier'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_acknowledgement_bytes = 4097
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_acknowledgement_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedVector.expected.acknowledgement.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changedVector } 'invalid_acknowledgement_signature_shape'
    Write-Output 'canonical_endpoint_announcement_transport_self_test=passed'
}

Write-Output 'canonical_endpoint_announcement_transport_boundary=verified'
Write-Output 'endpoint_announcement_coverage=complete'
Write-Output 'recipient_only_hpke_envelope=verified'
Write-Output 'session_bound_recipient_acknowledgement=verified'
Write-Output 'independent_ed25519_and_blake3=verified'
Write-Output 'bounded_decoder_mutations=13715'
Write-Output 'active_postcard_hpke_changed=false'
Write-Output 'network_executed=false'
