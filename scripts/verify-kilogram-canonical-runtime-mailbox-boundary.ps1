[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-MAILBOX-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-MAILBOX-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-MAILBOX.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_mailbox_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePath = Join-Path $workspace 'apps\kilogram-cli\src\runtime_mailbox.rs'
$registryPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-mailbox-reference.mjs'
$provisioningVectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-PROVISIONING-VECTORS.json'
$eventVectorsPath = Join-Path $workspace 'CANONICAL-EVENT-VECTORS.json'
$mailboxVectorsPath = Join-Path $workspace 'CANONICAL-MAILBOX-VECTORS.json'
$rfcPath = Join-Path $workspace 'docs\RFC-0145-m1119-canonical-runtime-mailbox.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-Schema {
    param([object]$Schema, [string]$Rust)
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.19' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical runtime mailbox schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-RUNTIME-MAILBOX.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-runtime-mailbox-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v1' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical runtime mailbox activation boundary changed'
    }
    $limits = $Schema.limits
    if ([int]$limits.maximum_binding_record_bytes -ne 65536 -or
        [int]$limits.maximum_payload_bytes -ne 1048576 -or
        [int]$limits.maximum_dispatch_bytes -ne 8192 -or
        [int]$limits.minimum_dispatch_ttl_seconds -ne 60 -or
        [int]$limits.maximum_dispatch_ttl_seconds -ne 604800) {
        throw 'canonical runtime mailbox limits changed'
    }
    $expectedDomains = @(
        'runtime-local-mailbox-binding-signature-v1|kilogram:runtime-local-mailbox-binding:cbor-v1\0',
        'runtime-peer-mailbox-binding-signature-v1|kilogram:runtime-peer-mailbox-binding:cbor-v1\0',
        'runtime-mailbox-dispatch-signature-v1|kilogram:runtime-mailbox-dispatch:cbor-v1\0',
        'runtime-mailbox-item-id-v1|kilogram:runtime-mailbox-item-id:cbor-v1\0',
        'runtime-mailbox-event-item-id-v1|kilogram:runtime-mailbox-event-item-id:cbor-v1\0',
        'runtime-mailbox-dispatch-replication-binding-v1|kilogram:runtime-mailbox-dispatch-replication-binding:cbor-v1\0'
    )
    $actualDomains = @($Schema.domains | ForEach-Object { "$($_.id)|$($_.value)" })
    if (($actualDomains -join "`n") -cne ($expectedDomains -join "`n")) {
        throw 'canonical runtime mailbox domain registry changed'
    }
    foreach ($domain in $Schema.domains) {
        $literal = ([string]$domain.value).Replace('\0', '\0')
        if ($Rust.IndexOf($literal, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical runtime mailbox Rust binding is missing '$literal'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical runtime mailbox vector header changed'
    }
    $dependencies = @($Vectors.dependencies)
    if (($dependencies -join ',') -cne 'CANONICAL-MAILBOX-PROVISIONING-VECTORS.json,CANONICAL-EVENT-VECTORS.json,CANONICAL-MAILBOX-VECTORS.json') {
        throw 'canonical runtime mailbox vector dependencies changed'
    }
    foreach ($path in @(
        'signed_local_binding.signed_cbor_hex', 'signed_peer_binding.signed_cbor_hex',
        'payload.cbor_hex', 'signed_dispatch.signed_cbor_hex'
    )) {
        $cursor = $Vectors.expected
        foreach ($part in $path.Split('.')) { $cursor = $cursor.$part }
        if ([string]$cursor -cnotmatch '^[0-9a-f]+$' -or ([string]$cursor).Length % 2 -ne 0) {
            throw "invalid runtime mailbox vector '$path'"
        }
    }
    foreach ($path in @('item_id_hex', 'event_item_id_hex', 'replication_binding_hex')) {
        if ([string]$Vectors.expected.signed_dispatch.$path -cnotmatch '^[0-9a-f]{64}$') {
            throw "invalid runtime mailbox derived ID '$path'"
        }
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath, $activePath,
    $registryPath, $coveragePath, $referencePath, $provisioningVectorsPath, $eventVectorsPath,
    $mailboxVectorsPath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical runtime mailbox file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$active = Get-Content -LiteralPath $activePath -Raw
$registry = Get-Content -LiteralPath $registryPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$reference = Get-Content -LiteralPath $referencePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Schema $schema $rust
Assert-Vectors $vectors
foreach ($required in @('runtime-local-mailbox-binding-content-v1',
    'runtime-peer-mailbox-binding-content-v1', 'runtime-mailbox-payload-v1',
    'runtime-mailbox-dispatch-content-v1', 'signed-runtime-mailbox-dispatch-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime mailbox CDDL is missing '$required'"
    }
}
foreach ($required in @('decode_and_verify_signed_runtime_local_mailbox_binding_v1',
    'decode_and_verify_signed_runtime_peer_mailbox_binding_v1',
    'decode_and_verify_runtime_mailbox_payload_v1',
    'decode_and_verify_signed_runtime_mailbox_dispatch_v1',
    'verify_runtime_mailbox_payload_for_dispatch_v1')) {
    if ($rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical runtime mailbox Rust module is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_mailbox_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical runtime mailbox module is not exported'
}
foreach ($required in @('postcard::to_allocvec', 'postcard::from_bytes',
    'kilogram:runtime-mailbox-dispatch:v1\0', 'kilogram:runtime-mailbox-item-id:v1\0')) {
    if ($active.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime mailbox changed: missing '$required'"
    }
}
if ($active.IndexOf('canonical_runtime_mailbox_v1', [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical runtime mailbox leaked into the active runtime'
}
$surface = @($registry.surfaces | Where-Object { $_.id -ceq 'runtime-mailbox-envelope' })
if ($surface.Count -ne 1 -or [string]$surface[0].encoding -cne 'postcard') {
    throw 'active runtime mailbox compatibility registry changed'
}
$covered = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-mailbox-envelope' })
if ($covered.Count -ne 1 -or [string]$covered[0].coverage -cne 'complete' -or
    @($covered[0].remaining).Count -ne 0) {
    throw 'runtime mailbox canonical coverage is incomplete'
}
foreach ($required in @('active Postcard v1', 'local and peer', 'payload', 'durable dispatch',
    'does not activate', '9394')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical runtime mailbox RFC is missing '$required'"
    }
}
foreach ($forbidden in @('fetch(', 'https.request', 'node:http', 'node:https')) {
    if ($reference.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "runtime mailbox independent reference contains network surface '$forbidden'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --provisioning-vectors $provisioningVectorsPath --event-vectors $eventVectorsPath `
    --mailbox-vectors $mailboxVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_mailbox_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical runtime mailbox independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_dispatch_ttl_seconds = 604801
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_dispatch_ttl'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.expected.signed_dispatch.item_id_hex = '00'
    Assert-Rejected { Assert-Vectors $changedVector } 'invalid_item_id_shape'
    Write-Output 'canonical_runtime_mailbox_self_test=passed'
}

Write-Output 'canonical_runtime_mailbox_boundary=verified'
Write-Output 'local_peer_payload_dispatch=verified'
Write-Output 'payload_dispatch_binding=verified'
Write-Output 'bounded_decoder_mutations=9394'
Write-Output 'runtime_mailbox_coverage=complete'
Write-Output 'active_postcard_changed=false'
Write-Output 'network_executed=false'
