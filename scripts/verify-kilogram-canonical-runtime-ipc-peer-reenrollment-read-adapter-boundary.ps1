[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-PEER-REENROLLMENT-READ-ADAPTER.json'
$completionPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-COMPLETION.json'
$schemaPath = Join-Path $workspace 'CANONICAL-RUNTIME-IPC-SCHEMA.json'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$ipcPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\lib.rs'
$contractPath = Join-Path $workspace 'crates\kilogram-runtime-ipc\src\peer_reenrollment_contract.rs'
$stagingPath = Join-Path $workspace 'crates\kilogram-peer-reenrollment-staging\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$desktopPath = Join-Path $workspace 'apps\kilogram-windows\src\lib.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0239-m1215-read-only-runtime-peer-reenrollment-adapter.md'

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ContainsAll {
    param([string]$Source, [string[]]$Needles, [string]$Label)
    foreach ($needle in $Needles) {
        if ($Source.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) {
            throw "$Label is missing '$needle'"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Completion,
        [object]$Schema,
        [object]$Compatibility,
        [string]$Ipc,
        [string]$Contract,
        [string]$Staging,
        [string]$Cli,
        [string]$Desktop,
        [string]$Rfc
    )

    if ([int]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.2.15' -or
        [string]$Manifest.status -cne 'canonical-runtime-ipc-peer-reenrollment-read-adapter-verified' -or
        [string]$Manifest.contract_source -cne 'M1.2.14' -or
        [bool]$Manifest.changes_connection_ticket_wire -or
        [bool]$Manifest.changes_sync_wire -or
        -not [bool]$Manifest.changes_active_runtime_ipc -or
        [bool]$Manifest.changes_public_default -or
        [string]$Manifest.public_default -cne 'legacy-only') {
        throw 'peer re-enrollment read adapter identity or scope changed'
    }

    $runtime = $Manifest.runtime_ipc
    if ([string]$runtime.command -cne 'PeerReenrollmentRead' -or
        [string]$runtime.response -cne 'PeerReenrollmentRead' -or
        (@($runtime.allowed_nested_commands) -join ',') -cne 'inspect,status' -or
        (@($runtime.rejected_nested_commands) -join ',') -cne 'plan,approve,accept' -or
        [int]$runtime.version -ne 33 -or
        [string]$runtime.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        [int]$runtime.previous_version -ne 32 -or
        [string]$runtime.previous_descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v13\0' -or
        [string]$runtime.migration -cne 'exact-version-hard-cut-for-co-installed-binaries' -or
        [bool]$runtime.dual_reader -or
        [int]$runtime.maximum_nested_request_bytes -ne 4096 -or
        [int]$runtime.maximum_signed_response_bytes -ne 16384 -or
        [bool]$runtime.mutation_or_recovery_authority) {
        throw 'peer re-enrollment runtime IPC boundary changed'
    }

    $stagingBoundary = $Manifest.staging
    if ([string]$stagingBoundary.operation -cne 'explicit-offline-import' -or
        -not [bool]$stagingBoundary.owner_bound_to_account_and_current_device -or
        -not [bool]$stagingBoundary.record_signed_by_current_device -or
        -not [bool]$stagingBoundary.content_addressed -or
        -not [bool]$stagingBoundary.canonical_reencode_required -or
        [int]$stagingBoundary.maximum_records -ne 4096 -or
        [int]$stagingBoundary.maximum_record_bytes -ne 282624 -or
        -not [bool]$stagingBoundary.idempotent_exact_replay -or
        [bool]$stagingBoundary.invitation_ticket_or_endpoint_exposed_to_desktop -or
        [bool]$stagingBoundary.filesystem_path_exposed_to_runtime_observer -or
        [bool]$stagingBoundary.acceptance_authority) {
        throw 'peer re-enrollment invitation staging boundary changed'
    }

    $stores = $Manifest.read_only_stores
    foreach ($required in @(
        'capability_quarantine', 'acceptance_receipt', 'runtime_transition',
        'invitation_staging', 'missing_store_is_honest_empty_or_missing',
        'corrupt_or_foreign_store_fails_closed'
    )) {
        if (-not [bool]$stores.$required) { throw "read-only store boundary '$required' changed" }
    }
    foreach ($forbidden in @(
        'missing_store_is_created', 'observation_repairs_state', 'observation_removes_state'
    )) {
        if ([bool]$stores.$forbidden) { throw "read-only observer gained '$forbidden'" }
    }

    $projection = $Manifest.projection
    foreach ($required in @(
        'distinguishes_durable_quarantine_from_process_local_suppression',
        'distinguishes_restart_required_from_consumed', 'history_recovery_is_separate',
        'exposes_opaque_contact_id', 'exposes_opaque_artifact_ids'
    )) {
        if (-not [bool]$projection.$required) { throw "projection property '$required' changed" }
    }
    foreach ($forbidden in @(
        'exposes_raw_account_id', 'exposes_raw_device_id', 'exposes_ticket_or_endpoint',
        'exposes_filesystem_path', 'exposes_secret_or_capability_bytes',
        'exposes_queue_id_or_message_body'
    )) {
        if ([bool]$projection.$forbidden) { throw "projection leaked '$forbidden'" }
    }

    $desktopBoundary = $Manifest.desktop
    foreach ($required in @(
        'authenticated_descriptor_loaded', 'exact_request_and_response_verified',
        'manual_status_refresh', 'labels_view_read_only'
    )) {
        if (-not [bool]$desktopBoundary.$required) { throw "desktop property '$required' changed" }
    }
    foreach ($forbidden in @(
        'plan_action_present', 'approve_action_present', 'accept_action_present',
        'clear_action_present', 'retry_action_present', 'hot_apply_action_present',
        'automatic_recovery_present'
    )) {
        if ([bool]$desktopBoundary.$forbidden) { throw "desktop gained forbidden authority '$forbidden'" }
    }

    if ([int]$Completion.active_postcard.version -ne 33 -or
        [string]$Completion.active_postcard.descriptor_signature_domain -cne 'kilogram:runtime-ipc-descriptor:v14\0' -or
        @($Completion.canonical.commands).Count -ne 38 -or
        @($Completion.canonical.results).Count -ne 37 -or
        @($Completion.canonical.command_result_pairs).Count -ne 38 -or
        @($Completion.active_postcard.commands).Count -ne 33 -or
        @($Completion.active_postcard.responses).Count -ne 32 -or
        'peer-reenrollment-read' -cnotin @($Completion.canonical.commands) -or
        'peer-reenrollment-read-result' -cnotin @($Completion.canonical.results) -or
        'PeerReenrollmentRead' -cnotin @($Completion.active_postcard.commands.variant) -or
        'PeerReenrollmentRead' -cnotin @($Completion.active_postcard.responses.variant)) {
        throw 'peer re-enrollment completion inventory changed'
    }
    $pair = @($Completion.canonical.command_result_pairs | Where-Object {
        [string]$_.command -ceq 'peer-reenrollment-read'
    })
    if ($pair.Count -ne 1 -or [string]$pair[0].result -cne 'peer-reenrollment-read-result' -or
        [string]$pair[0].module -cne 'read' -or [string]$pair[0].kind -cne 'read') {
        throw 'peer re-enrollment canonical command/result pair changed'
    }
    if ([string]$Schema.activation.current_runtime_encoding -cne 'postcard-v33-with-json-descriptor-v14' -or
        @($Schema.covered_commands).Count -ne 38 -or @($Schema.covered_results).Count -ne 37 -or
        @($Schema.semantic_rules).Count -ne 192 -or
        'peer-reenrollment-read' -cnotin @($Schema.covered_commands) -or
        'peer-reenrollment-read-result' -cnotin @($Schema.covered_results)) {
        throw 'peer re-enrollment canonical schema changed'
    }
    $surface = @($Compatibility.surfaces | Where-Object { [string]$_.id -ceq 'runtime-ipc' })
    if ($surface.Count -ne 1 -or
        [int](@($surface[0].bindings | Where-Object { [string]$_.symbol -ceq 'IPC_VERSION' })[0].value) -ne 33 -or
        [string](@($surface[0].bindings | Where-Object {
            [string]$_.symbol -ceq 'DESCRIPTOR_SIGNATURE_DOMAIN'
        })[0].value) -cne 'kilogram:runtime-ipc-descriptor:v14\0') {
        throw 'peer re-enrollment compatibility binding changed'
    }

    Assert-ContainsAll $Ipc @(
        'const IPC_VERSION: u8 = 33;', 'kilogram:runtime-ipc-descriptor:v14\0',
        'PeerReenrollmentRead {', 'signed_response: Vec<u8>',
        'version_32_request_is_rejected_before_actor_dispatch'
    ) 'runtime IPC source'
    Assert-ContainsAll $Contract @(
        'MAX_RUNTIME_IPC_PEER_REENROLLMENT_REQUEST_BYTES: usize = 4 * 1024',
        'MAX_RUNTIME_IPC_PEER_REENROLLMENT_RESPONSE_BYTES: usize = 16 * 1024',
        'SignedRuntimeIpcPeerReenrollmentResponse',
        'runtime_descriptor_binding: descriptor_binding(descriptor)?',
        'current_device_binding: device_binding(descriptor.content.device_id.as_bytes())'
    ) 'nested peer re-enrollment contract'
    Assert-ContainsAll $Staging @(
        'MAX_PEER_REENROLLMENT_STAGING_RECORDS: usize = 4_096',
        'MAX_CURRENT_PEER_INVITATION_BYTES + 16 * 1024',
        'pub struct PeerReenrollmentStagingStore',
        'pub struct PeerReenrollmentStagingReadStore',
        'pub fn open_existing(', 'create_new(true)',
        'explicit_import_is_idempotent_and_read_only_reopen_does_not_create',
        'wrong_owner_and_changed_bytes_fail_closed',
        'oversized_and_unexpected_entries_fail_closed'
    ) 'invitation staging store'
    Assert-ContainsAll $Cli @(
        'PeerReenrollmentInvitationStage',
        'fn collect_runtime_peer_reenrollment_response(',
        'RuntimeIpcPeerReenrollmentCommand::Inspect',
        'RuntimeIpcPeerReenrollmentCommand::Status',
        'PeerReenrollmentStagingReadStore::open_existing(',
        'PeerReenrollmentAcceptanceReadStore::open_existing(',
        'PeerReenrollmentRuntimeReadStore::open_existing(',
        'RuntimeIpcCommand::PeerReenrollmentRead { request }',
        'runtime IPC peer re-enrollment adapter rejects mutation commands',
        'peer_reenrollment_observation_and_mutation_adapters_are_disjoint',
        'peer_reenrollment_consumes_only_at_fresh_start_and_retains_old_evidence'
    ) 'CLI runtime adapter'
    Assert-ContainsAll $Desktop @(
        'RefreshPeerReenrollmentStatus',
        'WorkerRequest::PeerReenrollmentStatus',
        'RuntimeIpcPeerReenrollmentCommand::Status',
        'SignedRuntimeIpcPeerReenrollmentResponse::decode_and_verify',
        'Peer re-enrollment safety (read only)',
        'No plan, approve, accept, retry, clear, hot-apply, or automatic recovery action is available here.'
    ) 'desktop read-only adapter'

    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'version 32 / domain v13',
        'active adapter accepts only `inspect` and `status`',
        'Staging is not acceptance',
        'absent store is an honest empty or missing state',
        'Old readable history is not implicitly granted',
        'There are no plan, approve, accept, clear, retry, downgrade',
        'public sync default remains `legacy-only`',
        'Aggregate readiness contains 141 closed gates'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "peer re-enrollment read adapter RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $completionPath, $schemaPath, $compatibilityPath, $ipcPath,
    $contractPath, $stagingPath, $cliPath, $desktopPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "missing file: $path" }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$completion = Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$ipc = Get-Content -LiteralPath $ipcPath -Raw
$contract = Get-Content -LiteralPath $contractPath -Raw
$staging = Get-Content -LiteralPath $stagingPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$desktop = Get-Content -LiteralPath $desktopPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Contract $manifest $completion $schema $compatibility $ipc $contract $staging $cli $desktop $rfc

if ($SelfTest) {
    $mutations = @()
    $changed = Copy-JsonObject $manifest
    $changed.runtime_ipc.version = 32
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $staging $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.runtime_ipc.allowed_nested_commands = @('inspect', 'plan', 'status')
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $staging $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.runtime_ipc.mutation_or_recovery_authority = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $staging $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.read_only_stores.missing_store_is_created = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $staging $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.projection.exposes_raw_device_id = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $staging $cli $desktop $rfc }
    $changed = Copy-JsonObject $manifest
    $changed.desktop.accept_action_present = $true
    $mutations += { Assert-Contract $changed $completion $schema $compatibility $ipc $contract $staging $cli $desktop $rfc }
    $changedSchema = Copy-JsonObject $schema
    $changedSchema.covered_commands = @($changedSchema.covered_commands | Where-Object { $_ -cne 'peer-reenrollment-read' })
    $mutations += { Assert-Contract $manifest $completion $changedSchema $compatibility $ipc $contract $staging $cli $desktop $rfc }
    $changedIpc = $ipc.Replace('const IPC_VERSION: u8 = 33;', 'const IPC_VERSION: u8 = 32;')
    $mutations += { Assert-Contract $manifest $completion $schema $compatibility $changedIpc $contract $staging $cli $desktop $rfc }

    $rejected = 0
    foreach ($mutation in $mutations) {
        try { & $mutation } catch { $rejected++ }
    }
    if ($rejected -ne 8) { throw "peer re-enrollment read adapter mutations rejected $rejected/8" }
    Write-Output 'negative_mutations=8/8'
}

Write-Output 'canonical_runtime_ipc_peer_reenrollment_read_adapter_boundary=verified'
Write-Output 'active_runtime_ipc_version=33'
Write-Output 'mutation_authority=false'
Write-Output 'network_executed=false'
Write-Output 'second_host_required=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
