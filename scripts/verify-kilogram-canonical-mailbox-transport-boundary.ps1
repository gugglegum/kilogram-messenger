[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-MAILBOX-TRANSPORT-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-TRANSPORT-VECTORS.json'
$mailboxVectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-MAILBOX-TRANSPORT.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_mailbox_transport_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeWirePath = Join-Path $workspace 'crates\kilogram-mailbox\src\wire.rs'
$activeTransportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-transport-reference.mjs'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0142-m1116-canonical-blind-mailbox-transport.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.16' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical mailbox transport schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-MAILBOX-TRANSPORT.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-mailbox-transport-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard' -or
        [string]$Schema.activation.current_runtime_alpn -cne 'kilogram/m0/blind-mailbox/1' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical mailbox transport activation boundary changed'
    }
    $limits = @(
        [int]$Schema.limits.transport_overhead_bytes,
        [int]$Schema.limits.maximum_request_bytes,
        [int]$Schema.limits.maximum_response_bytes,
        [int]$Schema.limits.maximum_peer_request_bytes,
        [int]$Schema.limits.maximum_peer_response_bytes,
        [int]$Schema.limits.maximum_page_items
    ) -join ','
    if ($limits -cne '4096,1052672,8425472,1056768,8429568,8') {
        throw 'canonical mailbox transport bounds changed'
    }
    if ([string]$Schema.registries.peer_operation.'1' -cne 'put' -or
        [string]$Schema.registries.peer_operation.'2' -cne 'list' -or
        [string]$Schema.registries.peer_operation.'3' -cne 'delete' -or
        [string]$Schema.registries.peer_rejection.'1' -cne 'rate-limited' -or
        [string]$Schema.registries.peer_rejection.'4' -cne 'store-unavailable') {
        throw 'canonical mailbox transport closed registry changed'
    }
    foreach ($required in @(
        'CANONICAL_MAILBOX_PEER_REQUEST_DIGEST_DOMAIN',
        'decode_and_verify_mailbox_put_request_v1',
        'decode_and_verify_mailbox_list_request_v1',
        'decode_and_verify_mailbox_delete_request_v1',
        'decode_and_verify_mailbox_peer_request_v1',
        'decode_and_verify_mailbox_put_response_v1',
        'decode_and_verify_mailbox_list_response_v1',
        'decode_and_verify_mailbox_delete_response_v1',
        'decode_and_verify_mailbox_peer_response_v1',
        'verify_mailbox_peer_response_for_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical mailbox transport Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        [string]$Vectors.dependency -cne 'CANONICAL-MAILBOX-VECTORS.json') {
        throw 'canonical mailbox transport vector header changed'
    }
    foreach ($field in @('store_key_hex', 'mailbox_id_hex', 'item_id_hex')) {
        Assert-LowerHex ([string]$Vectors.input.$field) 32 $field
    }
    Assert-LowerHex ([string]$Vectors.expected.peer_request_digest_domain_hex) -1 'peer digest domain'
    foreach ($operation in @('put', 'list', 'delete')) {
        Assert-LowerHex ([string]$Vectors.expected.peer_requests.$operation.cbor_hex) -1 "$operation request"
        Assert-LowerHex ([string]$Vectors.expected.peer_requests.$operation.digest_hex) 32 "$operation digest"
    }
    foreach ($field in @('put_success_cbor_hex', 'list_success_cbor_hex',
        'delete_success_cbor_hex', 'put_rejected_cbor_hex', 'list_rejected_cbor_hex',
        'delete_rejected_cbor_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.peer_responses.$field) -1 $field
    }
    if (@($Vectors.expected.response_variants.PSObject.Properties).Count -ne 6) {
        throw 'terminal response vector inventory changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $mailboxVectorsPath, $cddlPath, $rustPath,
    $libPath, $activeWirePath, $activeTransportPath, $referencePath, $coveragePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical mailbox transport file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$activeWire = Get-Content -LiteralPath $activeWirePath -Raw
$activeTransport = Get-Content -LiteralPath $activeTransportPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('mailbox-put-request-v1', 'mailbox-list-request-v1',
    'mailbox-delete-request-v1', 'mailbox-peer-request-v1', 'mailbox-put-response-v1',
    'mailbox-list-response-v1', 'mailbox-delete-response-v1', 'mailbox-peer-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical mailbox transport CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_mailbox_transport_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical mailbox transport module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes',
    'kilogram:blind-mailbox-peer-request:v1\0')) {
    if ($activeWire.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active mailbox wire changed: missing '$required'"
    }
}
if ($activeTransport.IndexOf('kilogram/m0/blind-mailbox/1', [StringComparison]::Ordinal) -lt 0) {
    throw 'active mailbox Iroh ALPN changed'
}
foreach ($activeSource in @($activeWire, $activeTransport)) {
    if ($activeSource.IndexOf('canonical_mailbox_transport_v1', [StringComparison]::Ordinal) -ge 0) {
        throw 'inactive canonical mailbox transport leaked into the active runtime'
    }
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'blind-mailbox-transport' })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'blind-mailbox-transport coverage is not complete'
}
foreach ($required in @('complete `blind-mailbox-transport`', 'put/list/delete',
    'request digest', 'Postcard', 'ALPN', 'No active mailbox')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical mailbox transport RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --mailbox-vectors $mailboxVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_mailbox_transport_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical mailbox transport independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $active = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $active.status = 'active'
    Assert-Rejected { Assert-Schema $active $rust } 'active_without_migration'
    $opened = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $opened.registries.peer_operation.'3' = 'peek'
    Assert-Rejected { Assert-Schema $opened $rust } 'changed_operation_registry'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.peer_requests.put.digest_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_request_digest_shape'
    Write-Output 'canonical_mailbox_transport_self_test=passed'
}

Write-Output 'canonical_mailbox_transport_boundary=verified'
Write-Output 'put_list_delete_frames=verified'
Write-Output 'peer_request_digest_and_outcomes=verified'
Write-Output 'blind_mailbox_transport_coverage=complete'
Write-Output 'active_postcard_alpn_changed=false'
Write-Output 'network_executed=false'
