[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-PUBLICATION-CONFLICT-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-PUBLICATION-CONFLICT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-PUBLICATION-CONFLICT.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_publication_conflict_v1.rs'
$resolutionRustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_publication_resolution_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'crates\kilogram-publication-conflict\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-publication-conflict-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0148-m1122-canonical-publication-conflict-resolution.md'

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
    param([object]$Schema, [string]$Rust, [string]$ResolutionRust)
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.22' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-PUBLICATION-CONFLICT.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-publication-conflict-v1' -or
        [string]$Schema.source_compatibility.current_runtime_encoding -cne 'postcard-base64url-uri' -or
        [bool]$Schema.source_compatibility.current_runtime_changed -or
        [string]$Schema.source_compatibility.candidate_protocol_identifier -cne 'unassigned' -or
        [string]$Schema.coverage.surface -cne 'publication-conflict' -or
        [string]$Schema.coverage.status -cne 'complete' -or @($Schema.coverage.remaining).Count -ne 0) {
        throw 'canonical publication-conflict schema boundary changed'
    }
    $limits = @(
        [int]$Schema.limits.minimum_publication_ttl_seconds,
        [int]$Schema.limits.maximum_publication_ttl_seconds,
        [int]$Schema.limits.maximum_ticket_bytes,
        [int]$Schema.limits.maximum_publication_bytes,
        [int]$Schema.limits.maximum_observation_bytes,
        [int]$Schema.limits.maximum_conflict_proof_bytes,
        [int]$Schema.limits.maximum_resolution_request_bytes,
        [int]$Schema.limits.maximum_resolution_bytes,
        [int]$Schema.limits.maximum_resolution_response_bytes,
        [int]$Schema.limits.maximum_replacement_ticket_bytes,
        [int]$Schema.limits.maximum_portable_claim_bytes,
        [int]$Schema.limits.maximum_portable_claim_uri_bytes,
        [int]$Schema.limits.confirmation_code_bytes,
        [int]$Schema.limits.maximum_clock_skew_seconds
    ) -join ','
    if ($limits -cne '30,3600,8388608,16777216,4096,131072,9437184,8192,9437184,8388608,1024,3072,12,300') {
        throw 'canonical publication-conflict limits changed'
    }
    $expectedDomains = @(
        'publication-signature|kilogram:ticket-publication:cbor-v1\0',
        'publication-id|kilogram:ticket-publication-id:cbor-v1\0',
        'ticket-digest|kilogram:ticket-publication-ticket-digest:v1\0',
        'channel-id|kilogram:ticket-publication-capability-channel:v1\0',
        'observation-signature|kilogram:ticket-publication-observation:cbor-v1\0',
        'observation-id|kilogram:ticket-publication-observation-id:cbor-v1\0',
        'proof-signature|kilogram:ticket-publication-conflict-proof:cbor-v1\0',
        'proof-id|kilogram:ticket-publication-conflict-proof-id:cbor-v1\0',
        'evidence-id|kilogram:ticket-publication-conflict-evidence-id:cbor-v1\0',
        'resolution-request-signature|kilogram:publication-conflict-resolution-request:cbor-v1\0',
        'resolution-request-id|kilogram:publication-conflict-resolution-request-id:cbor-v1\0',
        'resolution-root-signature|kilogram:publication-conflict-resolution-root-signature:cbor-v1\0',
        'resolution-id|kilogram:publication-conflict-resolution-id:cbor-v1\0',
        'replacement-ticket-digest|kilogram:publication-conflict-replacement-ticket-digest:cbor-v1\0',
        'confirmation-code|kilogram:publication-conflict-confirmation:cbor-v1\0'
    )
    $actualDomains = @($Schema.domains | ForEach-Object { "$($_.id)|$($_.value)" })
    if (($actualDomains -join "`n") -cne ($expectedDomains -join "`n")) {
        throw 'canonical publication-conflict domains changed'
    }
    foreach ($required in @(
        'CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_REQUEST_SIGNATURE_DOMAIN',
        'CANONICAL_PUBLICATION_CONFLICT_RESOLUTION_SIGNATURE_DOMAIN',
        'CANONICAL_PUBLICATION_CONFLICT_CLAIM_URI_PREFIX',
        'decode_and_verify_signed_publication_conflict_resolution_request_v1',
        'verify_publication_conflict_resolution_request_authority_v1',
        'decode_and_verify_root_signed_publication_conflict_resolution_v1',
        'decode_and_verify_publication_conflict_resolution_response_v1',
        'decode_and_verify_publication_conflict_claim_uri_v1',
        'publication_conflict_confirmation_code_v1')) {
        if ($ResolutionRust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical publication-conflict resolution Rust codec is missing '$required'"
        }
    }
    foreach ($required in @(
        'CANONICAL_TICKET_PUBLICATION_SIGNATURE_DOMAIN',
        'CANONICAL_TICKET_PUBLICATION_OBSERVATION_SIGNATURE_DOMAIN',
        'CANONICAL_PUBLICATION_CONFLICT_PROOF_SIGNATURE_DOMAIN',
        'decode_and_verify_signed_ticket_publication_v1',
        'verify_ticket_publication_chain_link_v1',
        'decode_and_verify_signed_ticket_publication_observation_v1',
        'verify_ticket_publication_observation_chain_link_v1',
        'decode_and_verify_signed_publication_conflict_proof_v1',
        'publication_conflict_evidence_id_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical publication-conflict Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical publication-conflict vector header changed'
    }
    foreach ($key in $Vectors.expected.public_keys.PSObject.Properties) {
        Assert-LowerHex ([string]$key.Value) 32 $key.Name
    }
    foreach ($name in @('first_publication', 'conflicting_publication')) {
        $value = $Vectors.expected.$name
        Assert-LowerHex ([string]$value.content_cbor_hex) -1 "$name content"
        Assert-LowerHex ([string]$value.signing_preimage_hex) -1 "$name preimage"
        Assert-LowerHex ([string]$value.signature_hex) 64 "$name signature"
        Assert-LowerHex ([string]$value.signed_cbor_hex) -1 "$name signed object"
        Assert-LowerHex ([string]$value.publication_id_hex) 32 "$name ID"
        Assert-LowerHex ([string]$value.ticket_digest_hex) 32 "$name ticket digest"
    }
    foreach ($name in @('first_observation', 'conflicting_observation')) {
        $value = $Vectors.expected.$name
        Assert-LowerHex ([string]$value.content_cbor_hex) -1 "$name content"
        Assert-LowerHex ([string]$value.signature_hex) 64 "$name signature"
        Assert-LowerHex ([string]$value.signed_cbor_hex) -1 "$name signed object"
        Assert-LowerHex ([string]$value.observation_id_hex) 32 "$name ID"
    }
    foreach ($field in @('content_cbor_hex', 'signing_preimage_hex', 'signed_cbor_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.conflict_proof.$field) -1 "proof $field"
    }
    Assert-LowerHex ([string]$Vectors.expected.conflict_proof.signature_hex) 64 'proof signature'
    foreach ($field in @('proof_id_hex', 'evidence_id_hex', 'first_observation_id_hex', 'conflicting_observation_id_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.conflict_proof.$field) 32 "proof $field"
    }
    foreach ($name in @('resolution_request', 'resolution')) {
        $value = $Vectors.expected.$name
        Assert-LowerHex ([string]$value.content_cbor_hex) -1 "$name content"
        Assert-LowerHex ([string]$value.signing_preimage_hex) -1 "$name preimage"
        Assert-LowerHex ([string]$value.signature_hex) 64 "$name signature"
        Assert-LowerHex ([string]$value.signed_cbor_hex) -1 "$name signed object"
        Assert-LowerHex ([string]$value.replacement_ticket_digest_hex) 32 "$name replacement digest"
    }
    Assert-LowerHex ([string]$Vectors.expected.resolution_request.request_id_hex) 32 'resolution request ID'
    Assert-LowerHex ([string]$Vectors.expected.resolution.resolution_id_hex) 32 'resolution ID'
    Assert-LowerHex ([string]$Vectors.expected.resolution_response.cbor_hex) -1 'resolution response'
    foreach ($name in @('request_claim', 'response_claim')) {
        $value = $Vectors.expected.$name
        Assert-LowerHex ([string]$value.cbor_hex) -1 "$name CBOR"
        Assert-LowerHex ([string]$value.artifact_id_hex) 32 "$name artifact ID"
        Assert-LowerHex ([string]$value.confirmation_code_hex) 12 "$name confirmation code"
        if ([string]$value.uri -cnotlike 'kilogram://publication-conflict/cbor-v1/*' -or
            [string]$value.confirmation_code -cnotmatch '^KPC1(-[0-9A-F]{4}){6}$') {
            throw "$name portable representation changed"
        }
    }
    if ([string]$Vectors.expected.request_claim.confirmation_code_hex -cne
        [string]$Vectors.expected.response_claim.confirmation_code_hex -or
        [string]$Vectors.expected.request_claim.artifact_id_hex -ceq
        [string]$Vectors.expected.response_claim.artifact_id_hex) {
        throw 'request and response claim binding changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath, $activePath,
    $resolutionRustPath, $compatibilityPath, $coveragePath, $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical publication-conflict file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$resolutionRust = Get-Content -LiteralPath $resolutionRustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust $resolutionRust
Assert-Vectors $vectors

foreach ($required in @('ticket-publication-content-v1', 'signed-ticket-publication-v1',
    'ticket-publication-observation-content-v1', 'signed-ticket-publication-observation-v1',
    'publication-conflict-proof-content-v1', 'signed-publication-conflict-proof-v1',
    'publication-conflict-resolution-request-content-v1',
    'signed-publication-conflict-resolution-request-v1',
    'publication-conflict-resolution-content-v1',
    'root-signed-publication-conflict-resolution-v1',
    'publication-conflict-resolution-response-v1', 'publication-conflict-claim-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical publication-conflict CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_publication_conflict_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical publication-conflict module is not exported'
}
if ($lib.IndexOf('pub mod canonical_publication_resolution_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical publication-conflict resolution module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes',
    'kilogram:ticket-publication:v1\0', 'kilogram:ticket-publication-observation:v1\0',
    'kilogram:ticket-publication-conflict-proof:v1\0',
    'kilogram://publication-conflict/v1/')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active publication-conflict contract changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_publication_conflict_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('canonical_publication_resolution_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical publication-conflict leaked into active code'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'publication-conflict' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard-base64url-uri') {
    throw 'active compatibility registry no longer records Postcard/base64url publication conflict'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'publication-conflict' })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    (@($surface[0].covered) -join ',') -cne (@($schema.coverage.covered) -join ',') -or
    (@($surface[0].remaining) -join ',') -cne (@($schema.coverage.remaining) -join ',')) {
    throw 'publication-conflict complete coverage boundary changed'
}
if ([int]$mutations.expected_mutations -ne 25623 -or [int]$mutations.maximum_mutations -ne 30000) {
    throw 'publication-conflict decoder mutation bound changed'
}
$expectedMutationIds = @('signed-ticket-publication-v1',
    'signed-ticket-publication-observation-v1', 'signed-publication-conflict-proof-v1',
    'signed-publication-conflict-resolution-request-v1',
    'root-signed-publication-conflict-resolution-v1',
    'publication-conflict-resolution-response-v1',
    'publication-conflict-response-claim-v1')
$actualMutationIds = @($mutations.fixtures | Where-Object { $_.vector_file -ceq 'CANONICAL-PUBLICATION-CONFLICT-VECTORS.json' } | ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'publication-conflict decoder mutation fixtures changed'
}
foreach ($required in @('complete `publication-conflict`', 'current Root-signed device list',
    'Postcard/base64url', 'does not activate', 'portable claim', 'KPC1')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical publication-conflict RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_publication_conflict_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_blake3=verified' -notin @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_ed25519=verified' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical publication-conflict independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $downgraded = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $downgraded.coverage.status = 'partial'
    Assert-Rejected { Assert-Schema $downgraded $rust $resolutionRust } 'downgraded_coverage'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_observation_bytes = 4097
    Assert-Rejected { Assert-Schema $changedLimit $rust $resolutionRust } 'changed_observation_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedVector.expected.conflict_proof.evidence_id_hex = '00'
    Assert-Rejected { Assert-Vectors $changedVector } 'invalid_evidence_id_shape'
    Write-Output 'canonical_publication_conflict_self_test=passed'
}

Write-Output 'canonical_publication_conflict_boundary=verified'
Write-Output 'publication_conflict_coverage=complete'
Write-Output 'publication_conflict_resolution=verified'
Write-Output 'portable_confirmation_claim=verified'
Write-Output 'independent_blake3=verified'
Write-Output 'independent_ed25519=verified'
Write-Output 'bounded_decoder_mutations=25623'
Write-Output 'active_postcard_base64url_changed=false'
Write-Output 'network_executed=false'
