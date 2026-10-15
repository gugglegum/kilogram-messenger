[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json'
$conflictVectorsPath = Join-Path $workspace 'CANONICAL-PUBLICATION-CONFLICT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-ENDPOINT-ANNOUNCEMENT.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_endpoint_announcement_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'apps\kilogram-cli\src\runtime_endpoint_announcement.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-endpoint-announcement-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0149-m1123-canonical-endpoint-announcement-plaintext.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.23' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.coverage -cne 'partial' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-ENDPOINT-ANNOUNCEMENT.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-endpoint-announcement-plaintext-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v4-bundle-hpke-v1-envelope-ack-v2' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical endpoint-announcement schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_bundle_bytes,
        [int]$Schema.limits.maximum_accepted_observation_bytes,
        [int]$Schema.limits.maximum_contacts,
        [int]$Schema.limits.maximum_endpoints_per_contact,
        [int]$Schema.limits.maximum_conversation_label_bytes,
        [int]$Schema.limits.maximum_ticket_bytes,
        [int]$Schema.limits.maximum_validity_seconds,
        [int]$Schema.limits.maximum_clock_skew_seconds
    ) -join ','
    if ($limits -cne '16777216,65536,256,4,4096,8388608,3600,300') {
        throw 'canonical endpoint-announcement limits changed'
    }
    $remaining = @($Schema.remaining)
    if (($remaining -join ',') -cne 'recipient-only-hpke-envelope,session-bound-recipient-acknowledgement') {
        throw 'canonical endpoint-announcement remaining coverage changed'
    }
    foreach ($required in @(
        'MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_BYTES: usize = 16 * 1024 * 1024',
        'MAX_CANONICAL_ACCEPTED_ENDPOINT_OBSERVATION_BYTES: usize = 64 * 1024',
        'MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_CONTACTS: usize = 256',
        'MAX_CANONICAL_ENDPOINTS_PER_ANNOUNCEMENT_CONTACT: usize = 4',
        'MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_LABEL_BYTES: usize = 4096',
        'MAX_CANONICAL_ENDPOINT_ANNOUNCEMENT_TICKET_BYTES: usize = 8 * 1024 * 1024',
        'kilogram:endpoint-announcement-bundle:cbor-v1\0',
        'kilogram:endpoint-announcement-bundle-id:cbor-v1\0',
        'kilogram:accepted-endpoint-observation:cbor-v1\0',
        'kilogram:accepted-endpoint-observation-id:cbor-v1\0',
        'decode_and_verify_signed_accepted_endpoint_observation_v1',
        'decode_and_verify_signed_endpoint_announcement_bundle_v1',
        'verify_endpoint_announcement_bundle_at_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical endpoint-announcement Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        (@($Vectors.dependencies) -join ',') -cne 'CANONICAL-PUBLICATION-CONFLICT-VECTORS.json') {
        throw 'canonical endpoint-announcement vectors header changed'
    }
    foreach ($entry in @($Vectors.expected.accepted_observation, $Vectors.expected.signed_bundle)) {
        Assert-LowerHex ([string]$entry.content_cbor_hex) -Label 'content CBOR'
        Assert-LowerHex ([string]$entry.signing_preimage_hex) -Label 'signing preimage'
        Assert-LowerHex ([string]$entry.signature_hex) 64 'signature'
        Assert-LowerHex ([string]$entry.signed_cbor_hex) -Label 'signed CBOR'
        Assert-LowerHex ([string]$entry.object_id_hex) 32 'object ID'
    }
    $created = [UInt64]$Vectors.input.created_at_unix_seconds
    $expires = [UInt64]$Vectors.input.expires_at_unix_seconds
    if ($expires -le $created -or ($expires - $created) -gt 3600) {
        throw 'canonical endpoint-announcement vector validity changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $conflictVectorsPath, $cddlPath, $rustPath,
    $libPath, $activePath, $compatibilityPath, $coveragePath, $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical endpoint-announcement file is missing: $path"
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

foreach ($required in @('signed-accepted-endpoint-observation-v1',
    'endpoint-announcement-state-v1', 'endpoint-candidate-announcement-v1',
    'contact-endpoint-announcement-v1', 'signed-endpoint-announcement-bundle-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical endpoint-announcement CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_endpoint_announcement_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical endpoint-announcement module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes', 'SealedMessage',
    'kilogram:endpoint-announcement-bundle:v4\0',
    'kilogram:endpoint-announcement-envelope:v1',
    'kilogram:endpoint-announcement-acknowledgement:v2\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active endpoint-announcement contract changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_endpoint_announcement_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical endpoint-announcement leaked into active runtime'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'endpoint-announcement' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard') {
    throw 'active compatibility registry no longer records Postcard endpoint announcement'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'endpoint-announcement' })
$expectedCovered = @('signed-plaintext-bundle', 'exact-current-account-device-list',
    'direct-and-accepted-publication-observations', 'publication-conflict-and-root-resolution-states',
    'bundle-and-accepted-observation-identifiers', 'bounded-validity-and-canonical-contact-order',
    'recipient-only-hpke-envelope', 'exact-header-associated-data',
    'session-bound-recipient-acknowledgement', 'derived-total-and-applied-count-binding')
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'canonical endpoint-announcement aggregate coverage boundary changed'
}
if ([int]$mutations.expected_mutations -ne 21451 -or [int]$mutations.maximum_mutations -ne 25000) {
    throw 'canonical endpoint-announcement decoder mutation bound changed'
}
$expectedMutationIds = @('signed-accepted-endpoint-observation-v1',
    'signed-endpoint-announcement-bundle-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-ENDPOINT-ANNOUNCEMENT-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical endpoint-announcement mutation fixtures changed'
}
foreach ($required in @('partial `endpoint-announcement`', 'exact current Root-signed device list',
    'recipient-only HPKE envelope', 'session-bound recipient acknowledgement',
    'active Postcard', 'does not activate')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical endpoint-announcement RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --conflict-vectors $conflictVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_endpoint_announcement_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical endpoint-announcement independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_complete_surface'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_contacts = 257
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_contact_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedVector.expected.signed_bundle.object_id_hex = '00'
    Assert-Rejected { Assert-Vectors $changedVector } 'invalid_bundle_id_shape'
    Write-Output 'canonical_endpoint_announcement_self_test=passed'
}

Write-Output 'canonical_endpoint_announcement_boundary=verified'
Write-Output 'endpoint_announcement_plaintext_coverage=partial'
Write-Output 'endpoint_announcement_aggregate_coverage=complete'
Write-Output 'plaintext_bundle_and_evidence=verified'
Write-Output 'hpke_envelope_covered=false'
Write-Output 'session_acknowledgement_covered=false'
Write-Output 'independent_ed25519_and_blake3=verified'
Write-Output 'bounded_decoder_mutations=21451'
Write-Output 'active_postcard_hpke_changed=false'
Write-Output 'network_executed=false'
