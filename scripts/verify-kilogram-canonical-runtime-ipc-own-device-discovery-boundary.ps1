[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_own_device_discovery_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-own-device-discovery-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0167-m1141-canonical-runtime-ipc-own-device-ticket-discovery.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.49' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v29-with-json-descriptor-v10' -or
        -not [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical own-device discovery schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_own_device_discovery_request_bytes,
        [int]$Schema.limits.maximum_own_device_discovery_response_bytes,
        [int]$Schema.limits.maximum_own_device_discovery_policies,
        [int]$Schema.limits.maximum_own_device_discovery_service_origin_bytes,
        [int]$Schema.limits.minimum_own_device_discovery_publication_ttl_seconds,
        [int]$Schema.limits.maximum_own_device_discovery_publication_ttl_seconds,
        [int]$Schema.limits.minimum_own_device_discovery_refresh_before_seconds,
        [int]$Schema.limits.maximum_own_device_discovery_refresh_before_seconds,
        [int]$Schema.limits.minimum_own_device_discovery_announcement_interval_seconds,
        [int]$Schema.limits.maximum_own_device_discovery_announcement_interval_seconds,
        [int]$Schema.limits.maximum_own_device_discovery_announcement_validity_seconds,
        [int]$Schema.limits.maximum_own_device_discovery_retry_seconds
    ) -join ','
    if ($limits -cne '786432,524288,32,512,30,3600,10,1800,30,86400,2592000,3600') {
        throw 'canonical own-device discovery limits changed'
    }
    if (@($Schema.domains).Count -ne 78 -or @($Schema.objects).Count -ne 38 -or
        @($Schema.covered_commands).Count -ne 34 -or @($Schema.covered_results).Count -ne 33 -or
        @($Schema.semantic_rules).Count -ne 179 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 0) {
        throw 'canonical own-device discovery inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-own-device-discovery-policy-id:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-discovery-snapshot-id:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-discovery-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-discovery-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-discovery-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical own-device discovery domain is missing: $domain"
        }
    }
    foreach ($required in @('configure-own-device-ticket-discovery',
        'own-device-ticket-discovery-status')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical own-device discovery command is missing: $required"
        }
    }
    foreach ($required in @('own-device-ticket-discovery-configured',
        'own-device-ticket-discovery-status')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical own-device discovery result is missing: $required"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_REQUEST_BYTES: usize = 768 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_DISCOVERY_RESPONSE_BYTES: usize =',
        'ConfigureOwnDeviceTicketDiscovery',
        'OwnDeviceTicketDiscoveryStatus', 'ForegroundRuntimeOnly',
        'runtime_ipc_own_device_discovery_policy_id_v1',
        'runtime_ipc_own_device_discovery_snapshot_id_v1',
        'decode_and_verify_runtime_ipc_own_device_discovery_request_v1',
        'decode_and_verify_signed_runtime_ipc_own_device_discovery_response_v1',
        'verify_runtime_ipc_own_device_discovery_request_for_descriptor_v1',
        'verify_runtime_ipc_own_device_discovery_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical own-device discovery Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub recipient_ticket_plaintext:', 'pub ticket_path:',
        'pub local_path:', 'pub root_secret:', 'pub publication_write_key:',
        'pub derived_write_capability:', 'pub device_encryption_secret:',
        'pub hpke_ciphertext:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical own-device discovery leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.milestone -cne 'M1.1.41' -or
        [string]$Vectors.status -cne 'candidate-not-active' -or
        (@($Vectors.dependencies) -join ',') -cne
        'CANONICAL-RUNTIME-IPC-VECTORS.json,CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json') {
        throw 'canonical own-device discovery vector header changed'
    }
    foreach ($value in @($Vectors.input.descriptor_id_hex, $Vectors.input.account_id_hex,
        $Vectors.input.local_device_id_hex, $Vectors.input.device_list_digest_hex,
        $Vectors.input.recipient_device_id_hex, $Vectors.input.recipient_ticket_digest_hex,
        $Vectors.input.outbound_channel_id_hex, $Vectors.input.inbound_channel_id_hex,
        $Vectors.input.observation_id_hex, $Vectors.input.announcement_policy_id_hex,
        $Vectors.input.last_bundle_id_hex,
        $Vectors.input.configure_request_id_hex, $Vectors.input.status_request_id_hex,
        $Vectors.expected.policy_id_hex, $Vectors.expected.snapshot_id_hex,
        $Vectors.expected.configure.request.object_id_hex,
        $Vectors.expected.configure.response.object_id_hex,
        $Vectors.expected.status.request.object_id_hex,
        $Vectors.expected.status.response.object_id_hex)) {
        Assert-LowerHex ([string]$value) 32 'own-device discovery identifier'
    }
    foreach ($value in @($Vectors.expected.configure.response.signature_hex,
        $Vectors.expected.status.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'own-device discovery response signature'
    }
    foreach ($value in @($Vectors.input.current_device_list_signed_cbor_hex,
        $Vectors.input.recipient_ticket_hex, $Vectors.expected.configure.request.cbor_hex,
        $Vectors.expected.configure.response.content_cbor_hex,
        $Vectors.expected.configure.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.content_cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'own-device discovery canonical object'
    }
    if ([int]$Vectors.input.recipient_ticket_bytes -lt 1 -or
        [int]$Vectors.input.recipient_ticket_bytes -gt 8388608 -or
        [string]$Vectors.input.current_network -cne 'wifi' -or
        [string]$Vectors.input.last_transport -cne 'relay' -or
        [string]$Vectors.input.service_origin_utf8 -cne 'https://tickets.example/' -or
        [int]$Vectors.input.publication_ttl_seconds -ne 900 -or
        [int]$Vectors.input.refresh_before_seconds -ne 120 -or
        -not [bool]$Vectors.input.enabled) {
        throw 'canonical own-device discovery fixture semantics changed'
    }
    $allIpcHex = @($Vectors.expected.configure.request.cbor_hex,
        $Vectors.expected.configure.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex) -join ''
    if ($allIpcHex.IndexOf([string]$Vectors.input.recipient_ticket_hex,
            [StringComparison]::Ordinal) -ge 0) {
        throw 'recipient ticket plaintext leaked into canonical IPC vectors'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath,
    $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical own-device discovery file is missing: $path"
    }
}

$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$activeIpc = Get-Content -LiteralPath $activeIpcPath -Raw
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$reference = Get-Content -LiteralPath $referencePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('runtime-ipc-own-device-discovery-configuration-v1',
    'runtime-ipc-own-device-discovery-command-v1',
    'runtime-ipc-own-device-discovery-request-v1',
    'runtime-ipc-own-device-discovery-installed-locator-v1',
    'runtime-ipc-own-device-discovery-policy-status-v1',
    'runtime-ipc-own-device-discovery-result-v1',
    'signed-runtime-ipc-own-device-discovery-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical own-device discovery CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_own_device_discovery_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical own-device discovery module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 29;',
    'kilogram:runtime-ipc-descriptor:v10\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_own_device_discovery_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive own-device discovery model leaked into active implementation'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
    [string]$source[0].compatibility -cne 'exact-version' -or
    [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 29) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v29'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($surface.Count -ne 1 -or
    [string]$surface[0].coverage -cne 'complete' -or
    'pairwise-own-device-ticket-discovery' -cnotin @($surface[0].covered) -or
    'roster-wide-own-device-automation' -cnotin @($surface[0].covered) -or
    (@($surface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical own-device discovery aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-own-device-discovery-configure-request-v1',
    'signed-runtime-ipc-own-device-discovery-configure-response-v1',
    'runtime-ipc-own-device-discovery-status-request-v1',
    'signed-runtime-ipc-own-device-discovery-status-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-OWN-DEVICE-DISCOVERY-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 31851 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical own-device discovery fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'policyPreimage', 'snapshotPreimage',
    'recipient ticket plaintext leaked into IPC',
    'canonical_runtime_ipc_own_device_discovery_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical own-device discovery reference is missing '$required'"
    }
}
foreach ($required in @('exact current Root-signed Account Device list',
    'directional pairwise', 'content-addressed ticket', 'plaintext',
    'local filesystem paths', 'foreground runtime only', 'OS background service',
    'idempotency key', 'does not activate canonical CBOR', 'Postcard IPC version 26',
    'executes no network operation')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical own-device discovery RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_own_device_discovery_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_blake3=verified' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical own-device discovery independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_own_device_automation_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_own_device_discovery_policies = 33
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_own_device_policy_limit'
    $leakedVector = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $leakedVector.expected.configure.request.cbor_hex =
        ([string]$leakedVector.expected.configure.request.cbor_hex) +
        ([string]$leakedVector.input.recipient_ticket_hex)
    Assert-Rejected { Assert-Vectors $leakedVector } 'recipient_ticket_plaintext_in_ipc'
    $backgroundSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $backgroundSchema.semantic_rules = @($backgroundSchema.semantic_rules |
        Where-Object { $_ -cnotmatch 'OS background service' })
    Assert-Rejected { Assert-Schema $backgroundSchema $rust } 'background_service_claim_removed'
    Write-Output 'canonical_runtime_ipc_own_device_discovery_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_own_device_discovery_boundary=verified'
Write-Output 'runtime_ipc_coverage=complete'
Write-Output 'configure_own_device_ticket_discovery=canonical-candidate'
Write-Output 'own_device_ticket_discovery_status=canonical-candidate'
Write-Output 'derived_pairwise_capability_in_ipc=false'
Write-Output 'recipient_ticket_plaintext_in_ipc=false'
Write-Output 'local_path_in_ipc=false'
Write-Output 'os_background_service_enabled=false'
Write-Output 'foreground_runtime_only=true'
Write-Output 'bounded_decoder_mutations=31851'
Write-Output 'active_postcard_v29_changed_by_canonical_candidate=false'
Write-Output 'network_executed=false'


