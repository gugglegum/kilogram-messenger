[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_own_device_roster_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-own-device-roster-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0168-m1142-canonical-runtime-ipc-own-device-roster-automation.md'

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
        throw 'canonical own-device roster schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_own_device_roster_request_bytes,
        [int]$Schema.limits.maximum_own_device_roster_response_bytes,
        [int]$Schema.limits.maximum_own_device_roster_children,
        [int]$Schema.limits.maximum_own_device_roster_parallel_workflows
    ) -join ','
    if ($limits -cne '786432,786432,64,1') {
        throw 'canonical own-device roster limits changed'
    }
    if (@($Schema.domains).Count -ne 78 -or @($Schema.objects).Count -ne 38 -or
        @($Schema.covered_commands).Count -ne 34 -or @($Schema.covered_results).Count -ne 33 -or
        @($Schema.semantic_rules).Count -ne 179 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 0) {
        throw 'canonical own-device roster inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-own-device-roster-policy-id:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-roster-snapshot-id:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-roster-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-roster-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-roster-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical own-device roster domain is missing: $domain"
        }
    }
    foreach ($required in @('configure-own-device-roster-automation',
        'own-device-roster-automation-status')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical own-device roster command is missing: $required"
        }
    }
    foreach ($required in @('own-device-roster-automation-configured',
        'own-device-roster-automation-status')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical own-device roster result is missing: $required"
        }
    }
    foreach ($required in @('MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_REQUEST_BYTES: usize = 768 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_RESPONSE_BYTES: usize = 768 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_CHILDREN: usize = 64',
        'MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ROSTER_PARALLEL_WORKFLOWS: u8 = 1',
        'ConfigureOwnDeviceRosterAutomation', 'OwnDeviceRosterAutomationStatus',
        'ForegroundRuntimeOnly', 'runtime_ipc_own_device_roster_policy_id_v1',
        'runtime_ipc_own_device_roster_snapshot_id_v1',
        'decode_and_verify_runtime_ipc_own_device_roster_request_v1',
        'decode_and_verify_signed_runtime_ipc_own_device_roster_response_v1',
        'verify_runtime_ipc_own_device_roster_request_for_descriptor_v1',
        'verify_runtime_ipc_own_device_roster_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical own-device roster Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub recipient_ticket_plaintext:', 'pub ticket_path:',
        'pub local_path:', 'pub root_secret:', 'pub publication_write_key:',
        'pub derived_write_capability:', 'pub derived_read_capability:',
        'pub device_encryption_secret:', 'pub device_signing_secret:', 'pub hpke_ciphertext:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical own-device roster leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.milestone -cne 'M1.1.42' -or
        [string]$Vectors.status -cne 'candidate-not-active' -or
        [bool]$Vectors.changes_active_runtime_ipc -or
        [string]$Vectors.encoding -cne 'kilogram-deterministic-cbor-v1') {
        throw 'canonical own-device roster vector header changed'
    }
    foreach ($value in @($Vectors.input.descriptor_id_hex, $Vectors.input.account_id_hex,
        $Vectors.input.local_device_id_hex, $Vectors.input.device_list_digest_hex,
        $Vectors.input.active_recipient_device_id_hex, $Vectors.input.retired_recipient_device_id_hex,
        $Vectors.input.active_discovery_policy_id_hex,
        $Vectors.input.retired_discovery_policy_id_hex,
        $Vectors.input.active_outbound_channel_id_hex, $Vectors.input.active_inbound_channel_id_hex,
        $Vectors.input.recipient_ticket_digest_hex, $Vectors.input.parent_policy_id_hex,
        $Vectors.input.configure_request_id_hex, $Vectors.input.status_request_id_hex,
        $Vectors.expected.configure.request.digest_hex,
        $Vectors.expected.configure.response.id_hex,
        $Vectors.expected.configure.response.parent_policy_id_hex,
        $Vectors.expected.configure.response.snapshot_id_hex,
        $Vectors.expected.status.request.digest_hex,
        $Vectors.expected.status.response.id_hex,
        $Vectors.expected.status.response.snapshot_id_hex)) {
        Assert-LowerHex ([string]$value) 32 'own-device roster identifier'
    }
    foreach ($value in @($Vectors.expected.configure.response.signature_hex,
        $Vectors.expected.status.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'own-device roster response signature'
    }
    foreach ($value in @($Vectors.input.current_device_list_signed_cbor_hex,
        $Vectors.input.recipient_ticket_hex, $Vectors.expected.configure.request.cbor_hex,
        $Vectors.expected.configure.response.content_cbor_hex,
        $Vectors.expected.configure.response.signing_preimage_hex,
        $Vectors.expected.configure.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.content_cbor_hex,
        $Vectors.expected.status.response.signing_preimage_hex,
        $Vectors.expected.status.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'own-device roster canonical object'
    }
    if ([string]$Vectors.input.service_origin_utf8 -cne 'https://tickets.example/' -or
        [int]$Vectors.input.allowed_network_mask -ne 3 -or
        -not [bool]$Vectors.input.enabled -or
        [int]$Vectors.expected.status.response.active_children -ne 1 -or
        [int]$Vectors.expected.status.response.retired_children -ne 1 -or
        [int]$Vectors.expected.status.response.max_parallel_workflows -ne 1) {
        throw 'canonical own-device roster fixture semantics changed'
    }
    $allIpcHex = @($Vectors.expected.configure.request.cbor_hex,
        $Vectors.expected.configure.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex) -join ''
    if ($allIpcHex.IndexOf([string]$Vectors.input.recipient_ticket_hex,
            [StringComparison]::Ordinal) -ge 0) {
        throw 'recipient ticket plaintext leaked into canonical roster IPC vectors'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $cddlPath, $rustPath, $libPath,
    $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical own-device roster file is missing: $path"
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

foreach ($required in @('runtime-ipc-own-device-roster-configuration-v1',
    'runtime-ipc-own-device-roster-policy-status-v1',
    'runtime-ipc-own-device-roster-child-status-v1',
    'discovery-policy-status-cbor', 'runtime-ipc-own-device-roster-command-v1',
    'runtime-ipc-own-device-roster-request-v1', 'runtime-ipc-own-device-roster-result-v1',
    'signed-runtime-ipc-own-device-roster-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical own-device roster CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_own_device_roster_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical own-device roster module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 29;',
    'kilogram:runtime-ipc-descriptor:v10\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_own_device_roster_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive own-device roster model leaked into active implementation'
}
$surface = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($surface.Count -ne 1 -or [string]$surface[0].encoding -cne 'postcard' -or
    [string]$surface[0].compatibility -cne 'exact-version' -or
    [int](@($surface[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 29) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v29'
}
$coverageSurface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($coverageSurface.Count -ne 1 -or
    [string]$coverageSurface[0].coverage -cne 'complete' -or
    'roster-wide-own-device-automation' -cnotin @($coverageSurface[0].covered) -or
    (@($coverageSurface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical own-device roster aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-own-device-roster-configure-request-v1',
    'signed-runtime-ipc-own-device-roster-configure-response-v1',
    'runtime-ipc-own-device-roster-status-request-v1',
    'signed-runtime-ipc-own-device-roster-status-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-OWN-DEVICE-ROSTER-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 31851 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical own-device roster fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'policyPreimage', 'discoveryPolicyId',
    'recipient ticket plaintext leaked into IPC',
    'canonical_runtime_ipc_own_device_roster_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical own-device roster reference is missing '$required'"
    }
}
foreach ($required in @('exact Root-signed roster', 'active sibling', 'retired tombstone',
    'strict Device-ID order', 'one foreground workflow', 'ticket plaintext',
    'local filesystem paths', 'OS background service', 'idempotency key',
    'does not activate canonical CBOR', 'Postcard IPC version 26',
    'executes no network operation')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical own-device roster RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_own_device_roster_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_blake3=verified' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical own-device roster independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_roster_automation_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_own_device_roster_children = 65
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_roster_child_limit'
    $leakedVector = $vectors | ConvertTo-Json -Depth 50 | ConvertFrom-Json
    $leakedVector.expected.status.response.signed_cbor_hex =
        ([string]$leakedVector.expected.status.response.signed_cbor_hex) +
        ([string]$leakedVector.input.recipient_ticket_hex)
    Assert-Rejected { Assert-Vectors $leakedVector } 'recipient_ticket_plaintext_in_roster_ipc'
    $parallelSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $parallelSchema.limits.maximum_own_device_roster_parallel_workflows = 2
    Assert-Rejected { Assert-Schema $parallelSchema $rust } 'parallel_roster_workflows'
    Write-Output 'canonical_runtime_ipc_own_device_roster_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_own_device_roster_boundary=verified'
Write-Output 'runtime_ipc_coverage=complete'
Write-Output 'configure_own_device_roster_automation=canonical-candidate'
Write-Output 'own_device_roster_automation_status=canonical-candidate'
Write-Output 'active_children=exact-current-roster'
Write-Output 'retired_tombstones=retained'
Write-Output 'max_parallel_workflows=1'
Write-Output 'recipient_ticket_plaintext_in_ipc=false'
Write-Output 'local_path_in_ipc=false'
Write-Output 'os_background_service_enabled=false'
Write-Output 'foreground_runtime_only=true'
Write-Output 'bounded_decoder_mutations=31851'
Write-Output 'active_postcard_v29_changed_by_canonical_candidate=false'
Write-Output 'network_executed=false'
