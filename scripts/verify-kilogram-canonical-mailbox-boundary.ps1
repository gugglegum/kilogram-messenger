[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-MAILBOX-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-MAILBOX.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_mailbox_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'crates\kilogram-mailbox\src\lib.rs'
$provisioningPath = Join-Path $workspace 'crates\kilogram-mailbox-provisioning\src\lib.rs'
$providerPath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-mailbox\src\wire.rs'
$runtimePath = Join-Path $workspace 'apps\kilogram-cli\src\runtime_mailbox.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0141-m1115-canonical-blind-mailbox-capability.md'
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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.15' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical mailbox schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-MAILBOX.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-mailbox-v1' -or
        [string]$Schema.activation.current_runtime_artifact_encoding -cne 'postcard' -or
        [string]$Schema.activation.current_runtime_wire_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical mailbox activation boundary changed'
    }
    $limits = @(
        [int]$Schema.limits.minimum_ttl_seconds,
        [int]$Schema.limits.maximum_ttl_seconds,
        [int]$Schema.limits.maximum_page_items,
        [int]$Schema.limits.maximum_address_bytes,
        [int]$Schema.limits.maximum_authorization_bytes,
        [int]$Schema.limits.maximum_receipt_bytes,
        [int]$Schema.limits.maximum_envelope_bytes,
        [int]$Schema.limits.maximum_storage_endpoint_bytes,
        [int]$Schema.limits.maximum_storage_offer_bytes,
        [int]$Schema.limits.minimum_storage_offer_validity_seconds,
        [int]$Schema.limits.maximum_storage_offer_validity_seconds,
        [int]$Schema.limits.maximum_admission_work_bits
    ) -join ','
    if ($limits -cne '60,604800,8,256,1024,1024,1048576,16384,24576,30,3600,20') {
        throw 'canonical mailbox bounds changed'
    }
    $objects = @($Schema.objects | ForEach-Object { "$($_.id):$($_.array_length)" })
    $expectedObjects = @(
        'mailbox-address-v1:3', 'mailbox-envelope-context-v1:5', 'mailbox-envelope-v1:4',
        'mailbox-write-authorization-content-v1:7', 'signed-mailbox-write-authorization-v1:3',
        'mailbox-read-authorization-content-v1:4', 'mailbox-read-list-v1:5',
        'mailbox-read-delete-v1:4', 'signed-mailbox-read-authorization-v1:3',
        'mailbox-storage-offer-content-v1:9', 'signed-mailbox-storage-offer-v1:3',
        'mailbox-stored-receipt-content-v1:8', 'signed-mailbox-stored-receipt-v1:3',
        'mailbox-delete-receipt-content-v1:7', 'signed-mailbox-delete-receipt-v1:3'
    )
    if (($objects -join ',') -cne ($expectedObjects -join ',')) { throw 'canonical mailbox objects changed' }
    if ([string]$Schema.registries.read_operation.'1' -cne 'list' -or
        [string]$Schema.registries.read_operation.'2' -cne 'delete' -or
        [string]$Schema.registries.storage_policy_class.'1' -cne 'bounded-volunteer') {
        throw 'canonical mailbox closed registry changed'
    }
    $separate = @($Schema.separate_surfaces)
    $expectedSeparate = @('mailbox-provisioning', 'blind-mailbox-provider-gossip',
        'blind-mailbox-transport', 'runtime-mailbox-envelope')
    if (($separate -join ',') -cne ($expectedSeparate -join ',')) {
        throw 'canonical mailbox surface separation changed'
    }
    foreach ($required in @(
        'CANONICAL_MAILBOX_ENVELOPE_HPKE_INFO', 'CANONICAL_MAILBOX_ENVELOPE_DIGEST_DOMAIN',
        'decode_and_verify_mailbox_address_v1', 'decode_and_verify_mailbox_envelope_v1',
        'decode_and_verify_signed_mailbox_write_authorization_v1',
        'decode_and_verify_signed_mailbox_read_authorization_v1',
        'decode_and_verify_signed_mailbox_storage_offer_v1',
        'decode_and_verify_signed_mailbox_stored_receipt_v1',
        'decode_and_verify_signed_mailbox_delete_receipt_v1',
        'verify_mailbox_write_authorization_for_v1', 'verify_mailbox_read_authorization_for_v1',
        'verify_mailbox_stored_receipt_for_v1', 'verify_mailbox_delete_receipt_for_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical mailbox Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical mailbox vector header changed'
    }
    foreach ($field in @('read_secret_key_hex', 'write_secret_key_hex', 'store_secret_key_hex',
        'item_id_hex', 'request_nonce_hex', 'delete_receipt_id_hex', 'encapsulated_key_hex')) {
        Assert-LowerHex ([string]$Vectors.input.$field) 32 $field
    }
    foreach ($field in @('read_key_hex', 'write_key_hex', 'store_key_hex', 'mailbox_id_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    foreach ($name in @('write_authorization', 'read_list_authorization',
        'read_delete_authorization', 'storage_offer', 'stored_receipt', 'delete_receipt')) {
        $object = $Vectors.expected.$name
        foreach ($field in @('content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex',
            'signature_hex', 'signed_cbor_hex')) {
            Assert-LowerHex ([string]$object.$field) $(if ($field -ceq 'signature_hex') { 64 } else { -1 }) "$name.$field"
        }
    }
    if ([int]$Vectors.expected.storage_offer.admission_work_bits -lt 8) {
        throw 'storage offer vector no longer proves bounded admission work'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath, $activePath,
    $provisioningPath, $providerPath, $wirePath, $runtimePath, $referencePath, $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical mailbox file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$provisioning = Get-Content -LiteralPath $provisioningPath -Raw
$provider = Get-Content -LiteralPath $providerPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$runtime = Get-Content -LiteralPath $runtimePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('mailbox-address-v1', 'mailbox-envelope-v1',
    'signed-mailbox-write-authorization-v1', 'signed-mailbox-read-authorization-v1',
    'signed-mailbox-storage-offer-v1', 'signed-mailbox-stored-receipt-v1',
    'signed-mailbox-delete-receipt-v1', 'mailbox-item-option-v1 = [] / [item-id: id-32]')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical mailbox CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_mailbox_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical mailbox module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes',
    'kilogram:blind-mailbox-envelope:v1\0', 'kilogram:blind-mailbox-write:v1\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active mailbox boundary changed: missing '$required'"
    }
}
foreach ($separateSource in @($active, $provisioning, $provider, $wire, $runtime)) {
    if ($separateSource.IndexOf('canonical_mailbox_v1', [StringComparison]::Ordinal) -ge 0) {
        throw 'inactive canonical mailbox leaked into an active or separate runtime surface'
    }
}
$mailboxCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'blind-mailbox-capability' })
$separateCoverage = @($coverage.surfaces | Where-Object {
    $_.id -in @('mailbox-provisioning', 'blind-mailbox-provider-gossip',
        'blind-mailbox-transport', 'runtime-mailbox-envelope')
})
if ($mailboxCoverage.Count -ne 1 -or [string]$mailboxCoverage[0].coverage -cne 'complete' -or
    @($mailboxCoverage[0].remaining).Count -ne 0 -or $separateCoverage.Count -ne 4 -or
    @($separateCoverage | Where-Object {
        ($_.id -in @('blind-mailbox-provider-gossip', 'blind-mailbox-transport') -and
            $_.coverage -cne 'complete') -or
        ($_.id -notin @('blind-mailbox-provider-gossip', 'blind-mailbox-transport') -and
            $_.coverage -cne 'not-started')
    }).Count -ne 0) {
    throw 'mailbox capability and separate-surface coverage changed'
}
foreach ($required in @('complete `blind-mailbox-capability`', 'HPKE cross-language',
    'Provisioning, provider gossip, transport', 'from 4,698 to 6,264',
    'No active ALPN, version, payload')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical mailbox RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_mailbox_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical mailbox independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedRegistry = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedRegistry.registries.read_operation.'2' = 'peek'
    Assert-Rejected { Assert-Schema $changedRegistry $rust } 'open_read_registry'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.stored_receipt.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_receipt_signature_shape'
    Write-Output 'changed_mailbox_capability=rejected-by-independent-reference'
    Write-Output 'canonical_mailbox_self_test=passed'
}

Write-Output 'canonical_mailbox_boundary=verified'
Write-Output 'address_envelope_authorizations_offers_receipts=verified'
Write-Output 'bounded_decoder_mutations=6264'
Write-Output 'blind_mailbox_capability_coverage=complete'
Write-Output 'separate_mailbox_surfaces=independently-tracked'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
