[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-MAILBOX-GOSSIP-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-GOSSIP-VECTORS.json'
$mailboxVectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-MAILBOX-GOSSIP.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_mailbox_gossip_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'crates\kilogram-mailbox-client\src\provider.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-mailbox-gossip-reference.mjs'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0143-m1117-canonical-blind-mailbox-provider-gossip.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.17' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-MAILBOX-GOSSIP.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-mailbox-provider-gossip-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical mailbox gossip schema boundary changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_selection,
        [int]$Schema.limits.maximum_gossip_offers,
        [int]$Schema.limits.maximum_gossip_hops,
        [int]$Schema.limits.maximum_gossip_age_seconds,
        [int]$Schema.limits.maximum_gossip_offer_bytes,
        [int]$Schema.limits.maximum_frame_validity_seconds,
        [int]$Schema.limits.maximum_frame_bytes,
        [int]$Schema.limits.default_admission_work_bits
    ) -join ','
    if ($limits -cne '8,8,2,900,2048,60,20480,8') {
        throw 'canonical mailbox gossip limits changed'
    }
    if (@($Schema.domains).Count -ne 4) { throw 'canonical mailbox gossip domain inventory changed' }
    foreach ($required in @(
        'CANONICAL_MAILBOX_PROVIDER_OFFER_ID_DOMAIN',
        'CANONICAL_MAILBOX_PROVIDER_SELECTION_DOMAIN',
        'CANONICAL_MAILBOX_PROVIDER_GOSSIP_SELECTION_DOMAIN',
        'CANONICAL_MAILBOX_PROVIDER_GOSSIP_FRAME_ID_DOMAIN',
        'encode_mailbox_provider_selection_transcript_v1',
        'select_mailbox_provider_candidates_v1',
        'select_mailbox_provider_gossip_candidates_v1',
        'decode_and_verify_mailbox_provider_gossip_frame_v1',
        'decode_and_verify_mailbox_provider_gossip_frame_for_id_v1',
        'verify_mailbox_provider_gossip_reply_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical mailbox gossip Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active' -or
        [string]$Vectors.dependency -cne 'CANONICAL-MAILBOX-VECTORS.json') {
        throw 'canonical mailbox gossip vector header changed'
    }
    foreach ($field in @('transport_identity_hex', 'selection_salt_hex',
        'request_entropy_hex', 'reply_entropy_hex')) {
        Assert-LowerHex ([string]$Vectors.input.$field) 32 $field
    }
    foreach ($field in @('offer_id_hex', 'selection_rank_hex', 'gossip_selection_rank_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    Assert-LowerHex ([string]$Vectors.expected.selection_transcript_cbor_hex) -1 'selection transcript'
    foreach ($frame in @('request', 'reply')) {
        Assert-LowerHex ([string]$Vectors.expected.$frame.entry_cbor_hex) -1 "$frame entry"
        Assert-LowerHex ([string]$Vectors.expected.$frame.frame_cbor_hex) -1 "$frame CBOR"
        Assert-LowerHex ([string]$Vectors.expected.$frame.frame_id_hex) 32 "$frame ID"
        if ([int]$Vectors.expected.$frame.frame_bytes -gt 20480) { throw "$frame exceeds frame bound" }
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $mailboxVectorsPath, $cddlPath, $rustPath,
    $libPath, $activePath, $referencePath, $coveragePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical mailbox gossip file is missing: $path"
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

foreach ($required in @('mailbox-provider-selection-transcript-v1',
    'mailbox-provider-gossip-entry-v1', 'mailbox-provider-gossip-frame-v1',
    'mailbox-provider-frame-option-v1 = [] / [frame-id: id-32]')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical mailbox gossip CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_mailbox_gossip_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical mailbox gossip module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes',
    'kilogram:mailbox-provider-offer-id:v1\0',
    'kilogram:mailbox-provider-selection:v1\0',
    'kilogram:mailbox-provider-gossip-selection:v1\0',
    'kilogram:mailbox-provider-gossip-frame-id:v1\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active mailbox provider gossip changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_mailbox_gossip_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical mailbox gossip leaked into the active runtime'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'blind-mailbox-provider-gossip' })
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'blind-mailbox-provider-gossip coverage is not complete'
}
foreach ($required in @('complete `blind-mailbox-provider-gossip`', 'offer ID',
    'selection', 'frame ID', 'Postcard', 'installation-local')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical mailbox gossip RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --mailbox-vectors $mailboxVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_mailbox_gossip_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical mailbox gossip independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_gossip_hops = 3
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_hop_limit'
    $changed = $vectors | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changed.expected.request.frame_id_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_frame_id_shape'
    Write-Output 'canonical_mailbox_gossip_self_test=passed'
}

Write-Output 'canonical_mailbox_gossip_boundary=verified'
Write-Output 'offer_selection_and_frame_ids=verified'
Write-Output 'bounded_request_reply_gossip=verified'
Write-Output 'blind_mailbox_provider_gossip_coverage=complete'
Write-Output 'active_postcard_changed=false'
Write-Output 'network_executed=false'
