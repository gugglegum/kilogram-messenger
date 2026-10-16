[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$cddlPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_runtime_ipc_ticket_automation_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeIpcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-runtime-ipc-ticket-automation-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0173-m1147-canonical-runtime-ipc-ticket-automation.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.47' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v27-with-json-descriptor-v8' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical ticket-automation schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_ticket_automation_request_bytes,
        [int]$Schema.limits.maximum_ticket_automation_response_bytes,
        [int]$Schema.limits.maximum_ticket_automation_policies,
        [int]$Schema.limits.maximum_ticket_automation_service_origin_bytes,
        [int]$Schema.limits.minimum_ticket_automation_ttl_seconds,
        [int]$Schema.limits.maximum_ticket_automation_ttl_seconds,
        [int]$Schema.limits.minimum_ticket_automation_refresh_before_seconds,
        [int]$Schema.limits.maximum_ticket_automation_refresh_before_seconds,
        [int]$Schema.limits.maximum_ticket_automation_retry_seconds,
        [int]$Schema.limits.minimum_ticket_automation_success_recheck_seconds
    ) -join ','
    if ($limits -cne '131072,524288,256,512,30,3600,10,1800,3600,30') {
        throw 'canonical ticket-automation limits changed'
    }
    if (@($Schema.domains).Count -ne 78 -or @($Schema.objects).Count -ne 38 -or
        @($Schema.covered_commands).Count -ne 34 -or @($Schema.covered_results).Count -ne 33 -or
        @($Schema.semantic_rules).Count -ne 175 -or
        (@($Schema.remaining_runtime_ipc_surface) -join ',') -cne
        'endpoint-announcement-bootstrap-helper-disposition,legacy-central-https-mailbox-command-removal') {
        throw 'canonical ticket-automation inventory changed'
    }
    foreach ($domain in @(
        'kilogram:runtime-ipc-ticket-automation-policy-id:cbor-v1\0',
        'kilogram:runtime-ipc-ticket-automation-snapshot-id:cbor-v1\0',
        'kilogram:runtime-ipc-ticket-automation-request-digest:cbor-v1\0',
        'kilogram:runtime-ipc-ticket-automation-response-signature:cbor-v1\0',
        'kilogram:runtime-ipc-ticket-automation-response-id:cbor-v1\0')) {
        if ($domain -cnotin @($Schema.domains | ForEach-Object { [string]$_.value })) {
            throw "canonical ticket-automation domain is missing: $domain"
        }
    }
    foreach ($required in @('configure-ticket-automation', 'ticket-automation-status')) {
        if ($required -cnotin @($Schema.covered_commands)) {
            throw "canonical ticket-automation command is missing: $required"
        }
    }
    foreach ($required in @('ticket-automation-configured', 'ticket-automation-status')) {
        if ($required -cnotin @($Schema.covered_results)) {
            throw "canonical ticket-automation result is missing: $required"
        }
    }
    foreach ($required in @(
        'MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_REQUEST_BYTES: usize = 128 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_RESPONSE_BYTES: usize = 512 * 1024',
        'MAX_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_POLICIES: usize = 256',
        'MIN_CANONICAL_RUNTIME_IPC_TICKET_AUTOMATION_SUCCESS_RECHECK_SECONDS: u64 = 30',
        'ConfigureTicketAutomation', 'TicketAutomationStatus', 'ForegroundRuntimeOnly',
        'runtime_ipc_ticket_automation_policy_id_v1',
        'runtime_ipc_ticket_automation_snapshot_id_v1',
        'decode_and_verify_runtime_ipc_ticket_automation_request_v1',
        'decode_and_verify_signed_runtime_ipc_ticket_automation_response_v1',
        'verify_runtime_ipc_ticket_automation_request_for_descriptor_v1',
        'verify_runtime_ipc_ticket_automation_response_for_request_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical ticket-automation Rust contract is missing '$required'"
        }
    }
    foreach ($forbidden in @('pub root_secret:', 'pub device_secret:', 'pub ticket_plaintext:',
        'pub hpke_ciphertext:', 'pub read_capability:', 'pub write_capability:',
        'pub local_path:', 'pub filesystem_path:')) {
        if ($Rust.IndexOf($forbidden, [StringComparison]::Ordinal) -ge 0) {
            throw "canonical ticket-automation leaks protected material: $forbidden"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or [string]$Vectors.milestone -cne 'M1.1.47' -or
        [string]$Vectors.status -cne 'candidate-not-active' -or
        [string]$Vectors.input.service_origin_utf8 -cne 'https://tickets.example/' -or
        [UInt64]$Vectors.input.ttl_seconds -ne 900 -or
        [UInt64]$Vectors.input.refresh_before_seconds -ne 300 -or
        [UInt64]$Vectors.input.allowed_network_mask -ne 3 -or
        [string]$Vectors.input.current_network -cne 'wifi') {
        throw 'canonical ticket-automation vector fixture changed'
    }
    foreach ($value in @($Vectors.input.descriptor_id_hex, $Vectors.input.account_id_hex,
        $Vectors.input.device_id_hex, $Vectors.input.contact_id_hex,
        $Vectors.input.peer_account_id_hex, $Vectors.expected.policy_id_hex,
        $Vectors.expected.snapshot_id_hex,
        $Vectors.expected.configure.request.object_id_hex,
        $Vectors.expected.configure.response.object_id_hex,
        $Vectors.expected.status.request.object_id_hex,
        $Vectors.expected.status.response.object_id_hex)) {
        Assert-LowerHex ([string]$value) 32 'ticket-automation identifier'
    }
    foreach ($value in @($Vectors.expected.configure.response.signature_hex,
        $Vectors.expected.status.response.signature_hex)) {
        Assert-LowerHex ([string]$value) 64 'ticket-automation signature'
    }
    foreach ($value in @($Vectors.input.descriptor_signed_cbor_hex,
        $Vectors.expected.configure.request.cbor_hex,
        $Vectors.expected.configure.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex)) {
        Assert-LowerHex ([string]$value) -Label 'ticket-automation canonical object'
    }
    $allIpcHex = @($Vectors.expected.configure.request.cbor_hex,
        $Vectors.expected.configure.response.signed_cbor_hex,
        $Vectors.expected.status.request.cbor_hex,
        $Vectors.expected.status.response.signed_cbor_hex) -join ''
    if ($allIpcHex.IndexOf([string]$Vectors.input.device_secret_key_hex,
            [StringComparison]::Ordinal) -ge 0) {
        throw 'Device secret leaked into canonical ticket-automation IPC vectors'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $completionPath, $cddlPath, $rustPath,
    $libPath, $activeIpcPath, $compatibilityPath, $coveragePath, $mutationsPath,
    $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical ticket-automation file is missing: $path"
    }
}

$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
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

foreach ($required in @('runtime-ipc-ticket-automation-configuration-v1',
    'configure-ticket-automation', 'ticket-automation-status',
    'runtime-ipc-ticket-automation-action-status-v1',
    'runtime-ipc-ticket-automation-policy-status-v1',
    'signed-runtime-ipc-ticket-automation-response-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical ticket-automation CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_runtime_ipc_ticket_automation_v1;',
        [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical ticket-automation module is not exported'
}
foreach ($required in @('const IPC_VERSION: u8 = 27;',
    'kilogram:runtime-ipc-descriptor:v8\0', 'postcard::to_allocvec', 'postcard::from_bytes')) {
    if ($activeIpc.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "active runtime IPC boundary is missing '$required'"
    }
}
if ($activeIpc.IndexOf('canonical_runtime_ipc_ticket_automation_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive ticket-automation model leaked into active implementation'
}
$surface = @($compatibility.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ($surface.Count -ne 1 -or [string]$surface[0].encoding -cne 'postcard' -or
    [string]$surface[0].compatibility -cne 'exact-version' -or
    [int](@($surface[0].bindings | Where-Object { $_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 27) {
    throw 'active compatibility registry no longer records Postcard runtime IPC v27'
}
$coverageSurface = @($coverage.surfaces | Where-Object { $_.id -ceq 'runtime-ipc' })
if ([string]$coverage.milestone -cne 'M1.1.47' -or $coverageSurface.Count -ne 1 -or
    [string]$coverageSurface[0].coverage -cne 'partial' -or
    'foreground-contact-ticket-automation-policy-and-status' -cnotin @($coverageSurface[0].covered) -or
    (@($coverageSurface[0].remaining) -join ',') -cne
    (@($schema.remaining_runtime_ipc_surface) -join ',')) {
    throw 'canonical ticket-automation aggregate coverage boundary changed'
}
$pairs = @($completion.canonical.command_result_pairs | Where-Object {
    [string]$_.module -ceq 'ticket-automation'
})
if ([string]$completion.milestone -cne 'M1.1.47' -or $pairs.Count -ne 2 -or
    (@($pairs | ForEach-Object { "$([string]$_.command):$([string]$_.result)" }) -join ',') -cne
    'configure-ticket-automation:ticket-automation-configured,ticket-automation-status:ticket-automation-status' -or
    @($completion.postcard_only_activation_blockers).Count -ne 5 -or
    @($completion.remaining_canonical_work).Count -ne 2) {
    throw 'runtime IPC completion inventory did not retire the ticket-automation blocker'
}
$expectedMutationIds = @('runtime-ipc-ticket-automation-configure-request-v1',
    'signed-runtime-ipc-ticket-automation-configure-response-v1',
    'runtime-ipc-ticket-automation-status-request-v1',
    'signed-runtime-ipc-ticket-automation-status-response-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-RUNTIME-IPC-TICKET-AUTOMATION-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',') -or
    [int]$mutations.expected_mutations -ne 31851 -or
    [int]$mutations.maximum_mutations -ne 35000 -or
    [int]$mutations.resource_bounds.maximum_total_mutations -ne 35000) {
    throw 'canonical ticket-automation fixtures or mutation capacity changed'
}
foreach ($required in @('node:crypto', 'blake3Hash', 'policyPreimage',
    'canonical_runtime_ipc_ticket_automation_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical ticket-automation reference is missing '$required'"
    }
}
foreach ($required in @('foreground runtime', 'OS background service', 'Windows Task Scheduler',
    'central HTTPS mailbox', 'strict contact-ID order', 'durable idempotency key',
    'does not activate canonical CBOR', 'Postcard IPC version 27', 'local filesystem paths')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical ticket-automation RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_runtime_ipc_ticket_automation_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ }) -or
    'independent_blake3=verified' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical ticket-automation independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 40 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_ticket_automation_without_migration'
    $badTtl = $schema | ConvertTo-Json -Depth 40 | ConvertFrom-Json
    $badTtl.limits.minimum_ticket_automation_ttl_seconds = 29
    Assert-Rejected { Assert-Schema $badTtl $rust } 'short_ticket_automation_ttl'
    $leakedVector = $vectors | ConvertTo-Json -Depth 40 | ConvertFrom-Json
    $leakedVector.expected.status.response.signed_cbor_hex =
        ([string]$leakedVector.expected.status.response.signed_cbor_hex) +
        ([string]$leakedVector.input.device_secret_key_hex)
    Assert-Rejected { Assert-Vectors $leakedVector } 'device_secret_in_ticket_automation_ipc'
    Write-Output 'canonical_runtime_ipc_ticket_automation_self_test=passed'
}

Write-Output 'canonical_runtime_ipc_ticket_automation_boundary=verified'
Write-Output 'runtime_ipc_coverage=partial'
Write-Output 'configure_ticket_automation=canonical-candidate'
Write-Output 'ticket_automation_status=canonical-candidate'
Write-Output 'https_mailbox_store=false'
Write-Output 'os_background_service_enabled=false'
Write-Output 'foreground_runtime_only=true'
