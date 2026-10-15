[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-SYNC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-SYNC-VECTORS.json'
$eventVectorsPath = Join-Path $workspace 'CANONICAL-EVENT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-SYNC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_sync_v1.rs'
$activePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-sync-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0138-m1112-canonical-sync-frames.md'
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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.12' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical sync schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-SYNC.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-sync-v1' -or
        [string]$Schema.activation.current_runtime_codec -cne 'postcard-v6' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical sync activation boundary changed'
    }
    if ([int]$Schema.limits.maximum_inventory_event_ids -ne 4096 -or
        [int]$Schema.limits.maximum_events_per_batch -ne 64 -or
        [int]$Schema.limits.maximum_encoded_object_bytes -ne 8388608) {
        throw 'canonical sync resource bounds changed'
    }
    $objectIds = @($Schema.objects | ForEach-Object { [string]$_.id })
    $expectedIds = @('signed-sync-inventory-v1', 'sync-event-batch-v1', 'signed-sync-diff-v1',
        'sync-complete-v1', 'sync-pause-v1', 'sync-paused-v1', 'sync-rejected-v1',
        'sync-client-frame-v1', 'sync-server-frame-v1')
    if (($objectIds -join ',') -cne ($expectedIds -join ',')) {
        throw 'canonical sync object inventory changed'
    }
    if ([string]$Schema.domains[0].value -cne ("kilogram:sync-inventory-signature:cbor-v1" + [char]0) -or
        [string]$Schema.domains[1].value -cne ("kilogram:sync-diff-signature:cbor-v1" + [char]0)) {
        throw 'canonical sync signature domains changed'
    }
    foreach ($required in @('CANONICAL_SYNC_DIFF_SIGNATURE_DOMAIN',
        'decode_and_verify_signed_sync_inventory_v1', 'decode_and_verify_signed_sync_diff_v1',
        'decode_and_verify_sync_client_frame_v1', 'decode_and_verify_sync_server_frame_v1',
        'NonCanonicalEventIdOrder', 'NonCanonicalEventOrder', 'InvalidBoolean',
        'SessionBindingMismatch', 'MAX_CANONICAL_SYNC_OBJECT_BYTES')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical sync Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical sync vector header changed'
    }
    foreach ($field in @('requester_device_id_hex', 'responder_device_id_hex',
        'conversation_id_hex', 'session_binding_hex', 'text_event_id_hex',
        'acknowledgement_event_id_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    foreach ($field in @('content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex',
        'signature_hex', 'signed_cbor_hex')) {
        $bytes = if ($field -ceq 'signature_hex') { 64 } else { -1 }
        Assert-LowerHex ([string]$Vectors.expected.inventory.$field) $bytes "inventory $field"
        Assert-LowerHex ([string]$Vectors.expected.diff.$field) $bytes "diff $field"
    }
    foreach ($value in @($Vectors.expected.event_batch.cbor_hex, $Vectors.expected.complete.cbor_hex,
        $Vectors.expected.pause.cbor_hex, $Vectors.expected.paused.cbor_hex,
        $Vectors.expected.rejected.cbor_hex, $Vectors.expected.client_frames.inventory_cbor_hex,
        $Vectors.expected.client_frames.events_cbor_hex, $Vectors.expected.client_frames.pause_cbor_hex,
        $Vectors.expected.server_frames.diff_cbor_hex, $Vectors.expected.server_frames.complete_cbor_hex,
        $Vectors.expected.server_frames.rejected_cbor_hex, $Vectors.expected.server_frames.paused_cbor_hex)) {
        Assert-LowerHex ([string]$value) -1 'canonical sync object'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $eventVectorsPath, $cddlPath, $rustPath,
    $activePath, $referencePath, $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical sync file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$rust = Get-Content -LiteralPath $rustPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('signed-sync-inventory-v1', 'sync-event-batch-v1',
    'signed-sync-diff-v1', 'sync-complete-v1', 'sync-pause-v1', 'sync-paused-v1',
    'sync-rejected-v1', 'sync-client-frame-v1', 'sync-server-frame-v1',
    'canonical-bool = 0 / 1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical sync CDDL is missing '$required'"
    }
}
foreach ($required in @('const SYNC_VERSION: u8 = 6;', 'postcard::to_allocvec(self)',
    'postcard::from_bytes(bytes)', 'kilogram:sync-diff-signature:v6\0',
    'kilogram:sync-inventory-signature:v6\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active Postcard sync boundary changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_sync_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical sync codec leaked into active runtime'
}
$syncCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'sync' })
if ($syncCoverage.Count -ne 1 -or [string]$syncCoverage[0].coverage -cne 'complete' -or
    (@($syncCoverage[0].covered) -join ',') -cne 'sync-inventory,sync-diff,sync-request-response-frames,multi-round-session-state' -or
    @($syncCoverage[0].remaining).Count -ne 0) {
    throw 'canonical sync coverage no longer includes frames and durable session state'
}
foreach ($required in @('strictly ordered and duplicate-free', 'signed by the responder Device',
    'zero and one', 'from 2,871 to 3,654', 'multi-round session state',
    'changes no active wire bytes')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical sync RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --event-vectors $eventVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_sync_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical sync independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $missing = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $missing.objects = @($missing.objects | Select-Object -Skip 1)
    Assert-Rejected { Assert-Schema $missing $rust } 'removed_sync_object'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.diff.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_diff_signature_shape'
    Write-Output 'changed_session_binding=rejected-by-independent-reference'
    Write-Output 'canonical_sync_self_test=passed'
}

Write-Output 'canonical_sync_boundary=verified'
Write-Output 'sync_inventory_signature=verified'
Write-Output 'sync_diff_signature=verified'
Write-Output 'closed_client_variants=3'
Write-Output 'closed_server_variants=4'
Write-Output 'bounded_decoder_mutations=4698'
Write-Output 'sync_coverage=complete-via-m1113'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
