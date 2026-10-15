[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-OWN-DEVICE-ANNOUNCEMENT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_own_device_announcement_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-own-device-announcement-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0166-m1140-canonical-runtime-ipc-own-device-announcement-automation.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.41' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v26-with-json-descriptor-v7' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical own-device announcement schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_own_device_announcement_request_bytes,
        [int]$Schema.limits.maximum_own_device_announcement_response_bytes,
        [int]$Schema.limits.maximum_own_device_announcement_ticket_artifact_bytes,
        [int]$Schema.limits.maximum_own_device_announcement_policies,
        [int]$Schema.limits.minimum_own_device_announcement_interval_seconds,
        [int]$Schema.limits.maximum_own_device_announcement_interval_seconds,
        [int]$Schema.limits.maximum_own_device_announcement_validity_seconds,
        [int]$Schema.limits.maximum_own_device_announcement_retry_seconds
    ) -join ','
    if ($limits -cne '786432,524288,8388608,32,30,86400,2592000,3600') {
        throw 'canonical own-device announcement limits changed'
    }
    if (@($Schema.domains).Count -ne 57 -or @($Schema.objects).Count -ne 29 -or
        @($Schema.covered_commands).Count -ne 25 -or @($Schema.covered_results).Count -ne 24 -or
        @($Schema.semantic_rules).Count -ne 110 -or
        @($Schema.remaining_runtime_ipc_surface).Count -ne 4) {
        throw 'canonical own-device announcement inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-own-device-announcement-artifact-digest:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-announcement-policy-id:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-announcement-snapshot-id:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-announcement-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-announcement-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-own-device-announcement-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical own-device announcement domain is missing: $domain"
        }
    }
    foreach ($required in @('configure-own-device-announcement-automation',
        'own-device-announcement-automation-status')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical own-device announcement command is missing: $required"
        }
    }
    foreach ($required in @('own-device-announcement-automation-configured',
        'own-device-announcement-automation-status')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical own-device announcement result is missing: $required"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ANNOUNCEMENT_REQUEST_BYTES: usize = 768 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_OWN_DEVICE_ANNOUNCEMENT_RESPONSE_BYTES: usize =',
        'ConfigureOwnDeviceAnnouncementAutomation',
        'OwnDeviceAnnouncementAutomationStatus', 'ForegroundRuntimeOnly',
        'runtime_ipc_own_device_announcement_policy_id_v1',
        'runtime_ipc_own_device_announcement_snapshot_id_v1',
        'decode_and_verify_runtime_ipc_own_device_announcement_request_v1',
        'decode_and_verify_signed_runtime_ipc_own_device_announcement_response_v1',
        'verify_runtime_ipc_own_device_announcement_request_for_descriptor_v1',
        'verify_runtime_ipc_own_device_announcement_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical own-device announcement Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub recipient_ticket_plaintext:', 'pub ticket_path:',
        'pub local_path:', 'pub root_secret:', 'pub publication_write_key:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical own-device announcement leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.milestone -cne 'M1.1.40' -or
        [string]$Vectors.status -cne 'candidate-not-active' -or
        (@($Vectors.dependencies) -join ',') -cne
        'CANONICAL-RUNTIME-IPC-VECTORS.json,CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json') {
        throw 'canonical own-device announcement vector header changed'
    }
    foreach ($value in @($Vectors.input.descriptor_id_hex, $Vectors.input.account_id_hex,
        $Vectors.input.local_device_id_hex, $Vectors.input.device_list_digest_hex,
        $Vectors.input.recipient_device_id_hex, $Vectors.input.recipient_ticket_digest_hex,
        $Vectors.input.configure_request_id_hex, $Vectors.input.status_request_id_hex,
        $Vectors.expected.policy_id_hex, $Vectors.expected.snapshot_id_hex,
        $Vectors.expected.configure.request.object_id_hex,
        $Vectors.expected.configure.response.object_id_hex,
        $Vectors.expected.status.request.object_id_hex,
        $Vectors.expected.status.response.object_id_hex)) {
        Assert-LowerHex ([string]$value) 32 'own-device announcement identifier'
    }
    foreach ($value in @($Vectors.expected.configure.response.signature_hex,
        $Vectors.expected.status.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'own-device announcement response signature'
    }
    foreach ($value in @($Vectors.input.current_device_list_signed_cbor_hex,
        $Vectors.input.recipient_ticket_hex, $Vectors.expected.configure.request.cbor_hex,
        $Vectors.expected.configure.response.content_cbor_hex,
        $Vectors.expected.configure.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.content_cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'own-device announcement canonical object'
    }
    if ([int]$Vectors.input.recipient_ticket_bytes -lt 1 -or
        [int]$Vectors.input.recipient_ticket_bytes -gt 8388608 -or
        [string]$Vectors.input.current_network -cne 'wifi' -or
        -not [bool]$Vectors.input.enabled) {
        throw 'canonical own-device announcement fixture semantics changed'
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
        throw "canonical own-device announcement file is missing: $path"
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

foreach ($required in @('runtime-ipc-own-device-announcement-artifact-v1',
    'runtime-ipc-own-device-announcement-configuration-v1',
    'runtime-ipc-own-device-announcement-command-v1',
    'runtime-ipc-own-device-announcement-request-v1',
    'runtime-ipc-own-device-announcement-policy-status-v1',
    'runtime-ipc-own-device-announcement-result-v1',
    'signed-runtime-ipc-own-device-announcement-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical own-device announcement CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_own_device_announcement_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical own-device announcement module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 26;',
    'kilogram:runtime-ipc-descriptor:v7\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_own_device_announcement_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive own-device announcement model leaked into active implementation'
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard' -or
    [string]$source[0].compatibility -cne 'exact-version' -or
    [int](@($source[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 26) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v26'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ([string]$coverage.milestone -cne 'M1.1.41' -or $surface.Count -ne 1 -or
    [string]$surface[0].coverage -cne 'partial' -or
    'content-addressed-own-device-announcement-automation' -cnotin @($surface[0].covered) -or
    'pairwise-own-device-ticket-discovery' -cnotin @($surface[0].covered) -or
    (@($surface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical own-device announcement aggregate coverage boundary changed'
}
$expectedMutationIds = @('runtime-ipc-own-device-announcement-configure-request-v1',
    'signed-runtime-ipc-own-device-announcement-configure-response-v1',
    'runtime-ipc-own-device-announcement-status-request-v1',
    'signed-runtime-ipc-own-device-announcement-status-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-OWN-DEVICE-ANNOUNCEMENT-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 27711 -or
    [int]$mutations.maximum_mutations -ne 30000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 30000) {
    throw 'canonical own-device announcement fixtures or capacity changed'
}
foreach ($required in @('node:crypto', 'policyPreimage', 'snapshotPreimage',
    'recipient ticket plaintext leaked into IPC',
    'canonical_runtime_ipc_own_device_announcement_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical own-device announcement reference is missing '$required'"
    }
}
foreach ($required in @('exact current Root-signed Account Device list',
    'content-addressed staging area', 'plaintext',
    'local filesystem paths', 'foreground runtime only', 'OS background service',
    'idempotency key', 'does not activate canonical CBOR', 'Postcard IPC version 26',
    'executes no network operation')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical own-device announcement RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_own_device_announcement_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_blake3=verified' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical own-device announcement independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_own_device_automation_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedLimit.limits.maximum_own_device_announcement_policies = 33
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
    Write-Output 'canonical_runtime_ipc_own_device_announcement_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_own_device_announcement_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'configure_own_device_announcement_automation=canonical-candidate'
Write-Output 'own_device_announcement_automation_status=canonical-candidate'
Write-Output 'recipient_ticket_plaintext_in_ipc=false'
Write-Output 'local_path_in_ipc=false'
Write-Output 'os_background_service_enabled=false'
Write-Output 'foreground_runtime_only=true'
Write-Output 'bounded_decoder_mutations=27711'
Write-Output 'active_postcard_v26_changed=false'
Write-Output 'network_executed=false'
