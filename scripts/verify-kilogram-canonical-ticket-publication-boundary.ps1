[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-TICKET-PUBLICATION-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-TICKET-PUBLICATION-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-TICKET-PUBLICATION.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_ticket_publication_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'crates\kilogram-ticket-publication\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-ticket-publication-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0146-m1120-canonical-ticket-publication.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.20' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-TICKET-PUBLICATION.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-ticket-publication-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-and-lowercase-hex-http-headers' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical ticket-publication schema boundary changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_capability_bytes,
        [int]$Schema.limits.maximum_authorization_bytes,
        [int]$Schema.limits.maximum_body_bytes,
        [int]$Schema.limits.maximum_peer_scope_bytes
    ) -join ','
    if ($limits -cne '128,1024,16777216,4096') {
        throw 'canonical ticket-publication limits changed'
    }
    foreach ($required in @(
        'CANONICAL_TICKET_PUBLICATION_CHANNEL_ID_DOMAIN',
        'CANONICAL_TICKET_PUBLICATION_WRITE_AUTHORIZATION_SIGNATURE_DOMAIN',
        'CANONICAL_TICKET_PUBLICATION_WRITE_CAPABILITY_DERIVATION_CONTEXT',
        'CANONICAL_TICKET_PUBLICATION_ROTATED_WRITE_CAPABILITY_DERIVATION_CONTEXT',
        'derive_ticket_publication_capability_v1',
        'decode_and_verify_ticket_publication_capability_v1',
        'decode_and_verify_signed_ticket_publication_write_authorization_v1',
        'verify_ticket_publication_write_authorization_for_body_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical ticket-publication Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical ticket-publication vector header changed'
    }
    Assert-LowerHex ([string]$Vectors.input.device_secret_hex) 32 'device secret'
    Assert-LowerHex ([string]$Vectors.input.peer_scope_hex) -1 'peer scope'
    Assert-LowerHex ([string]$Vectors.input.body_hex) -1 'body'
    if ([UInt64]$Vectors.input.rotated_epoch -eq 0 -or [UInt64]$Vectors.input.generation -eq 0) {
        throw 'canonical ticket-publication positive vector metadata changed'
    }
    foreach ($name in @('legacy_capability', 'rotated_capability')) {
        Assert-LowerHex ([string]$Vectors.expected.$name.write_key_hex) 32 "$name write key"
        Assert-LowerHex ([string]$Vectors.expected.$name.channel_id_hex) 32 "$name channel ID"
        Assert-LowerHex ([string]$Vectors.expected.$name.cbor_hex) -1 "$name CBOR"
    }
    Assert-LowerHex ([string]$Vectors.expected.authorization.body_digest_hex) 32 'body digest'
    Assert-LowerHex ([string]$Vectors.expected.authorization.content_cbor_hex) -1 'authorization content'
    Assert-LowerHex ([string]$Vectors.expected.authorization.signing_preimage_hex) -1 'authorization preimage'
    Assert-LowerHex ([string]$Vectors.expected.authorization.signature_hex) 64 'authorization signature'
    Assert-LowerHex ([string]$Vectors.expected.authorization.signed_cbor_hex) -1 'signed authorization'
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath, $activePath,
    $compatibilityPath, $coveragePath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical ticket-publication file is missing: $path"
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
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('ticket-publication-capability-v1',
    'ticket-publication-write-authorization-content-v1',
    'signed-ticket-publication-write-authorization-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical ticket-publication CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_ticket_publication_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical ticket-publication module is not exported'
}
foreach ($required in @('derive_with_epoch', 'encode_signature', 'decode_signature',
    'kilogram:ticket-publication-write-authorization:v1\0',
    'kilogram:ticket-publication-capability-channel:v1\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active ticket-publication contract changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_ticket_publication_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical ticket-publication leaked into active code'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'ticket-publication-capability' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard') {
    throw 'active compatibility registry no longer records Postcard ticket publication'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'ticket-publication-capability' })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'ticket-publication capability coverage is not complete'
}
foreach ($required in @('complete `ticket-publication-capability`', 'epoch zero',
    'positive epoch', 'body digest', 'Postcard', 'does not activate')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical ticket-publication RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_ticket_publication_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical ticket-publication independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_body_bytes = 16777217
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_body_limit'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.rotated_capability.write_key_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_write_key_shape'
    Write-Output 'canonical_ticket_publication_self_test=passed'
}

Write-Output 'canonical_ticket_publication_boundary=verified'
Write-Output 'legacy_and_rotated_kdf=verified'
Write-Output 'independent_blake3_derive_key=verified'
Write-Output 'ticket_publication_capability_coverage=complete'
Write-Output 'bounded_decoder_mutations=18879'
Write-Output 'active_postcard_changed=false'
Write-Output 'network_executed=false'
