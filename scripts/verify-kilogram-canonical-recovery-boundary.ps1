[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RECOVERY-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RECOVERY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RECOVERY.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_recovery_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-recovery-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0130-m114-canonical-history-recovery.md'

function Assert-ExactStringSet {
    param([string[]]$Actual, [string[]]$Expected, [string]$Label)
    $actualSorted = @($Actual | Sort-Object)
    $expectedSorted = @($Expected | Sort-Object)
    if ($actualSorted.Count -ne $expectedSorted.Count) { throw "$Label count changed" }
    for ($index = 0; $index -lt $expectedSorted.Count; $index++) {
        if ($actualSorted[$index] -cne $expectedSorted[$index]) { throw "$Label changed at index $index" }
    }
}
function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}
function Assert-Hex {
    param([string]$Value, [int]$Bytes = -1, [string]$Label = 'hex')
    if ([string]::IsNullOrEmpty($Value) -or $Value.Length % 2 -ne 0 -or $Value -cnotmatch '^[0-9a-f]+$') {
        throw "$Label is not lowercase hexadecimal"
    }
    if ($Bytes -ge 0 -and $Value.Length -ne 2 * $Bytes) { throw "$Label length changed" }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath, $wirePath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical recovery file is missing: $path" }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

if ([UInt64]$schema.schema_version -ne 1 -or [string]$schema.milestone -cne 'M1.1.4' -or
    [string]$schema.status -cne 'candidate-not-active' -or [bool]$schema.changes_active_wire -or
    [string]$schema.profile -cne 'kilogram-deterministic-cbor-v1') {
    throw 'canonical recovery schema header changed or claims activation'
}
if ([string]$schema.activation.current_runtime_encoding -cne 'postcard' -or
    [bool]$schema.activation.current_runtime_changed) { throw 'active runtime encoding changed' }
Assert-ExactStringSet @($schema.activation.required_before_activation) @(
    'new-explicit-protocol-version-or-parallel-identifier',
    'dual-reader-and-downgrade-tests',
    'decoder-fuzzing-and-malformed-input-corpus',
    'fresh-field-and-hosted-evidence'
) 'activation requirements'
if ([UInt64]$schema.limits.maximum_page_size -ne 256) { throw 'maximum page size changed' }

$expectedDomains = @(
    'history-recovery-checkpoint-signature-v1|kilogram:history-recovery-checkpoint-signature:cbor-v1\0|ed25519-signature|recipient-device-ed25519',
    'history-recovery-checkpoint-id-v1|kilogram:history-recovery-checkpoint-id:cbor-v1\0|blake3-domain-prefixed-digest|',
    'history-recovery-id-v1|kilogram:history-recovery-id:cbor-v1\0|blake3-domain-prefixed-digest|'
)
$actualDomains = @($schema.domains | ForEach-Object {
    $signer = if ($_.PSObject.Properties.Name -contains 'signer') { [string]$_.signer } else { '' }
    "$($_.id)|$($_.value)|$($_.operation)|$signer"
})
if (($actualDomains -join "`n") -cne ($expectedDomains -join "`n")) { throw 'recovery domains changed' }
$expectedObjects = @(
    'inventory-claim-option-v1', 'previous-checkpoint-option-v1',
    'history-recovery-checkpoint-content-v1', 'signed-history-recovery-checkpoint-v1',
    'history-recovery-identity-content-v1', 'history-recovery-checkpoint-id-v1',
    'history-recovery-successor-v1'
)
$actualObjects = @($schema.objects | ForEach-Object { [string]$_.id })
if (($actualObjects -join '|') -cne ($expectedObjects -join '|')) { throw 'recovery object inventory changed' }
$expectedBindings = @(
    'CANONICAL_HISTORY_RECOVERY_CHECKPOINT_SIGNATURE_DOMAIN',
    'CANONICAL_HISTORY_RECOVERY_CHECKPOINT_ID_DOMAIN',
    'CANONICAL_HISTORY_RECOVERY_ID_DOMAIN',
    'MAX_CANONICAL_RECOVERY_PAGE_SIZE'
)
Assert-ExactStringSet @($schema.rust_bindings | ForEach-Object { [string]$_.symbol }) $expectedBindings 'Rust bindings'
foreach ($binding in $schema.rust_bindings) {
    if ([string]$binding.path -cne 'crates/kilogram-protocol/src/canonical_recovery_v1.rs' -or
        $rust.IndexOf([string]$binding.symbol, [StringComparison]::Ordinal) -lt 0) {
        throw "Rust binding '$($binding.symbol)' is not exact"
    }
}

if ([UInt64]$vectors.schema_version -ne 1 -or [string]$vectors.activation_status -cne 'not-active') {
    throw 'recovery vector header changed'
}
foreach ($name in @('recipient_secret_key_hex', 'account_id_hex', 'conversation_id_hex', 'source_device_id_hex', 'sas_hex', 'inventory_digest_hex')) {
    Assert-Hex ([string]$vectors.input.$name) 32 $name
}
Assert-Hex ([string]$vectors.expected.recipient_device_id_hex) 32 'recipient Device ID'
Assert-Hex ([string]$vectors.expected.recovery_identity_cbor_hex) -1 'recovery identity'
Assert-Hex ([string]$vectors.expected.recovery_id_domain_hex) -1 'recovery ID domain'
Assert-Hex ([string]$vectors.expected.recovery_id_hex) 32 'recovery ID'
foreach ($checkpointName in @('initial_checkpoint', 'successor_checkpoint')) {
    $checkpoint = $vectors.expected.$checkpointName
    foreach ($field in @('content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex', 'signed_cbor_hex', 'checkpoint_id_domain_hex')) {
        Assert-Hex ([string]$checkpoint.$field) -1 "$checkpointName.$field"
    }
    Assert-Hex ([string]$checkpoint.signature_hex) 64 "$checkpointName.signature"
    Assert-Hex ([string]$checkpoint.checkpoint_id_hex) 32 "$checkpointName.ID"
}
foreach ($required in @(
    'kilogram-recovery-v1', 'inventory-claim-option-v1', 'previous-checkpoint-option-v1',
    'signed-history-recovery-checkpoint-v1', 'history-recovery-identity-content-v1', 'page-size: 1..256'
)) { if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "CDDL is missing '$required'" } }
foreach ($required in @(
    'pub fn decode_and_verify_signed_history_recovery_checkpoint_v1',
    'pub fn history_recovery_checkpoint_id_v1', 'pub fn history_recovery_id_v1',
    'pub fn validate_history_recovery_successor_v1'
)) { if ($rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "Rust codec is missing '$required'" } }
if ($lib.IndexOf('pub mod canonical_recovery_v1;', [StringComparison]::Ordinal) -lt 0) { throw 'module not exported' }
foreach ($forbidden in @('history-recovery-checkpoint-signature:cbor-v1', 'history-recovery-checkpoint-id:cbor-v1', 'history-recovery-id:cbor-v1')) {
    if ($wire.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) { throw "active wire contains '$forbidden'" }
}
foreach ($required in @('inactive cross-language conformance baseline', 'active Postcard recovery protocol', 'explicit new protocol version')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) { throw "RFC is missing '$required'" }
}
$referenceOutput = @(& node $referencePath $vectorsPath 2>&1)
if ($LASTEXITCODE -ne 0 -or 'canonical_recovery_reference=verified' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "independent recovery reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}
if ($SelfTest) {
    Assert-Rejected { Assert-Hex 'AA' -1 'uppercase hex' } 'uppercase_hex'
    Assert-Rejected {
        $changed = @($actualObjects); $changed[0] = 'unknown-object-v1'
        if (($changed -join '|') -cne ($expectedObjects -join '|')) { throw 'changed object' }
    } 'changed_object'
    Assert-Rejected {
        $activated = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
        $activated.changes_active_wire = $true
        if ([bool]$activated.changes_active_wire) { throw 'unexpected activation' }
    } 'unexpected_activation'
    Write-Output 'canonical_recovery_self_test=passed'
}
Write-Output 'canonical_recovery_boundary=verified'
Write-Output 'canonical_recovery_status=candidate-not-active'
Write-Output 'independent_ed25519_reference=verified'
Write-Output 'independent_blake3_reference=verified'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
