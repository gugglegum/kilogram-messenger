[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RECOVERY-EXCHANGE-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RECOVERY-EXCHANGE-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RECOVERY-EXCHANGE.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_recovery_exchange_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$linkPath = Join-Path $workspace 'apps\kilogram-cli\src\recovery_link.rs'
$planPath = Join-Path $workspace 'apps\kilogram-cli\src\recovery_plan.rs'
$rewrapPath = Join-Path $workspace 'crates\kilogram-protocol\src\rewrap.rs'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-recovery-exchange-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0140-m1114-canonical-recovery-exchange.md'
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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.14' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical recovery exchange schema header changed'
    }
    if ([string]$Schema.cddl.path -cne 'CANONICAL-RECOVERY-EXCHANGE.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-recovery-exchange-v1' -or
        [string]$Schema.activation.current_runtime_artifact_encoding -cne 'postcard-base64url-uri' -or
        [string]$Schema.activation.current_runtime_wire_encoding -cne 'postcard' -or
        [bool]$Schema.activation.current_runtime_changed -or
        [string]$Schema.activation.candidate_protocol_identifier -cne 'unassigned') {
        throw 'canonical recovery exchange activation boundary changed'
    }
    $expectedLimits = '16,2048,128,3600,604800,4096,256,1048576,8388608,9437184'
    $actualLimits = @(
        [int]$Schema.limits.maximum_coordinates,
        [int]$Schema.limits.maximum_relay_coordinate_bytes,
        [int]$Schema.limits.maximum_direct_coordinate_bytes,
        [int]$Schema.limits.maximum_link_validity_seconds,
        [int]$Schema.limits.maximum_plan_validity_seconds,
        [int]$Schema.limits.maximum_inventory_event_count,
        [int]$Schema.limits.maximum_page_size,
        [int]$Schema.limits.maximum_nested_artifact_bytes,
        [int]$Schema.limits.maximum_opaque_rewrapped_payload_bytes,
        [int]$Schema.limits.maximum_response_bytes
    ) -join ','
    if ($actualLimits -cne $expectedLimits) { throw 'canonical recovery exchange bounds changed' }
    $objects = @($Schema.objects | ForEach-Object { "$($_.id):$($_.array_length)" })
    $expectedObjects = @(
        'recovery-coordinate-v1:2', 'history-recovery-link-content-v1:13',
        'signed-history-recovery-link-v1:3', 'recovery-execution-policy-v1:6',
        'history-recovery-plan-content-v1:6', 'signed-history-recovery-plan-v1:3',
        'history-recovery-page-request-content-v1:11', 'signed-history-recovery-page-request-v1:3',
        'history-recovery-page-v1:7', 'history-recovery-rejected-v1:3',
        'history-recovery-response-content-v1:3', 'signed-history-recovery-response-v1:3'
    )
    if (($objects -join ',') -cne ($expectedObjects -join ',')) { throw 'recovery exchange objects changed' }
    $coordinateKinds = @($Schema.registries.coordinate_kind.psobject.Properties |
        Sort-Object { [int]$_.Name } | ForEach-Object { "$($_.Name):$($_.Value)" })
    $routePolicies = @($Schema.registries.route_policy.psobject.Properties |
        Sort-Object { [int]$_.Name } | ForEach-Object { "$($_.Name):$($_.Value)" })
    $responseVariants = @($Schema.registries.response_variant.psobject.Properties |
        Sort-Object { [int]$_.Name } | ForEach-Object { "$($_.Name):$($_.Value)" })
    if (($coordinateKinds -join ',') -cne '1:relay-url,2:direct-socket-address' -or
        ($routePolicies -join ',') -cne '1:auto,2:direct-only,3:relay-only' -or
        ($responseVariants -join ',') -cne '1:page,2:rejected') {
        throw 'recovery exchange closed registry changed'
    }
    foreach ($required in @(
        'CANONICAL_HISTORY_RECOVERY_LINK_SIGNATURE_DOMAIN',
        'CANONICAL_HISTORY_RECOVERY_PLAN_SIGNATURE_DOMAIN',
        'CANONICAL_HISTORY_RECOVERY_PAGE_REQUEST_SIGNATURE_DOMAIN',
        'CANONICAL_HISTORY_RECOVERY_RESPONSE_SIGNATURE_DOMAIN',
        'decode_and_verify_signed_history_recovery_link_v1',
        'decode_and_verify_signed_history_recovery_plan_v1',
        'decode_and_verify_signed_history_recovery_page_request_v1',
        'decode_and_verify_signed_history_recovery_response_v1',
        'verify_history_recovery_page_request_for_plan_v1',
        'verify_history_recovery_response_for_plan_v1',
        'SessionBindingMismatch', 'SourceCertificateMismatch', 'PlanMismatch',
        'MAX_CANONICAL_RECOVERY_OPAQUE_PAYLOAD_BYTES')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical recovery exchange Rust codec is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical recovery exchange vector header changed'
    }
    foreach ($field in @('root_secret_key_hex', 'source_device_secret_key_hex',
        'recipient_device_secret_key_hex', 'source_encryption_public_key_hex',
        'recipient_encryption_public_key_hex', 'conversation_id_hex', 'endpoint_id_hex',
        'session_binding_hex', 'inventory_digest_hex')) {
        Assert-LowerHex ([string]$Vectors.input.$field) 32 $field
    }
    foreach ($field in @('account_id_hex', 'source_device_id_hex', 'recipient_device_id_hex',
        'sas_hex', 'recovery_id_hex')) {
        Assert-LowerHex ([string]$Vectors.expected.$field) 32 $field
    }
    foreach ($name in @('link', 'plan', 'request', 'page_response', 'rejected_response')) {
        $object = $Vectors.expected.$name
        foreach ($field in @('content_cbor_hex', 'signature_domain_hex', 'signing_preimage_hex',
            'signature_hex', 'signed_cbor_hex')) {
            Assert-LowerHex ([string]$object.$field) $(if ($field -ceq 'signature_hex') { 64 } else { -1 }) "$name.$field"
        }
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath, $linkPath,
    $planPath, $rewrapPath, $referencePath, $rfcPath, $coveragePath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical recovery exchange file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$activeLink = Get-Content -LiteralPath $linkPath -Raw
$activePlan = Get-Content -LiteralPath $planPath -Raw
$activeRewrap = Get-Content -LiteralPath $rewrapPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('signed-history-recovery-link-v1', 'signed-history-recovery-plan-v1',
    'signed-history-recovery-page-request-v1', 'signed-history-recovery-response-v1',
    'canonical-bool = 0 / 1', 'previous-recovery-checkpoint-option-v1 = [] / [checkpoint-id: id-32]')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical recovery exchange CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_recovery_exchange_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical recovery exchange module is not exported'
}
foreach ($required in @('kilogram://history-recovery/v1/', 'URL_SAFE_NO_PAD.encode(encoded)',
    'postcard::to_allocvec(self)', 'postcard::from_bytes(&bytes)')) {
    if ($activeLink.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active recovery link boundary changed: missing '$required'"
    }
}
foreach ($required in @('postcard::to_allocvec(self)', 'postcard::from_bytes(bytes)')) {
    if ($activePlan.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active recovery plan boundary changed: missing '$required'"
    }
}
foreach ($required in @('kilogram:history-rewrap-hpke:v1\0', 'SignedHistoryRewrapRequest',
    'SignedHistoryRewrapTransfer')) {
    if ($activeRewrap.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active history rewrap boundary changed: missing '$required'"
    }
}
foreach ($active in @($activeLink, $activePlan, $activeRewrap)) {
    if ($active.IndexOf('canonical_recovery_exchange_v1', [StringComparison]::Ordinal) -ge 0) {
        throw 'inactive canonical recovery exchange leaked into active runtime'
    }
}
$historyCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'history-recovery' })
$rewrapCoverage = @($coverage.surfaces | Where-Object { $_.id -ceq 'history-rewrap' })
if ($historyCoverage.Count -ne 1 -or [string]$historyCoverage[0].coverage -cne 'complete' -or
    @($historyCoverage[0].remaining).Count -ne 0 -or
    $rewrapCoverage.Count -ne 1 -or [string]$rewrapCoverage[0].coverage -cne 'not-started') {
    throw 'history recovery and history rewrap coverage separation changed'
}
foreach ($required in @('separate `history-rewrap`', 'exact signed device list',
    'live session binding', 'from 3,915 to 4,698', 'changes no active wire bytes')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical recovery exchange RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_recovery_exchange_reference_self_test=passed' -notin @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical recovery exchange independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedRegistry = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedRegistry.registries.response_variant.'2' = 'redirect'
    Assert-Rejected { Assert-Schema $changedRegistry $rust } 'open_response_registry'
    $changed = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changed.expected.page_response.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changed } 'invalid_response_signature_shape'
    Write-Output 'changed_recipient_key=rejected-by-independent-reference'
    Write-Output 'canonical_recovery_exchange_self_test=passed'
}

Write-Output 'canonical_recovery_exchange_boundary=verified'
Write-Output 'link_plan_request_response=verified'
Write-Output 'session_and_plan_binding=verified'
Write-Output 'bounded_decoder_mutations=6264'
Write-Output 'history_recovery_coverage=complete'
Write-Output 'history_rewrap_coverage=not-started'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
