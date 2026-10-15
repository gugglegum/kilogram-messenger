[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RATCHET-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RATCHET-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RATCHET.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_ratchet_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-ratchet-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0129-m113-canonical-ratchet-bootstrap.md'

function Assert-ExactStringSet {
    param([string[]]$Actual, [string[]]$Expected, [string]$Label)
    $actualSorted = @($Actual | Sort-Object)
    $expectedSorted = @($Expected | Sort-Object)
    if ($actualSorted.Count -ne $expectedSorted.Count) {
        throw "$Label count mismatch: expected $($expectedSorted.Count), got $($actualSorted.Count)"
    }
    for ($index = 0; $index -lt $expectedSorted.Count; $index++) {
        if ($actualSorted[$index] -cne $expectedSorted[$index]) {
            throw "$Label mismatch at index ${index}: expected '$($expectedSorted[$index])', got '$($actualSorted[$index])'"
        }
    }
}

function Assert-ExactPropertySet {
    param([object]$Value, [string[]]$Expected, [string]$Label)
    $actual = @($Value.PSObject.Properties | ForEach-Object { $_.Name })
    Assert-ExactStringSet $actual $Expected "$Label properties"
}

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action }
    catch {
        Write-Output "$Label=rejected"
        return
    }
    throw "$Label was unexpectedly accepted"
}

function Assert-CanonicalHex {
    param([string]$Value, [int]$ExpectedBytes = -1, [string]$Label = 'hex value')
    if ([string]::IsNullOrEmpty($Value) -or
        $Value.Length % 2 -ne 0 -or
        $Value -cnotmatch '^[0-9a-f]+$') {
        throw "$Label is not non-empty lowercase hexadecimal"
    }
    if ($ExpectedBytes -ge 0 -and $Value.Length -ne $ExpectedBytes * 2) {
        throw "$Label does not contain $ExpectedBytes bytes"
    }
}

function Assert-Schema {
    param([object]$Schema, [string]$RustSource)
    Assert-ExactPropertySet $Schema @(
        'schema_version', 'milestone', 'status', 'changes_active_wire', 'profile',
        'cddl', 'activation', 'limits', 'external_objects', 'domains', 'rust_bindings', 'objects'
    ) 'canonical ratchet schema'
    if ([UInt64]$Schema.schema_version -ne 1 -or
        [string]$Schema.milestone -cne 'M1.1.3' -or
        [string]$Schema.status -cne 'candidate-not-active' -or
        [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical ratchet schema header changed or claims activation'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical ratchet schema changed the active runtime'
    }
    Assert-ExactStringSet @($Schema.activation.required_before_activation) @(
        'new-explicit-protocol-version-or-parallel-identifier',
        'dual-reader-and-downgrade-tests',
        'decoder-fuzzing-and-malformed-input-corpus',
        'fresh-field-and-hosted-evidence'
    ) 'ratchet activation requirements'
    if ([int]$Schema.limits.maximum_prekeys_per_pool -ne 64 -or
        [int]$Schema.limits.maximum_devices_per_directory -ne 32 -or
        [UInt64]$Schema.limits.maximum_prekey_validity_seconds -ne 2592000 -or
        [int]$Schema.limits.maximum_nested_cbor_bytes -ne 524288) {
        throw 'canonical ratchet limits changed'
    }

    $expectedDomains = @(
        'ratchet-identity-signature-v1|kilogram:ratchet-identity-signature:cbor-v1\0|ed25519-signature|device-ed25519',
        'prekey-bundle-signature-v1|kilogram:prekey-bundle-signature:cbor-v1\0|ed25519-signature|device-ed25519',
        'prekey-pool-signature-v1|kilogram:prekey-pool-signature:cbor-v1\0|ed25519-signature|device-ed25519',
        'prekey-pool-id-v1|kilogram:prekey-pool-id:cbor-v1\0|blake3-domain-prefixed-digest|'
    )
    $actualDomains = @($Schema.domains | ForEach-Object {
        $signer = if ($_.PSObject.Properties.Name -contains 'signer') { [string]$_.signer } else { '' }
        "$($_.id)|$($_.value)|$($_.operation)|$signer"
    })
    if ($actualDomains.Count -ne $expectedDomains.Count) { throw 'canonical ratchet domain count changed' }
    for ($index = 0; $index -lt $expectedDomains.Count; $index++) {
        if ($actualDomains[$index] -cne $expectedDomains[$index]) {
            throw "canonical ratchet domain changed at index $index"
        }
    }

    $expectedBindings = [ordered]@{
        'CANONICAL_RATCHET_IDENTITY_SIGNATURE_DOMAIN' = @('bytes', 'kilogram:ratchet-identity-signature:cbor-v1\0')
        'CANONICAL_PREKEY_BUNDLE_SIGNATURE_DOMAIN' = @('bytes', 'kilogram:prekey-bundle-signature:cbor-v1\0')
        'CANONICAL_PREKEY_POOL_SIGNATURE_DOMAIN' = @('bytes', 'kilogram:prekey-pool-signature:cbor-v1\0')
        'CANONICAL_PREKEY_POOL_ID_DOMAIN' = @('bytes', 'kilogram:prekey-pool-id:cbor-v1\0')
        'MAX_CANONICAL_PREKEY_POOL_SIZE' = @('usize', '64')
        'MAX_CANONICAL_PREKEY_DIRECTORY_DEVICES' = @('usize', '32')
        'MAX_CANONICAL_PREKEY_VALIDITY_SECONDS' = @('u64', '2592000')
        'MAX_CANONICAL_NESTED_RATCHET_BYTES' = @('usize', '524288')
    }
    $actualSymbols = @($Schema.rust_bindings | ForEach-Object { [string]$_.symbol })
    Assert-ExactStringSet $actualSymbols @($expectedBindings.Keys) 'canonical ratchet Rust bindings'
    foreach ($binding in $Schema.rust_bindings) {
        if ([string]$binding.path -cne 'crates/kilogram-protocol/src/canonical_ratchet_v1.rs') {
            throw "unexpected Rust binding path '$($binding.path)'"
        }
        $expected = $expectedBindings[[string]$binding.symbol]
        if ($null -eq $expected -or [string]$binding.rust_type -cne $expected[0] -or
            [string]$binding.value -cne $expected[1]) {
            throw "Rust binding '$($binding.symbol)' changed"
        }
        $symbol = [regex]::Escape([string]$binding.symbol)
        if ([string]$binding.rust_type -ceq 'bytes') {
            $value = [regex]::Escape([string]$binding.value)
            $pattern = "(?ms)^pub\s+const\s+$symbol\s*:\s*&\[u8\]\s*=\s*b`"$value`"\s*;"
        }
        else {
            $value = [regex]::Escape([string]$binding.value)
            $type = [regex]::Escape([string]$binding.rust_type)
            $pattern = "(?m)^pub\s+const\s+$symbol\s*:\s*$type\s*=\s*(?:30 \* 24 \* 60 \* 60|512 \* 1024|$value)\s*;"
        }
        if ([regex]::Matches($RustSource, $pattern).Count -ne 1) {
            throw "Rust binding '$($binding.symbol)' does not have one exact declaration"
        }
    }

    $expectedObjects = @(
        'ratchet-identity-content-v1', 'signed-ratchet-identity-v1',
        'prekey-bundle-content-v1', 'signed-prekey-bundle-v1',
        'prekey-pool-content-v1', 'signed-prekey-pool-v1',
        'prekey-pool-id-v1', 'account-prekey-directory-v1'
    )
    $actualObjects = @($Schema.objects | ForEach-Object { [string]$_.id })
    if ($actualObjects.Count -ne $expectedObjects.Count) { throw 'canonical ratchet object count changed' }
    for ($index = 0; $index -lt $expectedObjects.Count; $index++) {
        if ($actualObjects[$index] -cne $expectedObjects[$index]) {
            throw "canonical ratchet object order changed at index $index"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical ratchet vector header changed'
    }
    foreach ($property in @($Vectors.input.PSObject.Properties)) {
        if ($property.Name -like '*_hex') { Assert-CanonicalHex ([string]$property.Value) -Label $property.Name }
        if ($property.Name -like '*_hexes') {
            foreach ($value in @($property.Value)) { Assert-CanonicalHex ([string]$value) -Label $property.Name }
        }
    }
    Assert-CanonicalHex ([string]$Vectors.expected.account_id_hex) 32 'Account ID'
    Assert-CanonicalHex ([string]$Vectors.expected.device_id_hex) 32 'Device ID'
    foreach ($name in @('certificate', 'authority_snapshot', 'ratchet_identity', 'prekey_pool')) {
        $value = $Vectors.expected.$name
        foreach ($field in @('content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex', 'signature_hex', 'signed_cbor_hex')) {
            Assert-CanonicalHex ([string]$value.$field) -Label "$name.$field"
        }
        Assert-CanonicalHex ([string]$value.signature_hex) 64 "$name.signature"
    }
    if (@($Vectors.expected.prekey_bundles).Count -ne 2) { throw 'expected exactly two prekey bundle vectors' }
    foreach ($bundle in @($Vectors.expected.prekey_bundles)) {
        foreach ($field in @('content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex', 'signature_hex', 'signed_cbor_hex')) {
            Assert-CanonicalHex ([string]$bundle.$field) -Label "bundle.$field"
        }
        Assert-CanonicalHex ([string]$bundle.signature_hex) 64 'bundle.signature'
    }
    Assert-CanonicalHex ([string]$Vectors.expected.prekey_pool_id_domain_hex) -Label 'pool ID domain'
    Assert-CanonicalHex ([string]$Vectors.expected.prekey_pool_id_hex) 32 'pool ID'
    Assert-CanonicalHex ([string]$Vectors.expected.account_prekey_directory_cbor_hex) -Label 'directory CBOR'
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath, $wirePath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical ratchet file is missing: $path" }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Schema $schema $rust
Assert-Vectors $vectors
foreach ($required in @(
    'kilogram-ratchet-v1', 'signed-ratchet-identity-v1', 'signed-prekey-bundle-v1',
    'signed-prekey-pool-v1', 'account-prekey-directory-v1', '[1*64 canonical-signed-object-v1]',
    '[1*32 canonical-signed-object-v1]'
)) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "CDDL is missing '$required'" }
}
foreach ($required in @(
    'pub fn decode_and_verify_signed_ratchet_identity_v1',
    'pub fn decode_and_verify_signed_prekey_bundle_v1',
    'pub fn decode_and_verify_signed_prekey_pool_v1',
    'pub fn decode_and_verify_account_prekey_directory_v1',
    'pub fn prekey_pool_id_v1'
)) {
    if ($rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "Rust codec is missing '$required'" }
}
if ($lib.IndexOf('pub mod canonical_ratchet_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical ratchet module is not exported'
}
if ($wire.IndexOf('canonical_ratchet_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'active wire codec references the inactive canonical ratchet module'
}
foreach ($required in @('Secret Double Ratchet state', 'continues to use the versioned Postcard objects', 'changes no ALPN')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) { throw "RFC is missing '$required'" }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath 2>&1)
if ($LASTEXITCODE -ne 0 -or 'canonical_ratchet_reference=verified' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "independent canonical ratchet verifier failed.`n$($referenceOutput -join [Environment]::NewLine)"
}
$referenceSelfTest = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or 'canonical_ratchet_reference_self_test=passed' -notin @($referenceSelfTest | ForEach-Object { [string]$_ })) {
    throw "independent canonical ratchet self-test failed.`n$($referenceSelfTest -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $active = $schema | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $active.status = 'active'
    Assert-Rejected { Assert-Schema $active $rust } 'active_without_migration'

    $removed = $schema | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $removed.objects = @($removed.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-Schema $removed $rust } 'removed_object'

    $changed = $schema | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $changed.rust_bindings[0].value = 'kilogram:changed\0'
    Assert-Rejected { Assert-Schema $changed $rust } 'changed_rust_binding'

    $uppercase = $vectors | ConvertTo-Json -Depth 100 | ConvertFrom-Json
    $uppercase.input.curve25519_key_hex = ([string]$uppercase.input.curve25519_key_hex).ToUpperInvariant()
    Assert-Rejected { Assert-Vectors $uppercase } 'uppercase_hex'
    Write-Output 'canonical_ratchet_boundary_self_test=passed'
    exit 0
}

Write-Output 'canonical_ratchet_boundary=verified'
Write-Output 'schema_status=candidate-not-active'
Write-Output 'rust_codec=verified'
Write-Output 'independent_javascript_ed25519=verified'
Write-Output 'independent_blake3=pending'
Write-Output 'active_wire_changed=false'
