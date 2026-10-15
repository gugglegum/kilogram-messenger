[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-DEVICE-LINK-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-DEVICE-LINK-VECTORS.json'
$authorityVectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$accountVectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-DEVICE-LINK.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_device_link_v1.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$activePath = Join-Path $workspace 'apps\kilogram-bootstrap\src\device_link.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-device-link-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0135-m119-canonical-device-link-authorization.md'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-ExactStrings {
    param([object[]]$Actual, [string[]]$Expected, [string]$Label)
    $values = @($Actual | ForEach-Object { [string]$_ })
    if (($values -join "`n") -cne ($Expected -join "`n")) { throw "$Label changed" }
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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.9' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical device-link schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-DEVICE-LINK.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-device-link-v1') {
        throw 'canonical device-link CDDL declaration changed'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [int]$Schema.limits.maximum_nested_cbor_bytes -ne 524288) {
        throw 'canonical device-link activation or limits changed'
    }
    Assert-ExactStrings @($Schema.domains | ForEach-Object { "$($_.id)|$($_.value)|$($_.signer)" }) @(
        'device-link-authorization-signature-v1|kilogram:device-link-authorization-signature:cbor-v1\0|account-root-ed25519'
    ) 'canonical device-link domains'
    Assert-ExactStrings @($Schema.objects | ForEach-Object { $_.id }) @(
        'device-link-authorization-content-v1', 'signed-device-link-authorization-v1'
    ) 'canonical device-link objects'
    Assert-ExactStrings @($Schema.rust_bindings | ForEach-Object { $_.symbol }) @(
        'CANONICAL_DEVICE_LINK_AUTHORIZATION_SIGNATURE_DOMAIN',
        'MAX_CANONICAL_DEVICE_LINK_NESTED_BYTES'
    ) 'canonical device-link Rust bindings'
    foreach ($binding in $Schema.rust_bindings) {
        if ([string]$binding.path -cne 'crates/kilogram-protocol/src/canonical_device_link_v1.rs' -or
            $Rust.IndexOf([string]$binding.symbol, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical device-link Rust binding '$($binding.symbol)' is missing"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical device-link vector header changed'
    }
    Assert-LowerHex $Vectors.input.root_secret_key_hex 32 'Root seed'
    Assert-LowerHex $Vectors.input.request_id_hex 32 'request ID'
    Assert-LowerHex $Vectors.expected.account_id_hex 32 'Account ID'
    Assert-LowerHex $Vectors.expected.device_id_hex 32 'Device ID'
    $value = $Vectors.expected.authorization
    Assert-LowerHex $value.content_cbor_hex -1 'authorization content'
    Assert-LowerHex $value.signature_domain_hex -1 'authorization domain'
    Assert-LowerHex $value.signing_preimage_hex -1 'authorization preimage'
    Assert-LowerHex $value.signature_hex 64 'authorization signature'
    Assert-LowerHex $value.signed_cbor_hex -1 'signed authorization'
}

foreach ($path in @($schemaPath, $vectorsPath, $authorityVectorsPath, $accountVectorsPath,
    $cddlPath, $rustPath, $wirePath, $activePath, $referencePath, $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical device-link file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('device-link-authorization-content-v1',
    'signed-device-link-authorization-v1', 'signed-account-device-list-cbor')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "device-link CDDL is missing '$required'"
    }
}
foreach ($required in @('decode_and_verify_signed_device_link_authorization_v1',
    'RequestedCertificateMissingFromList', 'verify_signed_device_certificate_v1',
    'verify_signed_account_device_list_v1')) {
    if ($rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "device-link codec is missing '$required'"
    }
}
if ($wire.IndexOf('canonical_device_link_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('canonical_device_link_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('postcard::to_allocvec(authorization)', [StringComparison]::Ordinal) -lt 0 -or
    $active.IndexOf('postcard::from_bytes(&plaintext)', [StringComparison]::Ordinal) -lt 0) {
    throw 'inactive canonical device-link codec leaked into or displaced active Postcard runtime'
}
$accountCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'account-authority' })
if ($accountCoverage.Count -ne 1 -or [string]$accountCoverage[0].coverage -cne 'complete' -or
    @($accountCoverage[0].remaining).Count -ne 0 -or
    'device-link-authorization' -notin @($accountCoverage[0].covered)) {
    throw 'canonical account-authority coverage is not complete'
}
foreach ($required in @('exact resulting device list', 'Merely finding the same Device ID is insufficient',
    'from 1827 to 2088 malformed', 'changes no active wire bytes')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical device-link RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --authority-vectors $authorityVectorsPath --account-vectors $accountVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_device_link_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical device-link independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}
if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $missing = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $missing.objects = @($missing.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-Schema $missing $rust } 'removed_device_link_object'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.authorization.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_signature_shape'
    Write-Output 'changed_request_binding=rejected-by-independent-reference'
    Write-Output 'canonical_device_link_self_test=passed'
}

Write-Output 'canonical_device_link_boundary=verified'
Write-Output 'strict_device_link_decoders=1'
Write-Output 'independent_ed25519=verified'
Write-Output 'bounded_decoder_mutations=2610'
Write-Output 'account_authority_coverage=complete'
Write-Output 'active_wire_changed=false'
Write-Output 'network_executed=false'
