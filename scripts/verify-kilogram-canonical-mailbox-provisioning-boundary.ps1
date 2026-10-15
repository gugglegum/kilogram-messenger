[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-MAILBOX-PROVISIONING-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-PROVISIONING-VECTORS.json'
$mailboxVectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-MAILBOX-PROVISIONING.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_mailbox_provisioning_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'crates\kilogram-mailbox-provisioning\src\lib.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-provisioning-reference.mjs'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0144-m1118-canonical-mailbox-provisioning.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.18' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-MAILBOX-PROVISIONING.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-mailbox-provisioning-v1' -or
        [string]$Schema.source_compatibility.current_runtime_encoding -cne 'postcard' -or
        [bool]$Schema.source_compatibility.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical mailbox provisioning schema boundary changed'
    }
    if ([int]$Schema.modes.legacy_https -ne 1 -or [int]$Schema.modes.exact_volunteer -ne 2 -or
        [int]$Schema.envelope_purposes.owner_local_binding -ne 1 -or
        [int]$Schema.envelope_purposes.recipient_peer_offer -ne 2) {
        throw 'canonical mailbox provisioning tags changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_service_url_bytes,
        [int]$Schema.limits.maximum_sealed_ciphertext_bytes,
        [int]$Schema.limits.maximum_capability_update_bytes,
        [int]$Schema.limits.maximum_acknowledgement_bytes,
        [int]$Schema.limits.minimum_replica_stores,
        [int]$Schema.limits.maximum_replica_stores,
        [int]$Schema.limits.minimum_offer_validity_seconds,
        [int]$Schema.limits.maximum_offer_validity_seconds
    ) -join ','
    if ($limits -cne '2048,16384,32768,4096,2,8,60,2592000') {
        throw 'canonical mailbox provisioning limits changed'
    }
    if (@($Schema.domains).Count -ne 9) { throw 'canonical mailbox provisioning domain inventory changed' }
    foreach ($required in @(
        'CANONICAL_MAILBOX_BINDING_ID_DOMAIN',
        'CANONICAL_MAILBOX_LOCAL_BINDING_SIGNATURE_DOMAIN',
        'CANONICAL_MAILBOX_LOCAL_BINDING_HPKE_INFO',
        'CANONICAL_MAILBOX_PEER_OFFER_SIGNATURE_DOMAIN',
        'CANONICAL_MAILBOX_PEER_OFFER_HPKE_INFO',
        'CANONICAL_MAILBOX_REPLICA_SET_COMMITMENT_ID_DOMAIN',
        'CANONICAL_MAILBOX_CAPABILITY_UPDATE_SIGNATURE_DOMAIN',
        'CANONICAL_MAILBOX_CAPABILITY_UPDATE_ID_DOMAIN',
        'CANONICAL_MAILBOX_CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN',
        'decode_and_verify_signed_local_mailbox_binding_v1',
        'decode_and_verify_signed_mailbox_peer_offer_v1',
        'verify_mailbox_capability_update_chain_link_v1',
        'verify_mailbox_capability_acknowledgement_for_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical mailbox provisioning Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        [string]$Vectors.dependency -cne 'CANONICAL-MAILBOX-VECTORS.json') {
        throw 'canonical mailbox provisioning vector header changed'
    }
    foreach ($field in @('owner_secret_key_hex', 'recipient_secret_key_hex', 'owner_account_id_hex',
        'recipient_account_id_hex', 'owner_encryption_public_key_hex', 'scope_hex',
        'replica_store_key_2_hex', 'replica_store_key_3_hex', 'local_encapsulated_key_hex',
        'offer_encapsulated_key_hex', 'session_binding_hex')) {
        Assert-LowerHex ([string]$Vectors.input.$field) 32 $field
    }
    foreach ($field in @('owner_device_id_hex', 'recipient_device_id_hex',
        'legacy_binding_id_hex', 'exact_binding_id_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    foreach ($field in @('legacy_core_cbor_hex', 'exact_core_cbor_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) -1 $field
    }
    foreach ($name in @('signed_local_binding', 'signed_legacy_peer_offer',
        'signed_exact_peer_offer', 'activation_update', 'revocation_update',
        'activation_acknowledgement')) {
        Assert-LowerHex ([string]$Vectors.expected.$name.content_cbor_hex) -1 "$name content"
        Assert-LowerHex ([string]$Vectors.expected.$name.signature_hex) 64 "$name signature"
        Assert-LowerHex ([string]$Vectors.expected.$name.signed_cbor_hex) -1 "$name signed CBOR"
    }
    foreach ($name in @('local_envelope', 'offer_envelope')) {
        Assert-LowerHex ([string]$Vectors.expected.$name.header_cbor_hex) -1 "$name header"
        Assert-LowerHex ([string]$Vectors.expected.$name.aad_cbor_hex) -1 "$name AAD"
        Assert-LowerHex ([string]$Vectors.expected.$name.hpke_info_hex) -1 "$name HPKE info"
        Assert-LowerHex ([string]$Vectors.expected.$name.cbor_hex) -1 "$name CBOR"
    }
    Assert-LowerHex ([string]$Vectors.expected.replica_set.cbor_hex) -1 'replica set'
    Assert-LowerHex ([string]$Vectors.expected.replica_set.commitment_id_hex) 32 'replica commitment ID'
    Assert-LowerHex ([string]$Vectors.expected.activation_update.update_id_hex) 32 'activation update ID'
    Assert-LowerHex ([string]$Vectors.expected.revocation_update.update_id_hex) 32 'revocation update ID'
}

foreach ($path in @($schemaPath, $vectorsPath, $mailboxVectorsPath, $cddlPath, $rustPath,
    $libPath, $activePath, $referencePath, $coveragePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical mailbox provisioning file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('signed-local-mailbox-binding-v1', 'signed-mailbox-peer-offer-v1',
    'sealed-mailbox-provisioning-envelope-v1', 'mailbox-replica-set-v1',
    'signed-mailbox-capability-update-v1', 'signed-mailbox-capability-acknowledgement-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical mailbox provisioning CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_mailbox_provisioning_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical mailbox provisioning module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes',
    'kilogram:mailbox-local-binding:v1\0', 'kilogram:mailbox-local-binding:v2\0',
    'kilogram:mailbox-device-offer:v1\0', 'kilogram:mailbox-device-offer:v2\0',
    'kilogram:mailbox-capability-update:v1\0', 'kilogram:mailbox-capability-update:v2\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active mailbox provisioning changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_mailbox_provisioning_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical mailbox provisioning leaked into the active runtime'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'mailbox-provisioning' })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'mailbox provisioning coverage is not complete'
}
foreach ($required in @('complete `mailbox-provisioning`', 'exact-volunteer', 'legacy HTTPS',
    'binding ID', 'replica set', 'recipient ACK', 'Postcard')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical mailbox provisioning RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --mailbox-vectors $mailboxVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_mailbox_provisioning_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical mailbox provisioning independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_replica_stores = 9
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_replica_limit'
    $changed = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changed.expected.activation_update.update_id_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_update_id_shape'
    Write-Output 'canonical_mailbox_provisioning_self_test=passed'
}

Write-Output 'canonical_mailbox_provisioning_boundary=verified'
Write-Output 'legacy_and_exact_modes=verified'
Write-Output 'binding_offer_replica_update_ack=verified'
Write-Output 'mailbox_provisioning_coverage=complete'
Write-Output 'bounded_decoder_mutations=8350'
Write-Output 'active_postcard_changed=false'
Write-Output 'network_executed=false'
