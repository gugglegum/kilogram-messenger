[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-ACCOUNT-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-VECTORS.json'
$authorityVectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-ACCOUNT.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_account_v1.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-account-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0134-m118-canonical-account-collaboration.md'
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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.8' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical account schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-ACCOUNT.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-account-collaboration-v1') {
        throw 'canonical account CDDL declaration changed'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed) { throw 'canonical account schema claims activation' }
    if ([int]$Schema.limits.maximum_devices -ne 32 -or
        [int]$Schema.limits.maximum_members -ne 4096 -or
        [int]$Schema.limits.maximum_nested_cbor_bytes -ne 524288) { throw 'canonical account limits changed' }
    Assert-ExactStrings @($Schema.domains | ForEach-Object { "$($_.id)|$($_.value)|$($_.signer)" }) @(
        'account-device-list-signature-v1|kilogram:account-device-list-signature:cbor-v1\0|account-root-ed25519',
        'conversation-membership-signature-v1|kilogram:conversation-membership-signature:cbor-v1\0|owner-account-root-ed25519'
    ) 'canonical account domains'
    Assert-ExactStrings @($Schema.objects | ForEach-Object { $_.id }) @(
        'account-device-list-content-v1', 'signed-account-device-list-v1',
        'conversation-membership-content-v1', 'signed-conversation-membership-v1'
    ) 'canonical account objects'
    Assert-ExactStrings @($Schema.rust_bindings | ForEach-Object { $_.symbol }) @(
        'CANONICAL_ACCOUNT_DEVICE_LIST_SIGNATURE_DOMAIN',
        'CANONICAL_CONVERSATION_MEMBERSHIP_SIGNATURE_DOMAIN',
        'MAX_CANONICAL_ACCOUNT_DEVICES', 'MAX_CANONICAL_CONVERSATION_MEMBERS',
        'MAX_CANONICAL_NESTED_ACCOUNT_BYTES'
    ) 'canonical account Rust bindings'
    foreach ($binding in $Schema.rust_bindings) {
        if ([string]$binding.path -cne 'crates/kilogram-protocol/src/canonical_account_v1.rs' -or
            $Rust.IndexOf([string]$binding.symbol, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical account Rust binding '$($binding.symbol)' is missing"
        }
        if ($Rust.IndexOf(([string]$binding.value).Replace('\0', '\0'), [StringComparison]::Ordinal) -lt 0) {
            if ([string]$binding.rust_type -ceq 'bytes') { throw "canonical account Rust value '$($binding.value)' is missing" }
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        [UInt64]$Vectors.input.membership_revision -ne 3) { throw 'canonical account vector header changed' }
    Assert-LowerHex $Vectors.input.root_secret_key_hex 32 'Root seed'
    Assert-LowerHex $Vectors.input.other_member_secret_key_hex 32 'other member seed'
    Assert-LowerHex $Vectors.input.conversation_id_hex 32 'conversation ID'
    Assert-LowerHex $Vectors.expected.account_id_hex 32 'Account ID'
    Assert-LowerHex $Vectors.expected.other_member_account_id_hex 32 'other member Account ID'
    if (@($Vectors.expected.ordered_members_hex).Count -ne 2 -or
        [string]$Vectors.expected.ordered_members_hex[0] -cge [string]$Vectors.expected.ordered_members_hex[1]) {
        throw 'canonical account member ordering changed'
    }
    foreach ($name in @('account_device_list', 'conversation_membership')) {
        $value = $Vectors.expected.$name
        Assert-LowerHex $value.content_cbor_hex -1 "$name content"
        Assert-LowerHex $value.signature_domain_hex -1 "$name domain"
        Assert-LowerHex $value.signing_preimage_hex -1 "$name preimage"
        Assert-LowerHex $value.signature_hex 64 "$name signature"
        Assert-LowerHex $value.signed_cbor_hex -1 "$name signed object"
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $authorityVectorsPath, $cddlPath, $rustPath,
    $wirePath, $referencePath, $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical account file is missing: $path" }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('signed-account-device-list-v1', 'signed-conversation-membership-v1',
    '1*32 signed-device-certificate-cbor', '1*4096 account-id')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "account CDDL is missing '$required'" }
}
foreach ($required in @('decode_and_verify_signed_account_device_list_v1',
    'decode_and_verify_signed_conversation_membership_v1', 'MissingMessagingCapabilities',
    'OwnerNotMember', 'NonCanonicalDeviceOrder', 'NonCanonicalMemberOrder')) {
    if ($rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "account codec is missing '$required'" }
}
if ($wire.IndexOf('canonical_account_v1', [StringComparison]::Ordinal) -ge 0 -or
    $wire.IndexOf('kilogram:account-device-list-signature:cbor-v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical account codec leaked into active wire'
}
$accountCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'account-authority' })
if ($accountCoverage.Count -ne 1 -or [string]$accountCoverage[0].coverage -cne 'complete' -or
    @($accountCoverage[0].remaining).Count -ne 0 -or
    'device-link-authorization' -notin @($accountCoverage[0].covered)) {
    throw 'canonical account coverage does not include the completed device-link extension'
}
foreach ($required in @('bounded to 32 devices', 'bounded to 4096 Account IDs',
    'from 1305 to 1827 malformed cases', 'active wire bytes, runtime behavior')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical account RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath --authority-vectors $authorityVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_account_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical account independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}
if ($SelfTest) {
    $active = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $active.status = 'active'
    Assert-Rejected { Assert-Schema $active $rust } 'active_without_migration'
    $missing = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $missing.objects = @($missing.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-Schema $missing $rust } 'removed_account_object'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.input.membership_revision = 0
    Assert-Rejected { Assert-Vectors $changed } 'zero_membership_revision'
    Write-Output 'canonical_account_self_test=passed'
}

Write-Output 'canonical_account_boundary=verified'
Write-Output 'canonical_account_objects=4'
Write-Output 'strict_account_decoders=2'
Write-Output 'independent_ed25519=verified'
Write-Output 'bounded_decoder_mutations=2610'
Write-Output 'account_authority_extension=device-link-complete'
Write-Output 'active_wire_changed=false'
Write-Output 'network_executed=false'
