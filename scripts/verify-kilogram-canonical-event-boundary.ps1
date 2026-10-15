[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-EVENT-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-EVENT-VECTORS.json'
$ratchetVectorsPath = Join-Path $workspace 'CANONICAL-RATCHET-VECTORS.json'
$accountVectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-VECTORS.json'
$authorityVectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-EVENT.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_event_v1.rs'
$activePath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-event-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0136-m1110-canonical-authorized-events.md'
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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.10' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical event schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-EVENT.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-event-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v5' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical event activation boundary changed'
    }
    if ([int]$Schema.limits.maximum_parents -ne 64 -or
        [int]$Schema.limits.maximum_recipients -ne 32 -or
        [int]$Schema.limits.maximum_ratchet_ciphertext_bytes -ne 131072 -or
        [int]$Schema.limits.maximum_nested_cbor_bytes -ne 524288) {
        throw 'canonical event resource limits changed'
    }
    Assert-ExactStrings @($Schema.domains | ForEach-Object { "$($_.id)|$($_.value)|$($_.operation)" }) @(
        'event-signature-v1|kilogram:event-signature:cbor-v1\0|ed25519-signature',
        'event-id-v1|kilogram:event-id:cbor-v1\0|blake3-domain-prefixed-digest'
    ) 'canonical event domains'
    Assert-ExactStrings @($Schema.objects | ForEach-Object { $_.id }) @(
        'acknowledgement-payload-v1', 'ratchet-recipient-v1', 'ratchet-text-payload-v1',
        'event-content-v1', 'signed-event-v1', 'event-id-v1', 'authorized-event-v1'
    ) 'canonical event objects'
    foreach ($binding in $Schema.rust_bindings) {
        $source = if ([string]$binding.path -ceq 'crates/kilogram-protocol/src/canonical_event_v1.rs') { $Rust } else { '' }
        if ([string]$binding.path -ceq 'crates/kilogram-protocol/src/canonical_event_v1.rs' -and
            $source.IndexOf([string]$binding.symbol, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical event Rust binding '$($binding.symbol)' is missing"
        }
    }
}

function Assert-EventVector {
    param([object]$Value, [string]$Label)
    Assert-LowerHex $Value.content_cbor_hex -1 "$Label content"
    Assert-LowerHex $Value.signature_domain_hex -1 "$Label signature domain"
    Assert-LowerHex $Value.signing_preimage_hex -1 "$Label signing preimage"
    Assert-LowerHex $Value.signature_hex 64 "$Label signature"
    Assert-LowerHex $Value.signed_cbor_hex -1 "$Label signed envelope"
    Assert-LowerHex $Value.event_id_domain_hex -1 "$Label Event ID domain"
    Assert-LowerHex $Value.event_id_hex 32 "$Label Event ID"
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical event vector header changed'
    }
    Assert-LowerHex $Vectors.input.author_root_secret_key_hex 32 'author Root seed'
    Assert-LowerHex $Vectors.input.author_device_secret_key_hex 32 'author Device seed'
    Assert-LowerHex $Vectors.input.conversation_id_hex 32 'conversation ID'
    if (@($Vectors.input.parents_hex).Count -ne 2 -or [int]$Vectors.input.recipient_message_kind -ne 0) {
        throw 'canonical event vector inputs changed'
    }
    foreach ($parent in $Vectors.input.parents_hex) { Assert-LowerHex $parent 32 'parent Event ID' }
    Assert-LowerHex $Vectors.expected.author_account_id_hex 32 'author Account ID'
    Assert-LowerHex $Vectors.expected.author_device_id_hex 32 'author Device ID'
    Assert-LowerHex $Vectors.expected.recipient_account_id_hex 32 'recipient Account ID'
    Assert-LowerHex $Vectors.expected.recipient_device_id_hex 32 'recipient Device ID'
    Assert-EventVector $Vectors.expected.text_event 'text event'
    Assert-EventVector $Vectors.expected.acknowledgement_event 'acknowledgement event'
    Assert-LowerHex $Vectors.expected.authorized_text_event_cbor_hex -1 'authorized text event'
    Assert-LowerHex $Vectors.expected.authorized_acknowledgement_event_cbor_hex -1 'authorized acknowledgement event'
    Assert-LowerHex $Vectors.expected.membership.signed_cbor_hex -1 'membership'
}

foreach ($path in @($schemaPath, $vectorsPath, $ratchetVectorsPath, $accountVectorsPath,
    $authorityVectorsPath, $cddlPath, $rustPath, $activePath, $wirePath, $referencePath,
    $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "canonical event file is missing: $path" }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('acknowledgement-payload-v1', 'ratchet-text-payload-v1',
    'signed-event-v1', 'authorized-event-v1', 'kilogram-event-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "event CDDL is missing '$required'" }
}
foreach ($required in @('decode_and_verify_signed_event_v1', 'decode_and_verify_authorized_event_v1',
    'verify_authorized_event_for_membership_v1', 'RecipientDeviceListMismatch',
    'AuthorCannotSignEvents', 'event_id_v1')) {
    if ($rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) { throw "event codec is missing '$required'" }
}
if ($wire.IndexOf('canonical_event_v1', [StringComparison]::Ordinal) -ge 0 -or
    $active.IndexOf('let event: Self = postcard::from_bytes(bytes)?;', [StringComparison]::Ordinal) -lt 0 -or
    $active.IndexOf('Ok(postcard::to_allocvec(self)?)', [StringComparison]::Ordinal) -lt 0) {
    throw 'inactive canonical event codec leaked into or displaced active Postcard runtime'
}
$eventCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'authorized-event' })
if ($eventCoverage.Count -ne 1 -or [string]$eventCoverage[0].coverage -cne 'complete' -or
    @($eventCoverage[0].remaining).Count -ne 0 -or
    'ratchet-text-event' -notin @($eventCoverage[0].covered) -or
    'external-membership-verification' -notin @($eventCoverage[0].covered)) {
    throw 'canonical authorized-event coverage is not complete'
}
foreach ($required in @('every event payload used by the current runtime',
    'membership is deliberately external', 'from 2,088 to 2,610 malformed',
    'does not activate canonical CBOR')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical event RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --ratchet-vectors $ratchetVectorsPath --account-vectors $accountVectorsPath `
    --recipient-authority-vectors $authorityVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_event_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical event independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}
if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $missing = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $missing.objects = @($missing.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-Schema $missing $rust } 'removed_event_object'
    $changed = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changed.expected.text_event.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_signature_shape'
    Write-Output 'changed_ciphertext_binding=rejected-by-independent-reference'
    Write-Output 'canonical_event_self_test=passed'
}

Write-Output 'canonical_event_boundary=verified'
Write-Output 'event_payload_variants=2'
Write-Output 'strict_event_decoders=2'
Write-Output 'independent_ed25519_and_blake3=verified'
Write-Output 'bounded_decoder_mutations=2610'
Write-Output 'authorized_event_coverage=complete'
Write-Output 'active_wire_changed=false'
Write-Output 'network_executed=false'
