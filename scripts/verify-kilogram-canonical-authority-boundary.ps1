[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-AUTHORITY-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-AUTHORITY.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_authority_v1.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$cargoPath = Join-Path $workspace 'crates\kilogram-protocol\Cargo.toml'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-authority-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0128-m112-canonical-authority-and-session.md'

function Assert-ExactStringSet {
    param(
        [AllowEmptyCollection()][string[]]$Actual,
        [AllowEmptyCollection()][string[]]$Expected,
        [string]$Label
    )

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
    param(
        [object]$Value,
        [string[]]$Expected,
        [string]$Label
    )

    if ($null -eq $Value) {
        throw "$Label is null"
    }
    $actual = @($Value.PSObject.Properties | ForEach-Object { $_.Name })
    Assert-ExactStringSet $actual $Expected "$Label properties"
}

function Assert-Rejected {
    param(
        [scriptblock]$Action,
        [string]$Label
    )

    try {
        & $Action
    }
    catch {
        Write-Output "$Label=rejected"
        return
    }
    throw "$Label was unexpectedly accepted"
}

function ConvertFrom-CanonicalHex {
    param(
        [string]$Value,
        [int]$ExpectedBytes = -1,
        [string]$Label = 'hex value'
    )

    if ([string]::IsNullOrEmpty($Value) -or
        $Value.Length % 2 -ne 0 -or
        $Value -cnotmatch '^[0-9a-f]+$') {
        throw "$Label is not non-empty lowercase hexadecimal"
    }
    $bytes = [byte[]]::new($Value.Length / 2)
    for ($index = 0; $index -lt $bytes.Length; $index++) {
        $bytes[$index] = [Convert]::ToByte($Value.Substring($index * 2, 2), 16)
    }
    if ($ExpectedBytes -ge 0 -and $bytes.Length -ne $ExpectedBytes) {
        throw "$Label has $($bytes.Length) bytes; expected $ExpectedBytes"
    }
    return ,$bytes
}

function Get-FieldFingerprint {
    param(
        [object]$Field,
        [string]$Label
    )

    $known = @(
        'index', 'name', 'type', 'exact', 'exact_bytes', 'object', 'item_type',
        'minimum_items', 'maximum_items', 'ordering'
    )
    $actual = @($Field.PSObject.Properties | ForEach-Object { $_.Name })
    foreach ($name in $actual) {
        if ($name -notin $known) {
            throw "$Label contains unknown property '$name'"
        }
    }
    foreach ($required in @('index', 'name', 'type')) {
        if ($required -notin $actual) {
            throw "$Label is missing '$required'"
        }
    }
    $parts = [System.Collections.Generic.List[string]]::new()
    foreach ($name in $known) {
        if ($name -in $actual) {
            $parts.Add("$name=$($Field.$name)")
        }
    }
    return $parts -join ';'
}

function Assert-RustBindings {
    param(
        [object[]]$Bindings,
        [string]$Source
    )

    $expected = @(
        'CANONICAL_DEVICE_CERTIFICATE_SIGNATURE_DOMAIN',
        'CANONICAL_DEVICE_REVOCATION_SIGNATURE_DOMAIN',
        'CANONICAL_AUTHORITY_SNAPSHOT_SIGNATURE_DOMAIN',
        'CANONICAL_DEVICE_SESSION_AUTHORIZATION_SIGNATURE_DOMAIN',
        'MAX_CANONICAL_DEVICE_CAPABILITIES',
        'MAX_CANONICAL_AUTHORITY_REVOCATIONS'
    )
    $actual = @($Bindings | ForEach-Object { [string]$_.symbol })
    Assert-ExactStringSet $actual $expected 'canonical authority Rust bindings'
    for ($index = 0; $index -lt $expected.Count; $index++) {
        if ($actual[$index] -cne $expected[$index]) {
            throw 'canonical authority Rust bindings must remain in stable order'
        }
    }
    foreach ($binding in $Bindings) {
        Assert-ExactPropertySet $binding @('path', 'symbol', 'rust_type', 'value') 'Rust binding'
        if ([string]$binding.path -cne 'crates/kilogram-protocol/src/canonical_authority_v1.rs') {
            throw "unexpected canonical authority Rust path '$($binding.path)'"
        }
        $symbol = [regex]::Escape([string]$binding.symbol)
        switch ([string]$binding.rust_type) {
            'bytes' {
                $value = [regex]::Escape([string]$binding.value)
                $pattern = "(?ms)^\s*pub\s+const\s+$symbol\s*:\s*&\[u8\]\s*=\s*b`"$value`"\s*;"
            }
            'usize' {
                $value = [UInt64]$binding.value
                $pattern = "(?m)^\s*pub\s+const\s+$symbol\s*:\s*usize\s*=\s*$value\s*;"
            }
            default {
                throw "unsupported Rust binding type '$($binding.rust_type)'"
            }
        }
        if ([regex]::Matches($Source, $pattern).Count -ne 1) {
            throw "Rust binding '$($binding.symbol)' does not have one exact declaration"
        }
    }
}

function Assert-AuthoritySchema {
    param(
        [object]$Schema,
        [string]$RustSource
    )

    Assert-ExactPropertySet $Schema @(
        'schema_version', 'milestone', 'status', 'changes_active_wire', 'profile',
        'cddl', 'activation', 'limits', 'domains', 'rust_bindings', 'objects'
    ) 'canonical authority schema'
    if ([UInt64]$Schema.schema_version -ne 1 -or
        [string]$Schema.milestone -cne 'M1.1.2' -or
        [string]$Schema.status -cne 'candidate-not-active' -or
        [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical authority schema header changed or claims activation'
    }

    Assert-ExactPropertySet $Schema.cddl @('standard', 'standard_url', 'path', 'root_rule') 'CDDL declaration'
    if ([string]$Schema.cddl.standard -cne 'RFC 8610' -or
        [string]$Schema.cddl.standard_url -cne 'https://www.rfc-editor.org/rfc/rfc8610.html' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-AUTHORITY.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-authority-v1') {
        throw 'canonical authority CDDL declaration changed'
    }

    Assert-ExactPropertySet $Schema.activation @(
        'current_runtime_encoding', 'current_runtime_changed', 'required_before_activation'
    ) 'canonical authority activation policy'
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical authority candidate must not claim runtime activation'
    }
    Assert-ExactStringSet @($Schema.activation.required_before_activation) @(
        'new-explicit-protocol-version-or-parallel-identifier',
        'dual-reader-and-downgrade-tests',
        'remaining-object-family-schema-coverage',
        'decoder-fuzzing-and-malformed-input-corpus',
        'fresh-field-and-hosted-evidence'
    ) 'canonical authority activation requirements'

    Assert-ExactPropertySet $Schema.limits @('maximum_capabilities', 'maximum_revocations') 'authority limits'
    if ([int]$Schema.limits.maximum_capabilities -ne 2 -or
        [int]$Schema.limits.maximum_revocations -ne 32) {
        throw 'canonical authority limits changed'
    }

    $expectedDomains = @(
        'device-certificate-signature-v1|kilogram:device-certificate-signature:cbor-v1\0|account-root-ed25519',
        'device-revocation-signature-v1|kilogram:device-revocation-signature:cbor-v1\0|account-root-ed25519',
        'account-authority-snapshot-signature-v1|kilogram:account-authority-snapshot-signature:cbor-v1\0|account-root-ed25519',
        'device-session-authorization-signature-v1|kilogram:device-session-authorization-signature:cbor-v1\0|authorized-device-ed25519'
    )
    $actualDomains = @($Schema.domains | ForEach-Object {
        Assert-ExactPropertySet $_ @('id', 'value', 'signer') 'authority signature domain'
        "$($_.id)|$($_.value)|$($_.signer)"
    })
    if ($actualDomains.Count -ne $expectedDomains.Count) {
        throw 'canonical authority domain count changed'
    }
    for ($index = 0; $index -lt $expectedDomains.Count; $index++) {
        if ($actualDomains[$index] -cne $expectedDomains[$index]) {
            throw "canonical authority domain changed at index $index"
        }
    }

    Assert-RustBindings @($Schema.rust_bindings) $RustSource

    $expectedIds = @(
        'account-id-v1',
        'device-id-v1',
        'device-capability-v1',
        'device-certificate-content-v1',
        'signed-device-certificate-v1',
        'device-revocation-content-v1',
        'signed-device-revocation-v1',
        'account-authority-snapshot-content-v1',
        'signed-account-authority-snapshot-v1',
        'device-session-authorization-content-v1',
        'signed-device-session-authorization-v1'
    )
    $objects = @($Schema.objects)
    if ($objects.Count -ne $expectedIds.Count) {
        throw "canonical authority object count changed: expected $($expectedIds.Count), got $($objects.Count)"
    }
    for ($index = 0; $index -lt $expectedIds.Count; $index++) {
        if ([string]$objects[$index].id -cne $expectedIds[$index]) {
            throw "canonical authority object order changed at index $index"
        }
    }

    foreach ($index in @(0, 1)) {
        $object = $objects[$index]
        Assert-ExactPropertySet $object @('id', 'kind', 'type', 'exact_bytes', 'meaning') "primitive '$($object.id)'"
        if ([string]$object.kind -cne 'primitive' -or
            [string]$object.type -cne 'byte-string' -or
            [int]$object.exact_bytes -ne 32) {
            throw "primitive '$($object.id)' changed"
        }
    }
    if ([string]$objects[0].meaning -cne 'account-root-ed25519-verifying-key' -or
        [string]$objects[1].meaning -cne 'device-ed25519-verifying-key') {
        throw 'canonical authority identifier meaning changed'
    }

    $capability = $objects[2]
    Assert-ExactPropertySet $capability @('id', 'kind', 'type', 'values') 'device capability enumeration'
    if ([string]$capability.kind -cne 'enumeration' -or
        [string]$capability.type -cne 'unsigned-integer' -or
        @($capability.values).Count -ne 2) {
        throw 'device capability enumeration shape changed'
    }
    $capabilityValues = @($capability.values | ForEach-Object {
        Assert-ExactPropertySet $_ @('code', 'name') 'device capability value'
        "$($_.code)|$($_.name)"
    })
    if ($capabilityValues[0] -cne '1|sign-events' -or
        $capabilityValues[1] -cne '2|sync-history') {
        throw 'device capability codes changed'
    }

    $expectedArrays = [ordered]@{
        'device-certificate-content-v1' = [pscustomobject]@{
            kind = 'signed-content'; length = 6; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=account_id;type=object;object=account-id-v1',
                'index=2;name=device_id;type=object;object=device-id-v1',
                'index=3;name=encryption_public_key;type=byte-string;exact_bytes=32',
                'index=4;name=authority_sequence;type=unsigned-integer',
                'index=5;name=capabilities;type=array;item_type=device-capability-v1;minimum_items=1;maximum_items=2;ordering=unsigned-integer-strict'
            )
        }
        'signed-device-certificate-v1' = [pscustomobject]@{
            kind = 'signed-envelope'; length = 3; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=content;type=object;object=device-certificate-content-v1',
                'index=2;name=signature;type=byte-string;exact_bytes=64'
            )
        }
        'device-revocation-content-v1' = [pscustomobject]@{
            kind = 'signed-content'; length = 4; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=account_id;type=object;object=account-id-v1',
                'index=2;name=device_id;type=object;object=device-id-v1',
                'index=3;name=authority_sequence;type=unsigned-integer'
            )
        }
        'signed-device-revocation-v1' = [pscustomobject]@{
            kind = 'signed-envelope'; length = 3; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=content;type=object;object=device-revocation-content-v1',
                'index=2;name=signature;type=byte-string;exact_bytes=64'
            )
        }
        'account-authority-snapshot-content-v1' = [pscustomobject]@{
            kind = 'signed-content'; length = 4; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=account_id;type=object;object=account-id-v1',
                'index=2;name=revision;type=unsigned-integer',
                'index=3;name=revocations;type=array;item_type=signed-device-revocation-v1;maximum_items=32;ordering=device-id-unsigned-bytewise-lexicographic-strict'
            )
        }
        'signed-account-authority-snapshot-v1' = [pscustomobject]@{
            kind = 'signed-envelope'; length = 3; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=content;type=object;object=account-authority-snapshot-content-v1',
                'index=2;name=signature;type=byte-string;exact_bytes=64'
            )
        }
        'device-session-authorization-content-v1' = [pscustomobject]@{
            kind = 'signed-content'; length = 4; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=session_binding;type=byte-string;exact_bytes=32',
                'index=2;name=certificate;type=object;object=signed-device-certificate-v1',
                'index=3;name=authority_snapshot;type=object;object=signed-account-authority-snapshot-v1'
            )
        }
        'signed-device-session-authorization-v1' = [pscustomobject]@{
            kind = 'signed-envelope'; length = 3; fields = @(
                'index=0;name=schema_version;type=unsigned-integer;exact=1',
                'index=1;name=content;type=object;object=device-session-authorization-content-v1',
                'index=2;name=signature;type=byte-string;exact_bytes=64'
            )
        }
    }
    for ($objectIndex = 3; $objectIndex -lt $objects.Count; $objectIndex++) {
        $object = $objects[$objectIndex]
        $expected = $expectedArrays[[string]$object.id]
        if ($null -eq $expected) {
            throw "no closed schema expectation exists for '$($object.id)'"
        }
        Assert-ExactPropertySet $object @('id', 'kind', 'array_length', 'fields') "object '$($object.id)'"
        $fields = @($object.fields)
        if ([string]$object.kind -cne [string]$expected.kind -or
            [int]$object.array_length -ne [int]$expected.length -or
            $fields.Count -ne @($expected.fields).Count) {
            throw "object '$($object.id)' shape changed"
        }
        for ($fieldIndex = 0; $fieldIndex -lt $fields.Count; $fieldIndex++) {
            $actualFingerprint = Get-FieldFingerprint $fields[$fieldIndex] "$($object.id) field $fieldIndex"
            if ($actualFingerprint -cne [string]$expected.fields[$fieldIndex]) {
                throw "object '$($object.id)' field $fieldIndex changed"
            }
        }
    }
}

function Assert-AuthorityVectors {
    param([object]$Vectors)

    Assert-ExactPropertySet $Vectors @(
        'schema_version', 'profile', 'activation_status', 'input', 'expected'
    ) 'canonical authority vectors'
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical authority vector header changed or claims activation'
    }
    Assert-ExactPropertySet $Vectors.input @(
        'root_secret_key_hex', 'device_secret_key_hex', 'revoked_device_secret_key_hex',
        'encryption_public_key_hex', 'certificate_authority_sequence', 'capabilities',
        'revocation_authority_sequence', 'authority_revision', 'session_binding_hex'
    ) 'canonical authority vector input'
    Assert-ExactPropertySet $Vectors.expected @(
        'account_id_hex', 'device_id_hex', 'revoked_device_id_hex', 'certificate',
        'revocation', 'authority_snapshot', 'session_authorization'
    ) 'canonical authority vector expected'

    $fixedInputs = [ordered]@{
        root_secret_key_hex = '202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f'
        device_secret_key_hex = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f'
        revoked_device_secret_key_hex = '404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f'
        encryption_public_key_hex = 'a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf'
        session_binding_hex = 'c0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedf'
    }
    foreach ($entry in $fixedInputs.GetEnumerator()) {
        if ([string]$Vectors.input.($entry.Key) -cne [string]$entry.Value) {
            throw "authority vector input '$($entry.Key)' changed"
        }
        ConvertFrom-CanonicalHex ([string]$Vectors.input.($entry.Key)) 32 $entry.Key | Out-Null
    }
    if ([UInt64]$Vectors.input.certificate_authority_sequence -ne 7 -or
        [UInt64]$Vectors.input.revocation_authority_sequence -ne 3 -or
        [UInt64]$Vectors.input.authority_revision -ne 8 -or
        @($Vectors.input.capabilities).Count -ne 2 -or
        [UInt64]$Vectors.input.capabilities[0] -ne 1 -or
        [UInt64]$Vectors.input.capabilities[1] -ne 2) {
        throw 'authority vector sequence or capability fixture changed'
    }

    foreach ($name in @('account_id_hex', 'device_id_hex', 'revoked_device_id_hex')) {
        ConvertFrom-CanonicalHex ([string]$Vectors.expected.$name) 32 $name | Out-Null
    }
    $expectedDomains = [ordered]@{
        certificate = '6b696c6f6772616d3a6465766963652d63657274696669636174652d7369676e61747572653a63626f722d763100'
        revocation = '6b696c6f6772616d3a6465766963652d7265766f636174696f6e2d7369676e61747572653a63626f722d763100'
        authority_snapshot = '6b696c6f6772616d3a6163636f756e742d617574686f726974792d736e617073686f742d7369676e61747572653a63626f722d763100'
        session_authorization = '6b696c6f6772616d3a6465766963652d73657373696f6e2d617574686f72697a6174696f6e2d7369676e61747572653a63626f722d763100'
    }
    foreach ($entry in $expectedDomains.GetEnumerator()) {
        $object = $Vectors.expected.($entry.Key)
        Assert-ExactPropertySet $object @(
            'content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex',
            'signature_hex', 'signed_cbor_hex'
        ) "authority vector '$($entry.Key)'"
        if ([string]$object.signature_domain_hex -cne [string]$entry.Value) {
            throw "authority vector '$($entry.Key)' domain changed"
        }
        ConvertFrom-CanonicalHex ([string]$object.content_cbor_hex) -1 "$($entry.Key) content" | Out-Null
        ConvertFrom-CanonicalHex ([string]$object.signature_domain_hex) -1 "$($entry.Key) domain" | Out-Null
        ConvertFrom-CanonicalHex ([string]$object.signing_preimage_hex) -1 "$($entry.Key) preimage" | Out-Null
        ConvertFrom-CanonicalHex ([string]$object.signature_hex) 64 "$($entry.Key) signature" | Out-Null
        ConvertFrom-CanonicalHex ([string]$object.signed_cbor_hex) -1 "$($entry.Key) envelope" | Out-Null
        if (-not ([string]$object.signing_preimage_hex).StartsWith([string]$object.signature_domain_hex, [StringComparison]::Ordinal)) {
            throw "authority vector '$($entry.Key)' preimage is not domain-prefixed"
        }
    }
}

foreach ($path in @(
    $schemaPath, $vectorsPath, $cddlPath, $rustPath, $wirePath, $cargoPath,
    $referencePath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical authority boundary file is missing: $path"
    }
}

$schema = Get-Content -LiteralPath $schemaPath -Raw -Encoding UTF8 | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw -Encoding UTF8 | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw -Encoding UTF8
$rust = Get-Content -LiteralPath $rustPath -Raw -Encoding UTF8
$wire = Get-Content -LiteralPath $wirePath -Raw -Encoding UTF8
$cargo = Get-Content -LiteralPath $cargoPath -Raw -Encoding UTF8
$reference = Get-Content -LiteralPath $referencePath -Raw -Encoding UTF8
$rfc = Get-Content -LiteralPath $rfcPath -Raw -Encoding UTF8

Assert-AuthoritySchema $schema $rust
Assert-AuthorityVectors $vectors

$requiredRules = @(
    'kilogram-authority-v1', 'account-id-v1', 'device-id-v1',
    'encryption-public-key-v1', 'session-binding-v1', 'ed25519-signature-v1',
    'device-capability-v1', 'device-certificate-content-v1',
    'signed-device-certificate-v1', 'device-revocation-content-v1',
    'signed-device-revocation-v1', 'account-authority-snapshot-content-v1',
    'signed-account-authority-snapshot-v1',
    'device-session-authorization-content-v1',
    'signed-device-session-authorization-v1'
)
foreach ($rule in $requiredRules) {
    if ([regex]::Matches($cddl, "(?m)^$([regex]::Escape($rule))\s*=").Count -ne 1) {
        throw "CDDL rule '$rule' is missing or duplicated"
    }
}
foreach ($required in @('[1*2 device-capability-v1]', '[*32 signed-device-revocation-v1]')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "CDDL is missing '$required'"
    }
}
foreach ($forbidden in @('tstr', 'float16', 'float32', 'float64', '#6.')) {
    if ([regex]::IsMatch($cddl, "(?i)(^|[^a-z0-9-])$([regex]::Escape($forbidden))([^a-z0-9-]|$)")) {
        throw "CDDL contains forbidden profile surface '$forbidden'"
    }
}

foreach ($required in @(
    'decode_and_verify_signed_device_certificate_v1',
    'decode_and_verify_signed_device_revocation_v1',
    'decode_and_verify_signed_account_authority_snapshot_v1',
    'decode_and_verify_signed_device_session_authorization_v1',
    'verify_strict',
    'NonCanonicalCapabilities',
    'NonCanonicalRevocations',
    'CertificateOutsideSnapshot',
    'SessionDeviceRevoked'
)) {
    if ($rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical authority Rust implementation is missing '$required'"
    }
}
if ($wire.IndexOf('canonical_authority_v1', [StringComparison]::Ordinal) -ge 0 -or
    $wire.IndexOf(':cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'active wire module references the inactive canonical authority candidate'
}
if ($cargo.IndexOf('ed25519-dalek.workspace = true', [StringComparison]::Ordinal) -lt 0) {
    throw 'protocol crate does not declare its maintained Ed25519 implementation directly'
}
foreach ($required in @(
    'node:crypto', 'signEd25519', 'verifyEd25519',
    'canonical_authority_reference=verified',
    'canonical_authority_reference_self_test=passed'
)) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "independent JavaScript reference is missing '$required'"
    }
}
foreach ($required in @(
    'candidate format only',
    'active messenger continues to encode',
    'Postcard versions recorded',
    'does not activate deterministic CBOR',
    'independent JavaScript reconstruction',
    'not a production wire migration'
)) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "RFC-0128 is missing '$required'"
    }
}

$node = Get-Command node -CommandType Application -ErrorAction Stop | Select-Object -First 1
$referenceArguments = @(
    $referencePath, '--schema', $schemaPath, '--vectors', $vectorsPath
)

if ($SelfTest) {
    $active = ConvertFrom-Json (ConvertTo-Json $schema -Depth 40)
    $active.status = 'active'
    Assert-Rejected { Assert-AuthoritySchema $active $rust } 'active_without_migration'

    $removed = ConvertFrom-Json (ConvertTo-Json $schema -Depth 40)
    $removed.objects = @($removed.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-AuthoritySchema $removed $rust } 'removed_schema_object'

    $changedBinding = ConvertFrom-Json (ConvertTo-Json $schema -Depth 40)
    $changedBinding.rust_bindings[0].value = 'changed'
    Assert-Rejected { Assert-AuthoritySchema $changedBinding $rust } 'changed_rust_binding'

    $uppercase = ConvertFrom-Json (ConvertTo-Json $vectors -Depth 40)
    $uppercase.input.session_binding_hex = ([string]$uppercase.input.session_binding_hex).ToUpperInvariant()
    Assert-Rejected { Assert-AuthorityVectors $uppercase } 'noncanonical_hex'

    $referenceOutput = @(& $node.Source @referenceArguments --self-test 2>&1)
    $referenceExitCode = $LASTEXITCODE
    $referenceLines = @($referenceOutput | ForEach-Object { [string]$_ })
    if ($referenceExitCode -ne 0 -or
        'canonical_authority_reference_self_test=passed' -notin $referenceLines) {
        throw "independent reference self-test failed.`n$($referenceLines -join [Environment]::NewLine)"
    }
    Write-Output 'canonical_authority_boundary_self_test=passed'
    Write-Output 'network_executed=false'
    Write-Output 'rust_test_executables_started=false'
    Write-Output 'release_build=false'
    Write-Output 'archive_created=false'
    exit 0
}

$referenceOutput = @(& $node.Source @referenceArguments 2>&1)
$referenceExitCode = $LASTEXITCODE
$referenceLines = @($referenceOutput | ForEach-Object { [string]$_ })
if ($referenceExitCode -ne 0 -or 'canonical_authority_reference=verified' -notin $referenceLines) {
    throw "independent canonical authority reference failed.`n$($referenceLines -join [Environment]::NewLine)"
}

Write-Output 'canonical_authority_boundary=verified'
Write-Output 'canonical_authority_milestone=M1.1.2'
Write-Output 'canonical_authority_schema_objects=11'
Write-Output 'canonical_authority_golden_vectors=1'
Write-Output 'independent_reference=javascript-node-crypto-ed25519'
Write-Output 'canonical_authority_activation_status=not-active'
Write-Output 'current_runtime_encoding=postcard'
Write-Output 'wire_changed=false'
Write-Output 'network_executed=false'
Write-Output 'rust_test_executables_started=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
